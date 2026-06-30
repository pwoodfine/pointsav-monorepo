//! Integration tests for Phase 2 Step 5 — citation autocomplete endpoint.
//!
//! Tests hit the HTTP layer (`GET /api/citations`) via `tower::ServiceExt::oneshot`
//! so they exercise the full axum router, AppState, and citations module path.
//!
//! The tests depend on the real `/srv/foundry/citations.yaml` file (the live
//! workspace registry). This is intentional for Phase 2: the test suite runs
//! on the workspace VM where that file is always present, and checking against
//! a known-stable ID (`ni-51-102`) gives meaningful end-to-end confidence that
//! the YAML is being parsed correctly.

use app_mediakit_knowledge::search;
use app_mediakit_knowledge::server::{router, AppState};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tower::ServiceExt;

/// Build a minimal AppState for citation tests.
///
/// `content_dir` is a scratch directory (no pages needed for this test path).
/// `citations_yaml` points to the real workspace registry.
async fn citation_state() -> (AppState, tempfile::TempDir, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let state_dir = tempfile::tempdir().unwrap();
    let index = search::build_index(dir.path(), state_dir.path())
        .await
        .unwrap();
    let repo = app_mediakit_knowledge::git::open_or_init(dir.path()).unwrap();
    (
        AppState {
            mounts: app_mediakit_knowledge::mounts::resolve(dir.path(), None, None),
            citations_yaml: PathBuf::from("/srv/foundry/citations.yaml"),
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
            site_categories: vec![],
        },
        dir,
        state_dir,
    )
}

/// `GET /api/citations` returns HTTP 200 with Content-Type application/json.
#[tokio::test]
async fn get_citations_returns_json() {
    let (state, _dir, _state_dir) = citation_state().await;
    let app = router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/citations")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "expected 200 from /api/citations"
    );

    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        content_type.contains("application/json"),
        "expected application/json content-type, got: {content_type}"
    );
}

/// `GET /api/citations` returns a JSON array (not null, not an object).
#[tokio::test]
async fn get_citations_returns_array() {
    let (state, _dir, _state_dir) = citation_state().await;
    let app = router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/citations")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let parsed: serde_json::Value =
        serde_json::from_slice(&body).expect("response body must be valid JSON");
    assert!(parsed.is_array(), "expected JSON array, got: {}", parsed);
    let arr = parsed.as_array().unwrap();
    assert!(
        !arr.is_empty(),
        "citation registry should contain at least one entry"
    );
}

/// The well-known entry `ni-51-102` is present in the response array.
///
/// `ni-51-102` (National Instrument 51-102 — Continuous Disclosure Obligations)
/// is a foundational citation that appears in CLAUDE.md §6 and is confirmed
/// present in `/srv/foundry/citations.yaml`.
#[tokio::test]
async fn get_citations_contains_ni_51_102() {
    let (state, _dir, _state_dir) = citation_state().await;
    let app = router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/citations")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let entries: Vec<serde_json::Value> =
        serde_json::from_slice(&body).expect("response body must be a valid JSON array");

    let found = entries
        .iter()
        .any(|e| e.get("id").and_then(|v| v.as_str()) == Some("ni-51-102"));
    assert!(
        found,
        "expected ni-51-102 entry in /api/citations response; got {} entries",
        entries.len()
    );
}

/// Every entry in the array has at minimum `id` and `title` string fields.
#[tokio::test]
async fn every_citation_entry_has_id_and_title() {
    let (state, _dir, _state_dir) = citation_state().await;
    let app = router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/citations")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let entries: Vec<serde_json::Value> =
        serde_json::from_slice(&body).expect("response body must be a valid JSON array");

    for (i, entry) in entries.iter().enumerate() {
        let id = entry.get("id").and_then(|v| v.as_str());
        assert!(
            id.is_some() && !id.unwrap().is_empty(),
            "entry[{i}] is missing a non-empty `id` field"
        );
        let title = entry.get("title").and_then(|v| v.as_str());
        assert!(
            title.is_some(),
            "entry[{i}] (id={:?}) is missing a `title` field",
            id
        );
    }
}
