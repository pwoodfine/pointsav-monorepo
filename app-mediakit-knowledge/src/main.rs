//! Wiki engine binary entry — Phase 1 modular refactor.
//!
//! Preferred startup path: set `WIKI_KNOWLEDGE_TOML` to a `knowledge.toml`
//! instance file (see `config.rs` and §15 of the master BRIEF). Legacy path:
//! set individual `WIKI_CONTENT_DIR`, `WIKI_BIND`, etc. env vars — these
//! are synthesized into the same internal shape and continue to work unchanged.
//!
//! See ARCHITECTURE.md for the build-phase plan.

use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::net::SocketAddr;
use std::path::{Path as FsPath, PathBuf};
use std::sync::Arc;
use tokio::signal;
use tokio::sync::mpsc;
use tracing_subscriber::EnvFilter;

use app_mediakit_knowledge::routes::router;
use app_mediakit_knowledge::search;
use app_mediakit_knowledge::server::AppState;

#[derive(Parser)]
#[command(name = "app-mediakit-knowledge", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run the wiki engine HTTP server.
    Serve {
        /// Path to a knowledge.toml instance configuration file.
        /// When set, overrides all WIKI_* env vars except WIKI_ENABLE_MCP
        /// and WIKI_ADMIN_USERNAME / WIKI_ADMIN_PASSWORD_HASH.
        /// Set via env: WIKI_KNOWLEDGE_TOML=/etc/local-knowledge/documentation.toml
        #[arg(long, env = "WIKI_KNOWLEDGE_TOML")]
        knowledge_toml: Option<PathBuf>,

        /// Path to the directory holding Markdown content (legacy).
        /// Ignored when --knowledge-toml is set.
        #[arg(long, env = "WIKI_CONTENT_DIR")]
        content_dir: Option<PathBuf>,

        /// Address to bind (legacy). Ignored when --knowledge-toml is set.
        #[arg(long, env = "WIKI_BIND", default_value = "127.0.0.1:9090")]
        bind: SocketAddr,

        /// Path to the Foundry citation registry YAML file (legacy).
        #[arg(
            long,
            env = "WIKI_CITATIONS_YAML",
            default_value = "/srv/foundry/citations.yaml"
        )]
        citations_yaml: PathBuf,

        /// Path to the persistent state directory (legacy).
        #[arg(
            long,
            env = "WIKI_STATE_DIR",
            default_value = "/var/lib/local-knowledge/state"
        )]
        state_dir: PathBuf,

        /// Optional extra guide directory (legacy).
        #[arg(long, env = "WIKI_GUIDE_DIR")]
        guide_dir: Option<PathBuf>,

        /// Optional second guide directory (legacy).
        #[arg(long, env = "WIKI_GUIDE_DIR_2")]
        guide_dir_2: Option<PathBuf>,

        /// Display name shown in the browser tab and site header (legacy).
        #[arg(
            long,
            env = "WIKI_SITE_TITLE",
            default_value = "PointSav Documentation Wiki"
        )]
        site_title: String,

        /// Tenant name for the read-only git remote.
        #[arg(long, env = "WIKI_GIT_TENANT", default_value = "pointsav")]
        git_tenant: String,

        /// Enable the MCP JSON-RPC 2.0 endpoint at POST /mcp.
        #[arg(long, env = "WIKI_ENABLE_MCP")]
        enable_mcp: bool,

        /// Brand theme selector ("woodfine" for Woodfine instances).
        #[arg(long, env = "WIKI_BRAND_THEME")]
        brand_theme: Option<String>,
    },

    /// Validate content without serving: report dead wikilinks and
    /// frontmatter violations. Exits non-zero on dead links (CI gate).
    Check {
        #[arg(long, env = "WIKI_CONTENT_DIR")]
        content_dir: PathBuf,
        #[arg(long, env = "WIKI_GUIDE_DIR")]
        guide_dir: Option<PathBuf>,
        #[arg(long, env = "WIKI_GUIDE_DIR_2")]
        guide_dir_2: Option<PathBuf>,
        /// Directory of customer *.yaml blueprints.
        #[arg(long)]
        blueprints_dir: Option<PathBuf>,
        /// Treat missing-required-field findings as failures.
        #[arg(long)]
        strict: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();
    match cli.command {
        Command::Serve {
            knowledge_toml,
            content_dir,
            bind,
            citations_yaml,
            state_dir,
            guide_dir,
            guide_dir_2,
            site_title,
            git_tenant,
            enable_mcp,
            brand_theme,
        } => {
            // Resolve effective parameters: knowledge.toml takes precedence
            // over legacy env vars when present.
            let (
                eff_content_dir,
                eff_bind,
                eff_citations,
                eff_state_dir,
                eff_guide_dir,
                eff_guide_dir_2,
                eff_site_title,
                eff_brand_theme,
                eff_peers,
                eff_instance,
                eff_canonical_url,
                eff_activitypub_outbox_url,
                eff_start_here,
                eff_site_categories,
            ) = if let Some(ref toml_path) = knowledge_toml {
                let cfg = app_mediakit_knowledge::config::load_config(toml_path)?;
                let parsed_bind: SocketAddr = cfg.site.bind.parse()?;
                // Primary mount is the first [[mount]] with role = "primary".
                let primary = cfg
                    .mounts
                    .iter()
                    .find(|m| m.role == "primary")
                    .ok_or_else(|| anyhow::anyhow!("knowledge.toml: no primary mount defined"))?;
                // Guide mount is the first [[mount]] with role = "guide".
                let guide = cfg
                    .mounts
                    .iter()
                    .find(|m| m.role == "guide")
                    .map(|m| m.path.clone());
                let guide2 = cfg
                    .mounts
                    .iter()
                    .filter(|m| m.role == "guide")
                    .nth(1)
                    .map(|m| m.path.clone());
                let brand = if cfg.site.brand == "woodfine" {
                    Some("woodfine".to_string())
                } else {
                    None
                };
                tracing::info!(
                    toml = %toml_path.display(),
                    title = %cfg.site.title,
                    brand = %cfg.site.brand,
                    mounts = cfg.mounts.len(),
                    peers = cfg.peers.len(),
                    "loaded knowledge.toml"
                );
                // Resolve categories: TOML field → SITE_CATEGORIES env var → empty (handler fallback).
                let toml_cats = cfg.site.categories.clone();
                let site_categories = if !toml_cats.is_empty() {
                    toml_cats
                } else {
                    std::env::var("SITE_CATEGORIES")
                        .map(|v| v.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
                        .unwrap_or_default()
                };
                (
                    primary.path.clone(),
                    parsed_bind,
                    cfg.citations.path.clone(),
                    cfg.site.state_dir.clone(),
                    guide,
                    guide2,
                    cfg.site.title.clone(),
                    brand,
                    cfg.peers,
                    cfg.site.instance.clone(),
                    cfg.site.canonical_url.clone(),
                    cfg.federation.outbox_url.clone(),
                    cfg.start_here.clone(),
                    site_categories,
                )
            } else {
                // Legacy env-var path.
                let cd = content_dir.ok_or_else(|| {
                    anyhow::anyhow!(
                        "either --knowledge-toml or --content-dir / WIKI_CONTENT_DIR is required"
                    )
                })?;
                let legacy_site_categories = std::env::var("SITE_CATEGORIES")
                    .map(|v| v.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
                    .unwrap_or_default();
                (
                    cd,
                    bind,
                    citations_yaml,
                    state_dir,
                    guide_dir,
                    guide_dir_2,
                    site_title,
                    brand_theme,
                    vec![],
                    None,
                    None,
                    None,
                    vec![],
                    legacy_site_categories,
                )
            };

            serve(
                eff_content_dir,
                eff_guide_dir,
                eff_guide_dir_2,
                eff_bind,
                eff_citations,
                eff_state_dir,
                eff_site_title,
                git_tenant,
                enable_mcp,
                eff_brand_theme,
                eff_peers,
                eff_instance,
                eff_canonical_url,
                eff_activitypub_outbox_url,
                eff_start_here,
                eff_site_categories,
            )
            .await
        }
        Command::Check {
            content_dir,
            guide_dir,
            guide_dir_2,
            blueprints_dir,
            strict,
        } => {
            let report = app_mediakit_knowledge::check::run_check(
                &app_mediakit_knowledge::check::CheckOpts {
                    content_dir,
                    guide_dir,
                    guide_dir_2,
                    blueprints_dir,
                },
            )
            .await?;
            println!("checked {} pages", report.pages_checked);
            for d in &report.dead_links {
                println!("DEAD LINK   {} -> [[{}]]", d.page, d.target);
            }
            for m in &report.missing_fields {
                println!(
                    "MISSING     {} (type {}): {}",
                    m.page,
                    m.type_name,
                    m.missing.join(", ")
                );
            }
            println!(
                "{} dead link(s), {} page(s) with missing required fields",
                report.dead_links.len(),
                report.missing_fields.len()
            );
            let fail =
                !report.dead_links.is_empty() || (strict && !report.missing_fields.is_empty());
            if fail {
                std::process::exit(1);
            }
            Ok(())
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn serve(
    content_dir: PathBuf,
    guide_dir: Option<PathBuf>,
    guide_dir_2: Option<PathBuf>,
    bind: SocketAddr,
    citations_yaml: PathBuf,
    state_dir: PathBuf,
    site_title: String,
    git_tenant: String,
    enable_mcp: bool,
    brand_theme: Option<String>,
    peers: Vec<app_mediakit_knowledge::config::PeerConfig>,
    instance: Option<String>,
    canonical_url: Option<String>,
    activitypub_outbox_url: Option<String>,
    start_here: Vec<app_mediakit_knowledge::config::StartHereEntry>,
    site_categories: Vec<String>,
) -> Result<()> {
    if !content_dir.is_dir() {
        bail!(
            "content directory does not exist or is not a directory: {}",
            content_dir.display()
        );
    }
    if let Some(ref gd) = guide_dir {
        if !gd.is_dir() {
            bail!(
                "guide directory does not exist or is not a directory: {}",
                gd.display()
            );
        }
        tracing::info!(guide_dir = %gd.display(), "guide directory enabled");
    }
    if let Some(ref gd2) = guide_dir_2 {
        if !gd2.is_dir() {
            bail!(
                "guide directory 2 does not exist or is not a directory: {}",
                gd2.display()
            );
        }
        tracing::info!(guide_dir_2 = %gd2.display(), "guide directory 2 enabled");
    }

    // Phase 0 federation: resolve the content mount set.
    let mounts = app_mediakit_knowledge::mounts::resolve(
        &content_dir,
        guide_dir.as_deref(),
        guide_dir_2.as_deref(),
    );
    tracing::info!(
        mounts = mounts.len(),
        ids = ?mounts.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
        "content mounts resolved"
    );

    tracing::info!(
        content_dir = %content_dir.display(),
        state_dir = %state_dir.display(),
        citations_yaml = %citations_yaml.display(),
        %bind,
        "starting wiki engine"
    );

    // Build the search index on startup.
    tracing::info!("building search index");
    let search_index = search::build_index(&content_dir, &state_dir).await?;
    tracing::info!("search index ready");
    let search_arc = Arc::new(search_index);

    // Incremental search reindex: watch content_dir for .md changes.
    // Phase 7: after a successful write reindex, emit an ActivityPub Create/Article
    // activity to `activitypub_outbox_url` (best-effort, non-blocking).
    {
        let (tx, mut rx) = mpsc::channel::<notify::Result<notify::Event>>(64);
        let mut watcher: RecommendedWatcher = Watcher::new(
            move |res| {
                let _ = tx.blocking_send(res);
            },
            notify::Config::default(),
        )?;
        watcher.watch(&content_dir, RecursiveMode::Recursive)?;
        let idx = Arc::clone(&search_arc);
        let cdir = content_dir.clone();
        let ap_outbox = activitypub_outbox_url.clone();
        let ap_canonical = canonical_url.clone();
        tokio::spawn(async move {
            let _w = watcher;
            while let Some(event) = rx.recv().await {
                let Ok(ev) = event else { continue };
                let is_write = matches!(ev.kind, EventKind::Create(_) | EventKind::Modify(_));
                let is_remove = matches!(ev.kind, EventKind::Remove(_));
                if !is_write && !is_remove {
                    continue;
                }
                for path in &ev.paths {
                    if path.extension().map(|e| e == "md").unwrap_or(false) {
                        let slug = content_path_to_slug(&cdir, path);
                        if is_write {
                            if let Ok(text) = std::fs::read_to_string(path) {
                                if let Err(e) = search::reindex_topic(&idx, &slug, &text).await {
                                    tracing::warn!(slug, error = %e, "reindex failed");
                                } else if ap_outbox.is_some() {
                                    let base =
                                        ap_canonical.as_deref().unwrap_or("http://localhost:9090");
                                    let actor_id = format!("{}/activitypub/actor", base);
                                    let published =
                                        chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
                                    app_mediakit_knowledge::activitypub::on_article_saved(
                                        ap_outbox.as_deref(),
                                        &actor_id,
                                        base,
                                        &slug,
                                        &slug,
                                        "",
                                        &published,
                                    )
                                    .await;
                                }
                            }
                        }
                        if is_remove {
                            let _ = search::reindex_topic(&idx, &slug, "").await;
                        }
                    }
                }
            }
        });
    }

    // Open or init git repo.
    tracing::info!("opening git repository");
    let _ = std::process::Command::new("git")
        .args(["config", "--global", "--add", "safe.directory", "*"])
        .status();
    let git_repo = app_mediakit_knowledge::git::open_or_init(&content_dir)?;
    tracing::info!("git repository ready");

    let glossary = app_mediakit_knowledge::glossary::load_glossary(&content_dir);
    let blueprints =
        app_mediakit_knowledge::blueprints::Registry::load(&content_dir.join("blueprints"));

    // Open or create the redb link graph.
    tracing::info!("opening link graph");
    let link_graph =
        app_mediakit_knowledge::links::LinkGraph::open_or_create(&state_dir.join("links.redb"))?;
    let link_graph = Arc::new(link_graph);
    tracing::info!("link graph ready");

    tracing::info!(git_tenant = %git_tenant, "git remote enabled at /git-server/{}/info/refs", git_tenant);
    let brand_instance = instance.unwrap_or_else(|| {
        std::env::var("WIKI_BRAND_INSTANCE").unwrap_or_else(|_| "documentation".to_string())
    });
    let state = AppState {
        mounts,
        citations_yaml,
        search: search_arc,
        git: Arc::new(std::sync::Mutex::new(git_repo)),
        site_title,
        git_tenant,
        mcp_enabled: enable_mcp,
        glossary: Arc::new(glossary),
        links: link_graph,
        brand_theme,
        brand_instance,
        blueprints,
        peers,
        canonical_url,
        activitypub_outbox_url,
        start_here,
        site_categories,
    };
    let app = router(state);
    let listener = tokio::net::TcpListener::bind(bind).await?;
    tracing::info!(addr = %bind, "listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    tracing::info!("shut down cleanly");
    Ok(())
}

/// Derive a search slug from an absolute filesystem path relative to content_dir.
fn content_path_to_slug(content_dir: &FsPath, path: &FsPath) -> String {
    path.strip_prefix(content_dir)
        .ok()
        .and_then(|rel| rel.to_str())
        .map(|s| s.trim_end_matches(".md").replace('\\', "/"))
        .unwrap_or_else(|| {
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string()
        })
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
    tracing::info!("shutdown signal received");
}
