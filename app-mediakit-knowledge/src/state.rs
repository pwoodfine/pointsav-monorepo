//! Application state — shared across all route handlers.
//!
//! `AppState` is the central shared-state carrier injected into every axum
//! handler via `State<AppState>`. It wraps the fully-wired resource set
//! (search index, git repo, link graph, etc.) and is cheap to clone because
//! all heavy fields are `Arc`-wrapped.
//!
//! Phase 1: re-exports the existing `server::AppState` so the modular
//! `routes/` and `chrome/` modules can import from `crate::state` rather
//! than from `crate::server`, preparing for the full AppState migration
//! in Phase 4 when `server.rs` is retired.

// Re-export the live AppState from server.rs so downstream modules do not
// need to know the current location. When the Phase 4 routes/ migration
// moves AppState out of server.rs, only this file needs updating.
pub use crate::server::AppState;
