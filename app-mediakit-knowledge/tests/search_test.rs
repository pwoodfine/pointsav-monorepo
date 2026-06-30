//! Integration tests for Phase 3 Step 3.2 — search HTTP route + reindex.
//!
//! Verifies that:
//! - `GET /search?q=...` returns an HTML page with results from the index
//! - The empty `q=` case returns the form without errors
//! - `search::reindex_topic` updates the live index so subsequent searches see
//!   the new body (the file-watcher path; the removed write endpoints used the
//!   same function before the git-only workflow landed)

use app_mediakit_knowledge::search;
use app_mediakit_knowledge::server::{router, AppState};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use std::sync::{Arc, Mutex};
use tower::ServiceExt;

async fn fixture_state() -> (AppState, tempfile::TempDir, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let state_dir = tempfile::tempdir().unwrap();
    // Seed three TOPICs covering distinct keywords for ranking + miss tests.
    tokio::fs::write(
        dir.path().join("topic-alpha.md"),
        "---\ntitle: \"Alpha Subject\"\nslug: topic-alpha\n---\nAlpha discusses the substrate. Substrate is the load-bearing concept.\n",
    )
    .await
    .unwrap();
    tokio::fs::write(
        dir.path().join("topic-beta.md"),
        "---\ntitle: \"Beta Subject\"\nslug: topic-beta\n---\nBeta covers continuous disclosure and the BCSC posture.\n",
    )
    .await
    .unwrap();
    tokio::fs::write(
        dir.path().join("topic-gamma.md"),
        "---\ntitle: \"Gamma Subject\"\nslug: topic-gamma\n---\nGamma is unrelated content for control.\n",
    )
    .await
    .unwrap();
    let index = search::build_index(dir.path(), state_dir.path())
        .await
        .unwrap();
    let repo = app_mediakit_knowledge::git::open_or_init(dir.path()).unwrap();
    let state = AppState {
        mounts: app_mediakit_knowledge::mounts::resolve(dir.path(), None, None),
        citations_yaml: std::path::PathBuf::from("/nonexistent/citations.yaml"),
        search: Arc::new(index),
        git: Arc::new(Mutex::new(repo)),
        site_title: "PointSav Documentation Wiki".to_string(),
        git_tenant: "pointsav".to_string(),
        mcp_enabled: false,
        glossary: Arc::new(app_mediakit_knowledge::glossary::Glossary::default()),
        links: app_mediakit_knowledge::links::LinkGraph::for_testing(),
        brand_theme: None,
        brand_instance: "documentation".to_string(),
        blueprints: app_mediakit_knowledge::blueprints::Registry::builtin(),
        peers: vec![],
        canonical_url: None,
        activitypub_outbox_url: None,
        start_here: vec![],
        site_categories: vec![],
    };
    (state, dir, state_dir)
}

#[tokio::test]
async fn search_with_no_query_returns_empty_form() {
    let (state, _dir, _state_dir) = fixture_state().await;
    let app = router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/search")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let html = std::str::from_utf8(&resp.into_body().collect().await.unwrap().to_bytes())
        .unwrap()
        .to_string();
    assert!(html.contains("<form"), "search form missing: {html}");
    assert!(html.contains(r#"name="q""#), "search input missing: {html}");
    // No results section yet
    assert!(
        !html.contains("search-results"),
        "results list should not appear for empty query: {html}"
    );
}

#[tokio::test]
async fn search_returns_matching_topic() {
    let (state, _dir, _state_dir) = fixture_state().await;
    let app = router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/search?q=substrate")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let html = std::str::from_utf8(&resp.into_body().collect().await.unwrap().to_bytes())
        .unwrap()
        .to_string();
    assert!(
        html.contains("topic-alpha"),
        "alpha should match 'substrate': {html}"
    );
    assert!(
        html.contains("search-results"),
        "results list should render: {html}"
    );
}

#[tokio::test]
async fn search_returns_empty_for_no_match() {
    let (state, _dir, _state_dir) = fixture_state().await;
    let app = router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/search?q=xyzzy-no-such-term")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let html = std::str::from_utf8(&resp.into_body().collect().await.unwrap().to_bytes())
        .unwrap()
        .to_string();
    assert!(html.contains("No results"), "no-match copy missing: {html}");
}

#[tokio::test]
async fn reindex_topic_updates_live_index() {
    let (state, dir, _state_dir) = fixture_state().await;

    // Rewrite topic-alpha on disk: drop "substrate", add a unique new keyword,
    // then reindex through the same function the file-watcher uses.
    let new_body = "---\ntitle: \"Alpha v2\"\nslug: topic-alpha\n---\nAlpha now discusses tangerines and quokkas.\n";
    tokio::fs::write(dir.path().join("topic-alpha.md"), new_body)
        .await
        .unwrap();
    search::reindex_topic(&state.search, "topic-alpha", new_body)
        .await
        .unwrap();

    let app = router(state);

    // Search for the new keyword — should hit topic-alpha.
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/search?q=tangerines")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let html = std::str::from_utf8(&resp.into_body().collect().await.unwrap().to_bytes())
        .unwrap()
        .to_string();
    assert!(
        html.contains("topic-alpha"),
        "reindex should make new keyword searchable: {html}"
    );

    // Search for the old keyword — topic-alpha should no longer hit.
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/search?q=substrate")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let html = std::str::from_utf8(&resp.into_body().collect().await.unwrap().to_bytes())
        .unwrap()
        .to_string();
    assert!(
        html.contains("No results"),
        "old keyword should no longer match after reindex: {html}"
    );
}

#[tokio::test]
async fn reindex_topic_makes_new_topic_searchable() {
    let (state, dir, _state_dir) = fixture_state().await;

    // Add a brand-new TOPIC on disk and reindex it.
    let new_body =
        "---\ntitle: \"Brand New Topic\"\nslug: topic-brand-new\n---\nFresh content here.\n";
    tokio::fs::write(dir.path().join("topic-brand-new.md"), new_body)
        .await
        .unwrap();
    search::reindex_topic(&state.search, "topic-brand-new", new_body)
        .await
        .unwrap();

    let app = router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/search?q=Brand")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let html = std::str::from_utf8(&resp.into_body().collect().await.unwrap().to_bytes())
        .unwrap()
        .to_string();
    assert!(
        html.contains("topic-brand-new"),
        "newly-indexed TOPIC should be searchable by title: {html}"
    );
}
