//! Phase 0 federation — declarative content mounts.
//!
//! Generalizes the engine's two hardcoded content sources (one `WIKI_CONTENT_DIR`
//! plus up to two `WIKI_GUIDE_DIR`s) into an N-entry, per-instance mount manifest.
//! A customer or community member can declare any set of git repos to federate by
//! dropping a `knowledge.toml` in the content root; PointSav's own instances keep
//! using the env vars, which `synthesize_from_env` turns into the same `Mount` shape.
//!
//! This module is the *infrastructure* layer (types + loader + resolution helpers,
//! fully unit-tested). Deep integration — replacing `content_dir` throughout the
//! engine with the mount set and surfacing per-article provenance — is a follow-on
//! that builds on these types. See `BRIEF-knowledge-platform-master.md` §11.
//!
//! Manifest format (`knowledge.toml`):
//! ```toml
//! [[mount]]
//! id = "documentation"
//! path = "/srv/foundry/.../media-knowledge-documentation"
//! default_type = "topic"
//! editable = true
//!
//! [[mount]]
//! id = "fleet-pointsav"
//! path = "/srv/foundry/.../pointsav-fleet-deployment"
//! default_type = "guide"
//! section = "Operational guides"
//! editable = false
//! ```

use serde::Deserialize;
use std::path::{Path, PathBuf};

/// One federated content source.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Mount {
    /// Stable identifier, surfaced as article provenance ("Source: <id>").
    pub id: String,
    /// Filesystem path to the mount's root (a git working tree).
    pub path: PathBuf,
    /// Blueprint type new pages in this mount default to (e.g. `topic`, `guide`).
    #[serde(default = "default_type_topic")]
    pub default_type: String,
    /// Optional nav-section label (e.g. "Operational guides" for a guide mount).
    #[serde(default)]
    pub section: Option<String>,
    /// When false, the wiki edit action routes to "propose change in origin repo"
    /// rather than committing in place.
    #[serde(default)]
    pub editable: bool,
}

fn default_type_topic() -> String {
    "topic".to_string()
}

#[derive(Debug, Deserialize)]
struct Manifest {
    #[serde(default)]
    mount: Vec<Mount>,
}

/// Parse a `knowledge.toml` manifest into a list of mounts.
pub fn load_manifest(path: &Path) -> anyhow::Result<Vec<Mount>> {
    let text = std::fs::read_to_string(path)?;
    let manifest: Manifest = toml::from_str(&text)?;
    Ok(manifest.mount)
}

/// Build the equivalent mount set from the legacy env configuration: the content
/// dir is the primary editable `topic` mount; each guide dir is a read-only `guide`
/// mount. This keeps PointSav's existing instances working unchanged while exposing
/// the same `Mount` shape the manifest produces.
pub fn synthesize_from_env(
    content_dir: &Path,
    guide_dir: Option<&Path>,
    guide_dir_2: Option<&Path>,
) -> Vec<Mount> {
    let mut mounts = vec![Mount {
        id: "content".to_string(),
        path: content_dir.to_path_buf(),
        default_type: "topic".to_string(),
        section: None,
        editable: true,
    }];
    for (i, gd) in [guide_dir, guide_dir_2].into_iter().flatten().enumerate() {
        mounts.push(Mount {
            id: format!("guide-{}", i + 1),
            path: gd.to_path_buf(),
            default_type: "guide".to_string(),
            section: Some("Operational guides".to_string()),
            editable: false,
        });
    }
    mounts
}

/// Resolve the effective mount set for an instance: prefer `<content_dir>/knowledge.toml`
/// if present and parseable, else synthesize from the env configuration. A malformed
/// manifest falls back to env (logged by the caller) rather than failing startup.
pub fn resolve(
    content_dir: &Path,
    guide_dir: Option<&Path>,
    guide_dir_2: Option<&Path>,
) -> Vec<Mount> {
    let manifest_path = content_dir.join("knowledge.toml");
    if manifest_path.exists() {
        if let Ok(m) = load_manifest(&manifest_path) {
            if !m.is_empty() {
                return m;
            }
        }
    }
    synthesize_from_env(content_dir, guide_dir, guide_dir_2)
}

/// The non-primary (guide / read-only) mount paths — the extra roots a wikilink
/// resolver must also check so TOPIC↔GUIDE links resolve across the federation.
/// The primary mount (index 0) is the content root and is excluded.
pub fn link_roots(mounts: &[Mount]) -> Vec<&Path> {
    mounts.iter().skip(1).map(|m| m.path.as_path()).collect()
}

/// Map a mount set back onto the engine's current two-guide-dir AppState shape:
/// the first two non-primary mounts become `(guide_dir, guide_dir_2)`. (>2 guide
/// mounts require the full AppState refactor; see §11.) Used to let a `knowledge.toml`
/// drive the existing fields without changing AppState.
pub fn guide_dirs_from(mounts: &[Mount]) -> (Option<PathBuf>, Option<PathBuf>) {
    let guides: Vec<PathBuf> = mounts.iter().skip(1).map(|m| m.path.clone()).collect();
    (guides.first().cloned(), guides.get(1).cloned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_manifest_toml() {
        let toml = r#"
[[mount]]
id = "documentation"
path = "/srv/docs"
default_type = "topic"
editable = true

[[mount]]
id = "fleet"
path = "/srv/fleet"
default_type = "guide"
section = "Operational guides"
editable = false
"#;
        let dir = std::env::temp_dir().join(format!("mounts-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("knowledge.toml");
        std::fs::write(&p, toml).unwrap();
        let mounts = load_manifest(&p).unwrap();
        assert_eq!(mounts.len(), 2);
        assert_eq!(mounts[0].id, "documentation");
        assert!(mounts[0].editable);
        assert_eq!(mounts[1].default_type, "guide");
        assert_eq!(mounts[1].section.as_deref(), Some("Operational guides"));
        assert!(!mounts[1].editable);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn synthesizes_from_env_with_guides() {
        let m = synthesize_from_env(
            Path::new("/srv/content"),
            Some(Path::new("/srv/guide-a")),
            Some(Path::new("/srv/guide-b")),
        );
        assert_eq!(m.len(), 3);
        assert_eq!(m[0].path, PathBuf::from("/srv/content"));
        assert!(m[0].editable && m[0].default_type == "topic");
        assert_eq!(m[1].default_type, "guide");
        assert!(!m[1].editable);
        // link_roots excludes the primary content mount.
        let roots = link_roots(&m);
        assert_eq!(
            roots,
            vec![Path::new("/srv/guide-a"), Path::new("/srv/guide-b")]
        );
    }

    #[test]
    fn synthesizes_with_no_guides() {
        let m = synthesize_from_env(Path::new("/srv/content"), None, None);
        assert_eq!(m.len(), 1);
        assert!(link_roots(&m).is_empty());
    }

    #[test]
    fn guide_dirs_from_takes_first_two_nonprimary() {
        let m = synthesize_from_env(
            Path::new("/c"),
            Some(Path::new("/g1")),
            Some(Path::new("/g2")),
        );
        let (g1, g2) = guide_dirs_from(&m);
        assert_eq!(g1, Some(PathBuf::from("/g1")));
        assert_eq!(g2, Some(PathBuf::from("/g2")));
    }

    #[test]
    fn resolve_falls_back_to_env_when_no_manifest() {
        // A content dir with no knowledge.toml → synthesized env mounts.
        let dir = std::env::temp_dir().join(format!("mounts-noman-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let m = resolve(&dir, None, None);
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].id, "content");
        std::fs::remove_dir_all(&dir).ok();
    }
}
