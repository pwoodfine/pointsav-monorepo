//! Feed and sitemap routes.
//!
//! Phase 4: handlers for content syndication and discovery.
//!
//! Routes owned by this module:
//! - `GET /feed.atom`    — Atom RFC 4287 feed (20 most recent articles)
//! - `GET /feed.json`    — JSON Feed v1 (same content set)
//! - `GET /sitemap.xml`  — XML sitemap with all article URLs + lastmod
//! - `GET /robots.txt`   — robots.txt (allow all; link to sitemap)
//! - `GET /llms.txt`     — llms.txt listing the wiki and its content
//!
//! NOTE: These routes are currently wired through `server::router()` via the
//! delegation in `routes/mod.rs`. The real implementations live in
//! `crate::feeds` (atom + JSON) and `server/special_handlers.rs` (sitemap,
//! robots, llms). They are available for direct wiring as migration progresses.

use axum::http::StatusCode;
use axum::response::IntoResponse;

/// Phase 4: GET /feed.atom — Atom RFC 4287 feed.
///
/// Real implementation: `crate::feeds::get_atom` (in feeds.rs).
/// Generates Atom 1.0 XML with the 25 most recently-modified articles.
#[allow(dead_code)]
pub async fn atom_feed_handler() -> impl IntoResponse {
    // Phase 4 target: wire crate::feeds::get_atom directly here.
    StatusCode::NOT_IMPLEMENTED
}

/// Phase 4: GET /feed.json — JSON Feed v1.
///
/// Real implementation: `crate::feeds::get_json_feed` (in feeds.rs).
#[allow(dead_code)]
pub async fn json_feed_handler() -> impl IntoResponse {
    StatusCode::NOT_IMPLEMENTED
}

/// Phase 4: GET /sitemap.xml — XML sitemap.
///
/// Generates `<urlset>` with all article URLs and `<lastmod>` timestamps.
/// Real implementation: `sitemap_xml` in `server/special_handlers.rs`.
#[allow(dead_code)]
pub async fn sitemap_handler() -> impl IntoResponse {
    StatusCode::NOT_IMPLEMENTED
}

/// Phase 4: GET /robots.txt
///
/// Allows all crawlers; links to /sitemap.xml.
#[allow(dead_code)]
pub async fn robots_handler() -> impl IntoResponse {
    StatusCode::NOT_IMPLEMENTED
}

/// Phase 4: GET /llms.txt
///
/// Lists the wiki engine, its content repos, and available machine-readable
/// endpoints (MCP, OpenAPI, Atom feed). Format: LLMs.txt draft spec.
#[allow(dead_code)]
pub async fn llms_handler() -> impl IntoResponse {
    StatusCode::NOT_IMPLEMENTED
}
