//! Integration tests for Phase 6 Part A: slug normalisation + redirect hatnote.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

use app_mediakit_knowledge::links::LinkGraph;
use app_mediakit_knowledge::server::{router, AppState};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

async fn make_state(content_dir: &tempfile::TempDir, state_dir: &tempfile::TempDir) -> AppState {
    let index = app_mediakit_knowledge::search::build_index(content_dir.path(), state_dir.path())
        .await
        .unwrap();
    let repo = app_mediakit_knowledge::git::open_or_init(content_dir.path()).unwrap();
    AppState {
        mounts: app_mediakit_knowledge::mounts::resolve(content_dir.path(), None, None),
        citations_yaml: PathBuf::from("/nonexistent/citations.yaml"),
        search: Arc::new(index),
        git: Arc::new(Mutex::new(repo)),
        site_title: "Test Wiki".to_string(),
        git_tenant: "pointsav".to_string(),
        mcp_enabled: false,
        glossary: Arc::new(app_mediakit_knowledge::glossary::Glossary::default()),
        links: LinkGraph::for_testing(),
        brand_theme: None,
        brand_instance: "documentation".to_string(),
        blueprints: app_mediakit_knowledge::blueprints::Registry::builtin(),
        peers: vec![],
        canonical_url: None,
        activitypub_outbox_url: None,
        start_here: vec![],
    }
}

/// Navigating to a mixed-case slug that matches a lowercase file on disk
/// should return HTTP 301 to the canonical lowercase URL.
#[tokio::test]
async fn mixed_case_slug_redirects_to_lowercase() {
    let content_dir = tempfile::tempdir().unwrap();
    let state_dir = tempfile::tempdir().unwrap();
    tokio::fs::write(
        content_dir.path().join("lower-slug.md"),
        "---\ntitle: Lower Slug\n---\n# Body\n",
    )
    .await
    .unwrap();

    let state = make_state(&content_dir, &state_dir).await;
    let app = router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/wiki/Lower-Slug")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::MOVED_PERMANENTLY);
    let location = resp.headers().get("location").unwrap().to_str().unwrap();
    assert_eq!(location, "/wiki/lower-slug");
}

/// redirect_to frontmatter fires a 301 that includes ?redirectedfrom= query param.
#[tokio::test]
async fn redirect_to_includes_redirectedfrom_param() {
    let content_dir = tempfile::tempdir().unwrap();
    let state_dir = tempfile::tempdir().unwrap();
    tokio::fs::write(
        content_dir.path().join("old-name.md"),
        "---\ntitle: Old Name\nredirect_to: new-name\n---\n",
    )
    .await
    .unwrap();
    tokio::fs::write(
        content_dir.path().join("new-name.md"),
        "---\ntitle: New Name\n---\n# New\n",
    )
    .await
    .unwrap();

    let state = make_state(&content_dir, &state_dir).await;
    let app = router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/wiki/old-name")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::MOVED_PERMANENTLY);
    let location = resp.headers().get("location").unwrap().to_str().unwrap();
    assert!(
        location.contains("redirectedfrom=old-name"),
        "location should include redirectedfrom param, got: {location}"
    );
}

/// When ?redirectedfrom= is present, the target page renders a "Redirected from" hatnote.
#[tokio::test]
async fn redirected_from_hatnote_rendered() {
    let content_dir = tempfile::tempdir().unwrap();
    let state_dir = tempfile::tempdir().unwrap();
    tokio::fs::write(
        content_dir.path().join("target-page.md"),
        "---\ntitle: Target Page\n---\n# Target\n",
    )
    .await
    .unwrap();

    let state = make_state(&content_dir, &state_dir).await;
    let app = router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/wiki/target-page?redirectedfrom=old-page")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let html = std::str::from_utf8(&body).unwrap();
    assert!(
        html.contains("wiki-redirected-from"),
        "redirect hatnote div should appear: {html}"
    );
    assert!(
        html.contains("old-page"),
        "redirect source slug should appear in hatnote: {html}"
    );
}

/// Wikilinks in article body should have normalized hrefs (lowercase + hyphens)
/// with no trailing double-quote artifact from the href attribute parsing bug.
#[tokio::test]
async fn wikilink_href_normalised_no_trailing_quote() {
    let content_dir = tempfile::tempdir().unwrap();
    let state_dir = tempfile::tempdir().unwrap();
    tokio::fs::write(
        content_dir.path().join("article-with-links.md"),
        "---\ntitle: Link Test\n---\nSee [[Some Topic]] for details.\n",
    )
    .await
    .unwrap();
    // Target must exist so L18 (zero-dead-links) emits an anchor rather than plain text.
    tokio::fs::write(
        content_dir.path().join("some-topic.md"),
        "---\ntitle: Some Topic\n---\nTarget article.\n",
    )
    .await
    .unwrap();

    let state = make_state(&content_dir, &state_dir).await;
    let app = router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/wiki/article-with-links")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let html = std::str::from_utf8(&body).unwrap();
    assert!(
        html.contains(r#"href="/wiki/some-topic""#),
        "wikilink href should be normalised to /wiki/some-topic: {html}"
    );
    assert!(
        !html.contains("Some Topic\""),
        "wikilink href must not contain uppercase slug with trailing quote: {html}"
    );
}
