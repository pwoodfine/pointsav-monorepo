// SPDX-License-Identifier: FSL-1.1-ALv2
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

//! Marketing platform engine (rewrite) — binary entry.
//!
//! `app-mediakit-marketing-2 serve --content-dir <dir>` renders a tenant's
//! section-manifest pages. One binary, per-instance config — the model the
//! live deployment already uses. See `src/lib.rs` for the module map.

use clap::Parser;
use tracing_subscriber::EnvFilter;

use app_mediakit_marketing_2::app::{build_state, router};
use app_mediakit_marketing_2::config::{Cli, Command, Config};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
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
        Command::Check {
            content_dir,
            module_id,
        } => check(&content_dir, &module_id),
    }
}

async fn serve(cfg: Config) -> anyhow::Result<()> {
    if !cfg.content_dir.is_dir() {
        anyhow::bail!(
            "content directory does not exist or is not a directory: {}",
            cfg.content_dir.display()
        );
    }
    std::fs::create_dir_all(&cfg.state_dir).ok();

    tracing::info!(
        content_dir = %cfg.content_dir.display(),
        module_id = %cfg.module_id,
        mcp = cfg.enable_mcp,
        %cfg.bind,
        "starting marketing engine (rewrite)"
    );

    let state = build_state(&cfg)?;
    let listener = tokio::net::TcpListener::bind(cfg.bind).await?;
    tracing::info!(addr = %cfg.bind, "listening");
    axum::serve(listener, router(state, cfg.enable_mcp))
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    tracing::info!("shut down cleanly");
    Ok(())
}

fn check(content_dir: &std::path::Path, module_id: &str) -> anyhow::Result<()> {
    if !content_dir.is_dir() {
        anyhow::bail!(
            "content directory does not exist or is not a directory: {}",
            content_dir.display()
        );
    }
    println!(
        "ok: content_dir={} module_id={module_id}",
        content_dir.display()
    );
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("install ctrl-c handler");
    };
    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
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
