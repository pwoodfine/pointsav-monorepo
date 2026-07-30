//! `app-mediakit-marketing` — the marketing platform engine.
//!
//! Clean-sheet rewrite (2026-06): server-rendered, agent-first. Replaces the
//! prior 1.2 MB single-file HTML monolith + client-side bundler/template
//! DOM-swap with a fully server-side Rust render path.
//!
//! - **Content model:** typed section-manifests (`app-mediakit-shell::Page`),
//!   Git-tracked YAML. The schema is the contract an AI author writes against.
//! - **Chrome + components:** owned by `app-mediakit-shell` (shared chassis).
//! - **Authoring:** AI proposes via the MCP server ([`mcp`]); proposals stage
//!   to a review queue ([`pending`]); a human approves (F12) before content is
//!   persisted — SYS-ADR-10. There is no automated publish path (SYS-ADR-19).
//!
//! The HTTP surface and `AppState` live in [`server`]; the binary entry point
//! is `src/main.rs`.

pub mod config;
pub mod content;
pub mod mcp;
pub mod pending;
pub mod server;

pub use server::{router, AppState};
