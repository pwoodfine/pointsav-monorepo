//! Integration test: GET /wiki/{slug} embeds a JSON-LD script in <head>.

use http_body_util::BodyExt;
use tower::ServiceExt;

use app_mediakit_knowledge::search;
use app_mediakit_knowledge::server::{router, AppState};
use axum::{body::Body, http::Request};
use std::sync::{Arc, Mutex};

async fn fixture_state() -> (AppState, tempfile::TempDir, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let state_dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("topic-test.md");
    tokio::fs::write(
        &path,
        "---\ntitle: \"JSON-LD Test\"\nslug: topic-test\nforward_looking: false\n---\n# Body\n",
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
    };
    (state, dir, state_dir)
}

#[tokio::test]
async fn rendered_page_carries_jsonld_script() {
    let (state, _dir, _state_dir) = fixture_state().await;
    let app = router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/wiki/test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let html = std::str::from_utf8(&body).unwrap();

    let prefix = r#"<script type="application/ld+json">"#;
    assert!(
        html.contains(prefix),
        "JSON-LD script tag should appear in rendered page: {html}"
    );

    let start = html.find(prefix).unwrap() + prefix.len();
    let end = html[start..].find("</script>").unwrap() + start;
    let json_str = &html[start..end];
    let parsed: serde_json::Value =
        serde_json::from_str(json_str).expect("JSON-LD body should parse");

    assert_eq!(parsed["@context"], "https://schema.org");
    assert_eq!(parsed["@type"], "TechArticle");
    assert_eq!(parsed["name"], "JSON-LD Test");
    assert_eq!(parsed["identifier"], "test");
    assert_eq!(parsed["inLanguage"], "en");
    assert_eq!(parsed["isPartOf"]["name"], "PointSav Knowledge");
}

#[tokio::test]
async fn fli_topic_carries_additional_property() {
    let dir = tempfile::tempdir().unwrap();
    let state_dir = tempfile::tempdir().unwrap();
    tokio::fs::write(
        dir.path().join("topic-fli.md"),
        "---\ntitle: \"FLI Test\"\nslug: topic-fli\nforward_looking: true\n---\n# Body\n",
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
    };
    let app = router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/wiki/fli")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let html = std::str::from_utf8(&body).unwrap();

    let prefix = r#"<script type="application/ld+json">"#;
    let start = html.find(prefix).unwrap() + prefix.len();
    let end = html[start..].find("</script>").unwrap() + start;
    let parsed: serde_json::Value = serde_json::from_str(&html[start..end]).unwrap();

    assert!(
        parsed["additionalProperty"].is_array(),
        "FLI flag should produce additionalProperty array: {parsed}"
    );
    assert_eq!(parsed["additionalProperty"][0]["name"], "forward_looking");
}
