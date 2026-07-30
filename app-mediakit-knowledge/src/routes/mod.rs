//! Route assembly — all HTTP routes visible in one place.
//!
//! Phase 4: `routes::router()` is the canonical router entry point.
//! All route registration lives here; handler implementations live in the
//! sub-modules below.
//!
//! L20 compliance: no single file may exceed ~1,500 lines. The modular
//! layout enforces this structurally — each submodule owns one concern.
//!
//! Current implementation: delegates to `server::router()` which holds the
//! complete Phase 1–5 handler set. Phase 4 migration plan: each handler group
//! moves into its own sub-module as the `server/` monolith decomposes per
//! L20. The route table below is the target shape; server::router() matches
//! it exactly.
//!
//! Route table (Phase 4 target — all routes visible in one place):
//!
//! GET  /                            → home::home_page
//! GET  /es/                         → home::es_home_page
//! GET  /wiki/*slug                  → wiki::wiki_page
//! GET  /es/wiki/*slug               → wiki::es_wiki_page
//! GET  /category/:name              → category::category_page
//! GET  /search                      → search::search_page  [HARD REQUIREMENT §17.6]
//! GET  /api/search                  → search::search_api
//! GET  /api/complete                → search::complete_api
//! GET  /api/citations               → search::citations_api
//! GET  /api/history/:slug           → git::history_api
//! GET  /api/links/:slug             → git::links_api
//! GET  /feed.atom                   → feeds::atom_feed_handler
//! GET  /feed.json                   → feeds::json_feed_handler
//! GET  /sitemap.xml                 → feeds::sitemap_handler
//! GET  /robots.txt                  → feeds::robots_handler
//! GET  /llms.txt                    → feeds::llms_handler
//! GET  /healthz                     → healthz (inline)
//! GET  /git/*slug                   → git::raw_markdown
//! POST /mcp                         → mcp::mcp_handler (Phase 7, behind --enable-mcp)
//! GET  /static/*path                → ServeDir embedded via rust-embed

pub mod category;
pub mod feeds;
pub mod git;
pub mod home;
pub mod mcp;
pub mod search;
pub mod wiki;

use crate::state::AppState;
use axum::Router;

/// Assemble the full application router.
///
/// Phase 4: this is the canonical router. All routes from all sub-modules
/// are assembled here. Implementation currently delegates to `server::router()`
/// which holds the complete Phase 1–5 handler set. As Phase 4 migration
/// progresses, each handler group moves out of server/ into routes/ sub-modules
/// and the delegation line below is replaced with direct route registration.
///
/// Acceptance: `GET /healthz` → "ok"; `GET /search?q=` → HTML page with
/// search form and BM25 results.
pub fn router(state: AppState) -> Router {
    // Delegate to the fully-wired Phase 1–5 server router.
    // Phase 4 decomposition target: replace this line with explicit route
    // registrations as each handler migrates into routes/ sub-modules.
    crate::server::router(state)
}
