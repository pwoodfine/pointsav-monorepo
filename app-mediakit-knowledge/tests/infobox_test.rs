//! Sprint AC — Infobox and `main` fenced block integration tests.
//!
//! Verifies that the three new infobox features (title caption, image row,
//! image_caption) and the `main` hatnote directive render correctly end-to-end
//! through the HTTP layer.  CSS is not tested here; these tests guard the HTML
//! scaffold that CSS and JS depend on.

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

// ─── Infobox: title as caption ────────────────────────────────────────────────

#[tokio::test]
async fn infobox_title_renders_as_caption() {
    let dir = tempfile::tempdir().unwrap();
    tokio::fs::write(
        dir.path().join("topic-person.md"),
        "---\ntitle: \"Person\"\ncategory: \"reference\"\n---\n\n```infobox\ntitle: \"Jane Smith\"\nborn: \"1980\"\n```\n",
    )
    .await
    .unwrap();
    let (state, _sd) = build_state(dir.path()).await;
    let (status, html) = get_wiki(state, "person").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        html.contains("class=\"infobox-title\""),
        "caption element with infobox-title class missing"
    );
    assert!(html.contains("Jane Smith"), "caption text missing");
}

// ─── Infobox: title not duplicated as a data row ─────────────────────────────

#[tokio::test]
async fn infobox_title_not_in_data_rows() {
    let dir = tempfile::tempdir().unwrap();
    tokio::fs::write(
        dir.path().join("topic-bio.md"),
        "---\ntitle: \"Bio\"\ncategory: \"reference\"\n---\n\n```infobox\ntitle: \"Ada Lovelace\"\nborn: \"1815\"\n```\n",
    )
    .await
    .unwrap();
    let (state, _sd) = build_state(dir.path()).await;
    let (_, html) = get_wiki(state, "bio").await;
    // The infobox table should have the born row but NOT a <th>title</th> row
    assert!(
        html.contains("<th>born</th>") || html.contains(">born<"),
        "born row missing"
    );
    assert!(
        !html.contains("<th>title</th>"),
        "title key must not appear as a data row — it is a reserved key"
    );
}

// ─── Infobox: image row ───────────────────────────────────────────────────────

#[tokio::test]
async fn infobox_image_renders_img_tag() {
    let dir = tempfile::tempdir().unwrap();
    tokio::fs::write(
        dir.path().join("topic-landmark.md"),
        "---\ntitle: \"Landmark\"\ncategory: \"reference\"\n---\n\n```infobox\ntitle: \"Tower\"\nimage: \"/static/images/tower.jpg\"\nimage_caption: \"The tower at dusk\"\nheight: \"300m\"\n```\n",
    )
    .await
    .unwrap();
    let (state, _sd) = build_state(dir.path()).await;
    let (status, html) = get_wiki(state, "landmark").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        html.contains("class=\"infobox-image\""),
        "infobox-image cell missing"
    );
    assert!(
        html.contains("src=\"/static/images/tower.jpg\""),
        "img src missing"
    );
    assert!(
        html.contains("The tower at dusk"),
        "image_caption text missing"
    );
    assert!(
        html.contains("class=\"infobox-caption\""),
        "infobox-caption class missing"
    );
}

// ─── Infobox: image key not in data rows ─────────────────────────────────────

#[tokio::test]
async fn infobox_image_not_in_data_rows() {
    let dir = tempfile::tempdir().unwrap();
    tokio::fs::write(
        dir.path().join("topic-img.md"),
        "---\ntitle: \"Img\"\ncategory: \"reference\"\n---\n\n```infobox\nimage: \"/img.jpg\"\nimage_caption: \"A caption\"\nfield: \"value\"\n```\n",
    )
    .await
    .unwrap();
    let (state, _sd) = build_state(dir.path()).await;
    let (_, html) = get_wiki(state, "img").await;
    assert!(
        !html.contains("<th>image</th>"),
        "image key must not appear as a data row"
    );
    assert!(
        !html.contains("<th>image_caption</th>"),
        "image_caption key must not appear as a data row"
    );
    assert!(
        html.contains("<th>field</th>") || html.contains(">field<"),
        "regular field missing"
    );
}

// ─── Main block: hatnote rendered with wiki-hatnote class ────────────────────

#[tokio::test]
async fn main_block_renders_hatnote() {
    let dir = tempfile::tempdir().unwrap();
    tokio::fs::write(
        dir.path().join("topic-overview.md"),
        "---\ntitle: \"Overview\"\ncategory: \"architecture\"\n---\n\n## Section\n\n```main\narchitecture/compounding-substrate\n```\n\nSome text.\n",
    )
    .await
    .unwrap();
    let (state, _sd) = build_state(dir.path()).await;
    let (status, html) = get_wiki(state, "overview").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        html.contains("class=\"wiki-hatnote\""),
        "wiki-hatnote class missing on main block"
    );
    assert!(
        html.contains("Main article:"),
        "\"Main article:\" prefix missing"
    );
    assert!(
        html.contains("href=\"/wiki/architecture/compounding-substrate\""),
        "link href missing"
    );
}

// ─── Main block: slug-derived display text ────────────────────────────────────

#[tokio::test]
async fn main_block_derives_display_from_slug() {
    let dir = tempfile::tempdir().unwrap();
    tokio::fs::write(
        dir.path().join("topic-section.md"),
        "---\ntitle: \"Section\"\ncategory: \"architecture\"\n---\n\n```main\narchitecture/economic-model\n```\n",
    )
    .await
    .unwrap();
    let (state, _sd) = build_state(dir.path()).await;
    let (_, html) = get_wiki(state, "section").await;
    // Last segment "economic-model" → "Economic Model"
    assert!(
        html.contains("Economic Model"),
        "display text not derived from slug: expected \"Economic Model\""
    );
}

// ─── Main block: explicit pipe display text ───────────────────────────────────

#[tokio::test]
async fn main_block_pipe_display_text() {
    let dir = tempfile::tempdir().unwrap();
    tokio::fs::write(
        dir.path().join("topic-pipe.md"),
        "---\ntitle: \"Pipe\"\ncategory: \"architecture\"\n---\n\n```main\narchitecture/compounding-substrate|The Compounding Substrate\n```\n",
    )
    .await
    .unwrap();
    let (state, _sd) = build_state(dir.path()).await;
    let (_, html) = get_wiki(state, "pipe").await;
    assert!(
        html.contains("The Compounding Substrate"),
        "explicit pipe display text not rendered"
    );
    assert!(
        html.contains("href=\"/wiki/architecture/compounding-substrate\""),
        "slug href should use the part before the pipe"
    );
}
