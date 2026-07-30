//! Homepage routes.
//!
//! Phase 4: handlers for the wiki home pages.
//!
//! Routes owned by this module:
//! - `GET /`      — English home page
//! - `GET /es/`   — Spanish home page
//!
//! NOTE: These routes are currently wired through `server::router()` via the
//! delegation in `routes/mod.rs`. The implementations here document the Phase 4
//! target signature and are available for direct wiring as migration progresses.

use axum::http::StatusCode;
use axum::response::IntoResponse;

/// Phase 4: GET / handler.
///
/// Renders the wiki home page with the category grid, leapfrog facts panel,
/// and recently-updated articles strip. Locale: English.
#[allow(dead_code)]
pub async fn home_page() -> impl IntoResponse {
    // Phase 4 target: migrate index handler from server/home_handlers.rs.
    StatusCode::NOT_IMPLEMENTED
}

/// Phase 4: GET /es/ handler.
///
/// Renders the wiki home page in Spanish. All chrome strings come from the
/// `strings(Locale::Es)` map (L22 compliance). Falls back to English article
/// content when no `.es.md` sibling exists.
#[allow(dead_code)]
pub async fn es_home_page() -> impl IntoResponse {
    // Phase 4 target: migrate home_es handler from server/home_handlers.rs.
    StatusCode::NOT_IMPLEMENTED
}
