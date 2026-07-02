use anyhow::Result;
use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf, process::Command, sync::Arc};
use tower_http::services::ServeDir;

mod ui;
use ui::SoftwareSurface;

// ── State ─────────────────────────────────────────────────────────────────────
//
// The payment-config fields (`polygon_wallet_address`, `receipts_dir`,
// `claims_dir`, `polygon_rpc_url`, `tool_wallet_bin`) were wired as placeholders
// in P1 and are consumed by the P4 license/claim/wallet handlers below.
// `bind_addr` remains stored for symmetry but is only read via the local in
// `main`; it is retained behind `#[allow(dead_code)]`.
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
    // ── P4 payment config ───────────────────────────────────────────────────
    polygon_wallet_address: String,
    receipts_dir: PathBuf,
    claims_dir: PathBuf,
    source_base_url: String,
    polygon_rpc_url: String,
    // Name (or absolute path) of the `tool-wallet` binary shelled out to by
    // `v1_license`. Defaults to `"tool-wallet"` (resolved via PATH, matching the
    // OLD crate's `Command::new("tool-wallet")`). Overridable via the
    // `TOOL_WALLET_BIN` env var so tests can inject a JSON test-double without a
    // real Polygon RPC call. Production behaviour is unchanged.
    tool_wallet_bin: String,
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

// ── Receipt (mirrors tool-wallet's LicenseReceipt) ────────────────────────────
//
// Field-for-field port of the OLD crate's `LicenseReceipt`. tool-wallet writes
// receipt files that carry ONE extra field — `license_tier` — which serde
// silently ignores here on read (no `deny_unknown_fields`), so files written by
// either binary deserialize cleanly. When THIS crate writes a receipt (the
// fresh-check path in `v1_license`), it omits `license_tier`, exactly as the OLD
// crate did. `price_usdc` here stores `price_units` (micro-USDC), NOT the
// catalog's dollar-labelled value — a pre-existing field-name quirk, not renamed
// in this phase. See tool-wallet/src/main.rs for the writer side.
#[derive(Debug, Serialize, Deserialize)]
struct LicenseReceipt {
    product_id: String,
    version: String,
    customer_ref: String,
    price_usdc: u64,
    tx_hash: String,
    chain: String,
    confirmed_at: String,
    block_number: u64,
    license_key: String,
}

// ── Payment helpers ───────────────────────────────────────────────────────────

/// Deterministic license key: first 32 hex chars of SHA256("{product_id}:{tx_hash}:{customer_ref}"),
/// split into four hyphen-joined 8-char groups. EXACT construction — must stay
/// byte-identical to the OLD crate and tool-wallet so already-issued keys remain
/// reproducible.
fn generate_license_key(product_id: &str, tx_hash: &str, customer_ref: &str) -> String {
    let h = hex::encode(Sha256::digest(
        format!("{product_id}:{tx_hash}:{customer_ref}").as_bytes(),
    ));
    format!("{}-{}-{}-{}", &h[0..8], &h[8..16], &h[16..24], &h[24..32])
}

/// Receipt file path: `<receipts_dir>/<current-UTC-year>/<current-UTC-month>/<tx_hash>.json`.
///
/// NOTE (carried forward, NOT fixed in this phase): the year/month are TODAY's at
/// request time, not the transaction's confirmation date. A receipt written near a
/// month boundary and re-read the next month misses the cache and re-verifies. This
/// is a known, pre-existing gap shared with the OLD crate and tool-wallet's own
/// writer; it was not named as an in-scope fix for P4. Left as-is deliberately.
fn receipt_path(receipts_dir: &std::path::Path, tx_hash: &str) -> PathBuf {
    let now = Utc::now();
    receipts_dir
        .join(now.format("%Y").to_string())
        .join(now.format("%m").to_string())
        .join(format!("{tx_hash}.json"))
}

/// Convert a dollars-denominated USDC amount (as reported by `tool-wallet check`'s
/// `amount_usdc` float) into integer micro-USDC base units (6 decimals).
/// Correct for whole-dollar amounts ($1.00 → 1_000_000), but **lossy for many
/// cent-level values** due to the `f64` round-trip (Checkpoint 2 review finding —
/// see `price_units_from_check_json`, which prefers an exact integer and uses
/// this only as a defensive fallback). Do not call this directly on a
/// `tool-wallet check` response; go through `price_units_from_check_json`.
fn price_units_from_amount(amount_usdc: f64) -> u64 {
    (amount_usdc * 1_000_000.0) as u64
}

/// Extract the confirmed payment amount, in exact micro-USDC base units, from a
/// `tool-wallet check` confirmation JSON.
///
/// # Checkpoint 2 review finding
///
/// Prefers `amount_units` — the exact source integer `tool-wallet` already emits
/// (`tool-wallet/src/main.rs:568`) — over `price_units_from_amount(amount_usdc)`,
/// which round-trips through a lossy `f64` and is off-by-one for roughly 2% of
/// whole-cent prices (e.g. a genuine $2.01 arrives as 2,009,999, one micro-unit
/// short, and would silently fail to match — the same failure class this phase's
/// P4 pricing-unit fix exists to eliminate, just latent rather than fixed). The
/// float path is kept only as a defensive fallback for a `tool-wallet` response
/// that, contrary to its current and expected behavior, omits `amount_units`.
fn price_units_from_check_json(check_json: &Value) -> u64 {
    check_json
        .get("amount_units")
        .and_then(|v| v.as_u64())
        .unwrap_or_else(|| {
            let amount_usdc = check_json
                .get("amount_usdc")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            price_units_from_amount(amount_usdc)
        })
}

/// Resolve the catalog `product_id` whose price equals an on-chain payment of
/// `price_units` (micro-USDC base units).
///
/// # P4 pricing-unit fix — the one deliberate behavioural change in this rewrite
///
/// The OLD crate (`app-privategit-marketplace/src/main.rs`) matched with:
///
/// ```text
/// c.licenses.iter().find(|l| l.price_usdc * 1_000_000 == price_units)
/// ```
///
/// That is WRONG. `products.yaml` already stores `price_usdc` in micro-USDC base
/// units (`apache: 1000000` = $1.00, `fsl: 19000000` = $19.00 — the field is
/// misleadingly *named* dollars but its *value* is micro-units). Re-multiplying
/// the catalog side by 1_000_000 double-counts the unit conversion, so a genuine
/// $1.00 payment (`price_units == 1_000_000`) was compared against
/// `1_000_000 * 1_000_000` and could never match a real payment except by
/// coincidence.
///
/// **Correction (Checkpoint 2 review):** `apache` and `fsl` are live, non-zero
/// priced entries — NOT every catalog price is `0`. The actual safety argument
/// is: the OLD formula never matched a real payment (any historical paid tx would
/// have fallen through to `unknown-<price_units>`), so switching to the correct
/// comparison cannot invalidate any previously-issued license key — receipts are
/// read from disk verbatim and replay identically regardless of this fix.
///
/// THE FIX: compare `l.price_usdc == price_units` directly — both operands are
/// already micro-USDC units, so no multiplication belongs on the catalog side.
/// Full rationale: `docs/P4-PRICING-FIX.md`.
fn match_license_product_id(catalog: &Catalog, price_units: u64) -> Option<String> {
    catalog
        .licenses
        .iter()
        // FIX: direct micro-unit equality. The OLD crate wrote `l.price_usdc *
        // 1_000_000 == price_units`, which re-applied the dollars→micro-units
        // scale to a value already in micro-units. No re-multiplication here.
        .find(|l| l.price_usdc == price_units)
        .map(|l| l.id.clone())
}

/// Outcome of shelling out to `tool-wallet check`.
enum WalletCheck {
    /// Subprocess exited 0 and its JSON reported `confirmed: true`.
    Confirmed(Value),
    /// Subprocess exited 0 and its JSON reported `confirmed: false`.
    Pending,
    /// Subprocess failed to spawn, exited non-zero, or emitted unparseable stdout.
    NotFound,
}

/// Run `tool-wallet check <tx_hash> --rpc-url <url> --wallet-address <addr>` as an
/// external subprocess and classify the result. Consumes tool-wallet's CLI
/// contract exactly as-is; never modifies it. `bin` is normally `"tool-wallet"`
/// (PATH-resolved); tests inject a JSON test-double path via it.
fn run_tool_wallet_check(
    bin: &str,
    tx_hash: &str,
    rpc_url: &str,
    wallet_addr: &str,
) -> WalletCheck {
    let result = Command::new(bin)
        .args([
            "check",
            tx_hash,
            "--rpc-url",
            rpc_url,
            "--wallet-address",
            wallet_addr,
        ])
        .output();

    match result {
        Ok(out) if out.status.success() => match serde_json::from_slice::<Value>(&out.stdout) {
            Ok(check_json) => {
                let confirmed = check_json
                    .get("confirmed")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                if confirmed {
                    WalletCheck::Confirmed(check_json)
                } else {
                    WalletCheck::Pending
                }
            }
            Err(e) => {
                tracing::warn!("tool-wallet check: unparseable stdout: {e}");
                WalletCheck::NotFound
            }
        },
        Ok(out) => {
            tracing::warn!(
                "tool-wallet check exit {:?}: {}",
                out.status,
                String::from_utf8_lossy(&out.stderr)
            );
            WalletCheck::NotFound
        }
        Err(e) => {
            tracing::error!("tool-wallet not available: {e}");
            WalletCheck::NotFound
        }
    }
}

/// The shared `200 OK` confirmed-license JSON shape (receipt-cache path and
/// fresh-check path return identical bodies).
fn confirmed_response(
    license_key: &str,
    product_id: &str,
    confirmed_at: &str,
    customer_ref: &str,
) -> (StatusCode, Json<Value>) {
    (
        StatusCode::OK,
        Json(json!({
            "status": "confirmed",
            "license_key": license_key,
            "product_id": product_id,
            "confirmed_at": confirmed_at,
            "customer_ref": customer_ref
        })),
    )
}

// ── Handlers ──────────────────────────────────────────────────────────────────

// GET / -> 302 Found redirect to /software.
//
// Note: axum's `Redirect::to` emits 303 See Other, not 302. The P1 contract
// specifies 302, so we build the response explicitly with StatusCode::FOUND.
async fn root() -> Response {
    (StatusCode::FOUND, [(header::LOCATION, "/software")]).into_response()
}

// GET /software — dynamic product catalog.
//
// Replaces the P1 static-HTML read (`software.html` + `wrap_static_html`). The page is
// now rendered from the SAME `Catalog` that `v1_products` loads, so the product cards
// can never drift from `products.yaml` again (the bug this phase fixes). The Sovereign
// Editorial chrome is supplied by `ui::render_page`.
async fn software_page(State(state): State<Arc<AppState>>) -> Response {
    match load_catalog(&state.catalog_path) {
        Ok(catalog) => {
            let content = ui::catalog_markup(&catalog, &state.source_base_url);
            let body = ui::render_page(
                SoftwareSurface::Marketplace,
                "Products — PointSav Software",
                content,
            )
            .into_string();
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
                body,
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!("catalog load failed for /software: {e:#}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                "catalog unavailable",
            )
                .into_response()
        }
    }
}

// GET /licensing — UNCHANGED. Static legal/terms document (not catalog data): keeps the
// P1 static-file read + P2 chrome-wrap exactly as-is.
async fn licensing_page(State(state): State<Arc<AppState>>) -> Response {
    serve_chrome_page(&state.static_dir.join("licensing.html"))
}

// Read the prerendered static page from disk (P1 logic, unchanged) and wrap it in
// the Sovereign Editorial chrome (navy masthead + near-black footer) before serving.
fn serve_chrome_page(path: &PathBuf) -> Response {
    match fs::read_to_string(path) {
        Ok(raw) => {
            let body = ui::wrap_static_html(&raw, SoftwareSurface::Marketplace);
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
                body,
            )
                .into_response()
        }
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
                        "payment_address": state.polygon_wallet_address,
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

// GET /v1/license/:tx_hash — payment state machine (receipt cache → tool-wallet check).
async fn v1_license(
    State(state): State<Arc<AppState>>,
    Path(tx_hash): Path<String>,
) -> (StatusCode, Json<Value>) {
    let tx_hash = tx_hash.to_lowercase();

    // 1. Check local receipt file (idempotent replay of a prior confirmation).
    let rpath = receipt_path(&state.receipts_dir, &tx_hash);
    if rpath.exists() {
        if let Ok(raw) = fs::read_to_string(&rpath) {
            if let Ok(receipt) = serde_json::from_str::<LicenseReceipt>(&raw) {
                return confirmed_response(
                    &receipt.license_key,
                    &receipt.product_id,
                    &receipt.confirmed_at,
                    &receipt.customer_ref,
                );
            }
        }
    }

    // 2. No receipt on file — verify on-chain via the tool-wallet subprocess.
    match run_tool_wallet_check(
        &state.tool_wallet_bin,
        &tx_hash,
        &state.polygon_rpc_url,
        &state.polygon_wallet_address,
    ) {
        WalletCheck::Confirmed(check_json) => {
            let customer_ref = check_json
                .get("from")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let block_number = check_json
                .get("block")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);

            let price_units = price_units_from_check_json(&check_json);
            let product_id = load_catalog(&state.catalog_path)
                .ok()
                .and_then(|c| match_license_product_id(&c, price_units))
                .unwrap_or_else(|| format!("unknown-{price_units}"));

            let license_key = generate_license_key(&product_id, &tx_hash, &customer_ref);
            let confirmed_at = Utc::now().to_rfc3339();

            let receipt = LicenseReceipt {
                product_id: product_id.clone(),
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

            confirmed_response(&license_key, &product_id, &confirmed_at, &customer_ref)
        }
        WalletCheck::Pending => (
            StatusCode::ACCEPTED,
            Json(json!({
                "status": "pending",
                "retry_after": 30,
                "message": "Transaction not yet confirmed on Polygon. Retry in 30 seconds."
            })),
        ),
        WalletCheck::NotFound => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "status": "not_found",
                "message": "Transaction not found or not a recognised USDC payment to this address."
            })),
        ),
    }
}

#[derive(Debug, Deserialize)]
struct ClaimRequest {
    binary_sha256: String,
    wallet_address: String,
}

// POST /v1/claim — placeholder token issuance (on-chain mint arrives v0.0.2). Ported
// as-is from the OLD crate; not made "more real" in this phase.
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

    let claim_dir = state
        .claims_dir
        .join(req.wallet_address.trim_start_matches("0x"));
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

// GET /v1/wallet/address — the receiving wallet + chain/token/contract descriptor.
// The USDC contract is a hardcoded public constant (native USDC on Polygon PoS).
async fn v1_wallet_address(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({
        "address": state.polygon_wallet_address,
        "chain": "polygon-pos",
        "token": "USDC",
        "contract": "0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359"
    }))
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
    let tool_wallet_bin = std::env::var("TOOL_WALLET_BIN").unwrap_or_else(|_| "tool-wallet".into());

    let state = Arc::new(AppState {
        catalog_path,
        bind_addr: bind_addr.clone(),
        static_dir: static_dir.clone(),
        polygon_wallet_address,
        receipts_dir,
        claims_dir,
        source_base_url,
        polygon_rpc_url,
        tool_wallet_bin,
    });

    let app = Router::new()
        .route("/", get(root))
        .route("/software", get(software_page))
        .route("/licensing", get(licensing_page))
        .route("/healthz", get(healthz))
        .route("/v1/products", get(v1_products))
        .route("/v1/license/:tx_hash", get(v1_license))
        .route("/v1/claim", post(v1_claim))
        .route("/v1/wallet/address", get(v1_wallet_address))
        .nest_service("/static", ServeDir::new(static_dir))
        .with_state(state);

    tracing::info!("app-privategit-marketplace-2 listening on {bind_addr}");
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────
//
// SAFETY: no test binds a TCP port (handlers are called directly) and every test
// writes ONLY under a unique scratch dir inside `std::env::temp_dir()` (`/tmp`).
// Nothing here touches `/var/lib/local-software/` or ports 9201/9202. The
// subprocess path is exercised via a locally-written JSON test-double — no real
// Polygon RPC call is ever made.
#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static SEQ: AtomicUsize = AtomicUsize::new(0);

    /// Fresh, unique scratch directory under /tmp for one test.
    fn scratch_dir(tag: &str) -> PathBuf {
        let n = SEQ.fetch_add(1, Ordering::SeqCst);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "mkt2-test-{tag}-{}-{n}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Minimal products.yaml with a realistic paid tier (apache = $1.00 = 1_000_000
    /// micro-USDC), written into `dir`. Returns the catalog path.
    fn write_catalog(dir: &std::path::Path) -> PathBuf {
        let path = dir.join("products.yaml");
        fs::write(
            &path,
            r#"installers: []
licenses:
  - id: apache
    name: Apache 2.0
    module_tag: ""
    price_usdc: 1000000
    description: Source-readable, fully open.
  - id: fsl
    name: FSL
    module_tag: ""
    price_usdc: 19000000
    description: Source-readable, non-compete.
"#,
        )
        .unwrap();
        path
    }

    /// Write an executable bash test-double that mimics `tool-wallet check`.
    /// Branches on the (lowercased) tx_hash argument:
    ///   *confirmed* -> confirmed:true, $1.00 payment, exit 0
    ///   *pending*   -> confirmed:false, exit 0
    ///   otherwise   -> confirmed:false + exit 1 (mirrors real tool-wallet's not-found)
    fn write_tool_wallet_double(dir: &std::path::Path) -> PathBuf {
        let path = dir.join("tool-wallet-double.sh");
        fs::write(
            &path,
            r#"#!/usr/bin/env bash
# args: check <tx_hash> --rpc-url <url> --wallet-address <addr>
tx="$2"
case "$tx" in
  *confirmed*) echo '{"confirmed":true,"amount_usdc":1.00,"amount_units":1000000,"from":"0xcaffee","block":123,"tx_hash":"'"$tx"'"}'; exit 0;;
  *pending*)   echo '{"confirmed":false,"reason":"not yet mined"}'; exit 0;;
  *)           echo '{"confirmed":false,"reason":"transaction not found"}'; exit 1;;
esac
"#,
        )
        .unwrap();
        let mut perms = fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).unwrap();
        path
    }

    fn test_state(scratch: &std::path::Path, tool_wallet_bin: String) -> Arc<AppState> {
        Arc::new(AppState {
            catalog_path: write_catalog(scratch),
            bind_addr: "127.0.0.1:0".into(),
            static_dir: scratch.to_path_buf(),
            polygon_wallet_address: "0xTESTWALLET".into(),
            receipts_dir: scratch.join("receipts"),
            claims_dir: scratch.join("claims"),
            source_base_url: "https://example.invalid/releases".into(),
            polygon_rpc_url: "https://rpc.invalid".into(),
            tool_wallet_bin,
        })
    }

    /// Richer products.yaml fixture for the P1–P3 tests: one installer plus a free
    /// (BETA) license and two paid tiers — the full shape production serves.
    fn write_full_catalog(dir: &std::path::Path) -> PathBuf {
        let path = dir.join("products.yaml");
        fs::write(
            &path,
            r#"installers:
  - id: os-mediakit
    name: MediaKit OS
    description: Sovereign media workstation image.
    edition: "1.2.0"
    platform: linux-x86_64
    size_mb: 812
    path: os-mediakit/1.2.0/installer.run
licenses:
  - id: beta-module
    name: Beta Module
    description: Platform module, free during BETA.
    module_tag: mod-beta
    price_usdc: 0
  - id: apache
    name: Apache 2.0
    module_tag: ""
    price_usdc: 1000000
    description: Source-readable, fully open.
  - id: fsl
    name: FSL
    module_tag: ""
    price_usdc: 19000000
    description: Source-readable, non-compete.
"#,
        )
        .unwrap();
        path
    }

    /// State whose catalog path is caller-supplied (P1–P3 tests; never shells out,
    /// so `tool_wallet_bin` is the PATH default and irrelevant).
    fn test_state_at(scratch: &std::path::Path, catalog_path: PathBuf) -> Arc<AppState> {
        Arc::new(AppState {
            catalog_path,
            bind_addr: "127.0.0.1:0".into(),
            static_dir: scratch.to_path_buf(),
            polygon_wallet_address: "0xTESTWALLET".into(),
            receipts_dir: scratch.join("receipts"),
            claims_dir: scratch.join("claims"),
            source_base_url: "https://example.invalid/releases".into(),
            polygon_rpc_url: "https://rpc.invalid".into(),
            tool_wallet_bin: "tool-wallet".into(),
        })
    }

    fn test_state_full(scratch: &std::path::Path) -> Arc<AppState> {
        let catalog_path = write_full_catalog(scratch);
        test_state_at(scratch, catalog_path)
    }

    /// Collect a handler `Response` body into a String (handlers are called
    /// directly — no TCP port is ever bound).
    async fn body_text(body: axum::body::Body) -> String {
        let bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();
        String::from_utf8(bytes.to_vec()).unwrap()
    }

    // ── Pure functions ────────────────────────────────────────────────────────

    #[test]
    fn license_key_construction_is_exact() {
        // Must stay byte-identical to the OLD binary + tool-wallet.
        let key = generate_license_key("apache", "0xabc", "0xcaffee");
        let full = hex::encode(Sha256::digest(b"apache:0xabc:0xcaffee"));
        let expected = format!(
            "{}-{}-{}-{}",
            &full[0..8],
            &full[8..16],
            &full[16..24],
            &full[24..32]
        );
        assert_eq!(key, expected);
        // Shape: four hyphen-joined 8-hex-char groups.
        let parts: Vec<&str> = key.split('-').collect();
        assert_eq!(parts.len(), 4);
        assert!(parts
            .iter()
            .all(|p| p.len() == 8 && p.chars().all(|c| c.is_ascii_hexdigit())));
    }

    #[test]
    fn price_units_conversion_is_unchanged() {
        // Dollars -> micro-USDC. THIS conversion is correct and not the bug.
        assert_eq!(price_units_from_amount(1.00), 1_000_000);
        assert_eq!(price_units_from_amount(19.00), 19_000_000);
    }

    /// THE reviewable proof: for a realistic $1.00 apache-tier payment, the OLD
    /// formula fails to match and the NEW (fixed) formula matches. Catalog
    /// `price_usdc` is already micro-USDC (apache = 1_000_000).
    #[test]
    fn pricing_fix_old_formula_fails_new_formula_matches() {
        let scratch = scratch_dir("pricematch");
        let catalog = load_catalog(&write_catalog(&scratch)).unwrap();

        // A confirmed $1.00 payment -> 1_000_000 micro-USDC.
        let price_units = price_units_from_amount(1.00);
        assert_eq!(price_units, 1_000_000);

        // OLD (buggy) matcher, reproduced verbatim for the before/after proof:
        //   l.price_usdc * 1_000_000 == price_units
        // apache.price_usdc (1_000_000) * 1_000_000 = 1_000_000_000_000 != 1_000_000.
        let old_match: Option<String> = catalog
            .licenses
            .iter()
            .find(|l| l.price_usdc * 1_000_000 == price_units)
            .map(|l| l.id.clone());
        assert_eq!(
            old_match, None,
            "OLD formula must FAIL to match a real $1.00 payment (this was the bug)"
        );

        // NEW (fixed) matcher: direct micro-unit equality.
        let new_match = match_license_product_id(&catalog, price_units);
        assert_eq!(
            new_match,
            Some("apache".to_string()),
            "NEW formula must match apache for a $1.00 payment"
        );

        // Sanity: $19.00 -> fsl under the fix as well.
        assert_eq!(
            match_license_product_id(&catalog, price_units_from_amount(19.00)),
            Some("fsl".to_string())
        );

        let _ = fs::remove_dir_all(&scratch);
    }

    /// Checkpoint 2 review finding: the float round-trip in
    /// `price_units_from_amount` is lossy for many whole-cent prices. $2.01 is a
    /// concrete failure case — verify it, then verify `price_units_from_check_json`
    /// avoids it by preferring the exact `amount_units` integer.
    #[test]
    fn amount_units_precision_fix() {
        // The lossy float path: tool-wallet's own display conversion
        // (amount_units as f64 / 1_000_000.0) fed back through
        // price_units_from_amount does NOT reliably round-trip.
        let amount_units: u64 = 2_010_000; // a genuine $2.01 payment
        let amount_usdc = amount_units as f64 / 1_000_000.0;
        let float_roundtrip = price_units_from_amount(amount_usdc);
        assert_ne!(
            float_roundtrip, amount_units,
            "float round-trip must reproduce the known $2.01 precision loss \
             (if this now passes, tool-wallet's amount_usdc formatting or Rust's \
             float behavior changed — re-verify the exact-integer path is still \
             the one actually used in production before relaxing this test)"
        );

        // The fixed path: given tool-wallet's real response shape (both fields
        // present, as it always emits), the exact integer wins.
        let check_json = json!({
            "confirmed": true,
            "amount_usdc": amount_usdc,
            "amount_units": amount_units,
            "from": "0xbuyer",
            "block": 1
        });
        assert_eq!(
            price_units_from_check_json(&check_json),
            amount_units,
            "must use the exact amount_units field, not the lossy float"
        );

        // Defensive fallback: if amount_units is ever absent, the float path is
        // still exercised (documented limitation, not silently broken).
        let check_json_no_units = json!({
            "confirmed": true,
            "amount_usdc": 1.00,
            "from": "0xbuyer",
            "block": 1
        });
        assert_eq!(price_units_from_check_json(&check_json_no_units), 1_000_000);
    }

    // ── Handler: receipt-cache path ───────────────────────────────────────────

    #[tokio::test]
    async fn license_confirmed_via_existing_receipt() {
        let scratch = scratch_dir("receipt");
        // tool_wallet_bin points at a non-existent binary: this path must NOT shell out.
        let state = test_state(&scratch, "/nonexistent/tool-wallet".into());

        let tx = "0xdeadbeefreceipt";
        let rpath = receipt_path(&state.receipts_dir, tx);
        fs::create_dir_all(rpath.parent().unwrap()).unwrap();
        // Fixture INCLUDES `license_tier` — the extra field tool-wallet writes.
        // It must be ignored on read (proves cross-binary receipt compatibility).
        fs::write(
            &rpath,
            r#"{
  "product_id": "apache",
  "license_tier": "apache",
  "version": "0.0.1",
  "customer_ref": "0xcaffee",
  "price_usdc": 1000000,
  "tx_hash": "0xdeadbeefreceipt",
  "chain": "polygon-pos",
  "confirmed_at": "2026-07-01T00:00:00+00:00",
  "block_number": 123,
  "license_key": "aaaaaaaa-bbbbbbbb-cccccccc-dddddddd"
}"#,
        )
        .unwrap();

        let (status, Json(body)) = v1_license(State(state.clone()), Path(tx.to_string())).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["status"], "confirmed");
        assert_eq!(body["product_id"], "apache");
        assert_eq!(body["license_key"], "aaaaaaaa-bbbbbbbb-cccccccc-dddddddd");
        assert_eq!(body["customer_ref"], "0xcaffee");

        let _ = fs::remove_dir_all(&scratch);
    }

    // ── Handler: fresh tool-wallet check (mocked) ─────────────────────────────

    #[tokio::test]
    async fn license_confirmed_via_fresh_check_matches_apache_and_writes_receipt() {
        let scratch = scratch_dir("fresh");
        let double = write_tool_wallet_double(&scratch);
        let state = test_state(&scratch, double.to_string_lossy().into_owned());

        let tx = "0xconfirmedpayment01";
        let (status, Json(body)) = v1_license(State(state.clone()), Path(tx.to_string())).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["status"], "confirmed");
        // The FIX in action end-to-end: $1.00 -> 1_000_000 -> catalog "apache".
        assert_eq!(body["product_id"], "apache");
        assert_eq!(body["customer_ref"], "0xcaffee");
        let expected_key = generate_license_key("apache", tx, "0xcaffee");
        assert_eq!(body["license_key"], expected_key);

        // A receipt must have been written to the SCRATCH dir (never /var/lib).
        let rpath = receipt_path(&state.receipts_dir, tx);
        assert!(
            rpath.exists(),
            "receipt should be persisted to scratch receipts dir"
        );
        assert!(rpath.starts_with(&scratch));
        let written: LicenseReceipt =
            serde_json::from_str(&fs::read_to_string(&rpath).unwrap()).unwrap();
        assert_eq!(written.product_id, "apache");
        assert_eq!(written.price_usdc, 1_000_000); // micro-USDC, not dollars

        let _ = fs::remove_dir_all(&scratch);
    }

    #[tokio::test]
    async fn license_pending_when_check_unconfirmed() {
        let scratch = scratch_dir("pending");
        let double = write_tool_wallet_double(&scratch);
        let state = test_state(&scratch, double.to_string_lossy().into_owned());

        let (status, Json(body)) =
            v1_license(State(state), Path("0xpendingtx01".to_string())).await;
        assert_eq!(status, StatusCode::ACCEPTED);
        assert_eq!(body["status"], "pending");
        assert_eq!(body["retry_after"], 30);

        let _ = fs::remove_dir_all(&scratch);
    }

    #[tokio::test]
    async fn license_not_found_when_check_fails() {
        let scratch = scratch_dir("notfound");
        let double = write_tool_wallet_double(&scratch);
        let state = test_state(&scratch, double.to_string_lossy().into_owned());

        // Neither *confirmed* nor *pending* -> test-double exits 1 -> NotFound.
        let (status, Json(body)) =
            v1_license(State(state), Path("0xunknowntx01".to_string())).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["status"], "not_found");

        let _ = fs::remove_dir_all(&scratch);
    }

    #[tokio::test]
    async fn not_found_when_tool_wallet_binary_missing() {
        let scratch = scratch_dir("nobin");
        let state = test_state(&scratch, "/nonexistent/tool-wallet".into());

        let (status, Json(body)) = v1_license(State(state), Path("0xanytx01".to_string())).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["status"], "not_found");

        let _ = fs::remove_dir_all(&scratch);
    }

    // ── Handler: wallet address + claim ───────────────────────────────────────

    #[tokio::test]
    async fn wallet_address_shape_and_hardcoded_contract() {
        let scratch = scratch_dir("wallet");
        let state = test_state(&scratch, "tool-wallet".into());
        let Json(body) = v1_wallet_address(State(state)).await;
        assert_eq!(body["address"], "0xTESTWALLET");
        assert_eq!(body["chain"], "polygon-pos");
        assert_eq!(body["token"], "USDC");
        assert_eq!(
            body["contract"],
            "0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359"
        );
        let _ = fs::remove_dir_all(&scratch);
    }

    #[tokio::test]
    async fn claim_writes_placeholder_and_returns_ok() {
        let scratch = scratch_dir("claim");
        let state = test_state(&scratch, "tool-wallet".into());
        let req = ClaimRequest {
            binary_sha256: "0123456789abcdef0123456789abcdef".into(),
            wallet_address: "0xWALLET123".into(),
        };
        let (status, Json(body)) = v1_claim(State(state.clone()), Json(req)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["status"], "ok");
        assert_eq!(body["note"], "on-chain mint arrives v0.0.2");
        assert!(body["token"].as_str().unwrap().len() == 64);
        // File written under claims/<addr-without-0x>/<first16>.json in scratch.
        let claim_file = state
            .claims_dir
            .join("WALLET123")
            .join("0123456789abcdef.json");
        assert!(claim_file.exists());
        let _ = fs::remove_dir_all(&scratch);
    }

    // ── P1: catalog loading ───────────────────────────────────────────────────

    #[test]
    fn load_catalog_parses_realistic_fixture() {
        let scratch = scratch_dir("loadcat");
        let catalog = load_catalog(&write_full_catalog(&scratch)).unwrap();

        assert_eq!(catalog.installers.len(), 1);
        let i = &catalog.installers[0];
        assert_eq!(i.id, "os-mediakit");
        assert_eq!(i.name, "MediaKit OS");
        assert_eq!(i.edition, "1.2.0");
        assert_eq!(i.platform, "linux-x86_64");
        assert_eq!(i.size_mb, 812);
        assert_eq!(i.path, "os-mediakit/1.2.0/installer.run");

        assert_eq!(catalog.licenses.len(), 3);
        assert_eq!(catalog.licenses[0].id, "beta-module");
        assert_eq!(catalog.licenses[0].price_usdc, 0); // BETA/free tier
        assert_eq!(catalog.licenses[0].module_tag, "mod-beta");
        assert_eq!(catalog.licenses[2].id, "fsl");
        assert_eq!(catalog.licenses[2].price_usdc, 19_000_000); // micro-USDC

        let _ = fs::remove_dir_all(&scratch);
    }

    #[test]
    fn load_catalog_missing_file_is_err() {
        let missing = PathBuf::from("/nonexistent/mkt2-test/products.yaml");
        assert!(load_catalog(&missing).is_err());
    }

    #[test]
    fn load_catalog_malformed_or_incomplete_yaml_is_err() {
        let scratch = scratch_dir("badcat");
        let path = scratch.join("products.yaml");

        // Syntactically invalid YAML (unterminated flow sequence).
        fs::write(&path, "installers: [this is: not, valid yaml").unwrap();
        assert!(load_catalog(&path).is_err());

        // Well-formed YAML missing a required field (`licenses`) must also fail.
        fs::write(&path, "installers: []\n").unwrap();
        assert!(load_catalog(&path).is_err());

        let _ = fs::remove_dir_all(&scratch);
    }

    // ── P1: /v1/products JSON shape ───────────────────────────────────────────

    /// Regression guard for the Checkpoint 1 finding: every documented field must
    /// be present on every entry — a missing field here is exactly the class of
    /// gap that diff caught.
    #[tokio::test]
    async fn v1_products_documented_field_shape() {
        let scratch = scratch_dir("products");
        let state = test_state_full(&scratch);

        let (status, Json(body)) = v1_products(State(state)).await;
        assert_eq!(status, StatusCode::OK);

        let installers = body["installers"].as_array().unwrap();
        assert_eq!(installers.len(), 1);
        let i = &installers[0];
        for field in [
            "id",
            "name",
            "description",
            "edition",
            "platform",
            "size_mb",
            "download_url",
            "manifest_url",
            "type",
            "cost",
        ] {
            assert!(
                i.get(field).is_some(),
                "installer entry missing documented field `{field}`"
            );
        }
        assert_eq!(i["id"], "os-mediakit");
        assert_eq!(i["edition"], "1.2.0");
        assert_eq!(i["platform"], "linux-x86_64");
        assert_eq!(i["size_mb"], 812);
        assert_eq!(i["type"], "installer");
        assert_eq!(i["cost"], "free");
        assert_eq!(
            i["download_url"],
            "https://example.invalid/releases/os-mediakit/1.2.0/installer.run"
        );
        assert_eq!(
            i["manifest_url"],
            "https://example.invalid/releases/os-mediakit/1.2.0/installer.run/MANIFEST"
        );

        let licenses = body["licenses"].as_array().unwrap();
        assert_eq!(licenses.len(), 3);
        for l in licenses {
            for field in [
                "id",
                "name",
                "description",
                "module_tag",
                "price_usdc",
                "type",
                "payment_address",
                "payment_chain",
                "payment_token",
            ] {
                assert!(
                    l.get(field).is_some(),
                    "license entry missing documented field `{field}`"
                );
            }
            assert_eq!(l["type"], "license");
            assert_eq!(l["payment_address"], "0xTESTWALLET");
            assert_eq!(l["payment_chain"], "polygon-pos");
            assert_eq!(l["payment_token"], "USDC");
        }
        assert_eq!(licenses[1]["id"], "apache");
        assert_eq!(licenses[1]["price_usdc"], 1_000_000); // micro-USDC passthrough

        let _ = fs::remove_dir_all(&scratch);
    }

    #[tokio::test]
    async fn v1_products_500_when_catalog_unavailable() {
        let scratch = scratch_dir("products500");
        let state = test_state_at(&scratch, scratch.join("no-such-products.yaml"));

        let (status, Json(body)) = v1_products(State(state)).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body, json!({"error": "catalog unavailable"}));

        let _ = fs::remove_dir_all(&scratch);
    }

    // ── P3: dynamic /software catalog page ────────────────────────────────────

    #[tokio::test]
    async fn software_page_renders_dynamic_catalog_with_chrome() {
        let scratch = scratch_dir("swpage");
        let state = test_state_full(&scratch);

        let (parts, body) = software_page(State(state)).await.into_parts();
        assert_eq!(parts.status, StatusCode::OK);
        assert_eq!(
            parts.headers.get(header::CONTENT_TYPE).unwrap(),
            "text/html; charset=utf-8"
        );
        let html = body_text(body).await;

        // Catalog data actually drives the page (the drift bug this phase fixed).
        assert!(html.contains("os-mediakit"));
        assert!(html.contains("MediaKit OS"));
        assert!(html.contains("Beta Module"));
        assert!(html.contains("Apache 2.0"));
        assert!(html.contains("<title>Products — PointSav Software</title>"));

        // P2 chrome wraps the dynamic content.
        assert!(html.contains("sw-masthead"));
        assert!(html.contains(SoftwareSurface::Marketplace.trademark_line()));
        assert!(html.contains(SoftwareSurface::Marketplace.copyright_holder()));

        let _ = fs::remove_dir_all(&scratch);
    }

    #[tokio::test]
    async fn software_page_500_when_catalog_unavailable() {
        let scratch = scratch_dir("swpage500");
        let state = test_state_at(&scratch, scratch.join("no-such-products.yaml"));

        let (parts, body) = software_page(State(state)).await.into_parts();
        assert_eq!(parts.status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body_text(body).await, "catalog unavailable");

        let _ = fs::remove_dir_all(&scratch);
    }

    // ── P1/P2: /licensing static page (UNCHANGED by the P3 dynamic-catalog work) ──

    const LICENSING_FIXTURE: &str = r#"<!doctype html>
<html><head><title>Licensing</title></head>
<body>
<header class="topnav">OLD LIGHT NAV</header>
<main><h1>Licensing terms</h1><p>Static legal content, verbatim.</p></main>
<footer>OLD THIN FOOTER</footer>
</body></html>"#;

    #[tokio::test]
    async fn licensing_page_serves_static_content_with_chrome() {
        let scratch = scratch_dir("licensing");
        let state = test_state_full(&scratch);
        fs::write(state.static_dir.join("licensing.html"), LICENSING_FIXTURE).unwrap();

        let (parts, body) = licensing_page(State(state)).await.into_parts();
        assert_eq!(parts.status, StatusCode::OK);
        assert_eq!(
            parts.headers.get(header::CONTENT_TYPE).unwrap(),
            "text/html; charset=utf-8"
        );
        let html = body_text(body).await;

        // Static document content served unchanged.
        assert!(html.contains("Licensing terms"));
        assert!(html.contains("Static legal content, verbatim."));

        // Old light chrome stripped; Sovereign chrome mounted.
        assert!(!html.contains("OLD LIGHT NAV"));
        assert!(!html.contains("OLD THIN FOOTER"));
        assert!(html.contains("sw-masthead"));
        assert!(html.contains(SoftwareSurface::Marketplace.trademark_line()));

        // NOT affected by P3: no dynamic catalog cards on /licensing.
        assert!(!html.contains("sw-cat-card"));

        let _ = fs::remove_dir_all(&scratch);
    }

    #[tokio::test]
    async fn licensing_page_500_when_static_file_missing() {
        let scratch = scratch_dir("licensing500");
        let state = test_state_full(&scratch); // no licensing.html written

        let (parts, body) = licensing_page(State(state)).await.into_parts();
        assert_eq!(parts.status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body_text(body).await, "page unavailable");

        let _ = fs::remove_dir_all(&scratch);
    }

    // ── P1: /healthz + / redirect ─────────────────────────────────────────────

    #[tokio::test]
    async fn healthz_shape() {
        let Json(body) = healthz().await;
        assert_eq!(
            body,
            json!({"status": "ok", "service": "app-privategit-marketplace"})
        );
    }

    #[tokio::test]
    async fn root_redirects_302_to_software() {
        let resp = root().await;
        // P1 contract: 302 Found (NOT axum Redirect::to's 303).
        assert_eq!(resp.status(), StatusCode::FOUND);
        assert_eq!(resp.headers().get(header::LOCATION).unwrap(), "/software");
    }
}
