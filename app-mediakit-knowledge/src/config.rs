// SPDX-License-Identifier: FSL-1.1-ALv2
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

//! Runtime configuration — parsed from an instance `knowledge.toml`.
//!
//! The schema is the contract the three live instances depend on
//! (documentation / projects / corporate). It is preserved verbatim so a
//! deployed `/etc/local-knowledge/<instance>.toml` loads unchanged after the
//! engine swap. This is a fresh implementation of that schema, not the old one.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// Top-level parsed `knowledge.toml`.
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub site: Site,
    #[serde(default, rename = "mount")]
    pub mounts: Vec<Mount>,
    #[serde(default)]
    pub citations: Option<Citations>,
    #[serde(default, rename = "peer")]
    pub peers: Vec<Peer>,
    #[serde(default, rename = "start_here")]
    pub start_here: Vec<StartHere>,
    #[serde(default)]
    pub federation: Option<Federation>,
}

/// `[site]` — identity, bind address, brand, and navigation categories.
#[derive(Debug, Clone, Deserialize)]
pub struct Site {
    pub title: String,
    /// Token set selector: "pointsav" or "woodfine".
    #[serde(default = "default_brand")]
    pub brand: String,
    #[serde(default = "default_bind")]
    pub bind: String,
    #[serde(default = "default_state_dir")]
    pub state_dir: PathBuf,
    /// Instance selector — drives per-tenant chrome. One of
    /// "documentation" | "projects" | "corporate".
    #[serde(default)]
    pub instance: Option<String>,
    /// Absolute base URL for sitemap `<loc>` entries and canonical links.
    #[serde(default)]
    pub canonical_url: Option<String>,
    /// Navigation category slugs in display order. Empty → engine default.
    #[serde(default)]
    pub categories: Vec<String>,
}

/// `[[mount]]` — a content source. First primary mount is editable; guide
/// mounts are read-only.
#[derive(Debug, Clone, Deserialize)]
pub struct Mount {
    pub path: PathBuf,
    #[serde(default = "default_role")]
    pub role: String,
    #[serde(default)]
    pub blueprint_set: Vec<String>,
}

/// `[citations]` — path to the workspace citation registry YAML.
#[derive(Debug, Clone, Deserialize)]
pub struct Citations {
    pub path: PathBuf,
}

/// `[[peer]]` — sibling instance for cross-instance federated search.
#[derive(Debug, Clone, Deserialize)]
pub struct Peer {
    pub url: String,
    #[serde(default)]
    pub label: Option<String>,
}

/// `[[start_here]]` — a "start here" chip on the home page.
#[derive(Debug, Clone, Deserialize)]
pub struct StartHere {
    pub href: String,
    pub label: String,
    #[serde(default)]
    pub kind: Option<String>,
}

/// `[federation]` — ActivityPub outbox target (stub).
#[derive(Debug, Clone, Deserialize)]
pub struct Federation {
    #[serde(default)]
    pub outbox_url: Option<String>,
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
fn default_role() -> String {
    "primary".to_string()
}

impl Config {
    /// Load and parse a `knowledge.toml` from disk.
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("reading {}: {e}", path.display()))?;
        let cfg: Config = toml::from_str(&text)
            .map_err(|e| anyhow::anyhow!("parsing {}: {e}", path.display()))?;
        Ok(cfg)
    }

    /// Resolve the bind address to a `SocketAddr`.
    pub fn socket_addr(&self) -> anyhow::Result<SocketAddr> {
        self.site
            .bind
            .parse()
            .map_err(|e| anyhow::anyhow!("invalid bind address {:?}: {e}", self.site.bind))
    }

    /// The primary (editable) mount, if any.
    pub fn primary_mount(&self) -> Option<&Mount> {
        self.mounts.iter().find(|m| m.role == "primary")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_documentation_shape() {
        let toml = r#"
[site]
title = "PointSav Documentation"
brand = "pointsav"
bind = "127.0.0.1:9090"
state_dir = "/var/lib/local-knowledge/state"
instance = "documentation"
canonical_url = "https://documentation.pointsav.com"
categories = ["architecture", "substrate", "reference"]

[[mount]]
path = "/srv/foundry/content"
role = "primary"
blueprint_set = ["TOPIC", "GUIDE"]

[citations]
path = "/srv/foundry/citations.yaml"
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        assert_eq!(cfg.site.title, "PointSav Documentation");
        assert_eq!(cfg.site.instance.as_deref(), Some("documentation"));
        assert_eq!(cfg.site.categories.len(), 3);
        assert_eq!(cfg.mounts.len(), 1);
        assert_eq!(cfg.primary_mount().unwrap().role, "primary");
        assert!(cfg.socket_addr().is_ok());
    }

    #[test]
    fn defaults_apply_when_minimal() {
        let cfg: Config = toml::from_str("[site]\ntitle = \"X\"\n").unwrap();
        assert_eq!(cfg.site.brand, "pointsav");
        assert_eq!(cfg.site.bind, "127.0.0.1:9090");
        assert!(cfg.mounts.is_empty());
    }
}
