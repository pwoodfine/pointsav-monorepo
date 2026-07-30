use axum::{
    extract::{Json, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone)]
struct Config {
    module_id: String,
    ledger_root: String,
    watch_drop_dir: String,
}

#[derive(Deserialize)]
struct AppendRequest {
    payload_id: String,
    payload: Value,
}

#[derive(Serialize)]
struct AppendResponse {
    payload_id: String,
    module_id: String,
    sha256: String,
    ts: u64,
    ledger_root: String,
}

async fn healthz(State(cfg): State<Arc<Config>>) -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "envelope": "A",
        "module_id": cfg.module_id,
    }))
}

async fn append(
    State(cfg): State<Arc<Config>>,
    headers: HeaderMap,
    Json(req): Json<AppendRequest>,
) -> impl IntoResponse {
    let module_id = headers
        .get("x-foundry-module-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| cfg.module_id.clone());

    let payload_str = req.payload.to_string();
    let mut hasher = Sha256::new();
    hasher.update(payload_str.as_bytes());
    let sha = hex::encode(hasher.finalize());

    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Atomic WORM ledger append: write to .tmp then rename
    let ledger_dir = format!("{}/{}", cfg.ledger_root, module_id);
    if let Err(e) = std::fs::create_dir_all(&ledger_dir) {
        eprintln!("[service-fs] create_dir_all {ledger_dir}: {e}");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    let ledger_path = format!("{}/log.jsonl", ledger_dir);
    let record = format!(
        "{}\n",
        serde_json::json!({
            "payload_id": req.payload_id,
            "module_id": module_id,
            "sha256": sha,
            "ts": ts,
        })
    );
    let existing = std::fs::read_to_string(&ledger_path).unwrap_or_default();
    let tmp_path = format!("{}.tmp", ledger_path);
    if let Err(e) = std::fs::write(&tmp_path, format!("{}{}", existing, record)) {
        eprintln!("[service-fs] write {tmp_path}: {e}");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    if let Err(e) = std::fs::rename(&tmp_path, &ledger_path) {
        eprintln!("[service-fs] rename {tmp_path} → {ledger_path}: {e}");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    // Drop payload to watch dir for service-extraction pickup
    if let Err(e) = std::fs::create_dir_all(&cfg.watch_drop_dir) {
        eprintln!("[service-fs] create_dir_all {}: {e}", cfg.watch_drop_dir);
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    let drop_path = format!("{}/{}.json", cfg.watch_drop_dir, req.payload_id);
    if let Err(e) = std::fs::write(&drop_path, &payload_str) {
        eprintln!("[service-fs] write {drop_path}: {e}");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    println!(
        "[service-fs] {module_id}/{} → sha256:{}",
        req.payload_id,
        &sha[..12]
    );

    Json(AppendResponse {
        payload_id: req.payload_id,
        module_id,
        sha256: sha,
        ts,
        ledger_root: cfg.ledger_root.clone(),
    })
    .into_response()
}

#[tokio::main]
async fn main() {
    let bind = std::env::var("FS_BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:9100".into());
    let module_id = std::env::var("FS_MODULE_ID").unwrap_or_else(|_| "local".into());
    let ledger_root = std::env::var("FS_LEDGER_ROOT")
        .unwrap_or_else(|_| "/var/lib/pointsav/service-fs/worm".into());
    let watch_drop_dir = std::env::var("FS_WATCH_DROP_DIR")
        .unwrap_or_else(|_| "/var/lib/pointsav/service-extraction/watch".into());

    let cfg = Arc::new(Config {
        module_id,
        ledger_root,
        watch_drop_dir,
    });

    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/v1/append", post(append))
        .with_state(cfg.clone());

    println!(
        "[service-fs] Envelope A ready on {bind} (module: {})",
        cfg.module_id
    );
    let listener = tokio::net::TcpListener::bind(&bind)
        .await
        .unwrap_or_else(|e| panic!("Cannot bind {bind}: {e}"));
    axum::serve(listener, app).await.unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Method, Request};
    use tower::ServiceExt;

    fn test_cfg(ledger_root: &str, watch_dir: &str) -> Arc<Config> {
        Arc::new(Config {
            module_id: "test".into(),
            ledger_root: ledger_root.into(),
            watch_drop_dir: watch_dir.into(),
        })
    }

    fn test_app(cfg: Arc<Config>) -> Router {
        Router::new()
            .route("/healthz", get(healthz))
            .route("/v1/append", post(append))
            .with_state(cfg)
    }

    #[tokio::test]
    async fn healthz_ok() {
        let cfg = test_cfg("/tmp", "/tmp");
        let app = test_app(cfg);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/healthz")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test]
    async fn append_creates_jsonl() {
        let dir = tempfile::tempdir().unwrap();
        let ledger_root = dir.path().join("worm").to_string_lossy().into_owned();
        let watch_dir = dir.path().join("watch").to_string_lossy().into_owned();
        let cfg = test_cfg(&ledger_root, &watch_dir);
        let app = test_app(cfg);

        let body = serde_json::json!({
            "payload_id": "test-001",
            "payload": {"file": {"filename": "test.md", "data": "aGVsbG8="}}
        });
        let resp = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/v1/append")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);

        let log_path = format!("{}/test/log.jsonl", ledger_root);
        assert!(
            std::path::Path::new(&log_path).exists(),
            "WORM ledger not created"
        );
    }

    #[tokio::test]
    async fn append_drops_watch_file() {
        let dir = tempfile::tempdir().unwrap();
        let ledger_root = dir.path().join("worm").to_string_lossy().into_owned();
        let watch_dir = dir.path().join("watch").to_string_lossy().into_owned();
        let cfg = test_cfg(&ledger_root, &watch_dir);
        let app = test_app(cfg);

        let body = serde_json::json!({
            "payload_id": "test-drop-001",
            "payload": {"file": {"filename": "hello.md", "data": "aGVsbG8="}}
        });
        let resp = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/v1/append")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);

        let drop_path = format!("{}/test-drop-001.json", watch_dir);
        assert!(
            std::path::Path::new(&drop_path).exists(),
            "watch dir file not dropped"
        );
    }
}
