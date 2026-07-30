//! Content walker — frontmatter parsing and bilingual pair detection.
//!
//! Parses `foundry-doc-v1` frontmatter from Markdown files and walks
//! content directories, collecting `(path, frontmatter, body)` triples.
//! This is the metadata layer consumed by `mounts.rs` and `check.rs`.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Full `foundry-doc-v1` frontmatter schema.
///
/// Fields match the content-contract at `naming-convention.md` and the
/// schema cross-referenced by `BRIEF-knowledge-platform-master.md` §17.9.
/// Optional fields are `None` when absent from the source file.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Frontmatter {
    /// Page title (required by all blueprints).
    pub title: Option<String>,

    /// Canonical slug — must match the filename stem.
    pub slug: Option<String>,

    /// Category / nav section. Maps to the nine category tiles on
    /// documentation.pointsav.com.
    pub category: Option<String>,

    /// Publication status. Values: stub | pre-build | active | complete.
    /// Rendered as a badge in article chrome (§17.3).
    pub status: Option<String>,

    /// ISO 8601 date of last editorial edit.
    pub last_edited: Option<String>,

    /// One-sentence description, ≤160 chars. Required for
    /// projects.woodfinegroup.com article cards (§17.9).
    pub summary: Option<String>,

    /// When true, article is surfaced in featured-article rotation.
    #[serde(default)]
    pub featured: bool,

    /// When true, article renders as a map-of-content hub.
    /// Hub articles expand `[[wikilinks]]` to full article cards (§17.5).
    #[serde(default)]
    pub hub: bool,

    /// Sort position within category for prev/next navigation.
    /// Articles without `position` sort alphabetically (§17.9).
    pub position: Option<i32>,

    /// Bilingual sibling slug. EN articles set this to the `.es` sibling;
    /// ES siblings set it to the EN parent. Auto-detected at walk time
    /// when absent.
    pub paired_with: Option<String>,

    /// Slugs of related articles rendered as a "See Also" sidebar card.
    #[serde(default)]
    pub relates_to: Vec<String>,

    /// Named reading sequence membership (§17.9). Deferred to a later phase.
    pub sequence: Option<SequenceMembership>,

    /// When true, article contains BCSC forward-looking statements.
    #[serde(default)]
    pub forward_looking: bool,

    /// BCSC disclosure class. Values: public-disclosure-safe |
    /// no-disclosure-implication | pending-review.
    pub bcsc_class: Option<String>,

    /// Citation IDs from `~/Foundry/citations.yaml` referenced by this article.
    #[serde(default)]
    pub cites: Vec<String>,

    /// Schema version. When present, must be `foundry-doc-v1`.
    pub schema: Option<String>,

    /// Content-type tag (used by blueprints dispatcher). Values: topic | guide
    /// | comms | journal.
    #[serde(rename = "type", alias = "content_type")]
    pub content_type: Option<String>,

    /// Short description / lede — rendered below the article H1 in article
    /// chrome. May duplicate `summary`; the renderer prefers this field.
    pub short_description: Option<String>,

    /// Hatnote text rendered at the top of the article body (italic, indented).
    pub hatnote: Option<String>,

    /// Target audience chips shown below the article H1 and status badge.
    /// E.g. ["operator", "developer", "public"]. Rendered as inline chips.
    #[serde(default)]
    pub audience: Vec<String>,

    /// Alternative slugs that 301-redirect to this article's canonical slug.
    /// Allows renaming articles without breaking existing links.
    #[serde(default)]
    pub aliases: Vec<String>,
}

/// Named sequence membership (deferred to a later phase; stub).
#[derive(Debug, Clone, Deserialize)]
pub struct SequenceMembership {
    pub name: String,
    pub position: i32,
}

/// Parsed result of a single Markdown file.
pub struct ParsedFile {
    pub path: PathBuf,
    pub frontmatter: Frontmatter,
    pub body: String,
}

/// Split a Markdown file's raw text into a YAML frontmatter block and body.
///
/// The frontmatter delimiter is `---` on its own line. Returns
/// `(Some(yaml_str), body_str)` when frontmatter is present, or
/// `(None, full_text)` when absent.
pub fn parse_frontmatter(content: &str) -> Result<(Frontmatter, String)> {
    if let Some(rest) = content.strip_prefix("---\n") {
        if let Some(end) = rest.find("\n---\n") {
            let yaml = &rest[..end];
            let body = rest[end + "\n---\n".len()..].to_string();
            let fm: Frontmatter = serde_yaml::from_str(yaml)
                .with_context(|| format!("failed to parse frontmatter YAML: {yaml:.120}"))?;
            return Ok((fm, body));
        }
    }
    Ok((Frontmatter::default(), content.to_string()))
}

/// Walk a content directory recursively, returning all Markdown files with
/// their parsed frontmatter and body. Files matching `*.es.md` are included
/// (they carry their own frontmatter with ES field values).
pub fn walk_content_dir(path: &Path) -> Result<Vec<ParsedFile>> {
    let mut results = Vec::new();
    walk_recursive(path, &mut results)?;
    Ok(results)
}

fn walk_recursive(dir: &Path, out: &mut Vec<ParsedFile>) -> Result<()> {
    for entry in std::fs::read_dir(dir)
        .with_context(|| format!("cannot read directory: {}", dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            walk_recursive(&path, out)?;
        } else if path.extension().map(|e| e == "md").unwrap_or(false) {
            let content = std::fs::read_to_string(&path)
                .with_context(|| format!("cannot read file: {}", path.display()))?;
            let (frontmatter, body) = parse_frontmatter(&content).unwrap_or_default();
            out.push(ParsedFile {
                path,
                frontmatter,
                body,
            });
        }
    }
    Ok(())
}

/// Given an `.md` file path, return the path of its bilingual sibling if
/// one exists on disk.
///
/// For an EN file `slug.md`, looks for `slug.es.md`.
/// For an ES file `slug.es.md`, looks for `slug.md`.
pub fn find_bilingual_pair(path: &Path) -> Option<PathBuf> {
    let name = path.file_name()?.to_str()?;
    let sibling_name = if name.ends_with(".es.md") {
        // ES → look for EN
        name.strip_suffix(".es.md").map(|stem| format!("{stem}.md"))
    } else if name.ends_with(".md") {
        // EN → look for ES
        name.strip_suffix(".md").map(|stem| format!("{stem}.es.md"))
    } else {
        return None;
    };
    let sibling_path = path.with_file_name(sibling_name?);
    if sibling_path.exists() {
        Some(sibling_path)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_frontmatter() {
        let md = "---\ntitle: Test Article\nslug: test-article\ncategory: architecture\nstatus: active\nfeatured: true\nhub: false\nrelates_to:\n  - other-article\n---\nBody text here.\n";
        let (fm, body) = parse_frontmatter(md).unwrap();
        assert_eq!(fm.title.as_deref(), Some("Test Article"));
        assert_eq!(fm.slug.as_deref(), Some("test-article"));
        assert_eq!(fm.category.as_deref(), Some("architecture"));
        assert_eq!(fm.status.as_deref(), Some("active"));
        assert!(fm.featured);
        assert!(!fm.hub);
        assert_eq!(fm.relates_to, vec!["other-article"]);
        assert_eq!(body.trim(), "Body text here.");
    }

    #[test]
    fn handles_missing_frontmatter() {
        let md = "No frontmatter here.\n";
        let (fm, body) = parse_frontmatter(md).unwrap();
        assert!(fm.title.is_none());
        assert_eq!(body.trim(), "No frontmatter here.");
    }

    #[test]
    fn bilingual_pair_en_to_es() {
        let tmp = std::env::temp_dir().join(format!("walker-test-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        let en = tmp.join("article.md");
        let es = tmp.join("article.es.md");
        std::fs::write(&en, "# EN").unwrap();
        std::fs::write(&es, "# ES").unwrap();
        assert_eq!(find_bilingual_pair(&en), Some(es.clone()));
        assert_eq!(find_bilingual_pair(&es), Some(en));
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn bilingual_pair_returns_none_when_missing() {
        let tmp = std::env::temp_dir().join(format!("walker-solo-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        let en = tmp.join("solo.md");
        std::fs::write(&en, "# EN").unwrap();
        assert_eq!(find_bilingual_pair(&en), None);
        std::fs::remove_dir_all(&tmp).ok();
    }
}
