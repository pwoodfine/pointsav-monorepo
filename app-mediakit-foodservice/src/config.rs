//! Runtime configuration — per-instance via env/args. Env var names use the
//! `SERVICE_FOODSERVICE_*` prefix, matching the deployment contract for this
//! site so `os-mediakit` can launch instances unchanged.

use std::net::SocketAddr;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    /// Directory holding page manifests (`<content_dir>/<slug>/page.yaml`).
    pub content_dir: PathBuf,
    /// Persistent state dir (the AI-proposal review queue lives here).
    pub state_dir: PathBuf,
    /// Tenant module id selecting the brand chrome ("woodfine", "pointsav").
    pub module_id: String,
    /// Optional site-title override (otherwise the brand default).
    pub site_title: Option<String>,
    /// Optional external DTCG tokens.css (Style-Dictionary output).
    pub tokens_css_path: Option<PathBuf>,
    /// HTTP bind address.
    pub bind: SocketAddr,
    /// Mount the MCP JSON-RPC endpoint at `POST /api/mcp`.
    pub enable_mcp: bool,
}
