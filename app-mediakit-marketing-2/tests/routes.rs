// SPDX-License-Identifier: FSL-1.1-ALv2
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

//! Integration tests — exercise the full axum `Router` (handler → content →
//! chrome) without binding a socket, via `tower::ServiceExt::oneshot`. Each
//! test owns its own content/state fixtures (tempdirs); none depend on the
//! shipped `app-mediakit-marketing/content/` tree.

use app_mediakit_marketing_2::app::{build_state, router};
use app_mediakit_marketing_2::config::Config;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tempfile::TempDir;
use tower::ServiceExt;

fn fixture(module_id: &str, enable_mcp: bool) -> (TempDir, TempDir, axum::Router) {
    let content_dir = tempfile::tempdir().unwrap();
    let state_dir = tempfile::tempdir().unwrap();

    std::fs::create_dir_all(content_dir.path().join("home")).unwrap();
    std::fs::write(
        content_dir.path().join("home/page.yaml"),
        "title: Home\nslug: home\ndescription: Test home.\nsections:\n  - type: hero\n    headline: Hello\n",
    )
    .unwrap();
    std::fs::write(
        content_dir.path().join("home/page.es.yaml"),
        "title: Inicio\nslug: home\ndescription: Prueba.\nlang: es\nsections:\n  - type: hero\n    headline: Hola\n",
    )
    .unwrap();

    std::fs::create_dir_all(content_dir.path().join("contact")).unwrap();
    std::fs::write(
        content_dir.path().join("contact/page.yaml"),
        "title: Contact\nslug: contact\ndescription: Test contact.\nsections:\n  - type: hero\n    headline: Contact us\n",
    )
    .unwrap();
    std::fs::write(
        content_dir.path().join("contact/page.es.yaml"),
        "title: Contacto\nslug: contact\ndescription: Prueba.\nlang: es\nsections:\n  - type: hero\n    headline: Contáctenos\n",
    )
    .unwrap();

    let cfg = Config {
        content_dir: content_dir.path().to_path_buf(),
        state_dir: state_dir.path().to_path_buf(),
        module_id: module_id.to_string(),
        site_title: None,
        tokens_css_path: None,
        bind: "127.0.0.1:0".parse().unwrap(),
        enable_mcp,
    };
    let state = build_state(&cfg).unwrap();
    (content_dir, state_dir, router(state, enable_mcp))
}

async fn get(app: &axum::Router, uri: &str) -> (StatusCode, String) {
    let response = app
        .clone()
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8(body.to_vec()).unwrap())
}

#[tokio::test]
async fn home_renders_with_no_bundler_dom_swap_pattern() {
    let (_c, _s, app) = fixture("woodfine", false);
    let (status, body) = get(&app, "/").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.starts_with("<!DOCTYPE html>"));
    assert!(body.contains("Hello"));
    assert!(!body.contains("__bundler"));
    assert!(body.contains(r#"lang="en""#));
    assert!(body.contains(r#"data-brand="woodfine""#));
}

#[tokio::test]
async fn home_has_no_spanish_route() {
    // Operator call 2026-07-02: home is English-only, no /es route.
    let (_c, _s, app) = fixture("woodfine", false);
    let (status, _) = get(&app, "/es").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn spanish_subpage_route_still_serves_spanish_content() {
    // Non-home pages keep their /es/page/{slug} variant.
    let (_c, _s, app) = fixture("woodfine", false);
    let (status, body) = get(&app, "/es/page/contact").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("Contáctenos"));
    assert!(body.contains(r#"lang="es""#));
}

#[tokio::test]
async fn page_by_slug_renders() {
    let (_c, _s, app) = fixture("woodfine", false);
    let (status, body) = get(&app, "/page/contact").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("Contact us"));
}

#[tokio::test]
async fn missing_page_returns_404() {
    let (_c, _s, app) = fixture("woodfine", false);
    let (status, _) = get(&app, "/page/nonexistent").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn healthz_returns_ok() {
    let (_c, _s, app) = fixture("woodfine", false);
    let (status, body) = get(&app, "/healthz").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "ok");
}

#[tokio::test]
async fn static_assets_are_served() {
    let (_c, _s, app) = fixture("woodfine", false);
    let (status, body) = get(&app, "/static/tokens.css").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("--m-navy-700"));
}

#[tokio::test]
async fn robots_and_sitemap_are_served() {
    let (_c, _s, app) = fixture("woodfine", false);
    let (status, body) = get(&app, "/robots.txt").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("Sitemap:"));

    let (status, body) = get(&app, "/sitemap.xml").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("<urlset"));
    assert!(body.contains("home.woodfinegroup.com"));
}

#[tokio::test]
async fn mcp_routes_absent_without_enable_mcp() {
    let (_c, _s, app) = fixture("woodfine", false);
    let (status, _) = get(&app, "/api/pending").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn mcp_routes_present_with_enable_mcp() {
    let (_c, _s, app) = fixture("woodfine", true);
    let (status, body) = get(&app, "/api/pending").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "[]");
}

#[tokio::test]
async fn pointsav_tenant_renders_distinct_chrome() {
    let (_c, _s, app) = fixture("pointsav", false);
    let (status, body) = get(&app, "/").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains(r#"data-brand="pointsav""#));
    assert!(body.contains("PointSav Digital Systems"));
}
