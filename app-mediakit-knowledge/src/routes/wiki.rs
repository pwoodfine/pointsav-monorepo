//! Wiki article routes.
//!
//! Phase 4: handlers for wiki article pages.
//!
//! Routes owned by this module:
//! - `GET /wiki/{*slug}`    — article page (English)
//! - `GET /es/wiki/{*slug}` — article page (Spanish; falls back to EN redirect)
//!
//! NOTE: These routes are currently wired through `server::router()` via the
//! delegation in `routes/mod.rs`. The implementations here mirror the server
//! handlers and are available for direct wiring as Phase 4 migration progresses.

use axum::http::StatusCode;
use axum::response::IntoResponse;

/// Phase 4: GET /wiki/{*slug} handler.
///
/// Reads the article from the content directory, renders Markdown to HTML,
/// applies article chrome (tabs, TOC, hatnote, language switcher), and
/// returns the full page. Returns 404 when the slug is not found in any
/// configured mount.
///
/// Implementation currently lives in `server/wiki_handlers.rs` and is wired
/// via `server::router()`. This stub documents the Phase 4 target signature.
#[allow(dead_code)]
pub async fn wiki_page() -> impl IntoResponse {
    // Phase 4 target: migrate wiki_page handler from server/wiki_handlers.rs.
    // Blocked on L19 (mounts Vec<Mount> migration); currently served by server::router().
    StatusCode::NOT_IMPLEMENTED
}

/// Phase 4: GET /es/wiki/{*slug} handler.
///
/// Prefers the `.es.md` sibling when it exists; redirects to `/wiki/{slug}`
/// when no Spanish sibling is present (graceful degradation per L4).
#[allow(dead_code)]
pub async fn es_wiki_page() -> impl IntoResponse {
    // Phase 4 target: migrate wiki_page_es handler from server/wiki_handlers.rs.
    StatusCode::NOT_IMPLEMENTED
}
