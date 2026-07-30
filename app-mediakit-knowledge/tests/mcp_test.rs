use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;
use tower::ServiceExt;

use app_mediakit_knowledge::server::{router, AppState};

async fn fixture() -> (AppState, TempDir, TempDir) {
    let content_dir = tempfile::tempdir().unwrap();
    let state_dir = tempfile::tempdir().unwrap();

    let repo = app_mediakit_knowledge::git::open_or_init(content_dir.path()).unwrap();
    app_mediakit_knowledge::git::ensure_commit_identity_from_env(&repo).unwrap();

    let md = "---\ntitle: \"Test Topic\"\nslug: topic-test\ncategory: architecture\n---\n\nBody text here.";
    tokio::fs::write(content_dir.path().join("topic-test.md"), md)
        .await
        .unwrap();
    app_mediakit_knowledge::git::commit_topic(
        &repo,
        "topic-test",
        md,
        "j@woodfine.com",
        "Jennifer",
        "initial",
    )
    .unwrap();

    let search = app_mediakit_knowledge::search::build_index(content_dir.path(), state_dir.path())
        .await
        .unwrap();

    (
        AppState {
            mounts: app_mediakit_knowledge::mounts::resolve(content_dir.path(), None, None),
            citations_yaml: PathBuf::from("/nonexistent/citations.yaml"),
            search: Arc::new(search),
            git: Arc::new(Mutex::new(repo)),
            site_title: "Test Wiki".to_string(),
            git_tenant: "pointsav".to_string(),
            mcp_enabled: true,
            glossary: Arc::new(app_mediakit_knowledge::glossary::Glossary::default()),
            links: app_mediakit_knowledge::links::LinkGraph::for_testing(),
            brand_theme: None,
            brand_instance: "documentation".to_string(),
            blueprints: app_mediakit_knowledge::blueprints::Registry::builtin(),
            peers: vec![],
            canonical_url: None,
            activitypub_outbox_url: None,
            start_here: vec![],
        },
        content_dir,
        state_dir,
    )
}

async fn post_mcp(app: axum::Router, body: Value) -> Value {
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/mcp")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

// ─── initialize handshake ───────────────────────────────────────────────────

#[tokio::test]
async fn test_initialize_handshake() {
    let (state, _cd, _sd) = fixture().await;
    let app = router(state);
    let resp = post_mcp(
        app,
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": { "name": "test", "version": "1.0" }
            }
        }),
    )
    .await;
    assert_eq!(resp["jsonrpc"], "2.0");
    assert_eq!(resp["id"], 1);
    assert_eq!(resp["result"]["protocolVersion"], "2024-11-05");
    assert!(resp["result"]["serverInfo"]["name"]
        .as_str()
        .unwrap()
        .contains("app-mediakit-knowledge"));
    assert!(resp["result"]["capabilities"]["tools"].is_object());
    assert!(resp["result"]["capabilities"]["resources"].is_object());
    assert!(resp["result"]["capabilities"]["prompts"].is_object());
}

// ─── tools/list ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_tools_list_returns_three() {
    let (state, _cd, _sd) = fixture().await;
    let app = router(state);
    let resp = post_mcp(
        app,
        json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {} }),
    )
    .await;
    let tools = resp["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 3);
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"create_topic"));
    assert!(names.contains(&"propose_edit"));
    assert!(names.contains(&"link_citation"));
}

// ─── resources/read ─────────────────────────────────────────────────────────

#[tokio::test]
async fn test_resources_read_topic() {
    let (state, _cd, _sd) = fixture().await;
    let app = router(state);
    let resp = post_mcp(
        app,
        json!({
            "jsonrpc": "2.0",
            "id": 4,
            "method": "resources/read",
            "params": { "uri": "wiki://topic/topic-test" }
        }),
    )
    .await;
    assert!(
        resp["error"].is_null(),
        "unexpected error: {}",
        resp["error"]
    );
    let contents = &resp["result"]["contents"];
    assert!(contents.is_array());
    assert_eq!(contents[0]["mimeType"], "text/markdown");
    let text = contents[0]["text"].as_str().unwrap();
    assert!(
        text.contains("Test Topic"),
        "expected 'Test Topic' in body, got: {text}"
    );
}

// ─── invalid method → JSON-RPC error ────────────────────────────────────────

#[tokio::test]
async fn test_unknown_method_returns_error() {
    let (state, _cd, _sd) = fixture().await;
    let app = router(state);
    let resp = post_mcp(
        app,
        json!({ "jsonrpc": "2.0", "id": 5, "method": "no/such/method", "params": {} }),
    )
    .await;
    assert!(resp["result"].is_null());
    assert_eq!(resp["error"]["code"], -32601);
    let msg = resp["error"]["message"].as_str().unwrap();
    assert!(
        msg.contains("method not found"),
        "unexpected error message: {msg}"
    );
}

// ─── mcp_enabled = false → 404 ──────────────────────────────────────────────

#[tokio::test]
async fn test_mcp_disabled_returns_not_found() {
    let (mut state, _cd, _sd) = fixture().await;
    state.mcp_enabled = false;
    let app = router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/mcp")
                .header("content-type", "application/json")
                .body(axum::body::Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    // When the /mcp route is not mounted, axum returns 405 (route path
    // exists for other methods) or 404. Either signals the endpoint is absent.
    assert!(
        resp.status() == StatusCode::NOT_FOUND || resp.status() == StatusCode::METHOD_NOT_ALLOWED,
        "expected 404/405 when mcp disabled, got {}",
        resp.status()
    );
}
