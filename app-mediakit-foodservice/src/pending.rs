//! The AI-proposal review queue — the human approval gate (SYS-ADR-10).
//!
//! AI authors never write to the content tree directly. A proposal is
//! validated and staged here; a human approves (F12) before anything
//! persists. No automated publish path (SYS-ADR-19).

use std::path::{Path, PathBuf};

use app_mediakit_shell::Page;

#[derive(Debug, Clone)]
pub struct Queue {
    dir: PathBuf,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PendingItem {
    pub id: String,
    pub slug: String,
}

impl Queue {
    pub fn open(state_dir: &Path) -> std::io::Result<Self> {
        let dir = state_dir.join("pending");
        std::fs::create_dir_all(&dir)?;
        Ok(Self { dir })
    }

    fn item_path(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.yaml"))
    }

    pub fn stage(&self, manifest_yaml: &str) -> Result<String, String> {
        let page = Page::from_yaml(manifest_yaml)?;
        let slug = page.slug.clone().ok_or("manifest is missing a slug")?;
        if slug.contains("..") || slug.contains('/') {
            return Err("invalid slug".into());
        }
        let id = uuid::Uuid::new_v4().to_string();
        let canonical = page.to_yaml()?;
        std::fs::write(self.item_path(&id), canonical).map_err(|e| e.to_string())?;
        Ok(id)
    }

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

    pub fn read(&self, id: &str) -> Result<String, String> {
        if id.contains('/') || id.contains("..") {
            return Err("invalid id".into());
        }
        std::fs::read_to_string(self.item_path(id)).map_err(|e| e.to_string())
    }

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
        "title: Home\nslug: home\nsections:\n  - type: hero\n    headline: Welcome\n";

    #[test]
    fn stage_list_approve_round_trip() {
        let tmp = tempfile::tempdir().unwrap();
        let content = tmp.path().join("content");
        std::fs::create_dir_all(&content).unwrap();
        let q = Queue::open(tmp.path()).unwrap();

        let id = q.stage(MANIFEST).unwrap();
        let items = q.list();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].slug, "home");

        let dest = q.approve(&content, &id).unwrap();
        assert!(dest.ends_with("home/page.yaml"));
        assert!(dest.is_file());
        assert!(q.list().is_empty());
    }

    #[test]
    fn stage_rejects_invalid_manifest() {
        let tmp = tempfile::tempdir().unwrap();
        let q = Queue::open(tmp.path()).unwrap();
        assert!(q.stage("title: X\nsections:\n  - type: bogus\n").is_err());
    }
}
