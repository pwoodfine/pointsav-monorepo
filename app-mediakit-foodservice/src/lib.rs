//! `app-mediakit-foodservice` — the food-service platform engine.
//!
//! Server-rendered, agent-first. Follows the `app-mediakit-marketing` pattern:
//! typed section-manifests (`app-mediakit-shell::Page`), Git-tracked YAML,
//! MCP authoring surface, F12 human approval gate (SYS-ADR-10, SYS-ADR-19).
//!
//! Serves `foodservice.woodfinegroup.com`.

pub mod config;
pub mod content;
pub mod mcp;
pub mod pending;
pub mod server;

pub use server::{router, AppState};
