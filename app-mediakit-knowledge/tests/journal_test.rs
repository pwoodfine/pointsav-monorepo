//! Phase 7F — Tufte sidenote integration tests.
//!
//! Verifies that articles with `layout: journal` frontmatter have their
//! comrak footnotes transformed into sidenote-anchor structures, and that
//! the `<section class="footnotes">` block is removed from the output.
//! CSS/JS behaviour is not testable here; these tests guard the HTML scaffold.

use app_mediakit_knowledge::search;
use app_mediakit_knowledge::server::{router, AppState};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tower::ServiceExt;

async fn build_state(content_dir: &Path) -> (AppState, tempfile::TempDir) {
    let state_dir = tempfile::tempdir().unwrap();
    let index = search::build_index(content_dir, state_dir.path())
        .await
        .unwrap();
    let repo = app_mediakit_knowledge::git::open_or_init(content_dir).unwrap();
    let state = AppState {
        mounts: app_mediakit_knowledge::mounts::resolve(content_dir, None, None),
        citations_yaml: std::path::PathBuf::from("/nonexistent/citations.yaml"),
        search: Arc::new(index),
        git: Arc::new(Mutex::new(repo)),
        site_title: "Test Wiki".to_string(),
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
    };
    (state, state_dir)
}

/// Journal articles transform footnotes to sidenote-anchors, emit data-layout,
/// and remove the <section class="footnotes"> block.
#[tokio::test]
async fn journal_article_sidenote_scaffold() {
    let content_dir = std::path::Path::new("tests/fixtures");
    let (state, _dir) = build_state(content_dir).await;
    let app = router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/wiki/journal/sample")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let body = String::from_utf8_lossy(&bytes);

    assert!(
        body.contains(r#"class="sidenote-anchor""#),
        "expected sidenote-anchor in body"
    );
    assert!(
        body.contains(r#"class="sidenote""#),
        "expected sidenote span in body"
    );
    assert!(
        body.contains(r#"class="sn-toggle""#),
        "expected sn-toggle label in body"
    );
    assert!(
        !body.contains(r#"class="footnotes""#),
        "footnotes section should be removed in journal layout"
    );
    assert!(
        body.contains(r#"data-layout="journal""#),
        "expected data-layout=journal on prose div"
    );
}
