use anyhow::{Context, Result};
use axum::{
    extract::{Path, State},
    http::{header, StatusCode, Uri},
    response::{IntoResponse, Json, Redirect, Response},
    routing::{get, post},
    Router,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::Utc;
use ed25519_dalek::{Signer, SigningKey};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf, process::Command, sync::Arc};
use tower_http::services::ServeDir;

// ── State ─────────────────────────────────────────────────────────────────────

#[derive(Clone)]
struct AppState {
    wallet_address: String,
    catalog_path: String,
    receipts_dir: String,
    claims_dir: String,
    source_base_url: String,
    default_platform: String,
    signing_key_hex: Option<String>,
    wallet_seed_path: Option<String>,
    order_index_path: String,
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
    #[serde(default)]
    module_tag: String,
    price_usdc: u64,
}

#[derive(Debug, Deserialize)]
struct Catalog {
    installers: Vec<Installer>,
    licenses: Vec<License>,
}

// ── Receipt (mirrors tool-wallet struct) ──────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
struct LicenseReceipt {
    product_id: String,
    #[serde(default)]
    license_tier: String,
    version: String,
    customer_ref: String,
    price_usdc: u64,
    tx_hash: String,
    chain: String,
    confirmed_at: String,
    block_number: u64,
    license_key: String,
}

// ── License token types ───────────────────────────────────────────────────────

#[derive(Serialize)]
struct LicensePayload {
    product: String,
    channel_expiry: String,
    entitlements: Vec<String>,
    version_floor: Option<String>,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn generate_license_key(product_id: &str, tx_hash: &str, customer_ref: &str) -> String {
    let h = hex::encode(Sha256::digest(
        format!("{product_id}:{tx_hash}:{customer_ref}").as_bytes(),
    ));
    format!("{}-{}-{}-{}", &h[0..8], &h[8..16], &h[16..24], &h[24..32])
}

fn receipt_path(receipts_dir: &str, tx_hash: &str) -> PathBuf {
    let now = Utc::now();
    PathBuf::from(receipts_dir)
        .join(now.format("%Y").to_string())
        .join(now.format("%m").to_string())
        .join(format!("{tx_hash}.json"))
}

fn find_receipt(receipts_dir: &str, tx_hash: &str) -> Option<LicenseReceipt> {
    let base = PathBuf::from(receipts_dir);
    let filename = format!("{tx_hash}.json");
    for year_entry in fs::read_dir(&base).ok()?.flatten() {
        if !year_entry.path().is_dir() {
            continue;
        }
        for month_entry in fs::read_dir(year_entry.path()).ok()?.flatten() {
            if !month_entry.path().is_dir() {
                continue;
            }
            let candidate = month_entry.path().join(&filename);
            if candidate.exists() {
                if let Ok(raw) = fs::read_to_string(&candidate) {
                    if let Ok(receipt) = serde_json::from_str::<LicenseReceipt>(&raw) {
                        return Some(receipt);
                    }
                }
            }
        }
    }
    None
}

fn load_catalog(catalog_path: &str) -> Result<Catalog> {
    let raw = fs::read_to_string(catalog_path)
        .with_context(|| format!("reading catalog at {catalog_path}"))?;
    serde_yaml::from_str(&raw).context("parsing products.yaml")
}

// ── Handlers ──────────────────────────────────────────────────────────────────

async fn root() -> Redirect {
    Redirect::to("/products")
}

async fn products_page() -> Response {
    serve_static_html(include_str!("../static/products.html"))
}

async fn software_page() -> Redirect {
    Redirect::permanent("/products")
}

async fn licensing_page(uri: Uri) -> Response {
    let target = match uri.query() {
        Some(q) => format!("/products?{}", q),
        None => "/products".to_string(),
    };
    Redirect::permanent(&target).into_response()
}

fn serve_static_html(body: &'static str) -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        body,
    )
        .into_response()
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
                    json!({
                        "id": i.id,
                        "name": i.name,
                        "description": i.description,
                        "edition": i.edition,
                        "platform": i.platform,
                        "size_mb": i.size_mb,
                        "download_url": format!("{}/{}", state.source_base_url, i.path),
                        "manifest_url": format!("{}/{}/MANIFEST", state.source_base_url, i.path),
                        "type": "installer",
                        "cost": "free"
                    })
                })
                .collect();
            let licenses: Vec<Value> = catalog
                .licenses
                .iter()
                .map(|l| {
                    json!({
                        "id": l.id,
                        "name": l.name,
                        "description": l.description,
                        "module_tag": l.module_tag,
                        "price_usdc": l.price_usdc,
                        "type": "license",
                        "payment_address": state.wallet_address,
                        "payment_chain": "polygon-pos",
                        "payment_token": "USDC"
                    })
                })
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

async fn v1_license(
    State(state): State<Arc<AppState>>,
    Path(tx_hash): Path<String>,
) -> (StatusCode, Json<Value>) {
    let tx_hash = tx_hash.to_lowercase();

    // 1. Check local receipt file
    let rpath = receipt_path(&state.receipts_dir, &tx_hash);
    if rpath.exists() {
        if let Ok(raw) = fs::read_to_string(&rpath) {
            if let Ok(receipt) = serde_json::from_str::<LicenseReceipt>(&raw) {
                return (
                    StatusCode::OK,
                    Json(json!({
                        "status": "confirmed",
                        "license_key": receipt.license_key,
                        "product_id": receipt.product_id,
                        "confirmed_at": receipt.confirmed_at,
                        "customer_ref": receipt.customer_ref
                    })),
                );
            }
        }
    }

    // 2. Verify via tool-wallet check
    let wallet_addr = state.wallet_address.clone();
    let rpc_url =
        std::env::var("POLYGON_RPC_URL").unwrap_or_else(|_| "https://polygon-rpc.com".into());

    let result = Command::new("tool-wallet")
        .args([
            "check",
            &tx_hash,
            "--rpc-url",
            &rpc_url,
            "--wallet-address",
            &wallet_addr,
        ])
        .output();

    match result {
        Ok(out) if out.status.success() => {
            if let Ok(check_json) = serde_json::from_slice::<Value>(&out.stdout) {
                let confirmed = check_json
                    .get("confirmed")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                if confirmed {
                    let amount_usdc = check_json
                        .get("amount_usdc")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0);
                    let customer_ref = check_json
                        .get("from")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string();
                    let block_number = check_json
                        .get("block")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);

                    let price_units = (amount_usdc * 1_000_000.0) as u64;
                    let catalog = load_catalog(&state.catalog_path).ok();
                    let product_id = catalog
                        .as_ref()
                        .and_then(|c| {
                            c.licenses
                                .iter()
                                .find(|l| l.price_usdc * 1_000_000 == price_units)
                        })
                        .map(|l| l.id.clone())
                        .unwrap_or_else(|| format!("unknown-{price_units}"));

                    let license_key = generate_license_key(&product_id, &tx_hash, &customer_ref);
                    let confirmed_at = Utc::now().to_rfc3339();

                    let license_tier = if product_id.ends_with("-fsl") {
                        "fsl".to_string()
                    } else {
                        "apache".to_string()
                    };
                    let receipt = LicenseReceipt {
                        product_id: product_id.clone(),
                        license_tier: license_tier.clone(),
                        version: "0.0.1".into(),
                        customer_ref: customer_ref.clone(),
                        price_usdc: price_units,
                        tx_hash: tx_hash.clone(),
                        chain: "polygon-pos".into(),
                        confirmed_at: confirmed_at.clone(),
                        block_number,
                        license_key: license_key.clone(),
                    };

                    if let Some(parent) = rpath.parent() {
                        let _ = fs::create_dir_all(parent);
                    }
                    if let Ok(raw) = serde_json::to_string_pretty(&receipt) {
                        let _ = fs::write(&rpath, raw);
                    }

                    return (
                        StatusCode::OK,
                        Json(json!({
                            "status": "confirmed",
                            "license_key": license_key,
                            "product_id": product_id,
                            "confirmed_at": confirmed_at,
                            "customer_ref": customer_ref
                        })),
                    );
                } else {
                    return (
                        StatusCode::ACCEPTED,
                        Json(json!({
                            "status": "pending",
                            "retry_after": 30,
                            "message": "Transaction not yet confirmed on Polygon. Retry in 30 seconds."
                        })),
                    );
                }
            }
        }
        Ok(out) => tracing::warn!(
            "tool-wallet check exit {:?}: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        ),
        Err(e) => tracing::error!("tool-wallet not available: {e}"),
    }

    (
        StatusCode::NOT_FOUND,
        Json(json!({
            "status": "not_found",
            "message": "Transaction not found or not a recognised USDC payment to this address."
        })),
    )
}

#[derive(Debug, Deserialize)]
struct ClaimRequest {
    binary_sha256: String,
    wallet_address: String,
}

async fn v1_claim(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ClaimRequest>,
) -> (StatusCode, Json<Value>) {
    let claimed_at = Utc::now().to_rfc3339();
    let token = hex::encode(Sha256::digest(
        format!(
            "{}|{}|{}",
            req.binary_sha256, req.wallet_address, claimed_at
        )
        .as_bytes(),
    ));

    let claim_dir =
        PathBuf::from(&state.claims_dir).join(req.wallet_address.trim_start_matches("0x"));
    let _ = fs::create_dir_all(&claim_dir);
    let short = &req.binary_sha256[..16.min(req.binary_sha256.len())];
    let claim_file = claim_dir.join(format!("{short}.json"));
    let payload = json!({
        "token": token,
        "binary_sha256": req.binary_sha256,
        "wallet_address": req.wallet_address,
        "claimed_at": claimed_at
    });
    let _ = fs::write(
        claim_file,
        serde_json::to_string_pretty(&payload).unwrap_or_default(),
    );

    (
        StatusCode::OK,
        Json(json!({
            "token": token,
            "claimed_at": claimed_at,
            "status": "ok",
            "note": "on-chain mint arrives v0.0.2"
        })),
    )
}

async fn v1_wallet_address(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({
        "address": state.wallet_address,
        "chain": "polygon-pos",
        "token": "USDC",
        "contract": "0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359"
    }))
}

async fn v1_order_address(
    State(state): State<Arc<AppState>>,
    Path(order_id): Path<String>,
) -> (StatusCode, Json<Value>) {
    let seed_path = match &state.wallet_seed_path {
        Some(p) => p.clone(),
        None => {
            tracing::warn!("order-address: WALLET_SEED_PATH not configured");
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({"error": "wallet not configured — set WALLET_SEED_PATH"})),
            );
        }
    };

    let result = Command::new("tool-wallet")
        .args(["address", &order_id])
        .env("WALLET_SEED_PATH", &seed_path)
        .env("ORDER_INDEX_PATH", &state.order_index_path)
        .output();

    match result {
        Ok(out) if out.status.success() => match serde_json::from_slice::<Value>(&out.stdout) {
            Ok(v) => {
                tracing::info!(order_id = %order_id, "order address assigned");
                (StatusCode::OK, Json(v))
            }
            Err(e) => {
                tracing::error!(order_id = %order_id, "tool-wallet address bad output: {e}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "address derivation failed"})),
                )
            }
        },
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            tracing::warn!(order_id = %order_id, "tool-wallet address exit {:?}: {stderr}", out.status);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "address derivation failed"})),
            )
        }
        Err(e) => {
            tracing::error!("tool-wallet not available: {e}");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({"error": "wallet service unavailable"})),
            )
        }
    }
}

async fn v1_issue_token(
    State(state): State<Arc<AppState>>,
    Path(tx_hash): Path<String>,
) -> (StatusCode, Json<Value>) {
    let tx_hash = tx_hash.to_lowercase();

    let Some(ref key_hex) = state.signing_key_hex else {
        tracing::warn!(tx_hash = %tx_hash, "issue-token: LICENSE_SIGNING_KEY not configured");
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error": "license signing not configured — set LICENSE_SIGNING_KEY"})),
        );
    };

    let receipt = match find_receipt(&state.receipts_dir, &tx_hash) {
        Some(r) => r,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": "receipt not found",
                    "hint": "call /v1/license/:tx_hash first to confirm and cache the payment"
                })),
            )
        }
    };

    let seed_bytes = match hex::decode(key_hex.trim()) {
        Ok(b) => b,
        Err(_) => {
            tracing::error!("LICENSE_SIGNING_KEY is not valid hex");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "signing key misconfigured"})),
            );
        }
    };
    let seed_arr: [u8; 32] = match seed_bytes.try_into() {
        Ok(a) => a,
        Err(_) => {
            tracing::error!("LICENSE_SIGNING_KEY must be exactly 32 bytes (64 hex chars)");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "signing key wrong length"})),
            );
        }
    };
    let signing_key = SigningKey::from_bytes(&seed_arr);

    let tier = if receipt.license_tier.is_empty() {
        if receipt.product_id.ends_with("-fsl") {
            "fsl".to_string()
        } else {
            "apache".to_string()
        }
    } else {
        receipt.license_tier.clone()
    };

    let channel_expiry = (Utc::now() + chrono::Duration::days(365))
        .format("%Y-%m-%d")
        .to_string();

    let payload = LicensePayload {
        product: receipt.product_id.clone(),
        channel_expiry: channel_expiry.clone(),
        entitlements: vec![tier.clone()],
        version_floor: None,
    };
    let payload_json = match serde_json::to_string(&payload) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("payload serialization failed: {e}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal error"})),
            );
        }
    };

    let sig = signing_key.sign(payload_json.as_bytes());
    let mut token_bytes = sig.to_bytes().to_vec();
    token_bytes.extend_from_slice(payload_json.as_bytes());
    let token = URL_SAFE_NO_PAD.encode(&token_bytes);

    let download_url = format!(
        "{}/{}/latest/{}?token={}",
        state.source_base_url, receipt.product_id, state.default_platform, token
    );

    tracing::info!(
        tx_hash = %tx_hash,
        product_id = %receipt.product_id,
        tier = %tier,
        channel_expiry = %channel_expiry,
        "issue-token: signed download token issued"
    );

    (
        StatusCode::OK,
        Json(json!({
            "token": token,
            "product_id": receipt.product_id,
            "license_tier": tier,
            "license_key": receipt.license_key,
            "channel_expiry": channel_expiry,
            "download_url": download_url
        })),
    )
}

// ── Main ──────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()))
        .init();

    let bind_addr = std::env::var("MARKETPLACE_BIND").unwrap_or_else(|_| "127.0.0.1:9202".into());
    let wallet_address = std::env::var("POLYGON_WALLET_ADDRESS").unwrap_or_default();
    let catalog_path = std::env::var("CATALOG_PATH")
        .unwrap_or_else(|_| "/var/lib/local-software/catalog/products.yaml".into());
    let receipts_dir =
        std::env::var("RECEIPTS_DIR").unwrap_or_else(|_| "/var/lib/local-software/receipts".into());
    let claims_dir =
        std::env::var("CLAIMS_DIR").unwrap_or_else(|_| "/var/lib/local-software/claims".into());
    let source_base_url = std::env::var("SOURCE_BASE_URL")
        .unwrap_or_else(|_| "https://software.pointsav.com/releases".into());
    let default_platform =
        std::env::var("DEFAULT_PLATFORM").unwrap_or_else(|_| "x86_64-unknown-linux-gnu".into());
    let signing_key_hex = std::env::var("LICENSE_SIGNING_KEY").ok();
    if signing_key_hex.is_none() {
        tracing::warn!("LICENSE_SIGNING_KEY not set — /v1/issue-token will return 503");
    }
    let wallet_seed_path = std::env::var("WALLET_SEED_PATH").ok();
    if wallet_seed_path.is_none() {
        tracing::warn!("WALLET_SEED_PATH not set — /v1/order-address will return 503");
    }
    let order_index_path = std::env::var("ORDER_INDEX_PATH")
        .unwrap_or_else(|_| "/var/lib/local-software/data/order-index.json".into());

    let state = Arc::new(AppState {
        wallet_address,
        catalog_path,
        receipts_dir,
        claims_dir,
        source_base_url,
        default_platform,
        signing_key_hex,
        wallet_seed_path,
        order_index_path,
    });

    let app = Router::new()
        .route("/", get(root))
        .route("/products", get(products_page))
        .route("/software", get(software_page))
        .route("/licensing", get(licensing_page))
        .route("/healthz", get(healthz))
        .route("/v1/products", get(v1_products))
        .route("/v1/license/:tx_hash", get(v1_license))
        .route("/v1/issue-token/:tx_hash", get(v1_issue_token))
        .route("/v1/claim", post(v1_claim))
        .route("/v1/wallet/address", get(v1_wallet_address))
        .route("/v1/order-address/:order_id", get(v1_order_address))
        .nest_service("/static", ServeDir::new("static"))
        .with_state(state);

    tracing::info!("app-privategit-marketplace listening on {bind_addr}");
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
