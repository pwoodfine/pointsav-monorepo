//! Category listing routes.
//!
//! Phase 4: handler for category browse pages.
//!
//! Routes owned by this module:
//! - `GET /category/{name}` — category article listing
//!
//! The category page collects all articles tagged with the named category
//! across all mounts, sorts them (featured first, then by last_edited
//! descending), and renders article cards with auto-extracted lede.
//!
//! NOTE: This route is currently wired through `server::router()` via the
//! delegation in `routes/mod.rs`. The implementation here documents the
//! Phase 4 target signature.

use axum::http::StatusCode;
use axum::response::IntoResponse;

/// Phase 4: GET /category/{name} handler.
///
/// Collects all articles in the named category from all mounts.
/// Sort order: featured articles first, then by `last_edited` descending.
/// Renders article cards with: count badge, featured article card,
/// recently-updated strip (3 most recent).
#[allow(dead_code)]
pub async fn category_page() -> impl IntoResponse {
    // Phase 4 target: migrate category_page handler from server/home_handlers.rs.
    // Currently served by server::router() which has the full implementation.
    StatusCode::NOT_IMPLEMENTED
}
