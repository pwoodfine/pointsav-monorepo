//! Inline article annotations — YAML sidecar storage.
//!
//! Each article's notes live in `{content_dir}/annotations/{slug}.yaml`.
//! The format is a flat list of thread-heads; each may carry replies.
//! Status vocabulary: "open" | "resolved".
//! SYS-ADR-10 gate: write path requires an explicit operator confirm field.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AnnotationReply {
    pub id: String,
    pub author: String,
    pub body: String,
    pub created: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Annotation {
    pub id: String,
    /// Heading anchor id the note is attached to (empty = article-level).
    pub anchor: String,
    pub author: String,
    pub body: String,
    pub created: String,
    /// "open" or "resolved"
    pub status: String,
    #[serde(default)]
    pub replies: Vec<AnnotationReply>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AnnotationFile {
    #[serde(default)]
    pub annotations: Vec<Annotation>,
}

pub fn annotations_file_path(content_dir: &Path, slug: &str) -> PathBuf {
    content_dir
        .join("annotations")
        .join(format!("{slug}.yaml"))
}

pub fn load_annotations(content_dir: &Path, slug: &str) -> AnnotationFile {
    let path = annotations_file_path(content_dir, slug);
    if !path.is_file() {
        return AnnotationFile::default();
    }
    let raw = std::fs::read_to_string(&path).unwrap_or_default();
    serde_yaml::from_str(&raw).unwrap_or_default()
}

pub fn save_annotations(
    content_dir: &Path,
    slug: &str,
    file: &AnnotationFile,
) -> std::io::Result<()> {
    let path = annotations_file_path(content_dir, slug);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let yaml = serde_yaml::to_string(file)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    std::fs::write(&path, yaml)
}

pub fn count_open_annotations(content_dir: &Path, slug: &str) -> usize {
    load_annotations(content_dir, slug)
        .annotations
        .iter()
        .filter(|a| a.status == "open")
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn roundtrip_empty() {
        let dir = tempdir().unwrap();
        let file = AnnotationFile::default();
        save_annotations(dir.path(), "test-slug", &file).unwrap();
        let loaded = load_annotations(dir.path(), "test-slug");
        assert!(loaded.annotations.is_empty());
    }

    #[test]
    fn roundtrip_annotation() {
        let dir = tempdir().unwrap();
        let mut file = AnnotationFile::default();
        file.annotations.push(Annotation {
            id: "a1".to_string(),
            anchor: "intro".to_string(),
            author: "jennifer".to_string(),
            body: "Good intro.".to_string(),
            created: "2026-06-29T10:00:00Z".to_string(),
            status: "open".to_string(),
            replies: vec![],
        });
        save_annotations(dir.path(), "slug", &file).unwrap();
        let loaded = load_annotations(dir.path(), "slug");
        assert_eq!(loaded.annotations.len(), 1);
        assert_eq!(loaded.annotations[0].id, "a1");
        assert_eq!(loaded.annotations[0].anchor, "intro");
    }

    #[test]
    fn count_open_filters_resolved() {
        let dir = tempdir().unwrap();
        let mut file = AnnotationFile::default();
        file.annotations.push(Annotation {
            id: "a1".into(),
            anchor: "".into(),
            author: "j".into(),
            body: "open note".into(),
            created: "2026-06-29T10:00:00Z".into(),
            status: "open".into(),
            replies: vec![],
        });
        file.annotations.push(Annotation {
            id: "a2".into(),
            anchor: "".into(),
            author: "j".into(),
            body: "resolved note".into(),
            created: "2026-06-29T10:00:00Z".into(),
            status: "resolved".into(),
            replies: vec![],
        });
        save_annotations(dir.path(), "s", &file).unwrap();
        assert_eq!(count_open_annotations(dir.path(), "s"), 1);
    }

    #[test]
    fn missing_file_returns_empty() {
        let dir = tempdir().unwrap();
        let f = load_annotations(dir.path(), "nonexistent");
        assert!(f.annotations.is_empty());
    }
}
