//! In-process route tests — exercise the full axum stack (handler →
//! content::load_page_lang → shell::render_page) without binding a socket, via
//! `tower::ServiceExt::oneshot`. Each test owns its content fixtures (a
//! tempdir), so it is independent of the shipped `content/` directory.

use app_mediakit_marketing::pending::Queue;
use app_mediakit_marketing::server::{router, AppState};
use app_mediakit_shell::{tokens, Brand};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tempfile::TempDir;
use tower::ServiceExt;

// A fixture page exercising every P2 section type.
const HOME_EN: &str = r#"
title: Home
slug: home
lang: en
sections:
  - type: hero
    headline: Hello
    subhead: A line.
    image: { src: /m/a.webp, alt: aerial }
  - type: card-grid
    heading: What we do
    columns: 3
    cards:
      - { title: Develop }
      - { title: Operate, href: /page/op }
  - type: feature
    heading: Feature
    body: "**bold** text"
    media_side: right
  - type: media
    image: { src: /m/b.webp, alt: photo }
  - type: cta
    heading: Talk to us
    cta: { label: Contact, href: /page/contact }
"#;

const HOME_ES: &str = r#"
title: Inicio
slug: home
lang: es
sections:
  - type: card-grid
    heading: Qué hacemos
    cards:
      - { title: Desarrollar }
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
async fn home_renders_full_section_vocabulary() {
    let (_dir, app) = fixture();
    let (status, body) = get(app, "/").await;
    assert_eq!(status, StatusCode::OK);
    for marker in [
        "section-hero",
        "hero-media",
        "card-grid",
        "section-feature",
        "feature-media-right",
        "section-media",
        "section-cta",
        "header-cta",
    ] {
        assert!(body.contains(marker), "missing rendered marker: {marker}");
    }
    assert!(body.contains(r#"<html lang="en">"#));
    // The legacy fragile client-side pattern must be structurally absent.
    assert!(!body.contains("__bundler/template"));
    // SEO tags must be present.
    assert!(
        body.contains(r#"rel="canonical""#),
        "missing canonical link"
    );
    assert!(body.contains(r#"property="og:title""#), "missing og:title");
    assert!(body.contains("application/ld+json"), "missing ld+json");
}

#[tokio::test]
async fn es_route_serves_spanish_variant() {
    let (_dir, app) = fixture();
    let (status, body) = get(app, "/es").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains(r#"<html lang="es">"#));
    assert!(body.contains("Qué hacemos"));
}

#[tokio::test]
async fn es_page_falls_back_when_no_variant() {
    // `contact` has no page.es.yaml -> falls back to page.yaml, still 200.
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
    assert!(
        body.contains("/page/contact"),
        "canonical must include slug path"
    );
    assert!(
        body.contains(r#"property="og:url""#),
        "og:url must be present"
    );
}

/// Guard: the shipped (migrated) home manifest must parse against the section
/// contract. Catches a malformed real `content/home/page.yaml` at test time.
#[test]
fn shipped_home_manifest_is_valid() {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("content");
    let page = app_mediakit_marketing::content::load_page(&dir, "home").expect("home parses");
    assert!(
        page.sections.len() >= 3,
        "expected the migrated multi-section home"
    );
}
