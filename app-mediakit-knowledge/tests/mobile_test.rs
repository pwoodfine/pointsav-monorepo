//! Sprint AB — Mobile chrome integration tests.
//!
//! Verifies the mobile nav drawer, TOC drawer, and toggle buttons are
//! emitted with correct ARIA structure and that the overlay element is
//! present for JS-driven show/hide.  CSS animation and JS behaviour are
//! not testable here; these tests guard the HTML scaffold that JS and
//! CSS depend on.

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

// ─── Helpers ─────────────────────────────────────────────────────────────────

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
        site_categories: vec![],
    };
    (state, state_dir)
}

/// GET `/wiki/{slug}` and return (status, body).
async fn get_wiki(state: AppState, slug: &str) -> (StatusCode, String) {
    let app = router(state);
    let uri = format!("/wiki/{slug}");
    let resp = app
        .oneshot(Request::builder().uri(&uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

// ─── Test 1: hamburger button present with correct ARIA ──────────────────────

#[tokio::test]
async fn mobile_nav_toggle_button_present() {
    let dir = tempfile::tempdir().unwrap();
    tokio::fs::write(
        dir.path().join("topic-hello.md"),
        "---\ntitle: \"Hello\"\ncategory: \"architecture\"\n---\nTest article.\n",
    )
    .await
    .unwrap();
    let (state, _sd) = build_state(dir.path()).await;
    let (status, html) = get_wiki(state, "hello").await;
    assert_eq!(status, StatusCode::OK);
    // Button id and ARIA wiring
    assert!(
        html.contains("id=\"nav-toggle\""),
        "nav-toggle button missing"
    );
    assert!(
        html.contains("aria-controls=\"mobile-nav-drawer\""),
        "nav-toggle missing aria-controls"
    );
    assert!(
        html.contains("aria-expanded=\"false\""),
        "nav-toggle should start unexpanded"
    );
    // CSS class for touch-target sizing
    assert!(
        html.contains("nav-toggle-btn"),
        "nav-toggle-btn class missing"
    );
}

// ─── Test 2: nav drawer present, initially hidden ────────────────────────────

#[tokio::test]
async fn mobile_nav_drawer_present_and_hidden() {
    let dir = tempfile::tempdir().unwrap();
    tokio::fs::write(
        dir.path().join("topic-hello.md"),
        "---\ntitle: \"Hello\"\ncategory: \"architecture\"\n---\nTest article.\n",
    )
    .await
    .unwrap();
    let (state, _sd) = build_state(dir.path()).await;
    let (status, html) = get_wiki(state, "hello").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        html.contains("id=\"mobile-nav-drawer\""),
        "mobile-nav-drawer element missing"
    );
    // Must start hidden for JS to manage visibility
    assert!(
        html.contains("id=\"mobile-nav-drawer\" aria-hidden=\"true\"")
            || html.contains("aria-hidden=\"true\""),
        "mobile-nav-drawer should be aria-hidden on load"
    );
}

// ─── Test 3: nav drawer contains close button ────────────────────────────────

#[tokio::test]
async fn mobile_nav_drawer_has_close_button() {
    let dir = tempfile::tempdir().unwrap();
    tokio::fs::write(
        dir.path().join("topic-hello.md"),
        "---\ntitle: \"Hello\"\ncategory: \"architecture\"\n---\nTest article.\n",
    )
    .await
    .unwrap();
    let (state, _sd) = build_state(dir.path()).await;
    let (_, html) = get_wiki(state, "hello").await;
    assert!(
        html.contains("id=\"mobile-nav-close\""),
        "mobile-nav-close button missing"
    );
    assert!(
        html.contains("aria-label=\"Close navigation\""),
        "close button missing accessible label"
    );
}

// ─── Test 4: nav drawer contains nav links ───────────────────────────────────

#[tokio::test]
async fn mobile_nav_drawer_contains_nav_links() {
    let dir = tempfile::tempdir().unwrap();
    tokio::fs::write(
        dir.path().join("topic-hello.md"),
        "---\ntitle: \"Hello\"\ncategory: \"architecture\"\n---\nTest article.\n",
    )
    .await
    .unwrap();
    let (state, _sd) = build_state(dir.path()).await;
    let (_, html) = get_wiki(state, "hello").await;
    // mobile-nav-list should include standard nav entries
    assert!(html.contains("mobile-nav-list"), "mobile-nav-list missing");
    assert!(
        html.contains("href=\"/search\"") && html.contains("href=\"/random\""),
        "expected nav links (search, random) inside drawer"
    );
}

// ─── Test 5: overlay element present ─────────────────────────────────────────

#[tokio::test]
async fn mobile_nav_overlay_present() {
    let dir = tempfile::tempdir().unwrap();
    tokio::fs::write(
        dir.path().join("topic-hello.md"),
        "---\ntitle: \"Hello\"\ncategory: \"architecture\"\n---\nTest article.\n",
    )
    .await
    .unwrap();
    let (state, _sd) = build_state(dir.path()).await;
    let (_, html) = get_wiki(state, "hello").await;
    assert!(
        html.contains("id=\"mobile-nav-overlay\""),
        "mobile-nav-overlay missing — JS click-outside-to-close will break"
    );
}

// ─── Test 6: TOC drawer and toggle present on articles with headings ─────────

#[tokio::test]
async fn mobile_toc_drawer_present_when_article_has_headings() {
    let dir = tempfile::tempdir().unwrap();
    tokio::fs::write(
        dir.path().join("topic-sections.md"),
        "---\ntitle: \"Sections\"\ncategory: \"architecture\"\n---\n\n## First section\n\nSome content.\n\n## Second section\n\nMore content.\n",
    )
    .await
    .unwrap();
    let (state, _sd) = build_state(dir.path()).await;
    let (status, html) = get_wiki(state, "sections").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        html.contains("id=\"toc-toggle-btn\""),
        "toc-toggle-btn missing on article with headings"
    );
    assert!(
        html.contains("id=\"mobile-toc-drawer\""),
        "mobile-toc-drawer missing on article with headings"
    );
    assert!(
        html.contains("id=\"mobile-toc-close\""),
        "mobile-toc-close button missing"
    );
    assert!(
        html.contains("aria-label=\"Close contents\""),
        "toc close button missing accessible label"
    );
}

// ─── Test 7: TOC toggle absent on articles without headings ──────────────────

#[tokio::test]
async fn mobile_toc_drawer_absent_when_no_headings() {
    let dir = tempfile::tempdir().unwrap();
    tokio::fs::write(
        dir.path().join("topic-flat.md"),
        "---\ntitle: \"Flat\"\ncategory: \"architecture\"\n---\n\nJust a paragraph, no headings.\n",
    )
    .await
    .unwrap();
    let (state, _sd) = build_state(dir.path()).await;
    let (_, html) = get_wiki(state, "flat").await;
    assert!(
        !html.contains("id=\"toc-toggle-btn\""),
        "toc-toggle-btn should be absent on articles with no headings"
    );
    assert!(
        !html.contains("id=\"mobile-toc-drawer\""),
        "mobile-toc-drawer should be absent on articles with no headings"
    );
}

// ─── Test 8: toc-toggle-btn wired to mobile-toc-drawer via aria-controls ─────

#[tokio::test]
async fn mobile_toc_toggle_aria_controls_wired() {
    let dir = tempfile::tempdir().unwrap();
    tokio::fs::write(
        dir.path().join("topic-headings.md"),
        "---\ntitle: \"Has Headings\"\ncategory: \"architecture\"\n---\n\n## Section A\n\nContent.\n",
    )
    .await
    .unwrap();
    let (state, _sd) = build_state(dir.path()).await;
    let (_, html) = get_wiki(state, "headings").await;
    assert!(
        html.contains("aria-controls=\"mobile-toc-drawer\""),
        "toc-toggle-btn missing aria-controls=\"mobile-toc-drawer\""
    );
}
