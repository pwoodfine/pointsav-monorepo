//! Git history and raw markdown routes.
//!
//! Phase 4: handlers for git-versioned content access.
//!
//! Routes owned by this module:
//! - `GET /git/{*slug}`      — raw Markdown source for the article at {slug}
//! - `GET /api/history/{slug}` — JSON history for the article (Phase 5 target)
//! - `GET /api/links/{slug}`   — JSON inbound/outbound wikilink graph (Phase 5 target)
//!
//! NOTE: These routes are currently wired through `server::router()` via the
//! delegation in `routes/mod.rs`. The raw Markdown handler (GET /git/{slug})
//! is implemented in `server/mod.rs` as `git_markdown`. History and links API
//! handlers are in `server/misc_handlers.rs`.

use axum::http::StatusCode;
use axum::response::IntoResponse;

/// Phase 4: GET /git/{*slug} — raw Markdown source.
///
/// Returns the article's Markdown source with `Content-Type: text/plain`.
/// Strips `.md` suffix if present in the slug. Supports git-clone-style
/// access: `/git/architecture/foo.md` and `/git/architecture/foo` both work.
#[allow(dead_code)]
pub async fn raw_markdown() -> impl IntoResponse {
    // Phase 4 target: migrate git_markdown handler from server/mod.rs.
    StatusCode::NOT_IMPLEMENTED
}

/// Phase 5: GET /api/history/{slug} — article revision history JSON.
#[allow(dead_code)]
pub async fn history_api() -> impl IntoResponse {
    StatusCode::NOT_IMPLEMENTED
}

/// Phase 5: GET /api/links/{slug} — wikilink graph JSON.
#[allow(dead_code)]
pub async fn links_api() -> impl IntoResponse {
    StatusCode::NOT_IMPLEMENTED
}
