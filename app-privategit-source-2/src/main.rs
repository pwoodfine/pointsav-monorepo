// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

use anyhow::Result;
use axum::{
    body::Body,
    extract::{ConnectInfo, Path, Query, State},
    http::{header, HeaderMap, HeaderName, HeaderValue, StatusCode},
    response::{IntoResponse, Json, Redirect, Response},
    routing::{get, post},
    Router,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    collections::HashSet,
    fs,
    net::SocketAddr,
    path::PathBuf,
    sync::{Arc, RwLock},
};
use tokio::fs::File;
use tokio_util::io::ReaderStream;
use tower_http::set_header::SetResponseHeaderLayer;

// Security headers applied to every response. No Content-Security-Policy here —
// this server returns only JSON and binary/file downloads, never HTML, so CSP has
// no meaningful surface; X-Content-Type-Options matters most, to stop a served
// binary from being MIME-sniffed and executed in a browser context.
const HSTS_VALUE: &str = "max-age=63072000; includeSubDomains";

// ── State ─────────────────────────────────────────────────────────────────────

#[derive(Clone)]
struct AppState {
    releases_dir: String,
    verify_key: Option<VerifyingKey>,
    revocation_list_path: Option<String>,
    revoked_tokens: Arc<RwLock<HashSet<String>>>,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

// Reads RELEASES_DIR/<product>/MANIFEST.json (the PRODUCT-ROOT manifest — distinct
// from the version-dir MANIFEST.json served raw at /releases/:p/:v/MANIFEST) and
// returns the "requires_license" bool. Defaults to true (secure) on any failure:
// missing file, unreadable, unparsable JSON, absent field, or non-bool field.
fn product_requires_license(releases_dir: &str, product: &str) -> bool {
    let manifest = PathBuf::from(releases_dir)
        .join(product)
        .join("MANIFEST.json");
    let Ok(text) = fs::read_to_string(&manifest) else {
        return true; // default: license required
    };
    let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) else {
        return true;
    };
    val.get("requires_license")
        .and_then(|v| v.as_bool())
        .unwrap_or(true)
}

fn release_path(releases_dir: &str, parts: &[&str]) -> PathBuf {
    let mut p = PathBuf::from(releases_dir);
    for part in parts {
        p.push(part);
    }
    p
}

async fn stream_file(path: PathBuf, content_type: &'static str) -> Response {
    match File::open(&path).await {
        Ok(file) => {
            let stream = ReaderStream::new(file);
            let body = Body::from_stream(stream);
            (StatusCode::OK, [(header::CONTENT_TYPE, content_type)], body).into_response()
        }
        Err(_) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "not found", "path": path.display().to_string()})),
        )
            .into_response(),
    }
}

fn load_verify_key(val: &str) -> Option<VerifyingKey> {
    // Accept either a 64-char hex string directly or a path to a file containing one.
    let hex = if val.len() == 64 && val.chars().all(|c| c.is_ascii_hexdigit()) {
        val.to_string()
    } else {
        fs::read_to_string(val).ok()?.trim().to_string()
    };
    let bytes = hex::decode(&hex).ok()?;
    let arr: [u8; 32] = bytes.try_into().ok()?;
    VerifyingKey::from_bytes(&arr).ok()
}

// Deploy note: update this file atomically (write to .tmp then rename) to avoid
// partial reads during live reloads.
fn load_revocation_list(path: &str) -> std::io::Result<HashSet<String>> {
    let content = fs::read_to_string(path)?;
    let set = content
        .lines()
        .filter_map(|l| {
            let l = l.trim();
            if l.is_empty() || l.starts_with('#') {
                return None;
            }
            let lower = l.to_lowercase();
            if lower.len() != 64 || !lower.chars().all(|c| c.is_ascii_hexdigit()) {
                tracing::warn!(
                    line = l,
                    "revocation list: skipping non-fingerprint line (expected 64-char SHA256 hex; \
                     did you paste a raw token instead of its fingerprint?)"
                );
                return None;
            }
            Some(lower)
        })
        .collect();
    Ok(set)
}

fn token_fingerprint(raw_b64: &str) -> String {
    use sha2::{Digest, Sha256};
    // Fingerprint is SHA256 of the canonical URL_SAFE_NO_PAD base64url token string.
    // Stored fingerprints must use the same encoding — see tool-wallet fingerprint subcommand.
    hex::encode(Sha256::digest(raw_b64.as_bytes()))
}

// ── Version helpers ───────────────────────────────────────────────────────────

fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    let parse = |s: &str| -> Vec<u64> { s.split('.').map(|p| p.parse().unwrap_or(0)).collect() };
    parse(a).cmp(&parse(b))
}

fn latest_version_with_platform(
    releases_dir: &str,
    product: &str,
    platform: &str,
) -> Option<String> {
    let product_dir = PathBuf::from(releases_dir).join(product);
    let mut versions: Vec<String> = fs::read_dir(&product_dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|v| product_dir.join(v).join(platform).exists())
        .collect();
    versions.sort_by(|a, b| compare_versions(a, b));
    versions.into_iter().last()
}

// ── License verification ──────────────────────────────────────────────────────

enum LicenseVerifyErr {
    MalformedToken,
    TokenTooShort,
    InvalidSignature,
    InvalidPayload,
    WrongProduct,
    ChannelExpired(String),
    Revoked,
}

impl LicenseVerifyErr {
    fn status(&self) -> StatusCode {
        match self {
            Self::WrongProduct | Self::ChannelExpired(_) | Self::Revoked => StatusCode::FORBIDDEN,
            _ => StatusCode::UNAUTHORIZED,
        }
    }
    fn reason(&self) -> &'static str {
        match self {
            Self::MalformedToken => "malformed-token",
            Self::TokenTooShort => "token-too-short",
            Self::InvalidSignature => "invalid-signature",
            Self::InvalidPayload => "invalid-payload",
            Self::WrongProduct => "wrong-product",
            Self::ChannelExpired(_) => "channel-expired",
            Self::Revoked => "token-revoked",
        }
    }
}

fn verify_license_key(
    vk: &VerifyingKey,
    key_b64: &str,
    product_id: &str,
    revoked_tokens: &RwLock<HashSet<String>>,
) -> Result<LicensePayload, LicenseVerifyErr> {
    use LicenseVerifyErr::*;
    let token_bytes = URL_SAFE_NO_PAD
        .decode(key_b64)
        .map_err(|_| MalformedToken)?;
    if token_bytes.len() <= 64 {
        return Err(TokenTooShort);
    }
    let (sig_bytes, payload_bytes) = token_bytes.split_at(64);
    let sig_arr: [u8; 64] = sig_bytes.try_into().expect("exactly 64 bytes");
    let sig = Signature::from_bytes(&sig_arr);
    if vk.verify_strict(payload_bytes, &sig).is_err() {
        return Err(InvalidSignature);
    }
    let payload: LicensePayload =
        serde_json::from_slice(payload_bytes).map_err(|_| InvalidPayload)?;
    if payload.product != product_id {
        return Err(WrongProduct);
    }
    // Revocation check before expiry: a revoked key returns Revoked regardless of expiry date.
    // Fingerprint is of the RAW base64url token STRING (not the decoded bytes).
    {
        let revoked = revoked_tokens.read().unwrap();
        if !revoked.is_empty() && revoked.contains(&token_fingerprint(key_b64)) {
            return Err(Revoked);
        }
    }
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    if payload.channel_expiry < today {
        return Err(ChannelExpired(payload.channel_expiry.clone()));
    }
    Ok(payload)
}

// ── Request / payload types ───────────────────────────────────────────────────

#[derive(Deserialize)]
struct VerifyKeyRequest {
    license_key_b64: String,
    product_id: String,
}

#[derive(Deserialize, Default)]
struct BinaryQuery {
    token: Option<String>,
}

#[derive(Deserialize)]
struct LicensePayload {
    product: String,
    channel_expiry: String,
    entitlements: Vec<String>,
    version_floor: Option<String>,
}

// ── Handlers ──────────────────────────────────────────────────────────────────

async fn healthz() -> Json<Value> {
    Json(json!({"status": "ok", "service": "app-privategit-source"}))
}

async fn releases_index(State(state): State<Arc<AppState>>) -> (StatusCode, Json<Value>) {
    let base = PathBuf::from(&state.releases_dir);
    let products: Vec<String> = fs::read_dir(&base)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    (StatusCode::OK, Json(json!({"products": products})))
}

async fn product_index(
    State(state): State<Arc<AppState>>,
    Path(product): Path<String>,
) -> (StatusCode, Json<Value>) {
    let base = release_path(&state.releases_dir, &[&product]);
    if !base.exists() {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "product not found"})),
        );
    }
    let versions: Vec<String> = fs::read_dir(&base)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    (
        StatusCode::OK,
        Json(json!({"product": product, "versions": versions})),
    )
}

async fn manifest(
    State(state): State<Arc<AppState>>,
    Path((product, version)): Path<(String, String)>,
) -> Response {
    // NO AUTH CHECK AT ALL — the version-dir MANIFEST.json is served raw.
    let path = release_path(&state.releases_dir, &[&product, &version, "MANIFEST.json"]);
    stream_file(path, "application/json").await
}

async fn latest_redirect(
    State(state): State<Arc<AppState>>,
    Path((product, platform)): Path<(String, String)>,
    Query(query): Query<BinaryQuery>,
) -> Response {
    match latest_version_with_platform(&state.releases_dir, &product, &platform) {
        Some(version) => {
            let target = match &query.token {
                Some(tok) => format!("/releases/{product}/{version}/{platform}?token={tok}"),
                None => format!("/releases/{product}/{version}/{platform}"),
            };
            Redirect::temporary(&target).into_response()
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "no binary available for this platform",
                "hint": "The formal build pipeline has not produced a release for this platform yet."})),
        )
            .into_response(),
    }
}

async fn binary(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path((product, version, platform)): Path<(String, String, String)>,
    Query(query): Query<BinaryQuery>,
) -> Response {
    // 1. Detached .sig files are unauthenticated — no license required at all.
    if let Some(base_platform) = platform.strip_suffix(".sig") {
        let path = release_path(
            &state.releases_dir,
            &[&product, &version, &format!("{base_platform}.sig")],
        );
        return stream_file(path, "application/octet-stream").await;
    }

    // 2. Open products (requires_license: false in PRODUCT-ROOT MANIFEST.json) — serve without auth.
    if !product_requires_license(&state.releases_dir, &product) {
        tracing::info!(product_id = %product, result = "ok-open", "binary-download");
        let path = release_path(&state.releases_dir, &[&product, &version, &platform]);
        let filename = format!("{product}-{version}-{platform}");
        return match File::open(&path).await {
            Ok(file) => {
                let stream = ReaderStream::new(file);
                let body = Body::from_stream(stream);
                (
                    StatusCode::OK,
                    [
                        (header::CONTENT_TYPE, "application/octet-stream"),
                        (
                            header::CONTENT_DISPOSITION,
                            Box::leak(
                                format!("attachment; filename=\"{filename}\"").into_boxed_str(),
                            ),
                        ),
                    ],
                    body,
                )
                    .into_response()
            }
            Err(_) => (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "binary not found"})),
            )
                .into_response(),
        };
    }

    // 3. License required — accept Authorization: Bearer <token> header OR ?token= query param
    // (header takes precedence).
    let key_b64 = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(|s| s.to_string())
        .or_else(|| query.token.clone());

    let key_b64 = match key_b64 {
        Some(k) => k,
        None => {
            tracing::info!(product_id = %product, result = "unauthorized", reason = "missing-auth", "binary-download");
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "license key required",
                    "header": "Authorization: Bearer <license_key_b64>",
                    "query": "?token=<license_key_b64>"})),
            )
                .into_response();
        }
    };

    // 4. Server verify key never configured -> 503.
    let Some(vk) = &state.verify_key else {
        tracing::warn!(product_id = %product, result = "service-unavailable", "binary-download: VERIFY_KEY_PUB not set");
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error": "license verification not configured"})),
        )
            .into_response();
    };

    // 5. Verify the license key.
    let key_fp = hex::encode(&vk.as_bytes()[..4]);
    match verify_license_key(vk, &key_b64, &product, &state.revoked_tokens) {
        Err(e) => {
            let log_result = if e.status() == StatusCode::UNAUTHORIZED {
                "unauthorized"
            } else {
                "forbidden"
            };
            tracing::info!(product_id = %product, key_fp = %key_fp, result = log_result, reason = e.reason(), "binary-download");
            return (e.status(), Json(json!({"error": e.reason()}))).into_response();
        }
        Ok(_payload) => {
            tracing::info!(product_id = %product, key_fp = %key_fp, result = "ok", "binary-download");
        }
    }

    // 6. On success: stream the binary.
    let path = release_path(&state.releases_dir, &[&product, &version, &platform]);
    let filename = format!("{product}-{version}-{platform}");
    match File::open(&path).await {
        Ok(file) => {
            let stream = ReaderStream::new(file);
            let body = Body::from_stream(stream);
            (
                StatusCode::OK,
                [
                    (header::CONTENT_TYPE, "application/octet-stream"),
                    (
                        header::CONTENT_DISPOSITION,
                        Box::leak(format!("attachment; filename=\"{filename}\"").into_boxed_str()),
                    ),
                ],
                body,
            )
                .into_response()
        }
        Err(_) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "binary not found",
                "note": "Real OS binaries ship with the build pipeline. Check back soon."})),
        )
            .into_response(),
    }
}

async fn git_stub() -> (StatusCode, Json<Value>) {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(json!({
            "error": "smart-HTTP Git not yet enabled",
            "see": "https://github.com/pointsav/pointsav-monorepo",
            "arriving": "v0.0.2"
        })),
    )
}

// Token format: base64url( sig[64] || payload_json )
// sig is Ed25519 over payload_json bytes.
// 200: valid, authorized, not expired
// 401: bad signature or malformed token
// 403: valid sig but wrong product or channel expired
async fn verify_key_endpoint(
    State(state): State<Arc<AppState>>,
    Json(req): Json<VerifyKeyRequest>,
) -> (StatusCode, Json<Value>) {
    let Some(vk) = &state.verify_key else {
        tracing::warn!(
            result = "service-unavailable",
            "verify-key: VERIFY_KEY_PUB not set"
        );
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error": "verify key not configured — set VERIFY_KEY_PUB"})),
        );
    };
    let key_fp = hex::encode(&vk.as_bytes()[..4]);

    match verify_license_key(
        vk,
        &req.license_key_b64,
        &req.product_id,
        &state.revoked_tokens,
    ) {
        Err(ref e @ LicenseVerifyErr::ChannelExpired(ref expired)) => {
            tracing::info!(product_id = %req.product_id, key_fp = %key_fp, result = "forbidden", reason = "channel-expired", expired = %expired, "verify-key");
            (
                e.status(),
                Json(json!({"valid": false, "reason": "channel expired", "expired": expired})),
            )
        }
        Err(e) => {
            let log_result = if e.status() == StatusCode::UNAUTHORIZED {
                "unauthorized"
            } else {
                "forbidden"
            };
            tracing::info!(product_id = %req.product_id, key_fp = %key_fp, result = log_result, reason = e.reason(), "verify-key");
            (
                e.status(),
                Json(json!({"valid": false, "reason": e.reason()})),
            )
        }
        Ok(payload) => {
            tracing::info!(product_id = %payload.product, key_fp = %key_fp, result = "ok", "verify-key");
            (
                StatusCode::OK,
                Json(json!({
                    "valid": true,
                    "product": payload.product,
                    "version_floor": payload.version_floor,
                    "channel_expiry": payload.channel_expiry,
                    "entitlements": payload.entitlements,
                })),
            )
        }
    }
}

async fn reload_revocation_list(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<Arc<AppState>>,
) -> Response {
    if !addr.ip().is_loopback() {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "admin endpoints are localhost-only"})),
        )
            .into_response();
    }
    let Some(ref path) = state.revocation_list_path else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "no revocation list configured"})),
        )
            .into_response();
    };
    match load_revocation_list(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "revocation list file not found"})),
        )
            .into_response(),
        Err(e) => {
            tracing::warn!("reload_revocation_list failed: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response()
        }
        Ok(fresh) => {
            let count = fresh.len();
            *state.revoked_tokens.write().unwrap() = fresh;
            Json(json!({"reloaded": count})).into_response()
        }
    }
}

async fn verify_key_pub(State(state): State<Arc<AppState>>) -> Response {
    match &state.verify_key {
        Some(vk) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            hex::encode(vk.to_bytes()),
        )
            .into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "verify key not configured"})),
        )
            .into_response(),
    }
}

async fn install_script(
    State(state): State<Arc<AppState>>,
    Path(product): Path<String>,
) -> Response {
    let path = release_path(&state.releases_dir, &[&product, "install.sh"]);
    stream_file(path, "text/x-shellscript").await
}

// ── Main ──────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()))
        .init();

    // Same env var names as the old crate, except SOURCE_BIND defaults to a test
    // port (19201) so the ground-up rewrite never collides with the live
    // app-privategit-source service on 9201.
    let bind_addr = std::env::var("SOURCE_BIND").unwrap_or_else(|_| "127.0.0.1:19201".into());
    let releases_dir =
        std::env::var("RELEASES_DIR").unwrap_or_else(|_| "/var/lib/local-software/releases".into());

    let verify_key = std::env::var("VERIFY_KEY_PUB")
        .ok()
        .and_then(|path| load_verify_key(&path));
    if verify_key.is_none() {
        tracing::warn!("VERIFY_KEY_PUB not set — /verify-key will return 503");
    }

    let revocation_list_path = std::env::var("REVOCATION_LIST_PATH")
        .ok()
        .filter(|p| !p.is_empty());
    let revoked_tokens = Arc::new(RwLock::new(match &revocation_list_path {
        None => HashSet::new(),
        Some(path) => match load_revocation_list(path) {
            Ok(set) => {
                tracing::info!("loaded {} revoked token fingerprints", set.len());
                set
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                tracing::warn!("REVOCATION_LIST_PATH file not found: {e}");
                HashSet::new()
            }
            Err(e) => {
                tracing::warn!("REVOCATION_LIST_PATH unreadable: {e}");
                HashSet::new()
            }
        },
    }));

    let state = Arc::new(AppState {
        releases_dir,
        verify_key,
        revocation_list_path,
        revoked_tokens,
    });

    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/releases/", get(releases_index))
        .route("/releases/:product/", get(product_index))
        .route("/releases/:product/install.sh", get(install_script))
        .route("/releases/:product/:version/MANIFEST", get(manifest))
        .route("/releases/:product/latest/:platform", get(latest_redirect))
        .route("/releases/:product/:version/:platform", get(binary))
        .route("/git/*path", get(git_stub).post(git_stub))
        .route("/verify-key", post(verify_key_endpoint))
        .route("/verify-key.pub", get(verify_key_pub))
        .route(
            "/admin/reload-revocation-list",
            post(reload_revocation_list),
        )
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("strict-transport-security"),
            HeaderValue::from_static(HSTS_VALUE),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("x-content-type-options"),
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("x-frame-options"),
            HeaderValue::from_static("DENY"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("referrer-policy"),
            HeaderValue::from_static("strict-origin-when-cross-origin"),
        ))
        .with_state(state);

    tracing::info!("app-privategit-source listening on {bind_addr}");
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;
    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────
//
// SAFETY: no test binds a TCP port (handlers are called directly with hand-built
// extractors) and every test writes ONLY under a unique scratch dir inside
// `std::env::temp_dir()` (`/tmp`). Nothing here touches `/var/lib/local-software/`
// or ports 9201/9202. Ed25519 keypairs are generated in-test from fixed seeds —
// no production keys are read. House style matches app-privategit-marketplace-2.
#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
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
            "src2-test-{tag}-{}-{n}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Deterministic in-test Ed25519 keypair. Never a production key.
    fn test_signing_key() -> SigningKey {
        SigningKey::from_bytes(&[42u8; 32])
    }

    /// A second, different keypair for wrong-key tests.
    fn other_signing_key() -> SigningKey {
        SigningKey::from_bytes(&[43u8; 32])
    }

    /// Token format: base64url_no_pad( sig[64] || payload_json ), sig over payload_json.
    fn make_token(sk: &SigningKey, payload_json: &str) -> String {
        let sig = sk.sign(payload_json.as_bytes());
        let mut bytes = sig.to_bytes().to_vec();
        bytes.extend_from_slice(payload_json.as_bytes());
        URL_SAFE_NO_PAD.encode(bytes)
    }

    fn payload_json(product: &str, expiry: &str) -> String {
        format!(
            r#"{{"product":"{product}","channel_expiry":"{expiry}","entitlements":["source"]}}"#
        )
    }

    fn today_str() -> String {
        // Same formatting as verify_license_key uses internally.
        chrono::Utc::now().format("%Y-%m-%d").to_string()
    }

    fn no_revocations() -> RwLock<HashSet<String>> {
        RwLock::new(HashSet::new())
    }

    fn test_state(
        releases_dir: &std::path::Path,
        verify_key: Option<VerifyingKey>,
        revocation_list_path: Option<String>,
    ) -> Arc<AppState> {
        Arc::new(AppState {
            releases_dir: releases_dir.to_string_lossy().into_owned(),
            verify_key,
            revocation_list_path,
            revoked_tokens: Arc::new(RwLock::new(HashSet::new())),
        })
    }

    async fn body_json(resp: Response) -> Value {
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    // ── product_requires_license ──────────────────────────────────────────────

    #[test]
    fn requires_license_missing_manifest_defaults_true() {
        let scratch = scratch_dir("reqlic-missing");
        // Product dir exists but has no MANIFEST.json — secure default.
        fs::create_dir_all(scratch.join("prod")).unwrap();
        assert!(product_requires_license(scratch.to_str().unwrap(), "prod"));
        // Product dir does not exist at all.
        assert!(product_requires_license(
            scratch.to_str().unwrap(),
            "no-such-product"
        ));
        let _ = fs::remove_dir_all(&scratch);
    }

    #[test]
    fn requires_license_malformed_json_defaults_true() {
        let scratch = scratch_dir("reqlic-malformed");
        fs::create_dir_all(scratch.join("prod")).unwrap();
        fs::write(scratch.join("prod/MANIFEST.json"), "{ not json !!!").unwrap();
        assert!(product_requires_license(scratch.to_str().unwrap(), "prod"));
        let _ = fs::remove_dir_all(&scratch);
    }

    #[test]
    fn requires_license_field_semantics() {
        let scratch = scratch_dir("reqlic-field");
        let dir = scratch.join("prod");
        fs::create_dir_all(&dir).unwrap();
        let manifest = dir.join("MANIFEST.json");

        // Explicit false — the ONLY way to open a product.
        fs::write(&manifest, r#"{"requires_license": false}"#).unwrap();
        assert!(!product_requires_license(scratch.to_str().unwrap(), "prod"));

        // Explicit true.
        fs::write(&manifest, r#"{"requires_license": true}"#).unwrap();
        assert!(product_requires_license(scratch.to_str().unwrap(), "prod"));

        // Field absent — secure default.
        fs::write(&manifest, r#"{"name": "prod"}"#).unwrap();
        assert!(product_requires_license(scratch.to_str().unwrap(), "prod"));

        // Field present but not a bool — secure default.
        fs::write(&manifest, r#"{"requires_license": "false"}"#).unwrap();
        assert!(product_requires_license(scratch.to_str().unwrap(), "prod"));

        let _ = fs::remove_dir_all(&scratch);
    }

    // ── Version helpers ───────────────────────────────────────────────────────

    #[test]
    fn compare_versions_is_numeric_per_segment() {
        use std::cmp::Ordering::*;
        // Numeric-per-segment, NOT string compare: "0.0.9" < "0.0.10"
        // (a plain string compare would say "0.0.10" < "0.0.9").
        assert_eq!(compare_versions("0.0.9", "0.0.10"), Less);
        assert_eq!(compare_versions("1.2.3", "1.10.0"), Less);
        assert_eq!(compare_versions("2.0.0", "1.99.99"), Greater);
        assert_eq!(compare_versions("1.2.3", "1.2.3"), Equal);
        // Differing segment counts: shorter prefix sorts first.
        assert_eq!(compare_versions("1.2", "1.2.0"), Less);
    }

    #[test]
    fn compare_versions_non_numeric_segment_parses_to_zero() {
        use std::cmp::Ordering::*;
        // Documented quirk: a non-numeric dir name like "beta" parses to [0].
        assert_eq!(compare_versions("beta", "0"), Equal);
        assert_eq!(compare_versions("beta", "0.0.1"), Less);
        assert_eq!(compare_versions("1.0.0", "beta"), Greater);
    }

    #[test]
    fn latest_version_picks_numeric_max_with_platform_present() {
        let scratch = scratch_dir("latest");
        let prod = scratch.join("prod");
        for v in ["0.0.1", "0.0.9", "0.0.10"] {
            let d = prod.join(v).join("linux-x86_64");
            fs::create_dir_all(d.parent().unwrap()).unwrap();
            fs::write(&d, b"bin").unwrap();
        }
        // Newer version that LACKS the platform must be skipped.
        fs::create_dir_all(prod.join("0.0.11")).unwrap();
        // Non-numeric dir with the platform present parses to [0] — never the max here.
        let beta = prod.join("beta").join("linux-x86_64");
        fs::create_dir_all(beta.parent().unwrap()).unwrap();
        fs::write(&beta, b"bin").unwrap();
        // A stray FILE in the product dir must be filtered (dirs only).
        fs::write(prod.join("README.txt"), b"x").unwrap();

        assert_eq!(
            latest_version_with_platform(scratch.to_str().unwrap(), "prod", "linux-x86_64"),
            Some("0.0.10".to_string())
        );
        // No version has this platform.
        assert_eq!(
            latest_version_with_platform(scratch.to_str().unwrap(), "prod", "darwin-arm64"),
            None
        );
        // Product dir missing entirely.
        assert_eq!(
            latest_version_with_platform(scratch.to_str().unwrap(), "ghost", "linux-x86_64"),
            None
        );
        let _ = fs::remove_dir_all(&scratch);
    }

    // ── load_verify_key ───────────────────────────────────────────────────────

    #[test]
    fn load_verify_key_accepts_raw_hex_and_file_path() {
        let sk = test_signing_key();
        let hex_key = hex::encode(sk.verifying_key().to_bytes());

        // Form 1: raw 64-char hex string.
        let vk = load_verify_key(&hex_key).expect("raw hex form must load");
        assert_eq!(vk.to_bytes(), sk.verifying_key().to_bytes());

        // Form 2: path to a file containing the hex (with surrounding whitespace).
        let scratch = scratch_dir("vkey");
        let key_file = scratch.join("verify.pub");
        fs::write(&key_file, format!("  {hex_key}\n")).unwrap();
        let vk2 = load_verify_key(key_file.to_str().unwrap()).expect("file form must load");
        assert_eq!(vk2.to_bytes(), sk.verifying_key().to_bytes());
        let _ = fs::remove_dir_all(&scratch);
    }

    #[test]
    fn load_verify_key_rejects_bad_inputs() {
        // Not hex and not an existing file.
        assert!(load_verify_key("/nonexistent/path/to/key").is_none());
        // File exists but contains non-hex garbage.
        let scratch = scratch_dir("vkey-bad");
        let bad = scratch.join("bad.pub");
        fs::write(&bad, "this is not hex at all\n").unwrap();
        assert!(load_verify_key(bad.to_str().unwrap()).is_none());
        // File contains hex of the wrong length (16 bytes, not 32).
        fs::write(&bad, "deadbeefdeadbeefdeadbeefdeadbeef\n").unwrap();
        assert!(load_verify_key(bad.to_str().unwrap()).is_none());
        let _ = fs::remove_dir_all(&scratch);
    }

    // ── load_revocation_list ──────────────────────────────────────────────────

    #[test]
    fn load_revocation_list_skips_comments_blanks_and_malformed_lines() {
        let scratch = scratch_dir("revlist");
        let fp_a = token_fingerprint("token-a");
        let fp_b_upper = token_fingerprint("token-b").to_uppercase();
        let list = scratch.join("revoked.txt");
        fs::write(
            &list,
            format!(
                "# comment line\n\
                 \n\
                 {fp_a}\n\
                 not-a-fingerprint\n\
                 {fp_b_upper}\n\
                 zzzz0000zzzz0000zzzz0000zzzz0000zzzz0000zzzz0000zzzz0000zzzz0000\n"
            ),
        )
        .unwrap();
        // Malformed lines warn-and-skip — they must NOT error the whole load.
        let set = load_revocation_list(list.to_str().unwrap()).expect("load must succeed");
        assert_eq!(set.len(), 2);
        assert!(set.contains(&fp_a));
        // Uppercase hex is normalised to lowercase.
        assert!(set.contains(&token_fingerprint("token-b")));
        let _ = fs::remove_dir_all(&scratch);
    }

    #[test]
    fn load_revocation_list_missing_file_is_not_found() {
        let err = load_revocation_list("/nonexistent/revoked.txt").unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
    }

    // ── token_fingerprint ─────────────────────────────────────────────────────

    #[test]
    fn token_fingerprint_hashes_encoded_string_not_decoded_bytes() {
        use sha2::{Digest, Sha256};
        let decoded = b"some raw token payload bytes";
        let encoded = URL_SAFE_NO_PAD.encode(decoded);

        let fp = token_fingerprint(&encoded);
        // Concrete before/after: the fingerprint IS sha256 of the base64url STRING…
        assert_eq!(fp, hex::encode(Sha256::digest(encoded.as_bytes())));
        // …and is NOT sha256 of the decoded bytes.
        assert_ne!(fp, hex::encode(Sha256::digest(decoded)));
        // Shape: 64 lowercase hex chars.
        assert_eq!(fp.len(), 64);
        assert!(fp
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_uppercase()));
    }

    // ── verify_license_key — every branch of the auth state machine ───────────

    #[test]
    fn verify_rejects_malformed_base64() {
        let vk = test_signing_key().verifying_key();
        let err = verify_license_key(&vk, "!!!not base64url!!!", "prod", &no_revocations())
            .err()
            .unwrap();
        assert_eq!(err.reason(), "malformed-token");
        assert_eq!(err.status(), StatusCode::UNAUTHORIZED);
        // Standard-base64 padding is also rejected by URL_SAFE_NO_PAD.
        let err = verify_license_key(&vk, "aGVsbG8=", "prod", &no_revocations())
            .err()
            .unwrap();
        assert_eq!(err.reason(), "malformed-token");
    }

    #[test]
    fn verify_rejects_token_too_short() {
        let vk = test_signing_key().verifying_key();
        // Exactly 64 decoded bytes: signature with an EMPTY payload — still too short.
        let exactly_64 = URL_SAFE_NO_PAD.encode([0u8; 64]);
        let err = verify_license_key(&vk, &exactly_64, "prod", &no_revocations())
            .err()
            .unwrap();
        assert_eq!(err.reason(), "token-too-short");
        assert_eq!(err.status(), StatusCode::UNAUTHORIZED);
        // Far shorter than a signature.
        let tiny = URL_SAFE_NO_PAD.encode(b"tiny");
        let err = verify_license_key(&vk, &tiny, "prod", &no_revocations())
            .err()
            .unwrap();
        assert_eq!(err.reason(), "token-too-short");
    }

    #[test]
    fn verify_rejects_invalid_signature() {
        let sk = test_signing_key();
        let vk = sk.verifying_key();
        let payload = payload_json("prod", "9999-99-99");

        // Signed by a DIFFERENT key.
        let forged = make_token(&other_signing_key(), &payload);
        let err = verify_license_key(&vk, &forged, "prod", &no_revocations())
            .err()
            .unwrap();
        assert_eq!(err.reason(), "invalid-signature");
        assert_eq!(err.status(), StatusCode::UNAUTHORIZED);

        // Valid signature but payload tampered after signing.
        let good = make_token(&sk, &payload);
        let mut bytes = URL_SAFE_NO_PAD.decode(&good).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 0x01;
        let tampered = URL_SAFE_NO_PAD.encode(bytes);
        let err = verify_license_key(&vk, &tampered, "prod", &no_revocations())
            .err()
            .unwrap();
        assert_eq!(err.reason(), "invalid-signature");
    }

    #[test]
    fn verify_rejects_invalid_json_payload() {
        let sk = test_signing_key();
        // Correctly signed, but the payload is not LicensePayload JSON.
        let token = make_token(&sk, "this is not json");
        let err = verify_license_key(&sk.verifying_key(), &token, "prod", &no_revocations())
            .err()
            .unwrap();
        assert_eq!(err.reason(), "invalid-payload");
        assert_eq!(err.status(), StatusCode::UNAUTHORIZED);

        // Valid JSON but missing required fields is also invalid-payload.
        let token = make_token(&sk, r#"{"product":"prod"}"#);
        let err = verify_license_key(&sk.verifying_key(), &token, "prod", &no_revocations())
            .err()
            .unwrap();
        assert_eq!(err.reason(), "invalid-payload");
    }

    #[test]
    fn verify_rejects_wrong_product() {
        let sk = test_signing_key();
        let token = make_token(&sk, &payload_json("product-a", "9999-99-99"));
        let err = verify_license_key(&sk.verifying_key(), &token, "product-b", &no_revocations())
            .err()
            .unwrap();
        assert_eq!(err.reason(), "wrong-product");
        assert_eq!(err.status(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn verify_checks_revocation_before_expiry() {
        // A token that is BOTH revoked AND expired must return Revoked, not
        // ChannelExpired — revocation is the stronger, permanent state.
        let sk = test_signing_key();
        let token = make_token(&sk, &payload_json("prod", "2020-01-01")); // long expired
        let revoked = RwLock::new(HashSet::from([token_fingerprint(&token)]));
        let err = verify_license_key(&sk.verifying_key(), &token, "prod", &revoked)
            .err()
            .unwrap();
        assert_eq!(err.reason(), "token-revoked");
        assert_eq!(err.status(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn verify_revocation_fingerprint_is_of_raw_b64_string_not_decoded_bytes() {
        use sha2::{Digest, Sha256};
        let sk = test_signing_key();
        let token = make_token(&sk, &payload_json("prod", "9999-99-99"));

        // Revoking the fingerprint of the DECODED bytes must NOT revoke the token.
        let decoded_bytes = URL_SAFE_NO_PAD.decode(&token).unwrap();
        let wrong_fp = hex::encode(Sha256::digest(&decoded_bytes));
        let revoked_wrong = RwLock::new(HashSet::from([wrong_fp]));
        assert!(
            verify_license_key(&sk.verifying_key(), &token, "prod", &revoked_wrong).is_ok(),
            "decoded-bytes fingerprint must not match — revocation keys on the encoded string"
        );

        // Computing it the documented way — SHA256 of the raw base64url STRING —
        // does revoke it.
        let right_fp = hex::encode(Sha256::digest(token.as_bytes()));
        assert_eq!(right_fp, token_fingerprint(&token));
        let revoked_right = RwLock::new(HashSet::from([right_fp]));
        let err = verify_license_key(&sk.verifying_key(), &token, "prod", &revoked_right)
            .err()
            .unwrap();
        assert_eq!(err.reason(), "token-revoked");
    }

    #[test]
    fn verify_expiry_is_string_lexicographic_not_date_parsing() {
        let sk = test_signing_key();
        let vk = sk.verifying_key();

        // "9999-99-99" is NOT a parseable date (month 99) — under date parsing it
        // would error, but lexicographically it exceeds any real YYYY-MM-DD, so
        // the token is accepted. Proves string compare, not chrono parsing.
        let token = make_token(&sk, &payload_json("prod", "9999-99-99"));
        assert!(verify_license_key(&vk, &token, "prod", &no_revocations()).is_ok());

        // "0000-00-00" is likewise unparseable but lexicographically below any
        // real date — always expired.
        let token = make_token(&sk, &payload_json("prod", "0000-00-00"));
        let err = verify_license_key(&vk, &token, "prod", &no_revocations())
            .err()
            .unwrap();
        assert_eq!(err.reason(), "channel-expired");
        assert_eq!(err.status(), StatusCode::FORBIDDEN);

        // Expiring exactly today is still valid (strict `<` comparison).
        let token = make_token(&sk, &payload_json("prod", &today_str()));
        assert!(verify_license_key(&vk, &token, "prod", &no_revocations()).is_ok());

        // Numeric-vs-lexicographic divergence: yesterday's date written with an
        // UNPADDED month (e.g. "2026-7-01") is in the past under date semantics,
        // but lexicographically "7" > "0…" makes it sort AFTER today — so the
        // string compare treats it as unexpired. Only constructible when
        // yesterday is in the same calendar year.
        let now = chrono::Utc::now();
        let yesterday = now - chrono::Duration::days(1);
        if yesterday.format("%Y").to_string() == now.format("%Y").to_string() {
            let unpadded = format!(
                "{}-{}-{}",
                yesterday.format("%Y"),
                yesterday.format("%-m"),
                yesterday.format("%d")
            );
            let token = make_token(&sk, &payload_json("prod", &unpadded));
            assert!(
                verify_license_key(&vk, &token, "prod", &no_revocations()).is_ok(),
                "unpadded past date {unpadded} must be treated as unexpired under \
                 lexicographic compare — if this fails, expiry semantics changed"
            );
        }
    }

    #[test]
    fn verify_success_path_returns_payload() {
        let sk = test_signing_key();
        let payload = r#"{"product":"prod","channel_expiry":"9999-12-31","entitlements":["source","updates"],"version_floor":"0.0.2"}"#;
        let token = make_token(&sk, payload);
        let Ok(p) = verify_license_key(&sk.verifying_key(), &token, "prod", &no_revocations())
        else {
            panic!("valid unexpired non-revoked token for the correct product must verify");
        };
        assert_eq!(p.product, "prod");
        assert_eq!(p.channel_expiry, "9999-12-31");
        assert_eq!(p.entitlements, vec!["source", "updates"]);
        assert_eq!(p.version_floor.as_deref(), Some("0.0.2"));
    }

    #[test]
    fn verify_version_floor_is_optional() {
        let sk = test_signing_key();
        let token = make_token(&sk, &payload_json("prod", "9999-12-31"));
        let Ok(p) = verify_license_key(&sk.verifying_key(), &token, "prod", &no_revocations())
        else {
            panic!("payload without version_floor must verify");
        };
        assert_eq!(p.version_floor, None);
    }

    // ── Route-shape tests (direct handler invocation — no TCP port) ───────────

    #[tokio::test]
    async fn healthz_shape() {
        let Json(body) = healthz().await;
        assert_eq!(body["status"], "ok");
        assert_eq!(body["service"], "app-privategit-source");
    }

    #[tokio::test]
    async fn install_script_404_on_missing_file() {
        let scratch = scratch_dir("install");
        let state = test_state(&scratch, None, None);
        let resp = install_script(State(state), Path("ghost".to_string())).await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        let body = body_json(resp).await;
        assert_eq!(body["error"], "not found");
        let _ = fs::remove_dir_all(&scratch);
    }

    #[tokio::test]
    async fn install_script_streams_when_present() {
        let scratch = scratch_dir("install-ok");
        fs::create_dir_all(scratch.join("prod")).unwrap();
        fs::write(scratch.join("prod/install.sh"), "#!/bin/sh\necho hi\n").unwrap();
        let state = test_state(&scratch, None, None);
        let resp = install_script(State(state), Path("prod".to_string())).await;
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.headers()[header::CONTENT_TYPE].to_str().unwrap(),
            "text/x-shellscript"
        );
        let _ = fs::remove_dir_all(&scratch);
    }

    #[tokio::test]
    async fn git_stub_returns_503_shape() {
        let (status, Json(body)) = git_stub().await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["error"], "smart-HTTP Git not yet enabled");
        assert!(body["see"].is_string());
        assert!(body["arriving"].is_string());
    }

    // ── /admin/reload-revocation-list ─────────────────────────────────────────

    fn loopback() -> ConnectInfo<SocketAddr> {
        ConnectInfo("127.0.0.1:54321".parse().unwrap())
    }

    fn non_loopback() -> ConnectInfo<SocketAddr> {
        ConnectInfo("203.0.113.9:54321".parse().unwrap())
    }

    #[tokio::test]
    async fn reload_revocation_rejects_non_loopback() {
        let scratch = scratch_dir("reload-remote");
        let list = scratch.join("revoked.txt");
        fs::write(&list, "").unwrap();
        let state = test_state(&scratch, None, Some(list.to_string_lossy().into_owned()));
        let resp = reload_revocation_list(non_loopback(), State(state)).await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
        let body = body_json(resp).await;
        assert_eq!(body["error"], "admin endpoints are localhost-only");
        let _ = fs::remove_dir_all(&scratch);
    }

    #[tokio::test]
    async fn reload_revocation_404_when_not_configured() {
        let scratch = scratch_dir("reload-nopath");
        let state = test_state(&scratch, None, None);
        let resp = reload_revocation_list(loopback(), State(state)).await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        let body = body_json(resp).await;
        assert_eq!(body["error"], "no revocation list configured");
        let _ = fs::remove_dir_all(&scratch);
    }

    #[tokio::test]
    async fn reload_revocation_enoent_is_404_other_io_error_is_500() {
        let scratch = scratch_dir("reload-errs");

        // ENOENT — configured path does not exist -> 404, distinct message.
        let missing = scratch.join("ghost.txt");
        let state = test_state(&scratch, None, Some(missing.to_string_lossy().into_owned()));
        let resp = reload_revocation_list(loopback(), State(state)).await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        let body = body_json(resp).await;
        assert_eq!(body["error"], "revocation list file not found");

        // Other I/O error — path is a DIRECTORY, read_to_string fails but not
        // with NotFound -> 500.
        let dir_path = scratch.join("a-directory");
        fs::create_dir_all(&dir_path).unwrap();
        let state = test_state(
            &scratch,
            None,
            Some(dir_path.to_string_lossy().into_owned()),
        );
        let resp = reload_revocation_list(loopback(), State(state)).await;
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);

        let _ = fs::remove_dir_all(&scratch);
    }

    #[tokio::test]
    async fn reload_revocation_success_replaces_state() {
        let scratch = scratch_dir("reload-ok");
        let list = scratch.join("revoked.txt");
        let fp = token_fingerprint("some-token");
        fs::write(&list, format!("# header\n{fp}\n")).unwrap();
        let state = test_state(&scratch, None, Some(list.to_string_lossy().into_owned()));
        // Pre-seed with a stale entry that must be REPLACED, not merged.
        state.revoked_tokens.write().unwrap().insert("f".repeat(64));

        let resp = reload_revocation_list(loopback(), State(state.clone())).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let body = body_json(resp).await;
        assert_eq!(body["reloaded"], 1);
        let now = state.revoked_tokens.read().unwrap();
        assert_eq!(now.len(), 1);
        assert!(now.contains(&fp));
        let _ = fs::remove_dir_all(&scratch);
    }

    // ── POST /verify-key endpoint ─────────────────────────────────────────────

    #[tokio::test]
    async fn verify_key_endpoint_503_when_key_not_configured() {
        let scratch = scratch_dir("vke-503");
        let state = test_state(&scratch, None, None);
        let (status, Json(body)) = verify_key_endpoint(
            State(state),
            Json(VerifyKeyRequest {
                license_key_b64: "anything".into(),
                product_id: "prod".into(),
            }),
        )
        .await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert!(body["error"].as_str().unwrap().contains("VERIFY_KEY_PUB"));
        let _ = fs::remove_dir_all(&scratch);
    }

    #[tokio::test]
    async fn verify_key_endpoint_success_and_expired_shapes() {
        let scratch = scratch_dir("vke-ok");
        let sk = test_signing_key();
        let state = test_state(&scratch, Some(sk.verifying_key()), None);

        // Success shape.
        let token = make_token(&sk, &payload_json("prod", "9999-12-31"));
        let (status, Json(body)) = verify_key_endpoint(
            State(state.clone()),
            Json(VerifyKeyRequest {
                license_key_b64: token,
                product_id: "prod".into(),
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["valid"], true);
        assert_eq!(body["product"], "prod");
        assert_eq!(body["channel_expiry"], "9999-12-31");
        assert_eq!(body["entitlements"], json!(["source"]));

        // Expired shape carries the expired date back.
        let token = make_token(&sk, &payload_json("prod", "2020-01-01"));
        let (status, Json(body)) = verify_key_endpoint(
            State(state),
            Json(VerifyKeyRequest {
                license_key_b64: token,
                product_id: "prod".into(),
            }),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["valid"], false);
        assert_eq!(body["reason"], "channel expired");
        assert_eq!(body["expired"], "2020-01-01");
        let _ = fs::remove_dir_all(&scratch);
    }

    // ── GET /verify-key.pub ───────────────────────────────────────────────────

    #[tokio::test]
    async fn verify_key_pub_shapes() {
        let scratch = scratch_dir("vkp");
        let sk = test_signing_key();

        let state = test_state(&scratch, Some(sk.verifying_key()), None);
        let resp = verify_key_pub(State(state)).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(
            String::from_utf8(bytes.to_vec()).unwrap(),
            hex::encode(sk.verifying_key().to_bytes())
        );

        let state = test_state(&scratch, None, None);
        let resp = verify_key_pub(State(state)).await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        let _ = fs::remove_dir_all(&scratch);
    }

    // ── GET /releases/:p/:v/:platform (binary handler auth flow) ──────────────

    #[tokio::test]
    async fn binary_requires_auth_when_license_required() {
        let scratch = scratch_dir("bin-401");
        let sk = test_signing_key();
        let state = test_state(&scratch, Some(sk.verifying_key()), None);
        let resp = binary(
            State(state),
            HeaderMap::new(),
            Path((
                "prod".to_string(),
                "0.0.1".to_string(),
                "linux-x86_64".to_string(),
            )),
            Query(BinaryQuery::default()),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        let body = body_json(resp).await;
        assert_eq!(body["error"], "license key required");
        let _ = fs::remove_dir_all(&scratch);
    }

    #[tokio::test]
    async fn binary_503_when_verify_key_unconfigured() {
        let scratch = scratch_dir("bin-503");
        let state = test_state(&scratch, None, None);
        let resp = binary(
            State(state),
            HeaderMap::new(),
            Path((
                "prod".to_string(),
                "0.0.1".to_string(),
                "linux-x86_64".to_string(),
            )),
            Query(BinaryQuery {
                token: Some("sometoken".into()),
            }),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
        let _ = fs::remove_dir_all(&scratch);
    }

    #[tokio::test]
    async fn binary_open_product_serves_without_auth() {
        let scratch = scratch_dir("bin-open");
        let prod = scratch.join("prod");
        fs::create_dir_all(prod.join("0.0.1")).unwrap();
        fs::write(prod.join("MANIFEST.json"), r#"{"requires_license": false}"#).unwrap();
        fs::write(prod.join("0.0.1/linux-x86_64"), b"binary-bytes").unwrap();
        let state = test_state(&scratch, None, None);
        let resp = binary(
            State(state),
            HeaderMap::new(),
            Path((
                "prod".to_string(),
                "0.0.1".to_string(),
                "linux-x86_64".to_string(),
            )),
            Query(BinaryQuery::default()),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.headers()[header::CONTENT_TYPE].to_str().unwrap(),
            "application/octet-stream"
        );
        let _ = fs::remove_dir_all(&scratch);
    }

    #[tokio::test]
    async fn binary_sig_files_are_unauthenticated() {
        let scratch = scratch_dir("bin-sig");
        // Product REQUIRES a license, but detached .sig files are served openly.
        let prod = scratch.join("prod");
        fs::create_dir_all(prod.join("0.0.1")).unwrap();
        fs::write(prod.join("0.0.1/linux-x86_64.sig"), b"detached-sig").unwrap();
        let state = test_state(&scratch, None, None);
        let resp = binary(
            State(state),
            HeaderMap::new(),
            Path((
                "prod".to_string(),
                "0.0.1".to_string(),
                "linux-x86_64.sig".to_string(),
            )),
            Query(BinaryQuery::default()),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let _ = fs::remove_dir_all(&scratch);
    }

    #[tokio::test]
    async fn binary_valid_token_but_missing_file_is_404() {
        let scratch = scratch_dir("bin-404");
        let sk = test_signing_key();
        let state = test_state(&scratch, Some(sk.verifying_key()), None);
        let token = make_token(&sk, &payload_json("prod", "9999-12-31"));
        let resp = binary(
            State(state),
            HeaderMap::new(),
            Path((
                "prod".to_string(),
                "0.0.1".to_string(),
                "linux-x86_64".to_string(),
            )),
            Query(BinaryQuery { token: Some(token) }),
        )
        .await;
        // Auth passed; the file simply is not there.
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        let body = body_json(resp).await;
        assert_eq!(body["error"], "binary not found");
        let _ = fs::remove_dir_all(&scratch);
    }

    #[tokio::test]
    async fn binary_revoked_token_is_403() {
        let scratch = scratch_dir("bin-revoked");
        let sk = test_signing_key();
        let state = test_state(&scratch, Some(sk.verifying_key()), None);
        let token = make_token(&sk, &payload_json("prod", "9999-12-31"));
        state
            .revoked_tokens
            .write()
            .unwrap()
            .insert(token_fingerprint(&token));
        let resp = binary(
            State(state),
            HeaderMap::new(),
            Path((
                "prod".to_string(),
                "0.0.1".to_string(),
                "linux-x86_64".to_string(),
            )),
            Query(BinaryQuery { token: Some(token) }),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
        let body = body_json(resp).await;
        assert_eq!(body["error"], "token-revoked");
        let _ = fs::remove_dir_all(&scratch);
    }
}
