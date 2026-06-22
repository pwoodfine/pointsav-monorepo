use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Persisted session snapshot. Expanded (rebuild Phase I-4 groundwork) to carry
/// the Content cartridge's last query, selection, and scroll so a reopened
/// console restores where the operator left off. All fields default, so older
/// snapshots still load.
#[derive(Serialize, Deserialize, Default)]
pub struct SessionState {
    #[serde(default)]
    pub content_query: Option<String>,
    #[serde(default)]
    pub content_selected: Option<usize>,
    #[serde(default)]
    pub content_scroll: Option<u16>,
}

impl SessionState {
    /// Canonical on-disk location for the session snapshot.
    pub fn default_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        PathBuf::from(home).join(".local/share/os-console/session.toml")
    }

    pub fn load() -> Self {
        let path = Self::default_path();
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(s) = toml::to_string(self) {
            let _ = std::fs::write(path, s);
        }
    }
}
