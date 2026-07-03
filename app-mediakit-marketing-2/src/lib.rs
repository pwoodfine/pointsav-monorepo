// SPDX-License-Identifier: FSL-1.1-ALv2
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

//! app-mediakit-marketing-2 — marketing platform engine (ground-up rewrite).
//!
//! Single-binary, server-rendered marketing site engine. One binary, per-tenant
//! env/CLI config (`SERVICE_MARKETING_*`) — the same live-deployment model the
//! retired engine used, preserved here as a contract, not as reused code.
//!
//! Module map (grows phase by phase — see the plan of record):
//!   config   — CLI/env config (P0)
//!   assets   — embedded static/ (P0)
//!   error    — MarketingError + response mapping (P0)
//!   app      — AppState + axum Router (P0, grows each phase)
//!   content  — section-manifest load/validate/render (P1)
//!   ui       — chrome shell + section rendering (P2/P3)

pub mod app;
pub mod assets;
pub mod config;
pub mod content;
pub mod error;
pub mod mcp;
pub mod pending;
pub mod ui;
