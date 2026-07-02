use anyhow::Result;
use axum::{response::Json, routing::get, Router};
use serde_json::{json, Value};
use std::{path::PathBuf, sync::Arc};

// ── State ─────────────────────────────────────────────────────────────────────
//
// P0 scaffold. `catalog_path` and `bind_addr` are used now; the remaining
// fields are P4 payment-config placeholders — the env-var reads are wired here
// so the P4 phase does not have to redo config plumbing. They are intentionally
// unread until then.
#[derive(Clone)]
#[allow(dead_code)]
struct AppState {
    catalog_path: PathBuf,
    bind_addr: String,
    // ── P4 payment config (wired now, unused until P4) ──────────────────────
    polygon_wallet_address: String,
    receipts_dir: PathBuf,
    claims_dir: PathBuf,
    source_base_url: String,
    polygon_rpc_url: String,
}

// ── Handlers ──────────────────────────────────────────────────────────────────

async fn healthz() -> Json<Value> {
    Json(json!({"status": "ok", "service": "app-privategit-marketplace-2"}))
}

// ── Main ──────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()))
        .init();

    // P0 scaffold binds to a test port. Production port (9202) is owned by the
    // live app-privategit-marketplace service and must not be touched here.
    let bind_addr = std::env::var("MARKETPLACE_BIND").unwrap_or_else(|_| "127.0.0.1:19202".into());
    let catalog_path = PathBuf::from(
        std::env::var("CATALOG_PATH")
            .unwrap_or_else(|_| "/var/lib/local-software/catalog/products.yaml".into()),
    );
    let polygon_wallet_address = std::env::var("POLYGON_WALLET_ADDRESS").unwrap_or_default();
    let receipts_dir = PathBuf::from(
        std::env::var("RECEIPTS_DIR").unwrap_or_else(|_| "/var/lib/local-software/receipts".into()),
    );
    let claims_dir = PathBuf::from(
        std::env::var("CLAIMS_DIR").unwrap_or_else(|_| "/var/lib/local-software/claims".into()),
    );
    let source_base_url = std::env::var("SOURCE_BASE_URL")
        .unwrap_or_else(|_| "https://software.pointsav.com/releases".into());
    let polygon_rpc_url =
        std::env::var("POLYGON_RPC_URL").unwrap_or_else(|_| "https://polygon-rpc.com".into());

    let state = Arc::new(AppState {
        catalog_path,
        bind_addr: bind_addr.clone(),
        polygon_wallet_address,
        receipts_dir,
        claims_dir,
        source_base_url,
        polygon_rpc_url,
    });

    let app = Router::new()
        .route("/healthz", get(healthz))
        .with_state(state);

    tracing::info!("app-privategit-marketplace-2 listening on {bind_addr}");
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
