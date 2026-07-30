// SPDX-License-Identifier: FSL-1.1-ALv2
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

//! Content walk + slug index.
//!
//! At startup the engine walks every mount, parses each Markdown file's
//! frontmatter, and builds a slug → file index. Bilingual `.es.md` siblings
//! share a slug and differ only by language. The index is the fast path for
//! `/wiki/{slug}` resolution; the on-disk Markdown remains the source of truth.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::frontmatter::{self, ParsedDoc};
use super::mount::MountSet;

/// Document language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Lang {
    En,
    Es,
}

/// A resolved reference to one content file on disk.
#[derive(Debug, Clone)]
pub struct DocRef {
    pub slug: String,
    pub path: PathBuf,
    pub lang: Lang,
    pub title: String,
    pub category: Option<String>,
    pub short_description: Option<String>,
    pub last_edited: Option<String>,
    pub aliases: Vec<String>,
    pub mount_index: usize,
}

/// In-memory index of every content file, keyed by `(slug, lang)`.
#[derive(Debug, Clone, Default)]
pub struct ContentIndex {
    by_key: HashMap<(String, Lang), DocRef>,
    by_alias: HashMap<String, String>, // alias → canonical slug (English)
}

impl ContentIndex {
    /// Walk all mounts and build the index.
    pub fn build(mounts: &MountSet) -> Self {
        let mut idx = ContentIndex::default();
        for (mi, mount) in mounts.mounts.iter().enumerate() {
            walk_dir(&mount.path, &mount.path, mi, &mut idx);
        }
        idx
    }

    /// Number of unique English documents (the article count).
    pub fn article_count(&self) -> usize {
        self.by_key.keys().filter(|(_, l)| *l == Lang::En).count()
    }

    /// Resolve a slug in a given language, falling back to English.
    pub fn resolve(&self, slug: &str, lang: Lang) -> Option<&DocRef> {
        self.by_key
            .get(&(slug.to_string(), lang))
            .or_else(|| self.by_key.get(&(slug.to_string(), Lang::En)))
    }

    /// All English documents, unordered.
    pub fn documents(&self) -> impl Iterator<Item = &DocRef> {
        self.by_key
            .iter()
            .filter(|((_, l), _)| *l == Lang::En)
            .map(|(_, d)| d)
    }

    /// Count of English documents in each category (categories with content only).
    pub fn category_counts(&self) -> std::collections::BTreeMap<String, usize> {
        let mut counts = std::collections::BTreeMap::new();
        for doc in self.documents() {
            if let Some(cat) = doc.category.as_deref() {
                if !cat.is_empty() && cat != "root" {
                    *counts.entry(cat.to_string()).or_insert(0) += 1;
                }
            }
        }
        counts
    }

    /// English documents most-recently edited first (by `last_edited`; undated
    /// docs sort last), truncated to `n`. Used by the feeds.
    pub fn recent(&self, n: usize) -> Vec<&DocRef> {
        let mut docs: Vec<&DocRef> = self.documents().collect();
        docs.sort_by(|a, b| b.last_edited.cmp(&a.last_edited));
        docs.truncate(n);
        docs
    }

    /// English documents in a category, sorted by title.
    pub fn in_category(&self, category: &str) -> Vec<&DocRef> {
        let mut docs: Vec<&DocRef> = self
            .documents()
            .filter(|d| d.category.as_deref() == Some(category))
            .collect();
        docs.sort_by_key(|a| a.title.to_lowercase());
        docs
    }

    /// Resolve an alias to its canonical slug (for `aliases:`-based redirects).
    pub fn resolve_alias(&self, alias: &str) -> Option<&str> {
        self.by_alias.get(alias).map(String::as_str)
    }

    fn insert(&mut self, doc: DocRef) {
        if doc.lang == Lang::En {
            for a in &doc.aliases {
                self.by_alias
                    .entry(a.clone())
                    .or_insert_with(|| doc.slug.clone());
            }
        }
        self.by_key.insert((doc.slug.clone(), doc.lang), doc);
    }
}

fn walk_dir(root: &Path, dir: &Path, mount_index: usize, idx: &mut ContentIndex) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path.is_dir() {
            // Skip hidden and repo-meta directories.
            if name.starts_with('.') {
                continue;
            }
            walk_dir(root, &path, mount_index, idx);
        } else if is_content_file(&name) {
            if let Some(doc) = load_ref(root, &path, mount_index) {
                idx.insert(doc);
            }
        }
    }
}

/// A content file is a `.md` that is not a repo-meta doc or a section index.
fn is_content_file(name: &str) -> bool {
    if !name.ends_with(".md") {
        return false;
    }
    // `_index.md` files are category/section landing metadata, not articles —
    // they must not count, list, or appear in the guides/browse surfaces.
    if name == "_index.md" || name == "_index.es.md" {
        return false;
    }
    // Chrome content, not an article: rendered into the Important Information band.
    if name == "important-information.md" || name == "important-information.es.md" {
        return false;
    }
    // Exclude repo governance files that are not wiki content.
    const EXCLUDE: &[&str] = &[
        "README.md",
        "README.es.md",
        "CLAUDE.md",
        "AGENT.md",
        "AGENTS.md",
        "GEMINI.md",
        "NEXT.md",
        "CHANGELOG.md",
        "TRADEMARK.md",
        "LICENSE.md",
        "CODE_OF_CONDUCT.md",
        "CONTRIBUTING.md",
    ];
    !EXCLUDE.contains(&name)
}

fn load_ref(root: &Path, path: &Path, mount_index: usize) -> Option<DocRef> {
    let text = std::fs::read_to_string(path).ok()?;
    let doc = frontmatter::parse(&text);
    let rel = path.strip_prefix(root).unwrap_or(path);
    let lang = detect_lang(path);
    let slug = doc
        .frontmatter
        .slug
        .clone()
        .unwrap_or_else(|| path_slug(rel));
    let title = doc
        .frontmatter
        .title
        .clone()
        .unwrap_or_else(|| humanize_slug(&slug));
    Some(DocRef {
        slug,
        path: path.to_path_buf(),
        lang,
        title,
        category: doc.frontmatter.category.clone(),
        short_description: doc
            .frontmatter
            .short_description
            .clone()
            .filter(|s| !s.trim().is_empty())
            .or_else(|| first_body_summary(&doc.body_md)),
        last_edited: doc.frontmatter.last_edited.clone(),
        aliases: doc.frontmatter.aliases.clone(),
        mount_index,
    })
}

fn detect_lang(path: &Path) -> Lang {
    match path.to_string_lossy().ends_with(".es.md") {
        true => Lang::Es,
        false => Lang::En,
    }
}

/// Derive a slug from a relative path: drop the extension (and `.es`), take the
/// final path component (file stem), e.g. `architecture/foo.md` → `foo`.
fn path_slug(rel: &Path) -> String {
    let s = rel.to_string_lossy();
    let s = s.strip_suffix(".es.md").or_else(|| s.strip_suffix(".md")).unwrap_or(&s);
    s.rsplit('/').next().unwrap_or(s).to_string()
}

/// A one-line summary from the first real body paragraph, lightly de-marked and
/// truncated at a word boundary. Used when a doc has no `short_description`.
fn first_body_summary(body: &str) -> Option<String> {
    let line = body.lines().find(|l| {
        let t = l.trim();
        !t.is_empty()
            && !t.starts_with('#')
            && !t.starts_with("---")
            && !t.starts_with("<!--")
            && !t.starts_with('|')
            && !t.starts_with('>')
    })?;
    // Light markdown strip: emphasis, code ticks, and wikilink brackets.
    let mut s = line.trim().replace("**", "").replace('`', "");
    s = s.replace("[[", "").replace("]]", "");
    Some(truncate_words(&s, 150))
}

/// Truncate to at most `max_chars`, cutting on a word boundary, adding an ellipsis.
fn truncate_words(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let end = s
        .char_indices()
        .nth(max_chars)
        .map(|(i, _)| i)
        .unwrap_or(s.len());
    let cut = &s[..end];
    let cut = cut.rfind(' ').map(|i| &cut[..i]).unwrap_or(cut);
    format!("{}\u{2026}", cut.trim_end())
}

/// Turn a slug into a human title: "merkle-proofs" → "Merkle proofs".
/// Only the first word is capitalised (sentence case), like an article title.
pub fn humanize_slug(slug: &str) -> String {
    let last = slug.rsplit('/').next().unwrap_or(slug);
    let spaced = last.replace(['-', '_'], " ");
    let mut chars = spaced.chars();
    match chars.next() {
        Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
        None => spaced,
    }
}

/// Load and parse a document from disk.
pub fn load(doc: &DocRef) -> std::io::Result<ParsedDoc> {
    let text = std::fs::read_to_string(&doc.path)?;
    Ok(frontmatter::parse(&text))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write(dir: &Path, rel: &str, body: &str) {
        let p = dir.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        let mut f = std::fs::File::create(p).unwrap();
        f.write_all(body.as_bytes()).unwrap();
    }

    #[test]
    fn builds_index_and_resolves_by_slug() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        write(root, "architecture/zci.md", "---\ntitle: ZCI\nslug: zero-container-inference\ncategory: architecture\n---\nBody\n");
        write(root, "architecture/zci.es.md", "---\ntitle: ZCI (es)\nslug: zero-container-inference\ncategory: architecture\n---\nCuerpo\n");
        write(root, "README.md", "not content");

        let mounts = MountSet {
            mounts: vec![super::super::mount::Mount {
                path: root.to_path_buf(),
                role: "primary".into(),
                blueprint_set: vec![],
            }],
        };
        let idx = ContentIndex::build(&mounts);

        assert_eq!(idx.article_count(), 1); // English only; README excluded
        let en = idx.resolve("zero-container-inference", Lang::En).unwrap();
        assert_eq!(en.title, "ZCI");
        let es = idx.resolve("zero-container-inference", Lang::Es).unwrap();
        assert_eq!(es.lang, Lang::Es);
        assert!(idx.resolve("does-not-exist", Lang::En).is_none());
    }

    #[test]
    fn path_slug_falls_back_when_no_frontmatter_slug() {
        assert_eq!(path_slug(Path::new("architecture/foo.md")), "foo");
        assert_eq!(path_slug(Path::new("bar.es.md")), "bar");
    }

    #[test]
    fn resolves_aliases_to_canonical_slug() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        write(
            root,
            "reference/guide-catalog.md",
            "---\ntitle: Catalog\nslug: guide-catalog\naliases:\n  - developer-guide-index\n  - old-catalog\n---\nBody\n",
        );
        let mounts = MountSet {
            mounts: vec![super::super::mount::Mount {
                path: root.to_path_buf(),
                role: "primary".into(),
                blueprint_set: vec![],
            }],
        };
        let idx = ContentIndex::build(&mounts);
        assert_eq!(idx.resolve_alias("developer-guide-index"), Some("guide-catalog"));
        assert_eq!(idx.resolve_alias("old-catalog"), Some("guide-catalog"));
        assert_eq!(idx.resolve_alias("nope"), None);
    }

    #[test]
    fn excludes_index_and_chrome_files() {
        assert!(!is_content_file("_index.md"));
        assert!(!is_content_file("important-information.md"));
        assert!(!is_content_file("README.md"));
        assert!(is_content_file("architecture/topic-foo.md"));
    }
}
