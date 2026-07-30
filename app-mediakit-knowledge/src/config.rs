//! Configuration — `knowledge.toml` parser and legacy env-var fallback.
//!
//! Preferred path: set `WIKI_KNOWLEDGE_TOML=/etc/local-knowledge/<instance>.toml`
//! and supply a `knowledge.toml` per the §15 schema. Legacy path: the old
//! `WIKI_CONTENT_DIR` / `WIKI_BIND` / etc. env vars are synthesized into the
//! same `AppConfig` shape so existing systemd units continue to work unchanged.
//!
//! # knowledge.toml schema (§15)
//! ```toml
//! [site]
//! title     = "PointSav Documentation"
//! brand     = "pointsav"
//! bind      = "127.0.0.1:9090"
//! state_dir = "/var/lib/local-knowledge/state"
//!
//! [[mount]]
//! path          = "/srv/foundry/.../media-knowledge-documentation"
//! role          = "primary"
//! blueprint_set = ["TOPIC", "GUIDE"]
//!
//! [citations]
//! path = "/srv/foundry/citations.yaml"
//! ```

use anyhow::{Context, Result};
use serde::Deserialize;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// knowledge.toml structs
// ---------------------------------------------------------------------------

/// Top-level configuration loaded from `knowledge.toml`.
#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub site: SiteConfig,
    #[serde(default, rename = "mount")]
    pub mounts: Vec<MountConfig>,
    #[serde(default)]
    pub citations: CitationsConfig,
    /// Phase 7: peer instances for cross-instance federated search.
    /// Each `[[peer]]` entry names one sibling wiki to fan out to.
    #[serde(default, rename = "peer")]
    pub peers: Vec<PeerConfig>,
    /// Phase 7: ActivityPub federation outbox configuration.
    #[serde(default)]
    pub federation: FederationConfig,
    /// Per-instance "New here? Start with these" chips.
    /// When empty, the engine falls back to four hardcoded PointSav documentation chips.
    #[serde(default, rename = "start_here")]
    pub start_here: Vec<StartHereEntry>,
}

/// `[federation]` block — ActivityPub outbox configuration.
///
/// knowledge.toml example:
/// ```toml
/// [federation]
/// outbox_url = "https://relay.example.com/inbox"
/// ```
#[derive(Debug, Clone, Deserialize, Default)]
pub struct FederationConfig {
    /// ActivityPub outbox URL to POST `Create/Article` activities to.
    /// When absent, ActivityPub emission is disabled (best-effort, no-op).
    #[serde(default)]
    pub outbox_url: Option<String>,
}

/// One `[[start_here]]` entry — a chip shown in the "New here? Start with these" strip.
///
/// knowledge.toml example:
/// ```toml
/// [[start_here]]
/// href  = "/wiki/topic-co-location-methodology"
/// label = "Co-location methodology"
/// kind  = "topic"
/// ```
/// When no `[[start_here]]` entries are present, the engine renders the four
/// hardcoded PointSav documentation chips (backward-compatible default).
#[derive(Debug, Clone, Deserialize)]
pub struct StartHereEntry {
    /// Target URL for the chip link (absolute path, e.g. `/wiki/topic-foo`).
    pub href: String,
    /// Display label shown inside the chip.
    pub label: String,
    /// Badge type: `"topic"` (default) or `"guide"`.
    #[serde(default = "default_start_here_kind")]
    pub kind: String,
}

fn default_start_here_kind() -> String {
    "topic".to_string()
}

/// One `[[peer]]` entry — a sibling wiki instance for federated search.
///
/// knowledge.toml example:
/// ```toml
/// [[peer]]
/// url   = "http://127.0.0.1:9093"
/// label = "Woodfine Projects"
/// ```
#[derive(Debug, Clone, Deserialize)]
pub struct PeerConfig {
    /// Base URL of the peer wiki instance (no trailing slash).
    pub url: String,
    /// Human-readable label shown in merged search results.
    pub label: String,
}

/// `[site]` block.
#[derive(Debug, Clone, Deserialize)]
pub struct SiteConfig {
    /// Display name shown in the browser tab, site header, and home-page H1.
    #[serde(default = "default_site_title")]
    pub title: String,

    /// Brand selector: `"pointsav"` (default) or `"woodfine"`.
    /// Controls which token CSS file is loaded and which footer variant
    /// is rendered.
    #[serde(default = "default_brand")]
    pub brand: String,

    /// Socket address to bind. Default: `"127.0.0.1:9090"`.
    #[serde(default = "default_bind")]
    pub bind: String,

    /// Persistent state directory (search index, redb link graph, SQLite DB).
    /// Default: `"/var/lib/local-knowledge/state"`.
    #[serde(default = "default_state_dir")]
    pub state_dir: PathBuf,

    /// Canonical base URL for this instance (no trailing slash).
    /// Used to emit absolute `<loc>` values in `/sitemap.xml`.
    /// Example: `"https://documentation.pointsav.com"`.
    #[serde(default)]
    pub canonical_url: Option<String>,

    /// Instance selector written into `html[data-instance]`.
    /// Overrides the `WIKI_BRAND_INSTANCE` env var when set.
    /// Allowed values: `"documentation"`, `"projects"`, `"corporate"`.
    #[serde(default)]
    pub instance: Option<String>,
}

fn default_site_title() -> String {
    "PointSav Documentation".to_string()
}
fn default_brand() -> String {
    "pointsav".to_string()
}
fn default_bind() -> String {
    "127.0.0.1:9090".to_string()
}
fn default_state_dir() -> PathBuf {
    PathBuf::from("/var/lib/local-knowledge/state")
}

/// One `[[mount]]` entry.
#[derive(Debug, Clone, Deserialize)]
pub struct MountConfig {
    /// Filesystem path to the content repository root.
    pub path: PathBuf,

    /// Mount role: `"primary"` (editable, content articles) or `"guide"`
    /// (read-only, operational guides).
    #[serde(default = "default_role")]
    pub role: String,

    /// Blueprint types served from this mount.
    /// Allowed values: `"TOPIC"`, `"GUIDE"`.
    /// Documentation instance: `["TOPIC", "GUIDE"]`.
    /// Projects + corporate instances: `["TOPIC"]`.
    #[serde(default)]
    pub blueprint_set: Vec<String>,
}

fn default_role() -> String {
    "primary".to_string()
}

/// `[citations]` block.
#[derive(Debug, Clone, Deserialize)]
pub struct CitationsConfig {
    /// Absolute path to the Foundry citation registry YAML file.
    /// Default: `"/srv/foundry/citations.yaml"`.
    #[serde(default = "default_citations_path")]
    pub path: PathBuf,
}

impl Default for CitationsConfig {
    fn default() -> Self {
        CitationsConfig {
            path: default_citations_path(),
        }
    }
}

fn default_citations_path() -> PathBuf {
    PathBuf::from("/srv/foundry/citations.yaml")
}

// ---------------------------------------------------------------------------
// Loader
// ---------------------------------------------------------------------------

/// Load and parse a `knowledge.toml` file.
pub fn load_config(path: &Path) -> Result<AppConfig> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("cannot read knowledge.toml: {}", path.display()))?;
    let cfg: AppConfig = toml::from_str(&text)
        .with_context(|| format!("cannot parse knowledge.toml: {}", path.display()))?;
    Ok(cfg)
}

/// Parse `[site].bind` into a `SocketAddr`.
pub fn parse_bind(cfg: &AppConfig) -> Result<SocketAddr> {
    cfg.site
        .bind
        .parse::<SocketAddr>()
        .with_context(|| format!("invalid bind address: {}", cfg.site.bind))
}

// ---------------------------------------------------------------------------
// Legacy env-var Config (kept for backward compatibility)
// ---------------------------------------------------------------------------

/// Legacy configuration loaded from individual environment variables.
///
/// Used by the old `WIKI_CONTENT_DIR` / `WIKI_BIND` / etc. path. Kept
/// alongside `AppConfig` so existing instances continue to work while
/// operators migrate to `knowledge.toml`.
#[derive(Debug, Clone)]
pub struct LegacyConfig {
    pub content_path: PathBuf,
    pub git_remote: String,
    pub sync_interval_secs: u64,
    pub cache_size: usize,
    pub editor_auth_url: Option<String>,
    pub editor_enabled: bool,
    pub bind_addr: SocketAddr,
    pub site_title: String,
    pub base_url: String,
}

impl LegacyConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            content_path: std::env::var("CONTENT_PATH")
                .context("CONTENT_PATH is required")?
                .into(),
            git_remote: std::env::var("GIT_REMOTE").unwrap_or_else(|_| "origin".into()),
            sync_interval_secs: std::env::var("SYNC_INTERVAL")
                .unwrap_or_else(|_| "60".into())
                .parse()
                .context("SYNC_INTERVAL must be a positive integer")?,
            cache_size: std::env::var("CACHE_SIZE")
                .unwrap_or_else(|_| "256".into())
                .parse()
                .context("CACHE_SIZE must be a positive integer")?,
            editor_auth_url: std::env::var("EDITOR_AUTH").ok(),
            editor_enabled: std::env::var("EDITOR_ENABLED")
                .unwrap_or_else(|_| "false".into())
                .parse()
                .context("EDITOR_ENABLED must be 'true' or 'false'")?,
            bind_addr: std::env::var("BIND_ADDR")
                .unwrap_or_else(|_| "0.0.0.0:3000".into())
                .parse()
                .context("BIND_ADDR must be a valid socket address")?,
            site_title: std::env::var("SITE_TITLE")
                .unwrap_or_else(|_| "PointSav Documentation".into()),
            base_url: std::env::var("BASE_URL").unwrap_or_else(|_| "http://localhost:3000".into()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_knowledge_toml() {
        let toml = r#"
[site]
title = "PointSav Documentation"
brand = "pointsav"
bind  = "127.0.0.1:9090"
state_dir = "/var/lib/local-knowledge/state"

[[mount]]
path          = "/srv/media-knowledge-documentation"
role          = "primary"
blueprint_set = ["TOPIC", "GUIDE"]

[citations]
path = "/srv/foundry/citations.yaml"
"#;
        let tmp = std::env::temp_dir().join(format!("config-test-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        let p = tmp.join("knowledge.toml");
        std::fs::write(&p, toml).unwrap();
        let cfg = load_config(&p).unwrap();
        assert_eq!(cfg.site.title, "PointSav Documentation");
        assert_eq!(cfg.site.brand, "pointsav");
        assert_eq!(cfg.mounts.len(), 1);
        assert_eq!(cfg.mounts[0].role, "primary");
        assert!(cfg.mounts[0].blueprint_set.contains(&"TOPIC".to_string()));
        assert_eq!(
            cfg.citations.path,
            PathBuf::from("/srv/foundry/citations.yaml")
        );
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn defaults_when_optional_fields_absent() {
        let toml = r#"
[site]
title = "Minimal"

[[mount]]
path = "/srv/content"
"#;
        let tmp = std::env::temp_dir().join(format!("config-min-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        let p = tmp.join("knowledge.toml");
        std::fs::write(&p, toml).unwrap();
        let cfg = load_config(&p).unwrap();
        assert_eq!(cfg.site.brand, "pointsav");
        assert_eq!(cfg.site.bind, "127.0.0.1:9090");
        assert_eq!(cfg.mounts[0].role, "primary");
        std::fs::remove_dir_all(&tmp).ok();
    }
}
