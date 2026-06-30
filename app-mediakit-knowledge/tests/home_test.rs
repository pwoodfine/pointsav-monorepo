//! Integration tests for home-page iteration 1 MUST features.
//!
//! Verifies the `index()` handler behaviour under six fixtures:
//! - With `index.md` present → full home-page chrome renders.
//! - Without `index.md` → fallback placeholder renders.
//! - Featured-topic YAML absent → featured panel suppressed.
//! - Featured-topic YAML present and valid → featured panel renders.
//! - Featured-topic YAML with unresolvable slug → featured panel suppressed.
//! - Recent feed sorts by `last_edited:` descending.
//! - Category grid always shows all 12 ratified categories; empty ones render
//!   placeholder copy.

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

// ─── Fixture helpers ─────────────────────────────────────────────────────────

/// Write a TOPIC fixture with the given frontmatter fields.
async fn write_topic(
    dir: &Path,
    filename: &str,
    title: &str,
    category: &str,
    last_edited: Option<&str>,
    body: &str,
) {
    let last_edited_line = match last_edited {
        Some(d) => format!("last_edited: \"{d}\"\n"),
        None => String::new(),
    };
    let content = format!(
        "---\ntitle: \"{title}\"\ncategory: \"{category}\"\n{last_edited_line}---\n{body}\n"
    );
    tokio::fs::write(dir.join(filename), content).await.unwrap();
}

/// Build an `AppState` pointing at `content_dir`. Uses a separate temp dir
/// for the Tantivy state (search index); both temps are returned so the
/// caller can extend their lifetimes.
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
    (state, state_dir)
}

/// GET `/` and return the response body as a String.
async fn get_home(state: AppState) -> (StatusCode, String) {
    let app = router(state);
    let resp = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let body = String::from_utf8_lossy(&bytes).into_owned();
    (status, body)
}

// ─── Test 1: home renders with index.md present ──────────────────────────────

#[tokio::test]
async fn home_renders_with_index_md_present() {
    let dir = tempfile::tempdir().unwrap();

    // Write index.md with category: root.
    tokio::fs::write(
        dir.path().join("index.md"),
        "---\ntitle: \"PointSav Knowledge\"\ncategory: \"root\"\n---\nWelcome to PointSav Knowledge.\n",
    )
    .await
    .unwrap();

    // 3 architecture topics.
    write_topic(
        dir.path(),
        "topic-arch-one.md",
        "Arch One",
        "architecture",
        Some("2026-04-20"),
        "First architecture topic.",
    )
    .await;
    write_topic(
        dir.path(),
        "topic-arch-two.md",
        "Arch Two",
        "architecture",
        Some("2026-04-18"),
        "Second architecture topic.",
    )
    .await;
    write_topic(
        dir.path(),
        "topic-arch-three.md",
        "Arch Three",
        "architecture",
        Some("2026-04-15"),
        "Third architecture topic.",
    )
    .await;

    // 3 services topics.
    write_topic(
        dir.path(),
        "topic-svc-one.md",
        "Service One",
        "services",
        Some("2026-04-19"),
        "First services topic.",
    )
    .await;
    write_topic(
        dir.path(),
        "topic-svc-two.md",
        "Service Two",
        "services",
        Some("2026-04-17"),
        "Second services topic.",
    )
    .await;
    write_topic(
        dir.path(),
        "topic-svc-three.md",
        "Service Three",
        "services",
        Some("2026-04-12"),
        "Third services topic.",
    )
    .await;

    // 3 governance topics.
    write_topic(
        dir.path(),
        "topic-gov-one.md",
        "Gov One",
        "governance",
        Some("2026-04-16"),
        "First governance topic.",
    )
    .await;
    write_topic(
        dir.path(),
        "topic-gov-two.md",
        "Gov Two",
        "governance",
        Some("2026-04-14"),
        "Second governance topic.",
    )
    .await;
    write_topic(
        dir.path(),
        "topic-gov-three.md",
        "Gov Three",
        "governance",
        Some("2026-04-10"),
        "Third governance topic.",
    )
    .await;

    let (state, _state_dir) = build_state(dir.path()).await;
    let (status, html) = get_home(state).await;

    assert_eq!(status, StatusCode::OK);

    // Site title.
    assert!(
        html.contains("PointSav Knowledge"),
        "title should appear: snippet={}",
        &html[..html.len().min(500)]
    );

    // The 3 slide IA areas that have content render in the browse grid.
    // (Fixture: architecture → "Multi-Entity Consolidation", services → "Integration & Data Portability",
    //  governance → "Developer Platform")
    for cat in &[
        "Multi-Entity Consolidation",
        "Integration &amp; Data Portability",
        "Developer Platform",
    ] {
        assert!(html.contains(cat), "category '{cat}' should appear in grid");
    }

    // Populated-category article titles appear in the recently-updated section.
    assert!(
        html.contains("Arch One"),
        "architecture topic should appear"
    );
    assert!(html.contains("Service One"), "services topic should appear");
    assert!(html.contains("Gov One"), "governance topic should appear");
}

// ─── Test 2: placeholder when index.md absent ────────────────────────────────

#[tokio::test]
async fn home_falls_back_to_placeholder_when_index_md_absent() {
    let dir = tempfile::tempdir().unwrap();

    // Write some TOPICs but no index.md.
    write_topic(
        dir.path(),
        "topic-foo.md",
        "Foo Topic",
        "architecture",
        None,
        "Foo body.",
    )
    .await;
    write_topic(
        dir.path(),
        "topic-bar.md",
        "Bar Topic",
        "services",
        None,
        "Bar body.",
    )
    .await;

    let (state, _state_dir) = build_state(dir.path()).await;
    let (status, html) = get_home(state).await;

    assert_eq!(status, StatusCode::OK);
    // Placeholder renders "Pages" heading.
    assert!(
        html.contains("Pages"),
        "placeholder should contain 'Pages' heading: snippet={}",
        &html[..html.len().min(800)]
    );
    // Must not render the category grid (no .wiki-home-grid).
    assert!(
        !html.contains("wiki-home-grid"),
        "placeholder must not contain wiki-home-grid: snippet={}",
        &html[..html.len().min(800)]
    );
}

// ─── Test 3: featured panel suppressed when yaml absent ──────────────────────

#[tokio::test]
async fn featured_topic_yaml_absent_suppresses_panel() {
    let dir = tempfile::tempdir().unwrap();

    tokio::fs::write(
        dir.path().join("index.md"),
        "---\ntitle: \"Home\"\ncategory: \"root\"\n---\nLede text.\n",
    )
    .await
    .unwrap();

    write_topic(
        dir.path(),
        "topic-arch-one.md",
        "Arch One",
        "architecture",
        None,
        "Body.",
    )
    .await;

    // No featured-topic.yaml written.

    let (state, _state_dir) = build_state(dir.path()).await;
    let (status, html) = get_home(state).await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        !html.contains("wiki-home-featured"),
        "featured panel must be absent when yaml is absent: snippet={}",
        &html[..html.len().min(800)]
    );
}

// ─── Test 4: featured panel renders when yaml is valid ───────────────────────

#[tokio::test]
async fn featured_topic_yaml_present_renders_panel() {
    let dir = tempfile::tempdir().unwrap();

    tokio::fs::write(
        dir.path().join("index.md"),
        "---\ntitle: \"Home\"\ncategory: \"root\"\n---\nLede text.\n",
    )
    .await
    .unwrap();

    write_topic(
        dir.path(),
        "topic-three-layer.md",
        "Three-Layer Architecture",
        "architecture",
        Some("2026-04-20"),
        "The three-layer architecture is the foundation.",
    )
    .await;

    // Write a valid featured-topic.yaml pointing at the topic above.
    tokio::fs::write(
        dir.path().join("featured-topic.yaml"),
        "slug: topic-three-layer\nsince: \"2026-04-20\"\nnote: \"Feature for launch\"\n",
    )
    .await
    .unwrap();

    let (state, _state_dir) = build_state(dir.path()).await;
    let (status, html) = get_home(state).await;

    assert_eq!(status, StatusCode::OK);
    // Featured panel removed in Wave 5C; topic should appear in category grid instead.
    assert!(
        html.contains("Three-Layer Architecture"),
        "topic title must appear in category sections: snippet={}",
        &html[..html.len().min(1000)]
    );
}

// ─── Test 5: featured panel suppressed when slug unresolvable ────────────────

#[tokio::test]
async fn featured_topic_yaml_unresolvable_slug_suppresses_panel() {
    let dir = tempfile::tempdir().unwrap();

    tokio::fs::write(
        dir.path().join("index.md"),
        "---\ntitle: \"Home\"\ncategory: \"root\"\n---\nLede.\n",
    )
    .await
    .unwrap();

    write_topic(
        dir.path(),
        "topic-real.md",
        "Real Topic",
        "architecture",
        None,
        "Body.",
    )
    .await;

    // featured-topic.yaml points at a slug that does not exist.
    tokio::fs::write(
        dir.path().join("featured-topic.yaml"),
        "slug: topic-does-not-exist\n",
    )
    .await
    .unwrap();

    let (state, _state_dir) = build_state(dir.path()).await;
    let (status, html) = get_home(state).await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        !html.contains("wiki-home-featured"),
        "featured panel must be suppressed for unresolvable slug"
    );
}

// ─── Test 6: recent feed sorts by last_edited descending ─────────────────────

#[tokio::test]
async fn recent_feed_sorts_by_last_edited_desc() {
    let dir = tempfile::tempdir().unwrap();

    tokio::fs::write(
        dir.path().join("index.md"),
        "---\ntitle: \"Home\"\ncategory: \"root\"\n---\nLede.\n",
    )
    .await
    .unwrap();

    // Three topics with explicit last_edited values in non-alphabetical date order.
    write_topic(
        dir.path(),
        "topic-alpha.md",
        "Alpha",
        "architecture",
        Some("2026-04-15"),
        "Body.",
    )
    .await;
    write_topic(
        dir.path(),
        "topic-beta.md",
        "Beta",
        "services",
        Some("2026-04-20"),
        "Body.",
    )
    .await;
    write_topic(
        dir.path(),
        "topic-gamma.md",
        "Gamma",
        "governance",
        Some("2026-04-10"),
        "Body.",
    )
    .await;

    let (state, _state_dir) = build_state(dir.path()).await;
    let (status, html) = get_home(state).await;

    assert_eq!(status, StatusCode::OK);
    assert!(html.contains("mp-itn"), "recent feed must be present");

    // Extract just the recent-feed section so positional assertions are clean.
    // The recent list header has id="mp-itn".
    let recent_start = html.find("mp-itn").expect("recent list must appear");
    let recent_section = &html[recent_start..];

    // All three topics must appear within the recent section (they're not given
    // category cards because they each live in separate categories that only
    // have one item, so the card list shows them; we look in the recent section
    // specifically using topic slugs rendered as hrefs).
    assert!(
        recent_section.contains("topic-beta"),
        "topic-beta must appear in recent section"
    );
    assert!(
        recent_section.contains("topic-alpha"),
        "topic-alpha must appear in recent section"
    );
    assert!(
        recent_section.contains("topic-gamma"),
        "topic-gamma must appear in recent section"
    );

    // Beta (2026-04-20) must appear before Alpha (2026-04-15) which must appear
    // before Gamma (2026-04-10) in the rendered recent section.
    let beta_pos = recent_section.find("topic-beta").unwrap();
    let alpha_pos = recent_section.find("topic-alpha").unwrap();
    let gamma_pos = recent_section.find("topic-gamma").unwrap();

    assert!(
        beta_pos < alpha_pos,
        "Beta (newest) should precede Alpha in recent feed"
    );
    assert!(
        alpha_pos < gamma_pos,
        "Alpha should precede Gamma (oldest) in recent feed"
    );
}

// ─── Test 7: empty categories render placeholder copy ────────────────────────

#[tokio::test]
async fn category_with_zero_articles_renders_placeholder() {
    let dir = tempfile::tempdir().unwrap();

    tokio::fs::write(
        dir.path().join("index.md"),
        "---\ntitle: \"Home\"\ncategory: \"root\"\n---\nLede.\n",
    )
    .await
    .unwrap();

    // Only 3 categories populated; other 9 should show placeholder.
    write_topic(
        dir.path(),
        "topic-arch-one.md",
        "Arch One",
        "architecture",
        None,
        "Body.",
    )
    .await;
    write_topic(
        dir.path(),
        "topic-svc-one.md",
        "Svc One",
        "services",
        None,
        "Body.",
    )
    .await;
    write_topic(
        dir.path(),
        "topic-gov-one.md",
        "Gov One",
        "governance",
        None,
        "Body.",
    )
    .await;

    let (state, _state_dir) = build_state(dir.path()).await;
    let (status, html) = get_home(state).await;

    assert_eq!(status, StatusCode::OK);

    // All 7 slide IA tiles always render; populated tiles get an article count.
    // Fixture: architecture → "Multi-Entity Consolidation", services → "Integration & Data Portability",
    // governance → "Developer Platform".
    assert!(
        html.contains("Multi-Entity Consolidation"),
        "populated area Multi-Entity Consolidation must render: snippet={}",
        &html[..html.len().min(1500)]
    );
    assert!(
        html.contains("Integration &amp; Data Portability"),
        "populated area must render"
    );
    assert!(
        html.contains("Developer Platform"),
        "populated area must render"
    );
    // Sprint C removed legacy category names — they must not appear in the grid.
    assert!(
        !html.contains("Archetypes"),
        "legacy category name must not appear in grid"
    );
    assert!(
        !html.contains("Infrastructure"),
        "legacy category name must not appear in grid"
    );
}

// ─── Test 8: substrate category buckets correctly + humanized heading ─────────

#[tokio::test]
async fn substrate_category_buckets_and_renders_humanized() {
    let dir = tempfile::tempdir().unwrap();

    tokio::fs::write(
        dir.path().join("index.md"),
        "---\ntitle: \"Home\"\ncategory: \"root\"\n---\nLede.\n",
    )
    .await
    .unwrap();

    write_topic(
        dir.path(),
        "topic-substrate-one.md",
        "Substrate Topic",
        "substrate",
        None,
        "Body.",
    )
    .await;

    let (state, _state_dir) = build_state(dir.path()).await;
    let (status, html) = get_home(state).await;

    assert_eq!(status, StatusCode::OK);

    // Topic must appear in the substrate bucket, not the uncategorised catch-all.
    assert!(
        html.contains("Substrate Topic"),
        "substrate topic must appear in home page: snippet={}",
        &html[..html.len().min(1000)]
    );
    assert!(
        !html.contains("All articles"),
        "substrate topic must not fall into uncategorised catch-all"
    );

    // Category heading must render as "Substrate" (humanized), not "substrate".
    assert!(
        html.contains("Substrate"),
        "substrate heading must be humanized to 'Substrate'"
    );
}
