//! Integration tests for Phase 2 Step 6 — three-keystroke ladder stubs.
//!
//! Verifies that `POST /api/doorman/complete` and `POST /api/doorman/instruct`
//! both return `501 Not Implemented` with the expected JSON body shape.

use app_mediakit_knowledge::search;
use app_mediakit_knowledge::server::{router, AppState};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tower::ServiceExt;

/// Build a minimal AppState for doorman stub tests.
async fn doorman_state() -> (AppState, tempfile::TempDir, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let state_dir = tempfile::tempdir().unwrap();
    let index = search::build_index(dir.path(), state_dir.path())
        .await
        .unwrap();
    let repo = app_mediakit_knowledge::git::open_or_init(dir.path()).unwrap();
    (
        AppState {
            mounts: app_mediakit_knowledge::mounts::resolve(dir.path(), None, None),
            citations_yaml: PathBuf::from("/nonexistent/citations.yaml"),
            search: Arc::new(index),
            git: Arc::new(Mutex::new(repo)),
            git_tenant: "pointsav".to_string(),
            mcp_enabled: false,
            glossary: Arc::new(app_mediakit_knowledge::glossary::Glossary::default()),
            links: app_mediakit_knowledge::links::LinkGraph::for_testing(),
            brand_theme: None,
            brand_instance: "documentation".to_string(),
            site_title: "PointSav Documentation Wiki".to_string(),
            blueprints: app_mediakit_knowledge::blueprints::Registry::builtin(),
            peers: vec![],
            canonical_url: None,
            activitypub_outbox_url: None,
            start_here: vec![],
        },
        dir,
        state_dir,
    )
}

/// `POST /api/doorman/complete` returns 501 Not Implemented.
#[tokio::test]
async fn doorman_complete_returns_501() {
    let (state, _dir, _state_dir) = doorman_state().await;
    let app = router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/doorman/complete")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"context":"some text"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        resp.status(),
        StatusCode::NOT_IMPLEMENTED,
        "expected 501 from /api/doorman/complete"
    );
}

/// `POST /api/doorman/instruct` returns 501 Not Implemented.
#[tokio::test]
async fn doorman_instruct_returns_501() {
    let (state, _dir, _state_dir) = doorman_state().await;
    let app = router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/doorman/instruct")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"selection":"","instruction":"summarise"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        resp.status(),
        StatusCode::NOT_IMPLEMENTED,
        "expected 501 from /api/doorman/instruct"
    );
}

/// Both doorman stubs return a JSON body containing `phase: 4` and a
/// non-empty `reason` string.
#[tokio::test]
async fn doorman_stubs_return_correct_json_shape() {
    for path in ["/api/doorman/complete", "/api/doorman/instruct"] {
        let (state, _dir, _state_dir) = doorman_state().await;
        let app = router(state);
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(path)
                    .header("content-type", "application/json")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(
            resp.status(),
            StatusCode::NOT_IMPLEMENTED,
            "{path} should return 501"
        );

        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let parsed: serde_json::Value =
            serde_json::from_slice(&body).expect("doorman stub should return valid JSON");

        let phase = parsed.get("phase").and_then(|v| v.as_u64());
        assert_eq!(
            phase,
            Some(4),
            "{path}: expected phase == 4 in response body, got: {parsed}"
        );

        let reason = parsed.get("reason").and_then(|v| v.as_str());
        assert!(
            reason.map(|r| !r.is_empty()).unwrap_or(false),
            "{path}: expected non-empty reason string, got: {parsed}"
        );
    }
}
