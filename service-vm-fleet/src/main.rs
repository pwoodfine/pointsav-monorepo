use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{delete, get, post},
    Json, Router,
};
use serde::Deserialize;
use std::{net::SocketAddr, sync::Arc};
use system_vm_fleet_types::{
    CreateVmRequest, FleetStatus, NodeHeartbeat, NodeId, NodeRecord, VmId, VmRecord,
};
use tokio::sync::RwLock;

mod fleet;
mod placement;

use fleet::NodeRegistry;

#[derive(Clone)]
struct AppState {
    registry: Arc<RwLock<NodeRegistry>>,
    /// HTTP client for delegating spawn/destroy to each node's service-vm-host.
    http: reqwest::Client,
    /// Port on which each node's service-vm-host spawn server listens.
    host_spawn_port: u16,
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "service_vm_fleet=info".into()),
        )
        .init();

    let host_spawn_port: u16 = std::env::var("VM_HOST_SPAWN_PORT")
        .unwrap_or_else(|_| "9220".into())
        .parse()
        .expect("VM_HOST_SPAWN_PORT must be a valid port");

    let state = AppState {
        registry: Arc::new(RwLock::new(NodeRegistry::new())),
        http: reqwest::Client::new(),
        host_spawn_port,
    };

    let app = Router::new()
        .route("/v1/nodes/heartbeat", post(heartbeat_handler))
        .route("/v1/fleet", get(fleet_handler))
        .route("/v1/nodes", get(nodes_handler))
        .route("/v1/nodes/:node_id", get(node_handler))
        .route("/v1/vms", post(create_vm_handler).get(list_vms_handler))
        .route("/v1/vms/:vm_id", delete(destroy_vm_handler))
        .with_state(state);

    let addr: SocketAddr = "0.0.0.0:9203".parse().unwrap();
    tracing::info!("service-vm-fleet listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn heartbeat_handler(
    State(state): State<AppState>,
    Json(hb): Json<NodeHeartbeat>,
) -> StatusCode {
    let mut reg = state.registry.write().await;
    reg.update_node(&hb);
    StatusCode::OK
}

async fn fleet_handler(State(state): State<AppState>) -> Json<FleetStatus> {
    let mut reg = state.registry.write().await;
    reg.evict_stale();
    Json(reg.fleet_status())
}

async fn node_handler(
    State(state): State<AppState>,
    Path(node_id): Path<NodeId>,
) -> Result<Json<NodeRecord>, StatusCode> {
    let mut reg = state.registry.write().await;
    reg.evict_stale();
    match reg.get_node(&node_id) {
        Some(n) => Ok(Json(n)),
        None => Err(StatusCode::NOT_FOUND),
    }
}

/// POST /v1/vms — advisory placement then delegate spawn to the target node's vm-host.
///
/// The fleet picks the best node based on available RAM and KVM preference, then
/// forwards the CreateVmRequest to `http://{node.wg_ip}:{VM_HOST_SPAWN_PORT}/v1/spawn`.
/// The node's service-vm-host handles disk creation, cloud-init, and QEMU launch.
async fn create_vm_handler(
    State(state): State<AppState>,
    Json(req): Json<CreateVmRequest>,
) -> Result<Json<VmRecord>, (StatusCode, String)> {
    let (target, node_wg_ip) = {
        let mut reg = state.registry.write().await;
        reg.evict_stale();

        let target = if let Some(pref) = &req.preferred_node {
            let node = reg.get_node(pref).ok_or_else(|| {
                (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    format!("preferred_node '{pref}' is not registered"),
                )
            })?;
            if node.ram_available_mb < req.ram_mb {
                return Err((
                    StatusCode::UNPROCESSABLE_ENTITY,
                    format!(
                        "preferred_node '{pref}' has {}MB available, {}MB requested",
                        node.ram_available_mb, req.ram_mb
                    ),
                ));
            }
            pref.clone()
        } else {
            placement::select_node(&reg, req.ram_mb, req.prefer_kvm).ok_or_else(|| {
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    "insufficient RAM on all registered nodes".to_string(),
                )
            })?
        };

        let wg_ip = reg
            .get_node(&target)
            .map(|n| n.wg_ip.clone())
            .unwrap_or_default();

        (target, wg_ip)
    };

    let spawn_url = format!("http://{}:{}/v1/spawn", node_wg_ip, state.host_spawn_port);

    tracing::info!(
        node = %target,
        wg_ip = %node_wg_ip,
        vm_type = %req.vm_type,
        ram_mb = req.ram_mb,
        "delegating VM spawn to node"
    );

    let resp = state
        .http
        .post(&spawn_url)
        .json(&req)
        .send()
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_GATEWAY,
                format!("cannot reach vm-host at {spawn_url}: {e}"),
            )
        })?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err((
            StatusCode::BAD_GATEWAY,
            format!("vm-host {target} returned {status}: {body}"),
        ));
    }

    let mut record: VmRecord = resp.json().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("vm-host response not a VmRecord: {e}"),
        )
    })?;

    // Propagate tenant_id from request into the stored record so list queries can filter.
    record.tenant_id = req.tenant_id.clone();

    {
        let mut reg = state.registry.write().await;
        reg.register_vm(&target, record.clone());
    }

    tracing::info!(vm_id = %record.vm_id, node = %target, "VM spawn accepted");
    Ok(Json(record))
}

/// DELETE /v1/vms/:vm_id — find the owning node, delegate destroy, then remove from registry.
async fn destroy_vm_handler(State(state): State<AppState>, Path(vm_id): Path<VmId>) -> StatusCode {
    let node_wg_ip = {
        let reg = state.registry.read().await;
        reg.find_vm_node_wg_ip(&vm_id)
    };

    if let Some(wg_ip) = node_wg_ip {
        let destroy_url = format!(
            "http://{}:{}/v1/vms/{}",
            wg_ip, state.host_spawn_port, vm_id
        );
        if let Err(e) = state.http.delete(&destroy_url).send().await {
            tracing::warn!(vm_id = %vm_id, error = %e, "failed to reach vm-host for destroy");
        }
    }

    let mut reg = state.registry.write().await;
    if reg.remove_vm(&vm_id) {
        StatusCode::NO_CONTENT
    } else {
        StatusCode::NOT_FOUND
    }
}

async fn nodes_handler(State(state): State<AppState>) -> Json<Vec<NodeRecord>> {
    let mut reg = state.registry.write().await;
    reg.evict_stale();
    Json(reg.all_nodes())
}

#[derive(Deserialize)]
struct ListVmsQuery {
    tenant_id: Option<String>,
}

/// GET /v1/vms?tenant_id=<id> — list all VMs, optionally filtered by tenant.
/// Used by service-vm-tenant to enforce namespace isolation.
async fn list_vms_handler(
    State(state): State<AppState>,
    Query(q): Query<ListVmsQuery>,
) -> Json<Vec<VmRecord>> {
    let reg = state.registry.read().await;
    Json(reg.all_vms(q.tenant_id.as_deref()))
}
