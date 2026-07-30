//! MCP JSON-RPC 2.0 route — Phase 7.
//!
//! Routes owned by this module:
//! - `POST /mcp` — MCP JSON-RPC 2.0 endpoint (behind --enable-mcp flag)
//!
//! The MCP route is mounted ONLY when `--enable-mcp` / `WIKI_ENABLE_MCP` is
//! set. When the flag is absent, the route is not registered and POST /mcp
//! returns 404.
//!
//! The real implementation lives in `crate::mcp`. This module re-exports
//! the handler so routes/mod.rs can mount it via the standard `routes/`
//! pattern, keeping routing and logic separate.
//!
//! MCP methods implemented (Phase 7):
//!   initialize / initialized / notifications/initialized
//!   tools/list · tools/call  (tools: create_topic, propose_edit, link_citation)
//!   resources/list · resources/read  (scheme: wiki://topic/{slug})
//!   prompts/list · prompts/get  (cite-this-page, summarize-topic, draft-related-topic)
//!   query_claims  (Phase 11)
//!   query_page    (Phase 7 — fetch article by slug with html_body + backlinks)
//!   search        (Phase 7 — BM25 full-text search)
//!   list_pages    (Phase 7 — paginate articles with optional category/status filter)
//!   get_links     (Phase 7 — forward or backward wikilink graph query)
//!   get_citations (Phase 7 — citation registry lookup by ID list)

/// `POST /mcp` — JSON-RPC 2.0 endpoint.
///
/// Delegates to `crate::mcp::handler`, which is wired directly into the
/// axum router in `server::router()` behind the `mcp_enabled` flag.
/// This re-export exists so `routes/` remains the canonical route catalogue.
pub use crate::mcp::handler as mcp_handler;
