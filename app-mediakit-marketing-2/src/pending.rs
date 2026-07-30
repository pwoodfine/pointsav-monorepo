// SPDX-License-Identifier: FSL-1.1-ALv2
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

//! File-based AI-proposal review queue. Reimplements the retired engine's
//! stage→list→approve pattern fresh: `propose` NEVER writes into the content
//! tree — only `approve` does, and `approve` is only ever called by an
//! explicit human/operator action (the F12 gate, SYS-ADR-10). No code path
//! here can autonomously publish (SYS-ADR-19).

use std::path::{Path, PathBuf};

use serde::Serialize;
use uuid::Uuid;

use crate::content::{self, Page};
use crate::error::MarketingError;

#[derive(Debug, Clone)]
pub struct Queue {
    pending_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct PendingSummary {
    pub id: String,
    pub slug: String,
    pub lang: String,
}

impl Queue {
    pub fn open(state_dir: &Path) -> Result<Self, MarketingError> {
        let pending_dir = state_dir.join("pending");
        std::fs::create_dir_all(&pending_dir)?;
        Ok(Self { pending_dir })
    }

    /// Stage a proposed manifest for human review. Validates that the YAML
    /// at least parses as a `Page` before staging (fast feedback for the
    /// proposing agent) — this is NOT the approval gate, just a sanity check.
    pub fn stage(&self, slug: &str, lang: &str, manifest_yaml: &str) -> Result<String, MarketingError> {
        serde_yaml::from_str::<Page>(manifest_yaml).map_err(|source| MarketingError::Manifest {
            slug: slug.to_string(),
            source,
        })?;
        let id = Uuid::new_v4().to_string();
        let path = self.entry_path(&id, slug, lang);
        std::fs::write(&path, manifest_yaml)?;
        Ok(id)
    }

    pub fn list(&self) -> Result<Vec<PendingSummary>, MarketingError> {
        let mut entries = Vec::new();
        for entry in std::fs::read_dir(&self.pending_dir)? {
            let entry = entry?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            let Some((id, slug, lang)) = parse_entry_name(name) else {
                continue;
            };
            entries.push(PendingSummary {
                id: id.to_string(),
                slug: slug.to_string(),
                lang: lang.to_string(),
            });
        }
        entries.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(entries)
    }

    pub fn manifest(&self, id: &str) -> Result<String, MarketingError> {
        let path = self.find_entry(id)?;
        Ok(std::fs::read_to_string(path)?)
    }

    /// Approve a pending proposal: re-validate, write into the content tree,
    /// remove the pending entry. Called only from an explicit
    /// human/operator-triggered request — never from `stage`.
    pub fn approve(&self, id: &str, content_dir: &Path) -> Result<(), MarketingError> {
        let path = self.find_entry(id)?;
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .expect("entry path always has a filename");
        let (_, slug, lang) = parse_entry_name(name).expect("entry path is always well-formed");
        let manifest_yaml = std::fs::read_to_string(&path)?;
        serde_yaml::from_str::<Page>(&manifest_yaml).map_err(|source| MarketingError::Manifest {
            slug: slug.to_string(),
            source,
        })?;

        let filename = if lang == "en" {
            "page.yaml".to_string()
        } else {
            format!("page.{lang}.yaml")
        };
        let target_dir = content_dir.join(slug);
        std::fs::create_dir_all(&target_dir)?;
        std::fs::write(target_dir.join(filename), &manifest_yaml)?;
        std::fs::remove_file(&path)?;
        Ok(())
    }

    fn entry_path(&self, id: &str, slug: &str, lang: &str) -> PathBuf {
        self.pending_dir
            .join(format!("{id}__{slug}__{lang}.pending.yaml"))
    }

    fn find_entry(&self, id: &str) -> Result<PathBuf, MarketingError> {
        for entry in std::fs::read_dir(&self.pending_dir)? {
            let entry = entry?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            if let Some((entry_id, _, _)) = parse_entry_name(name) {
                if entry_id == id {
                    return Ok(entry.path());
                }
            }
        }
        Err(MarketingError::PageNotFound(format!("pending proposal {id}")))
    }
}

/// Parse `{id}__{slug}__{lang}.pending.yaml` back into its three parts.
fn parse_entry_name(name: &str) -> Option<(&str, &str, &str)> {
    let stem = name.strip_suffix(".pending.yaml")?;
    let mut parts = stem.splitn(3, "__");
    let id = parts.next()?;
    let slug = parts.next()?;
    let lang = parts.next()?;
    Some((id, slug, lang))
}

/// Load a page for read/reference purposes, exposed to the MCP layer.
pub fn read_page(content_dir: &Path, slug: &str, lang: Option<&str>) -> Result<Page, MarketingError> {
    content::load_page(content_dir, slug, lang)
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_YAML: &str = r#"
title: Home
slug: home
description: Test.
sections:
  - type: hero
    headline: Hi
"#;

    #[test]
    fn stage_then_approve_writes_into_content_tree() {
        let state_dir = tempfile::tempdir().unwrap();
        let content_dir = tempfile::tempdir().unwrap();
        let queue = Queue::open(state_dir.path()).unwrap();

        let id = queue.stage("home", "en", VALID_YAML).unwrap();
        assert_eq!(queue.list().unwrap().len(), 1);

        queue.approve(&id, content_dir.path()).unwrap();
        assert_eq!(queue.list().unwrap().len(), 0);

        let written = std::fs::read_to_string(content_dir.path().join("home/page.yaml")).unwrap();
        assert!(written.contains("headline: Hi"));
    }

    #[test]
    fn approve_uses_lang_suffix_for_non_english() {
        let state_dir = tempfile::tempdir().unwrap();
        let content_dir = tempfile::tempdir().unwrap();
        let queue = Queue::open(state_dir.path()).unwrap();

        let id = queue.stage("home", "es", VALID_YAML).unwrap();
        queue.approve(&id, content_dir.path()).unwrap();

        assert!(content_dir.path().join("home/page.es.yaml").exists());
        assert!(!content_dir.path().join("home/page.yaml").exists());
    }

    #[test]
    fn stage_rejects_invalid_manifest_without_touching_content() {
        let state_dir = tempfile::tempdir().unwrap();
        let queue = Queue::open(state_dir.path()).unwrap();
        let result = queue.stage("home", "en", "not: [valid, page");
        assert!(result.is_err());
        assert_eq!(queue.list().unwrap().len(), 0);
    }

    #[test]
    fn approve_unknown_id_errors() {
        let state_dir = tempfile::tempdir().unwrap();
        let content_dir = tempfile::tempdir().unwrap();
        let queue = Queue::open(state_dir.path()).unwrap();
        let result = queue.approve("nonexistent-id", content_dir.path());
        assert!(matches!(result, Err(MarketingError::PageNotFound(_))));
    }

    #[test]
    fn manifest_round_trips() {
        let state_dir = tempfile::tempdir().unwrap();
        let queue = Queue::open(state_dir.path()).unwrap();
        let id = queue.stage("home", "en", VALID_YAML).unwrap();
        let fetched = queue.manifest(&id).unwrap();
        assert_eq!(fetched, VALID_YAML);
    }
}
