//! Marketing platform engine — binary entry.
//!
//! `app-mediakit-marketing serve --content-dir <dir>` renders a tenant's
//! section-manifest pages. One binary, per-instance config (the model the live
//! deployment already uses). See `src/lib.rs` for the architecture overview.

use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use tokio::signal;
use tracing_subscriber::EnvFilter;

use app_mediakit_marketing::config::Config;
use app_mediakit_marketing::pending::Queue;
use app_mediakit_marketing::server::{router, AppState};
use app_mediakit_shell::{tokens, Brand};

#[derive(Parser)]
#[command(name = "app-mediakit-marketing", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run the marketing engine HTTP server.
    Serve {
        /// Directory of page manifests (`<dir>/<slug>/page.yaml`).
        #[arg(long, env = "SERVICE_MARKETING_CONTENT_DIR")]
        content_dir: PathBuf,

        /// Persistent state dir (AI-proposal review queue).
        #[arg(
            long,
            env = "SERVICE_MARKETING_STATE_DIR",
            default_value = "/var/lib/local-marketing/state"
        )]
        state_dir: PathBuf,

        /// Tenant module id selecting brand chrome.
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

        /// Mount the MCP JSON-RPC endpoint at POST /api/mcp.
        #[arg(long, env = "SERVICE_MARKETING_ENABLE_MCP")]
        enable_mcp: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let Cli { command } = Cli::parse();
    match command {
        Command::Serve {
            content_dir,
            state_dir,
            module_id,
            site_title,
            tokens_css,
            bind,
            enable_mcp,
        } => {
            serve(Config {
                content_dir,
                state_dir,
                module_id,
                site_title,
                tokens_css_path: tokens_css,
                bind,
                enable_mcp,
            })
            .await
        }
    }
}

async fn serve(cfg: Config) -> Result<()> {
    if !cfg.content_dir.is_dir() {
        bail!(
            "content directory does not exist or is not a directory: {}",
            cfg.content_dir.display()
        );
    }
    std::fs::create_dir_all(&cfg.state_dir).ok();

    let mut brand = Brand::by_module_id(&cfg.module_id);
    if let Some(title) = cfg.site_title.clone() {
        brand.site_title = title;
    }
    let tokens_css = tokens::load_tokens(cfg.tokens_css_path.as_deref());
    let pending = Queue::open(&cfg.state_dir)?;

    tracing::info!(
        content_dir = %cfg.content_dir.display(),
        module_id = %cfg.module_id,
        mcp = cfg.enable_mcp,
        %cfg.bind,
        "starting marketing engine"
    );

    let state = AppState {
        content_dir: cfg.content_dir,
        brand,
        tokens_css,
        pending,
        mcp_enabled: cfg.enable_mcp,
    };

    let listener = tokio::net::TcpListener::bind(cfg.bind).await?;
    tracing::info!(addr = %cfg.bind, "listening");
    axum::serve(listener, router(state))
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    tracing::info!("shut down cleanly");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c().await.expect("install ctrl-c handler");
    };
    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {}
        _ = terminate => {}
    }
}
