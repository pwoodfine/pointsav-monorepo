//! Integration tests for the Phase 4 redb link-graph + blake3 hash store.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

use app_mediakit_knowledge::links::LinkGraph;
use app_mediakit_knowledge::server::{router, AppState};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

// ─── Unit-level graph tests ────────────────────────────────────────────────

#[test]
fn backlink_added_by_rebuild() {
    let g = LinkGraph::for_testing();
    g.rebuild_for_slug("article-a", "See [[target-slug]].")
        .unwrap();
    let bl = g.backlinks("target-slug").unwrap();
    assert_eq!(bl, vec!["article-a"]);
}

#[test]
fn backlink_cleared_when_link_removed() {
    let g = LinkGraph::for_testing();
    g.rebuild_for_slug("article-a", "See [[target-slug]].")
        .unwrap();
    g.rebuild_for_slug("article-a", "No links here.").unwrap();
    let bl = g.backlinks("target-slug").unwrap();
    assert!(
        bl.is_empty(),
        "backlink should be gone after rebuild without the link"
    );
}

#[test]
fn multiple_sources_link_to_same_target() {
    let g = LinkGraph::for_testing();
    g.rebuild_for_slug("article-a", "See [[shared-target]].")
        .unwrap();
    g.rebuild_for_slug("article-b", "Also [[shared-target]].")
        .unwrap();
    let mut bl = g.backlinks("shared-target").unwrap();
    bl.sort();
    assert_eq!(bl, vec!["article-a", "article-b"]);
}

#[test]
fn self_links_are_recorded_in_graph() {
    let g = LinkGraph::for_testing();
    g.rebuild_for_slug("page", "This page references [[page]] itself.")
        .unwrap();
    let bl = g.backlinks("page").unwrap();
    assert_eq!(bl, vec!["page"]);
}

#[test]
fn blake3_hash_stored_and_retrieved() {
    let g = LinkGraph::for_testing();
    let body = b"Hello, federation!";
    g.record_hash("my-slug", "deadbeef", body).unwrap();
    let expected = blake3::hash(body);
    let result = g.lookup_by_hash(expected.as_bytes()).unwrap();
    assert_eq!(result, Some(("my-slug".to_owned(), "deadbeef".to_owned())));
}

#[test]
fn lookup_unknown_hash_returns_none() {
    let g = LinkGraph::for_testing();
    let unknown = [0u8; 32];
    let result = g.lookup_by_hash(&unknown).unwrap();
    assert!(result.is_none());
}

// ─── Route-level test — whatlinkshere uses graph ───────────────────────────

#[tokio::test]
async fn whatlinkshere_returns_backlinks_from_graph() {
    let content_dir = tempfile::tempdir().unwrap();
    let state_dir = tempfile::tempdir().unwrap();

    // Seed a topic file.
    tokio::fs::write(
        content_dir.path().join("destination-article.md"),
        "---\ntitle: Destination\nslug: destination-article\n---\n\nThe target.\n",
    )
    .await
    .unwrap();

    let index = app_mediakit_knowledge::search::build_index(content_dir.path(), state_dir.path())
        .await
        .unwrap();
    let repo = app_mediakit_knowledge::git::open_or_init(content_dir.path()).unwrap();
    let links = LinkGraph::for_testing();

    // Pre-populate the link graph (simulates what /edit would do).
    links
        .rebuild_for_slug("origin-article", "See [[destination-article]].")
        .unwrap();

    let state = AppState {
        mounts: app_mediakit_knowledge::mounts::resolve(content_dir.path(), None, None),
        citations_yaml: PathBuf::from("/nonexistent/citations.yaml"),
        search: Arc::new(index),
        git: Arc::new(Mutex::new(repo)),
        site_title: "Test Wiki".to_string(),
        git_tenant: "pointsav".to_string(),
        mcp_enabled: false,
        glossary: Arc::new(app_mediakit_knowledge::glossary::Glossary::default()),
        links,
        brand_theme: None,
        brand_instance: "documentation".to_string(),
        blueprints: app_mediakit_knowledge::blueprints::Registry::builtin(),
        peers: vec![],
        canonical_url: None,
        activitypub_outbox_url: None,
        start_here: vec![],
    };

    let app = router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/special/whatlinkshere/destination-article")
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
        html.contains("origin-article"),
        "whatlinkshere should list the source article"
    );
}
