use anyhow::Result;
use axum::{
    response::Json,
    routing::get,
    Router,
};
use ed25519_dalek::VerifyingKey;
use serde_json::{json, Value};
use std::{
    collections::HashSet,
    fs,
    net::SocketAddr,
    path::PathBuf,
    sync::Arc,
};

// ── State ─────────────────────────────────────────────────────────────────────

#[derive(Clone)]
struct AppState {
    #[allow(dead_code)]
    releases_dir: PathBuf,
    #[allow(dead_code)]
    verify_key: Option<VerifyingKey>,
    #[allow(dead_code)]
    revoked_tokens: HashSet<String>,
    bind_addr: String,
}

// ── Config helpers ──────────────────────────────────────────────────────────────

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
                    "revocation list: skipping non-fingerprint line (expected 64-char SHA256 hex)"
                );
                return None;
            }
            Some(lower)
        })
        .collect();
    Ok(set)
}

// ── Handlers ──────────────────────────────────────────────────────────────────

async fn healthz() -> Json<Value> {
    Json(json!({"status": "ok", "service": "app-privategit-source-2"}))
}

// ── Main ──────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()))
        .init();

    // Same env var names and defaults as the old crate, except the default bind
    // address points at a test port (19201) so the ground-up rewrite never
    // collides with the live app-privategit-source service on 9201.
    let bind_addr = std::env::var("SOURCE_BIND").unwrap_or_else(|_| "127.0.0.1:19201".into());
    let releases_dir = PathBuf::from(
        std::env::var("RELEASES_DIR").unwrap_or_else(|_| "/var/lib/local-software/releases".into()),
    );

    let verify_key = std::env::var("VERIFY_KEY_PUB")
        .ok()
        .and_then(|path| load_verify_key(&path));
    if verify_key.is_none() {
        tracing::warn!("VERIFY_KEY_PUB not set — license verification unconfigured");
    }

    let revoked_tokens = match std::env::var("REVOCATION_LIST_PATH")
        .ok()
        .filter(|p| !p.is_empty())
    {
        None => HashSet::new(),
        Some(path) => match load_revocation_list(&path) {
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
    };

    let state = Arc::new(AppState {
        releases_dir,
        verify_key,
        revoked_tokens,
        bind_addr,
    });

    let app = Router::new()
        .route("/healthz", get(healthz))
        .with_state(state.clone());

    tracing::info!("app-privategit-source-2 listening on {}", state.bind_addr);
    let listener = tokio::net::TcpListener::bind(&state.bind_addr).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;
    Ok(())
}
