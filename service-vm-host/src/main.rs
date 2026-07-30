use axum::{
    extract::Path,
    http::StatusCode,
    routing::{delete, post},
    Json, Router,
};
use chrono::Utc;
use std::{net::SocketAddr, time::Duration};
use system_vm_fleet_types::{CreateVmRequest, VmId, VmRecord};
use tokio::time::sleep;

mod host_stats;
mod qemu_monitor;
mod vm_spawn;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "service_vm_host=info".into()),
        )
        .init();

    let fleet_endpoint = std::env::var("VM_FLEET_ENDPOINT")
        .expect("VM_FLEET_ENDPOINT must be set (e.g. http://10.8.0.9:9203)");
    let node_id = std::env::var("VM_NODE_ID").expect("VM_NODE_ID must be set");
    let wg_ip = std::env::var("VM_WG_IP").expect("VM_WG_IP must be set");
    let interval_secs: u64 = std::env::var("VM_HEARTBEAT_INTERVAL_S")
        .unwrap_or_else(|_| "10".into())
        .parse()
        .expect("VM_HEARTBEAT_INTERVAL_S must be a positive integer");
    let spawn_port: u16 = std::env::var("VM_SPAWN_PORT")
        .unwrap_or_else(|_| "9204".into())
        .parse()
        .expect("VM_SPAWN_PORT must be a valid port number");
    // When true, this node requests last-resort-only placement (won't receive VMs unless
    // all non-reserved nodes are exhausted). Set VM_RESERVED=true for nodes in active use.
    let node_reserved: bool = std::env::var("VM_RESERVED")
        .map(|v| v.eq_ignore_ascii_case("true") || v == "1")
        .unwrap_or(false);

    let hostname = read_file_trimmed("/etc/hostname");
    let boot_id = read_file_trimmed("/proc/sys/kernel/random/boot_id");
    let heartbeat_url = format!(
        "{}/v1/nodes/heartbeat",
        fleet_endpoint.trim_end_matches('/')
    );

    tracing::info!(
        node_id = %node_id,
        wg_ip = %wg_ip,
        fleet_endpoint = %fleet_endpoint,
        spawn_port,
        interval_secs,
        "service-vm-host starting"
    );

    // Spawn HTTP server for VM spawn/destroy delegated from service-vm-fleet
    let app = Router::new()
        .route("/v1/spawn", post(spawn_handler))
        .route("/v1/vms/:vm_id", delete(destroy_handler));

    let addr: SocketAddr = format!("0.0.0.0:{spawn_port}").parse().unwrap();
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    tracing::info!("service-vm-host spawn server on {addr}");

    let hb_node_id = node_id.clone();
    let hb_wg_ip = wg_ip.clone();
    let hb_hostname = hostname.clone();
    let hb_boot_id = boot_id.clone();

    tokio::join!(
        async move {
            axum::serve(listener, app).await.ok();
        },
        heartbeat_loop(
            heartbeat_url,
            hb_node_id,
            hb_wg_ip,
            hb_hostname,
            hb_boot_id,
            interval_secs,
            node_reserved,
        ),
    );
}

async fn heartbeat_loop(
    heartbeat_url: String,
    node_id: String,
    wg_ip: String,
    hostname: String,
    boot_id: String,
    interval_secs: u64,
    reserved: bool,
) {
    let client = reqwest::Client::new();
    loop {
        let stats = host_stats::read_host_stats();
        let vms: Vec<VmRecord> = qemu_monitor::poll_running_vms();

        let hb = system_vm_fleet_types::NodeHeartbeat {
            node_id: node_id.clone(),
            wg_ip: wg_ip.clone(),
            hostname: hostname.clone(),
            ram_total_mb: stats.ram_total_mb,
            ram_used_mb: stats.ram_used_mb,
            cpu_cores: stats.cpu_cores,
            cpu_load_pct: stats.cpu_load_pct,
            kvm_available: stats.kvm_available,
            reserved,
            vms,
            boot_id: boot_id.clone(),
            timestamp_utc: Utc::now(),
        };

        match client.post(&heartbeat_url).json(&hb).send().await {
            Ok(resp) if resp.status().is_success() => tracing::debug!("heartbeat ok"),
            Ok(resp) => tracing::warn!(status = %resp.status(), "fleet returned error"),
            Err(e) => tracing::warn!(error = %e, "heartbeat failed"),
        }

        sleep(Duration::from_secs(interval_secs)).await;
    }
}

/// POST /v1/spawn — receive a CreateVmRequest delegated from service-vm-fleet.
/// Creates a cloud-image-backed disk, builds a cloud-init seed ISO, and spawns QEMU.
async fn spawn_handler(
    Json(req): Json<CreateVmRequest>,
) -> Result<Json<VmRecord>, (StatusCode, String)> {
    let kvm_available = std::path::Path::new("/dev/kvm").exists();
    let kvm = req.prefer_kvm && kvm_available;

    let vm_id = format!(
        "{}-{}",
        req.vm_type.to_lowercase(),
        chrono::Utc::now().timestamp()
    );
    let disk_size_gb = (req.ram_mb / 1024).max(8) as u32;
    let record = vm_spawn::provisioning_record(&vm_id, &req.vm_type, req.ram_mb, req.vcpu_count);

    let spawn_record = record.clone();
    drop(tokio::task::spawn_blocking(move || {
        let id = &spawn_record.vm_id;
        match vm_spawn::create_boot_disk(id, disk_size_gb) {
            Err(e) => {
                tracing::warn!(vm_id = %id, error = %e, "disk creation failed");
                return;
            }
            Ok(p) => tracing::info!(vm_id = %id, path = %p.display(), "boot disk ready"),
        }
        match vm_spawn::build_seed_iso(id, &spawn_record.vm_type) {
            Err(e) => {
                tracing::warn!(vm_id = %id, error = %e, "seed ISO build failed");
                return;
            }
            Ok(p) => tracing::info!(vm_id = %id, iso = %p.display(), "seed ISO ready"),
        }
        match vm_spawn::spawn_qemu(&spawn_record, kvm) {
            Ok(_updated) => {}
            Err(e) => tracing::warn!(vm_id = %id, error = %e, "QEMU spawn failed"),
        }
    }));

    Ok(Json(record))
}

/// DELETE /v1/vms/:vm_id — terminate a QEMU VM by pid file.
async fn destroy_handler(Path(vm_id): Path<VmId>) -> StatusCode {
    drop(tokio::task::spawn_blocking(move || {
        vm_spawn::kill_qemu(&vm_id)
    }));
    StatusCode::NO_CONTENT
}

fn read_file_trimmed(path: &str) -> String {
    std::fs::read_to_string(path)
        .unwrap_or_else(|_| "unknown".to_string())
        .trim()
        .to_string()
}
