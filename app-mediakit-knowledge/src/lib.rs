//! Wiki engine library — modular architecture per §5 of the master BRIEF.
//!
//! Module layout follows `BRIEF-knowledge-platform-master.md` §5. Each module
//! owns one concern; `server.rs` is the Phase 1 legacy monolith being
//! decomposed into `routes/` and `chrome/` across Phases 2–4.
//!
//! L20 discipline: no `.rs` file may exceed ~1,500 lines.

// ── Foundation modules (real implementations) ──────────────────────────────
pub mod assets;
pub mod blueprints;
pub mod check;
pub mod config;
pub mod error;
pub mod mounts;
pub mod state;
pub mod walker;

// ── Feature modules (real implementations, live in production) ─────────────
pub mod annotations;
pub mod activitypub;
pub mod citations;
pub mod claim;
pub mod feeds;
pub mod git;
pub mod git_protocol;
pub mod glossary;
pub mod history;
pub mod jsonld;
pub mod links;
pub mod mcp;
pub mod render;
pub mod search;

// ── Modular route layer (Phase 4 migration target) ─────────────────────────
pub mod routes;

// ── Chrome layer (Phase 3 implementation target) ───────────────────────────
pub mod chrome;

// ── Server module (Phase 1 decomposition: server/mod.rs + handler includes) ─
// AppState + router() + handler implementations split across:
//   server/mod.rs               — AppState, router, shared helpers (~791 lines)
//   server/home_handlers.rs     — home page, category, bucketing (~918 lines)
//   server/wiki_handlers.rs     — wiki page, wiki chrome (~1085 lines)
//   server/special_handlers.rs  — special pages, sitemap, feeds (~1184 lines)
//   server/misc_handlers.rs     — chrome, history, diff, tests (~1321 lines)
// Phase 4 target: migrate each handler group into routes/ submodules.
pub mod server;
