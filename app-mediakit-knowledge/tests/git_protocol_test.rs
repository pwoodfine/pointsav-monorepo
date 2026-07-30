use axum::http::{Request, StatusCode};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;
use tower::ServiceExt;

use app_mediakit_knowledge::server::{router, AppState};

async fn fixture_state() -> (AppState, TempDir, TempDir) {
    let content_dir = tempfile::tempdir().unwrap();
    let state_dir = tempfile::tempdir().unwrap();

    // Initialise git repo and one topic
    let repo = app_mediakit_knowledge::git::open_or_init(content_dir.path()).unwrap();
    app_mediakit_knowledge::git::ensure_commit_identity_from_env(&repo).unwrap();

    tokio::fs::write(content_dir.path().join("topic-test.md"), "# Test Topic")
        .await
        .unwrap();
    app_mediakit_knowledge::git::commit_topic(
        &repo,
        "topic-test",
        "# Test Topic",
        "j@woodfine.com",
        "Jennifer",
        "initial",
    )
    .unwrap();

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
        },
        content_dir,
        state_dir,
    )
}

#[tokio::test]
async fn test_info_refs_validation() {
    let (state, _content_dir, _state_dir) = fixture_state().await;
    let app = router(state.clone());

    // 1. Success case
    let req = Request::builder()
        .method("GET")
        .uri("/git-server/pointsav/info/refs?service=git-upload-pack")
        .body(axum::body::Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers().get("content-type").unwrap(),
        "application/x-git-upload-pack-advertisement"
    );

    // 2. Wrong service
    let req = Request::builder()
        .method("GET")
        .uri("/git-server/pointsav/info/refs?service=git-receive-pack")
        .body(axum::body::Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    // 3. Wrong tenant
    let req = Request::builder()
        .method("GET")
        .uri("/git-server/other-tenant/info/refs?service=git-upload-pack")
        .body(axum::body::Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_git_clone_roundtrip() {
    // This test actually runs 'git clone' against a live local server instance.
    let (state, _content_dir, _state_dir) = fixture_state().await;
    let app = router(state.clone());

    // Bind to random port
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let clone_dir = tempfile::tempdir().unwrap();
    let clone_url = format!("http://{}/git-server/pointsav", addr);

    // Give server a moment to start
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let output = tokio::process::Command::new("git")
        .args(["clone", &clone_url, clone_dir.path().to_str().unwrap()])
        .output()
        .await
        .expect("failed to run git clone");

    assert!(
        output.status.success(),
        "git clone failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Verify content
    let cloned_file = clone_dir.path().join("topic-test.md");
    assert!(cloned_file.exists());
    let content = std::fs::read_to_string(cloned_file).unwrap();
    assert_eq!(content, "# Test Topic");
}
