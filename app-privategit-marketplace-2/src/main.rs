use anyhow::Result;
use axum::{
    extract::State,
    http::{header, StatusCode},
    response::{IntoResponse, Json, Response},
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{fs, path::PathBuf, sync::Arc};
use tower_http::services::ServeDir;

// ── State ─────────────────────────────────────────────────────────────────────
//
// P1 scope. `catalog_path`, `bind_addr`, and `static_dir` are used now; the
// remaining fields are P4 payment-config placeholders — the env-var reads are
// wired here so the P4 phase does not have to redo config plumbing. They are
// intentionally unread until then.
#[derive(Clone)]
#[allow(dead_code)]
struct AppState {
    catalog_path: PathBuf,
    bind_addr: String,
    // Static-HTML source of truth (single source: the on-disk directory). Both
    // /software and /licensing read from this directory at request time, and
    // /static/* mounts the same directory via ServeDir. Nothing is baked with
    // include_str! — see BRIEF/report for the rationale.
    static_dir: PathBuf,
    // ── P4 payment config (wired now, unused until P4) ──────────────────────
    polygon_wallet_address: String,
    receipts_dir: PathBuf,
    claims_dir: PathBuf,
    source_base_url: String,
    polygon_rpc_url: String,
}

// ── Catalog types ─────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
struct Installer {
    id: String,
    name: String,
    description: String,
    edition: String,
    platform: String,
    size_mb: u64,
    path: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct License {
    id: String,
    name: String,
    description: String,
    module_tag: String,
    price_usdc: u64,
}

#[derive(Debug, Deserialize)]
struct Catalog {
    installers: Vec<Installer>,
    licenses: Vec<License>,
}

fn load_catalog(catalog_path: &PathBuf) -> Result<Catalog> {
    let raw = fs::read_to_string(catalog_path)?;
    Ok(serde_yaml::from_str(&raw)?)
}

// ── Handlers ──────────────────────────────────────────────────────────────────

// GET / -> 302 Found redirect to /software.
//
// Note: axum's `Redirect::to` emits 303 See Other, not 302. The P1 contract
// specifies 302, so we build the response explicitly with StatusCode::FOUND.
async fn root() -> Response {
    (StatusCode::FOUND, [(header::LOCATION, "/software")]).into_response()
}

async fn software_page(State(state): State<Arc<AppState>>) -> Response {
    serve_static_html(&state.static_dir.join("software.html"))
}

async fn licensing_page(State(state): State<Arc<AppState>>) -> Response {
    serve_static_html(&state.static_dir.join("licensing.html"))
}

fn serve_static_html(path: &PathBuf) -> Response {
    match fs::read_to_string(path) {
        Ok(body) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
            body,
        )
            .into_response(),
        Err(e) => {
            tracing::error!("failed to read static page {}: {e}", path.display());
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                "page unavailable",
            )
                .into_response()
        }
    }
}

async fn healthz() -> Json<Value> {
    Json(json!({"status": "ok", "service": "app-privategit-marketplace"}))
}

async fn v1_products(State(state): State<Arc<AppState>>) -> (StatusCode, Json<Value>) {
    match load_catalog(&state.catalog_path) {
        Ok(catalog) => {
            let installers: Vec<Value> = catalog
                .installers
                .iter()
                .map(|i| {
                    let mut v = serde_json::to_value(i).unwrap_or_else(|_| json!({}));
                    // Every installer is free regardless of anything else.
                    if let Some(obj) = v.as_object_mut() {
                        obj.insert("cost".into(), json!("free"));
                    }
                    v
                })
                .collect();
            let licenses: Vec<Value> = catalog
                .licenses
                .iter()
                .map(|l| serde_json::to_value(l).unwrap_or_else(|_| json!({})))
                .collect();
            (
                StatusCode::OK,
                Json(json!({"installers": installers, "licenses": licenses})),
            )
        }
        Err(e) => {
            tracing::error!("catalog load failed: {e:#}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "catalog unavailable"})),
            )
        }
    }
}

// ── Main ──────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()))
        .init();

    // P1 testing binds to a test port. Production port (9202) is owned by the
    // live app-privategit-marketplace service and must not be touched here.
    let bind_addr = std::env::var("MARKETPLACE_BIND").unwrap_or_else(|_| "127.0.0.1:9202".into());
    let catalog_path = PathBuf::from(
        std::env::var("CATALOG_PATH")
            .unwrap_or_else(|_| "/var/lib/local-software/catalog/products.yaml".into()),
    );
    let static_dir = PathBuf::from(std::env::var("STATIC_DIR").unwrap_or_else(|_| "static".into()));
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
        static_dir: static_dir.clone(),
        polygon_wallet_address,
        receipts_dir,
        claims_dir,
        source_base_url,
        polygon_rpc_url,
    });

    let app = Router::new()
        .route("/", get(root))
        .route("/software", get(software_page))
        .route("/licensing", get(licensing_page))
        .route("/healthz", get(healthz))
        .route("/v1/products", get(v1_products))
        .nest_service("/static", ServeDir::new(static_dir))
        .with_state(state);

    tracing::info!("app-privategit-marketplace-2 listening on {bind_addr}");
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
