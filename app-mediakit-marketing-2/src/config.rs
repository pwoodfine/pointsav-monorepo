// SPDX-License-Identifier: FSL-1.1-ALv2
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

//! Runtime configuration. Per-instance via env/args so one binary serves
//! multiple tenants — the contract the live deployment already relies on
//! (two systemd units pointing the same binary at two content dirs). Env var
//! names mirror the retired engine's `SERVICE_MARKETING_*` contract exactly,
//! so the eventual swap-in (plan P8) requires zero/minimal config change.

use std::net::SocketAddr;
use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "app-mediakit-marketing-2",
    version,
    about = "Marketing platform engine (rewrite)"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Run the marketing engine HTTP server.
    Serve {
        /// Directory of page manifests (`<dir>/<slug>/page.yaml`).
        #[arg(long, env = "SERVICE_MARKETING_CONTENT_DIR")]
        content_dir: PathBuf,

        /// Persistent state dir (AI-proposal review queue, P5).
        #[arg(
            long,
            env = "SERVICE_MARKETING_STATE_DIR",
            default_value = "/var/lib/local-marketing/state"
        )]
        state_dir: PathBuf,

        /// Tenant module id selecting brand chrome ("woodfine", "pointsav").
        #[arg(long, env = "SERVICE_MARKETING_MODULE_ID", default_value = "woodfine")]
        module_id: String,

        /// Optional site-title override (otherwise the brand default).
        #[arg(long, env = "SERVICE_MARKETING_SITE_TITLE")]
        site_title: Option<String>,

        /// Optional external DTCG tokens.css (Style-Dictionary output).
        #[arg(long, env = "SERVICE_MARKETING_TOKENS_CSS")]
        tokens_css: Option<PathBuf>,

        /// HTTP bind address.
        #[arg(long, env = "SERVICE_MARKETING_BIND", default_value = "127.0.0.1:9102")]
        bind: SocketAddr,

        /// Mount the MCP JSON-RPC endpoint at POST /api/mcp (P5).
        #[arg(long, env = "SERVICE_MARKETING_ENABLE_MCP")]
        enable_mcp: bool,
    },
    /// Validate config/content without serving (CI gate; grows in P1).
    Check {
        #[arg(long, env = "SERVICE_MARKETING_CONTENT_DIR")]
        content_dir: PathBuf,

        #[arg(long, env = "SERVICE_MARKETING_MODULE_ID", default_value = "woodfine")]
        module_id: String,
    },
}

/// Resolved server configuration, built from `Command::Serve` fields.
#[derive(Debug, Clone)]
pub struct Config {
    /// Directory holding page manifests (`<content_dir>/<slug>/page.yaml`).
    pub content_dir: PathBuf,
    /// Persistent state dir (the AI-proposal review queue lives here, P5).
    pub state_dir: PathBuf,
    /// Tenant module id selecting the brand chrome ("woodfine", "pointsav").
    pub module_id: String,
    /// Optional site-title override (otherwise the brand default).
    pub site_title: Option<String>,
    /// Optional external DTCG tokens.css (Style-Dictionary output). Falls
    /// back to the built-in tokens when absent/unreadable.
    pub tokens_css_path: Option<PathBuf>,
    /// HTTP bind address.
    pub bind: SocketAddr,
    /// Mount the MCP JSON-RPC endpoint at `POST /api/mcp` (P5).
    pub enable_mcp: bool,
}
