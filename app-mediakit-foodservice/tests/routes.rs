//! In-process route tests — exercise the full axum stack without binding a
//! socket, via `tower::ServiceExt::oneshot`. Each test owns its content
//! fixtures (a tempdir), independent of the shipped `content/` directory.

use app_mediakit_foodservice::pending::Queue;
use app_mediakit_foodservice::server::{router, AppState};
use app_mediakit_shell::{tokens, Brand};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tempfile::TempDir;
use tower::ServiceExt;

const HOME_EN: &str = r#"
title: Home
slug: home
lang: en
sections:
  - type: hero
    headline: Welcome
    subhead: A food service platform.
  - type: card-grid
    heading: Links
    columns: 3
    cards:
      - { title: Home, href: /page/contact }
  - type: prose
    body: "**Footer** text."
"#;

const HOME_ES: &str = r#"
title: Inicio
slug: home
lang: es
sections:
  - type: card-grid
    heading: Inicio
    cards:
      - { title: Inicio }
"#;

const CONTACT: &str =
    "title: Contact\nslug: contact\nsections:\n  - type: hero\n    headline: Contact\n";

fn fixture() -> (TempDir, axum::Router) {
    let dir = tempfile::tempdir().unwrap();
    let c = dir.path();
    std::fs::create_dir_all(c.join("home")).unwrap();
    std::fs::create_dir_all(c.join("contact")).unwrap();
    std::fs::write(c.join("home/page.yaml"), HOME_EN).unwrap();
    std::fs::write(c.join("home/page.es.yaml"), HOME_ES).unwrap();
    std::fs::write(c.join("contact/page.yaml"), CONTACT).unwrap();
    let state = AppState {
        content_dir: c.to_path_buf(),
        brand: Brand::woodfine(),
        tokens_css: tokens::DEFAULT_TOKENS_CSS.to_string(),
        pending: Queue::open(c).unwrap(),
        mcp_enabled: false,
    };
    (dir, router(state))
}

async fn get(app: axum::Router, uri: &str) -> (StatusCode, String) {
    let resp = app
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

#[tokio::test]
async fn home_renders_ok() {
    let (_dir, app) = fixture();
    let (status, body) = get(app, "/").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains(r#"<html lang="en">"#));
    assert!(body.contains("section-hero"));
    assert!(!body.contains("__bundler/template"));
}

#[tokio::test]
async fn es_route_serves_spanish_variant() {
    let (_dir, app) = fixture();
    let (status, body) = get(app, "/es").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains(r#"<html lang="es">"#));
    assert!(body.contains("Inicio"));
}

#[tokio::test]
async fn es_page_falls_back_when_no_variant() {
    let (_dir, app) = fixture();
    let (status, _) = get(app, "/es/page/contact").await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn healthz_ok() {
    let (_dir, app) = fixture();
    let (status, body) = get(app, "/healthz").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "ok");
}

#[tokio::test]
async fn unknown_page_404() {
    let (_dir, app) = fixture();
    let (status, _) = get(app, "/page/nope").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn canonical_url_contains_request_path() {
    let (_dir, app) = fixture();
    let (status, body) = get(app.clone(), "/page/contact").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("/page/contact"));
    assert!(body.contains(r#"property="og:url""#));
}

/// Guard: the shipped home manifest must parse against the section contract.
#[test]
fn shipped_home_manifest_is_valid() {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("content");
    let page = app_mediakit_foodservice::content::load_page(&dir, "home").expect("home parses");
    assert!(!page.sections.is_empty(), "expected at least one section");
}
