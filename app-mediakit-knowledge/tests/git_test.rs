//! Integration tests for Phase 4 Step 4.1 — git2 wiring + commit layer.
//!
//! The HTTP write endpoints (`/create`, `/edit`) were removed when the wiki
//! moved to a git-only contribution workflow. These tests now exercise the
//! surviving git layer (`open_or_init`, `commit_topic`,
//! `ensure_commit_identity_from_env`) directly — the same functions the engine
//! uses internally.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use app_mediakit_knowledge::server::AppState;

async fn fixture_state() -> (AppState, tempfile::TempDir, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let state_dir = tempfile::tempdir().unwrap();

    // Initialize git repo in content_dir
    let repo = app_mediakit_knowledge::git::open_or_init(dir.path()).unwrap();

    let index = app_mediakit_knowledge::search::build_index(dir.path(), state_dir.path())
        .await
        .unwrap();
    let mounts = app_mediakit_knowledge::mounts::resolve(dir.path(), None, None);
    let state = AppState {
        mounts,
        citations_yaml: PathBuf::from("/nonexistent/citations.yaml"),
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

/// Write `<slug>.md` to disk and commit it through the git layer.
fn commit_topic(state: &AppState, slug: &str, body: &str, message: &str) {
    std::fs::write(state.primary_path().join(format!("{slug}.md")), body).unwrap();
    let repo = state.git.lock().unwrap();
    let _ = app_mediakit_knowledge::git::ensure_commit_identity_from_env(&repo);
    app_mediakit_knowledge::git::commit_topic(&repo, slug, body, "", "", message).unwrap();
}

fn last_commit_subject(dir: &std::path::Path) -> String {
    let output = std::process::Command::new("git")
        .args(["-C", dir.to_str().unwrap(), "log", "-1", "--format=%s"])
        .output()
        .expect("git log failed");
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

#[tokio::test]
async fn git_commit_on_create() {
    let (state, dir, _state_dir) = fixture_state().await;
    commit_topic(
        &state,
        "git-create",
        "# Git Create Test\n",
        "create: git-create",
    );
    assert_eq!(last_commit_subject(dir.path()), "create: git-create");
}

#[tokio::test]
async fn git_commit_on_edit() {
    let (state, dir, _state_dir) = fixture_state().await;
    commit_topic(&state, "git-edit", "# Initial", "create: git-edit");
    commit_topic(&state, "git-edit", "# Updated content", "edit: git-edit");
    assert_eq!(last_commit_subject(dir.path()), "edit: git-edit");
}

#[tokio::test]
async fn open_or_init_idempotency() {
    let dir = tempfile::tempdir().unwrap();

    // First call
    let _repo1 = app_mediakit_knowledge::git::open_or_init(dir.path()).unwrap();
    assert!(dir.path().join(".git").exists());

    // Second call
    let _repo2 = app_mediakit_knowledge::git::open_or_init(dir.path()).unwrap();
}

#[tokio::test]
async fn git_identity_alternation() {
    let (state, dir, _state_dir) = fixture_state().await;

    // Mock toggle file in a temp home dir
    let home_dir = tempfile::tempdir().unwrap();
    let toggle_path = home_dir.path().join("Foundry/identity/.toggle");
    std::fs::create_dir_all(toggle_path.parent().unwrap()).unwrap();

    // Set HOME to our temp home dir
    let original_home = std::env::var("HOME").ok();
    std::env::set_var("HOME", home_dir.path());

    // Identity 0 (Jennifer)
    std::fs::write(&toggle_path, "0").unwrap();
    commit_topic(&state, "t1", "# T1\n", "create: t1");
    let output = std::process::Command::new("git")
        .args([
            "-C",
            dir.path().to_str().unwrap(),
            "log",
            "-1",
            "--format=%an <%ae>",
        ])
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "Jennifer Woodfine <jwoodfine@users.noreply.github.com>"
    );

    // Identity 1 (Peter)
    std::fs::write(&toggle_path, "1").unwrap();
    commit_topic(&state, "t2", "# T2\n", "create: t2");
    let output = std::process::Command::new("git")
        .args([
            "-C",
            dir.path().to_str().unwrap(),
            "log",
            "-1",
            "--format=%an <%ae>",
        ])
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "Peter Woodfine <pwoodfine@users.noreply.github.com>"
    );

    // Restore HOME
    if let Some(h) = original_home {
        std::env::set_var("HOME", h);
    }
}
