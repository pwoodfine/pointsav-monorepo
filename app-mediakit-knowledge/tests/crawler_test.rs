//! Integration tests for Phase 3 Step 3.4 — sitemap, robots, llms.txt,
//! and raw Markdown source via `GET /git/{slug}.md`.
//!
//! Verifies that:
//! - `GET /sitemap.xml` returns 200, `application/xml`, valid sitemaps.org XML.
//! - `GET /robots.txt` returns 200, `text/plain`, crawler directives present.
//! - `GET /llms.txt` returns 200, `text/markdown`, expected header present.
//! - `GET /git/topic-foo.md` returns 200, `text/markdown`, body round-trips
//!   to the on-disk file.
//! - `GET /git/no-such-slug.md` returns 404.
//! - `GET /git/INVALID%20slug.md` returns 400 (slug validation rejected).

use app_mediakit_knowledge::search;
use app_mediakit_knowledge::server::{router, AppState};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use std::sync::{Arc, Mutex};
use tower::ServiceExt;

const TOPIC_FOO_CONTENT: &str =
    "---\ntitle: \"Foo\"\nslug: topic-foo\n---\n\nThis is the topic-foo fixture.\n";

async fn fixture_state() -> (AppState, tempfile::TempDir, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let state_dir = tempfile::tempdir().unwrap();

    tokio::fs::write(dir.path().join("topic-foo.md"), TOPIC_FOO_CONTENT)
        .await
        .unwrap();
    tokio::fs::write(
        dir.path().join("topic-bar.md"),
        "---\ntitle: \"Bar\"\nslug: topic-bar\n---\n\nBar content.\n",
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
        git_tenant: "pointsav".to_string(),
        mcp_enabled: false,
        glossary: Arc::new(app_mediakit_knowledge::glossary::Glossary::default()),
        links: app_mediakit_knowledge::links::LinkGraph::for_testing(),
        brand_theme: None,
        brand_instance: "documentation".to_string(),
        site_title: "PointSav Knowledge".to_string(),
        blueprints: app_mediakit_knowledge::blueprints::Registry::builtin(),
        peers: vec![],
        canonical_url: None,
        activitypub_outbox_url: None,
        start_here: vec![],
        site_categories: vec![],
    };

    (state, dir, state_dir)
}

// ─── sitemap.xml ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn sitemap_xml_returns_200_with_xml_content_type() {
    let (state, _dir, _state_dir) = fixture_state().await;
    let app = router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/sitemap.xml")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);

    let ct = resp
        .headers()
        .get("content-type")
        .expect("content-type header should be present")
        .to_str()
        .unwrap();
    assert!(
        ct.contains("application/xml"),
        "content-type should be application/xml: {ct}"
    );
}

#[tokio::test]
async fn sitemap_xml_body_contains_urlset_and_urls() {
    let (state, _dir, _state_dir) = fixture_state().await;
    let app = router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/sitemap.xml")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let xml = std::str::from_utf8(&bytes).unwrap();

    assert!(
        xml.contains("<urlset"),
        "sitemap should contain <urlset: {xml}"
    );
    assert!(xml.contains("<url>"), "sitemap should contain <url>: {xml}");
    assert!(
        xml.contains("/wiki/topic-foo"),
        "sitemap should contain topic-foo URL: {xml}"
    );
    assert!(
        xml.contains("/wiki/topic-bar"),
        "sitemap should contain topic-bar URL: {xml}"
    );
}

// ─── robots.txt ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn robots_txt_returns_200_with_text_plain_content_type() {
    let (state, _dir, _state_dir) = fixture_state().await;
    let app = router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/robots.txt")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);

    let ct = resp
        .headers()
        .get("content-type")
        .expect("content-type header should be present")
        .to_str()
        .unwrap();
    assert!(
        ct.contains("text/plain"),
        "content-type should be text/plain: {ct}"
    );
}

#[tokio::test]
async fn robots_txt_body_contains_directives() {
    let (state, _dir, _state_dir) = fixture_state().await;
    let app = router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/robots.txt")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let body = std::str::from_utf8(&bytes).unwrap();

    assert!(
        body.contains("User-agent:"),
        "robots.txt should contain User-agent: {body}"
    );
    assert!(
        body.contains("Sitemap:"),
        "robots.txt should contain Sitemap: {body}"
    );
}

// ─── llms.txt ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn llms_txt_returns_200_with_text_markdown_content_type() {
    let (state, _dir, _state_dir) = fixture_state().await;
    let app = router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/llms.txt")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);

    let ct = resp
        .headers()
        .get("content-type")
        .expect("content-type header should be present")
        .to_str()
        .unwrap();
    assert!(
        ct.contains("text/markdown"),
        "content-type should be text/markdown: {ct}"
    );
}

#[tokio::test]
async fn llms_txt_body_contains_expected_header_and_topics() {
    let (state, _dir, _state_dir) = fixture_state().await;
    let app = router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/llms.txt")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let body = std::str::from_utf8(&bytes).unwrap();

    assert!(
        body.contains("# PointSav Knowledge"),
        "llms.txt should start with # PointSav Knowledge: {body}"
    );
    assert!(
        body.contains("topic-foo"),
        "llms.txt should list topic-foo: {body}"
    );
    assert!(
        body.contains("topic-bar"),
        "llms.txt should list topic-bar: {body}"
    );
    // Structured data section must be present.
    assert!(
        body.contains("## Structured data"),
        "llms.txt should contain structured data section: {body}"
    );
    assert!(
        body.contains("/feed.atom"),
        "llms.txt should mention Atom feed: {body}"
    );
}

// ─── /git/{slug}.md ─────────────────────────────────────────────────────────

#[tokio::test]
async fn git_markdown_returns_200_with_text_markdown_content_type() {
    let (state, _dir, _state_dir) = fixture_state().await;
    let app = router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/git/topic-foo.md")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);

    let ct = resp
        .headers()
        .get("content-type")
        .expect("content-type header should be present")
        .to_str()
        .unwrap();
    assert!(
        ct.contains("text/markdown"),
        "content-type should be text/markdown: {ct}"
    );
}

#[tokio::test]
async fn git_markdown_body_round_trips_to_disk() {
    let (state, _dir, _state_dir) = fixture_state().await;
    let app = router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/git/topic-foo.md")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let body = std::str::from_utf8(&bytes).unwrap();

    assert_eq!(
        body, TOPIC_FOO_CONTENT,
        "GET /git/topic-foo.md should round-trip to the on-disk content"
    );
}

#[tokio::test]
async fn git_markdown_returns_404_for_missing_slug() {
    let (state, _dir, _state_dir) = fixture_state().await;
    let app = router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/git/no-such-slug.md")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn git_markdown_returns_400_for_invalid_slug() {
    let (state, _dir, _state_dir) = fixture_state().await;
    let app = router(state);
    // Uppercase letters are rejected by `validate_slug`.
    // URL-encode a space to produce an invalid slug that passes the router.
    let resp = app
        .oneshot(
            Request::builder()
                // `INVALID` contains uppercase — validate_slug rejects it → 400.
                .uri("/git/INVALID-SLUG.md")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "invalid slug should return 400"
    );
}
