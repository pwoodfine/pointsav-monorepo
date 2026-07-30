//! The AI-proposal review queue — the human approval gate (SYS-ADR-10).
//!
//! An AI author never writes to the content tree. It proposes a manifest,
//! which is validated against the section contract and staged here as a
//! pending item. A human reviews the proposed-vs-current manifest and approves;
//! only approval persists the manifest into the content tree. There is no
//! automated publish path (SYS-ADR-19).
//!
//! Scaffold storage: one YAML file per proposal under `<state_dir>/pending/`.
//! A true unified-diff view and a git commit-on-approve are later phases
//! (the knowledge engine's `pending.rs` + `git.rs` are the reference).

use std::path::{Path, PathBuf};

use app_mediakit_shell::Page;

/// Handle on the on-disk proposal queue.
#[derive(Debug, Clone)]
pub struct Queue {
    dir: PathBuf,
}

/// A staged proposal awaiting human approval.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PendingItem {
    pub id: String,
    pub slug: String,
}

impl Queue {
    /// Open (creating if needed) the queue under `<state_dir>/pending/`.
    pub fn open(state_dir: &Path) -> std::io::Result<Self> {
        let dir = state_dir.join("pending");
        std::fs::create_dir_all(&dir)?;
        Ok(Self { dir })
    }

    fn item_path(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.yaml"))
    }

    /// Validate and stage a proposed manifest. Returns the pending id.
    /// Rejects manifests that do not conform to the section contract.
    pub fn stage(&self, manifest_yaml: &str) -> Result<String, String> {
        let page = Page::from_yaml(manifest_yaml)?;
        let slug = page.slug.clone().ok_or("manifest is missing a slug")?;
        if slug.contains("..") || slug.contains('/') {
            return Err("invalid slug".into());
        }
        let id = uuid::Uuid::new_v4().to_string();
        // Re-serialize from the parsed Page so what we store is canonical.
        let canonical = page.to_yaml()?;
        std::fs::write(self.item_path(&id), canonical).map_err(|e| e.to_string())?;
        Ok(id)
    }

    /// List all pending proposals.
    pub fn list(&self) -> Vec<PendingItem> {
        let mut items = Vec::new();
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return items;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("yaml") {
                continue;
            }
            let id = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            let slug = std::fs::read_to_string(&path)
                .ok()
                .and_then(|t| Page::from_yaml(&t).ok())
                .and_then(|p| p.slug)
                .unwrap_or_default();
            items.push(PendingItem { id, slug });
        }
        items.sort_by(|a, b| a.id.cmp(&b.id));
        items
    }

    /// Read a proposed manifest's canonical YAML.
    pub fn read(&self, id: &str) -> Result<String, String> {
        if id.contains('/') || id.contains("..") {
            return Err("invalid id".into());
        }
        std::fs::read_to_string(self.item_path(id)).map_err(|e| e.to_string())
    }

    /// Approve a proposal (the human F12). Persists the manifest into the
    /// content tree at `<content_dir>/<slug>/page.yaml` and removes it from the
    /// queue. Returns the written path. Committing the resulting working-tree
    /// change to Git is the operator's separate signed-commit step.
    pub fn approve(&self, content_dir: &Path, id: &str) -> Result<PathBuf, String> {
        let manifest = self.read(id)?;
        let page = Page::from_yaml(&manifest)?;
        let slug = page.slug.clone().ok_or("manifest is missing a slug")?;
        if slug.contains("..") || slug.contains('/') {
            return Err("invalid slug".into());
        }
        let dest_dir = content_dir.join(&slug);
        std::fs::create_dir_all(&dest_dir).map_err(|e| e.to_string())?;
        let dest = dest_dir.join("page.yaml");
        std::fs::write(&dest, &manifest).map_err(|e| e.to_string())?;
        let _ = std::fs::remove_file(self.item_path(id));
        Ok(dest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str =
        "title: Promo\nslug: promo\nsections:\n  - type: hero\n    headline: Hi\n";

    #[test]
    fn stage_list_approve_round_trip() {
        let tmp = tempfile::tempdir().unwrap();
        let content = tmp.path().join("content");
        std::fs::create_dir_all(&content).unwrap();
        let q = Queue::open(tmp.path()).unwrap();

        let id = q.stage(MANIFEST).unwrap();
        let items = q.list();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].slug, "promo");

        let dest = q.approve(&content, &id).unwrap();
        assert!(dest.ends_with("promo/page.yaml"));
        assert!(dest.is_file());
        assert!(q.list().is_empty()); // dequeued
    }

    #[test]
    fn stage_rejects_invalid_manifest() {
        let tmp = tempfile::tempdir().unwrap();
        let q = Queue::open(tmp.path()).unwrap();
        assert!(q.stage("title: X\nsections:\n  - type: bogus\n").is_err());
    }
}
