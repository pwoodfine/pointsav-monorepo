use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{delete, get, post},
    Json, Router,
};
use serde::Serialize;
use std::{net::SocketAddr, sync::Arc};
use system_vm_fleet_types::{CreateVmRequest, VmId, VmRecord};
use tokio::sync::Mutex;

mod quota;
mod tenant;

use tenant::{extract_bearer, TenantRegistry};

#[derive(Clone)]
struct AppState {
    tenants: Arc<TenantRegistry>,
    fleet_url: String,
    http: reqwest::Client,
    /// Serializes VM creation quota checks to prevent TOCTOU races.
    create_lock: Arc<Mutex<()>>,
    audit_path: String,
    /// Base URL of the service-fs WORM ledger (default: http://127.0.0.1:9100).
    /// Audit entries are fire-and-forget posted here in addition to the local file.
    service_fs_url: String,
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "service_vm_tenant=info".into()),
        )
        .init();

    let tenants = TenantRegistry::from_env();
    if tenants.is_empty() {
        tracing::warn!("TENANT_IDS not set — no tenants registered; all requests will 401");
    }

    let fleet_url =
        std::env::var("FLEET_URL").unwrap_or_else(|_| "http://127.0.0.1:9203".to_string());
    let port: u16 = std::env::var("VM_TENANT_PORT")
        .unwrap_or_else(|_| "9221".to_string())
        .parse()
        .expect("VM_TENANT_PORT must be a valid port");
    let audit_path = std::env::var("VM_TENANT_AUDIT_LOG")
        .unwrap_or_else(|_| "/var/log/vm-tenant-audit.jsonl".to_string());
    let service_fs_url =
        std::env::var("SERVICE_FS_URL").unwrap_or_else(|_| "http://127.0.0.1:9100".to_string());

    let state = AppState {
        tenants: Arc::new(tenants),
        fleet_url,
        http: reqwest::Client::new(),
        create_lock: Arc::new(Mutex::new(())),
        audit_path,
        service_fs_url,
    };

    let app = Router::new()
        .route("/healthz", get(health_handler))
        .route("/v1/vms", post(create_vm_handler).get(list_vms_handler))
        .route("/v1/vms/:vm_id", delete(destroy_vm_handler))
        .route("/v1/status", get(status_handler))
        .with_state(state);

    let addr: SocketAddr = format!("0.0.0.0:{port}").parse().unwrap();
    tracing::info!("service-vm-tenant listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn health_handler() -> StatusCode {
    StatusCode::OK
}

/// POST /v1/vms — authenticate, check quota, forward to fleet with tenant_id injected.
async fn create_vm_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(mut req): Json<CreateVmRequest>,
) -> Result<Json<VmRecord>, (StatusCode, String)> {
    let tenant = authenticate(&state, &headers)?;

    // Hold the create lock for the duration of quota-check + fleet call to prevent
    // concurrent creates from the same tenant racing past the quota.
    let _guard = state.create_lock.lock().await;

    // Fetch current tenant VMs to evaluate quota.
    let current_vms = fetch_tenant_vms(&state, &tenant.tenant_id).await?;
    match quota::check(tenant, &current_vms, req.ram_mb) {
        quota::QuotaCheck::Ok => {}
        quota::QuotaCheck::VmLimitExceeded { current, max } => {
            return Err((
                StatusCode::TOO_MANY_REQUESTS,
                format!("VM limit reached ({current}/{max})"),
            ));
        }
        quota::QuotaCheck::RamLimitExceeded {
            current_mb,
            requested_mb,
            max_mb,
        } => {
            return Err((
                StatusCode::TOO_MANY_REQUESTS,
                format!(
                    "RAM quota exceeded: {current_mb}MB used + {requested_mb}MB requested > {max_mb}MB limit"
                ),
            ));
        }
    }

    // Inject tenant namespace before forwarding.
    req.tenant_id = Some(tenant.tenant_id.clone());

    let vm_url = format!("{}/v1/vms", state.fleet_url);
    let resp = state
        .http
        .post(&vm_url)
        .json(&req)
        .send()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("fleet unreachable: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err((StatusCode::BAD_GATEWAY, format!("fleet {status}: {body}")));
    }

    let record: VmRecord = resp.json().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("fleet response invalid: {e}"),
        )
    })?;

    audit(
        &state.audit_path,
        &state.service_fs_url,
        &state.http,
        &tenant.tenant_id,
        "create",
        &record.vm_id,
    )
    .await;

    tracing::info!(
        tenant_id = %tenant.tenant_id,
        vm_id = %record.vm_id,
        ram_mb = record.ram_alloc_mb,
        "VM created"
    );

    Ok(Json(record))
}

/// GET /v1/vms — list VMs owned by this tenant.
async fn list_vms_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<VmRecord>>, (StatusCode, String)> {
    let tenant = authenticate(&state, &headers)?;
    let vms = fetch_tenant_vms(&state, &tenant.tenant_id).await?;
    Ok(Json(vms))
}

/// DELETE /v1/vms/:vm_id — verify ownership then delegate to fleet.
async fn destroy_vm_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(vm_id): Path<VmId>,
) -> Result<StatusCode, (StatusCode, String)> {
    let tenant = authenticate(&state, &headers)?;

    // Confirm the tenant owns this VM.
    let owned = fetch_tenant_vms(&state, &tenant.tenant_id).await?;
    if !owned.iter().any(|v| v.vm_id == vm_id) {
        return Err((
            StatusCode::FORBIDDEN,
            format!("VM '{vm_id}' not owned by this tenant"),
        ));
    }

    let destroy_url = format!("{}/v1/vms/{vm_id}", state.fleet_url);
    let resp = state
        .http
        .delete(&destroy_url)
        .send()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("fleet unreachable: {e}")))?;

    audit(
        &state.audit_path,
        &state.service_fs_url,
        &state.http,
        &tenant.tenant_id,
        "destroy",
        &vm_id,
    )
    .await;

    tracing::info!(tenant_id = %tenant.tenant_id, vm_id = %vm_id, "VM destroyed");

    Ok(if resp.status() == reqwest::StatusCode::NOT_FOUND {
        StatusCode::NOT_FOUND
    } else {
        StatusCode::NO_CONTENT
    })
}

#[derive(Serialize)]
struct TenantStatus {
    tenant_id: String,
    vms_running: u32,
    ram_used_mb: u64,
    max_vms: u32,
    max_ram_mb: u64,
}

/// GET /v1/status — quota usage for this tenant.
async fn status_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<TenantStatus>, (StatusCode, String)> {
    let tenant = authenticate(&state, &headers)?;
    let vms = fetch_tenant_vms(&state, &tenant.tenant_id).await?;
    let qs = quota::QuotaStatus::from_vms(&vms);
    Ok(Json(TenantStatus {
        tenant_id: tenant.tenant_id.clone(),
        vms_running: qs.vms_running,
        ram_used_mb: qs.ram_used_mb,
        max_vms: tenant.max_vms,
        max_ram_mb: tenant.max_ram_mb,
    }))
}

// --- helpers ---

fn authenticate<'a>(
    state: &'a AppState,
    headers: &HeaderMap,
) -> Result<&'a tenant::TenantConfig, (StatusCode, String)> {
    let auth = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let token = extract_bearer(auth);
    state.tenants.authenticate(token).ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            "invalid or missing Bearer token".to_string(),
        )
    })
}

async fn fetch_tenant_vms(
    state: &AppState,
    tenant_id: &str,
) -> Result<Vec<VmRecord>, (StatusCode, String)> {
    let url = format!("{}/v1/vms?tenant_id={tenant_id}", state.fleet_url);
    let resp = state
        .http
        .get(&url)
        .send()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("fleet unreachable: {e}")))?;
    resp.json().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("fleet response invalid: {e}"),
        )
    })
}

/// Append a WORM audit entry to the local log file and fire-and-forget to service-fs.
///
/// The local file is the primary WORM record.  The service-fs POST is best-effort:
/// if service-fs is unavailable the entry is logged as WARN and the request succeeds.
async fn audit(
    path: &str,
    service_fs_url: &str,
    http: &reqwest::Client,
    tenant_id: &str,
    action: &str,
    vm_id: &str,
) {
    use tokio::io::AsyncWriteExt;

    let ts = chrono::Utc::now().to_rfc3339();
    let entry = format!(
        "{{\"ts\":\"{ts}\",\"tenant_id\":\"{tenant_id}\",\"action\":\"{action}\",\"vm_id\":\"{vm_id}\"}}\n",
    );

    // Primary: local WORM file append.
    match tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .await
    {
        Ok(mut f) => {
            if let Err(e) = f.write_all(entry.as_bytes()).await {
                tracing::warn!(error = %e, "audit write failed");
            }
        }
        Err(e) => tracing::warn!(error = %e, path, "audit file open failed"),
    }

    // Secondary: fire-and-forget POST to service-fs /v1/append.
    // key format: vm-tenant-audit/<tenant_id>/<vm_id>/<action>/<unix_ts>
    let unix_ts = chrono::Utc::now().timestamp();
    let payload_id = format!("vm-tenant-audit/{tenant_id}/{vm_id}/{action}/{unix_ts}");
    let body = serde_json::json!({
        "payload_id": payload_id,
        "payload": {
            "ts": ts,
            "tenant_id": tenant_id,
            "action": action,
            "vm_id": vm_id,
        }
    });
    let append_url = format!("{service_fs_url}/v1/append");
    match http.post(&append_url).json(&body).send().await {
        Ok(resp) if resp.status().is_success() => {
            tracing::debug!(payload_id, "audit forwarded to service-fs");
        }
        Ok(resp) => {
            tracing::warn!(
                status = %resp.status(),
                payload_id,
                "service-fs audit POST returned non-success — local file is primary WORM"
            );
        }
        Err(e) => {
            tracing::warn!(
                error = %e,
                payload_id,
                "service-fs unreachable for audit — local file is primary WORM"
            );
        }
    }
}
