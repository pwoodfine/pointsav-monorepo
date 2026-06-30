use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
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
        },
        content_dir,
        state_dir,
    )
}

#[tokio::test]
async fn test_openapi_yaml_returns_200() {
    let (state, _cd, _sd) = fixture().await;
    let app = router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/openapi.yaml")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_openapi_yaml_content_type() {
    let (state, _cd, _sd) = fixture().await;
    let app = router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/openapi.yaml")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let ct = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        ct.contains("yaml"),
        "expected content-type to contain 'yaml', got: {ct}"
    );
}

#[tokio::test]
async fn test_openapi_yaml_parses_as_valid_yaml() {
    let (state, _cd, _sd) = fixture().await;
    let app = router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/openapi.yaml")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let text = std::str::from_utf8(&bytes).expect("openapi.yaml should be UTF-8");
    let doc: serde_yaml::Value =
        serde_yaml::from_str(text).expect("openapi.yaml should parse as valid YAML");
    assert!(doc.is_mapping(), "openapi.yaml root should be a mapping");
}

#[tokio::test]
async fn test_openapi_yaml_well_known_paths_present() {
    let (state, _cd, _sd) = fixture().await;
    let app = router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/openapi.yaml")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let text = std::str::from_utf8(&bytes).unwrap();
    let doc: serde_yaml::Value = serde_yaml::from_str(text).unwrap();

    let paths = doc
        .get("paths")
        .expect("openapi.yaml should have a 'paths' key");
    assert!(paths.is_mapping(), "'paths' should be a mapping");

    let required_paths = [
        "/healthz",
        "/wiki/{slug}",
        "/search",
        "/mcp",
        "/sitemap.xml",
        "/history/{slug}",
        "/diff/{slug}",
        "/openapi.yaml",
    ];
    for path in &required_paths {
        assert!(
            paths.get(*path).is_some(),
            "openapi.yaml 'paths' missing required route: {path}"
        );
    }
}

#[tokio::test]
async fn test_openapi_yaml_declares_openapi_version() {
    let (state, _cd, _sd) = fixture().await;
    let app = router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/openapi.yaml")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let text = std::str::from_utf8(&bytes).unwrap();
    let doc: serde_yaml::Value = serde_yaml::from_str(text).unwrap();
    let version = doc
        .get("openapi")
        .and_then(|v| v.as_str())
        .expect("openapi.yaml should declare 'openapi' version string");
    assert!(
        version.starts_with("3."),
        "expected OpenAPI 3.x, got: {version}"
    );
}
