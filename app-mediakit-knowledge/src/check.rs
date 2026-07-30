//! `check` subcommand — build-time content gate.
//!
//! Walks every page across the resolved mount set and reports:
//!   * **dead links** — `[[wikilinks]]` whose target resolves nowhere in the
//!     federation (content mount + guide mounts). This is L18's enforcement half;
//!     the render-time plain-text fallback already ships in `render.rs`.
//!   * **missing required fields** — frontmatter that violates its content-type
//!     blueprint (`blueprints::validate`).
//!
//! Self-contained: it reuses `mounts`, `blueprints`, `links::parse_wikilinks`,
//! `render::page_exists`, and `server::collect_all_topic_files` — and never touches
//! the running server. Intended for CI / pre-promote (`exit 1` on dead links).

use crate::{blueprints, links, mounts, render};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

/// A wikilink whose target resolves to no page in the federation.
#[derive(Debug, PartialEq, Eq)]
pub struct DeadLink {
    pub page: String,
    pub target: String,
}

/// A page whose frontmatter is missing keys its blueprint requires.
#[derive(Debug, PartialEq, Eq)]
pub struct MissingFields {
    pub page: String,
    pub type_name: String,
    pub missing: Vec<String>,
}

#[derive(Debug, Default)]
pub struct CheckReport {
    pub pages_checked: usize,
    pub dead_links: Vec<DeadLink>,
    pub missing_fields: Vec<MissingFields>,
}

pub struct CheckOpts {
    pub content_dir: PathBuf,
    pub guide_dir: Option<PathBuf>,
    pub guide_dir_2: Option<PathBuf>,
    pub blueprints_dir: Option<PathBuf>,
}

/// Split a Markdown file into its frontmatter YAML block and body, mirroring
/// `render::parse_page`'s delimiter handling. Returns `(None, full_text)` when
/// there is no frontmatter.
fn split_frontmatter(text: &str) -> (Option<&str>, &str) {
    if let Some(rest) = text.strip_prefix("---\n") {
        if let Some(end) = rest.find("\n---\n") {
            return (Some(&rest[..end]), &rest[end + "\n---\n".len()..]);
        }
    }
    (None, text)
}

/// A wikilink target we should not treat as a wiki page (category facets,
/// external URLs). Category links are passed through by the renderer.
fn is_non_page_target(target: &str) -> bool {
    target.starts_with("/category/") || target.contains("://") || target.starts_with("http")
}

/// Run the content gate over all pages reachable from `opts`.
pub async fn run_check(opts: &CheckOpts) -> std::io::Result<CheckReport> {
    let mount_set = mounts::resolve(
        &opts.content_dir,
        opts.guide_dir.as_deref(),
        opts.guide_dir_2.as_deref(),
    );
    // Non-primary mounts (guide roots) checked alongside content_dir, exactly as
    // the render-time resolver does.
    let link_roots = mounts::link_roots(&mount_set);

    let registry = match &opts.blueprints_dir {
        Some(dir) => blueprints::Registry::load(dir),
        None => blueprints::Registry::builtin(),
    };

    let files = crate::server::collect_all_topic_files(
        &opts.content_dir,
        &[opts.guide_dir.as_deref(), opts.guide_dir_2.as_deref()],
    )
    .await?;

    let mut report = CheckReport::default();
    for tf in &files {
        let Ok(text) = std::fs::read_to_string(&tf.path) else {
            continue;
        };
        report.pages_checked += 1;
        let (yaml, body) = split_frontmatter(&text);

        // Dead-link gate.
        for target in links::parse_wikilinks(body) {
            if is_non_page_target(&target) {
                continue;
            }
            if !render::page_exists(&target, &opts.content_dir, &link_roots) {
                report.dead_links.push(DeadLink {
                    page: tf.slug.clone(),
                    target,
                });
            }
        }

        // Blueprint frontmatter validation.
        if let Some(map) =
            yaml.and_then(|y| serde_yaml::from_str::<BTreeMap<String, serde_yaml::Value>>(y).ok())
        {
            let type_name = map
                .get("type")
                .or_else(|| map.get("content_type"))
                .and_then(|v| v.as_str())
                .unwrap_or("topic")
                .to_string();
            if let Some(bp) = registry.find(&type_name) {
                let keys: BTreeSet<String> = map.keys().cloned().collect();
                let missing = blueprints::validate(&keys, bp);
                if !missing.is_empty() {
                    report.missing_fields.push(MissingFields {
                        page: tf.slug.clone(),
                        type_name,
                        missing,
                    });
                }
            }
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &std::path::Path, name: &str, body: &str) {
        std::fs::write(dir.join(name), body).unwrap();
    }

    #[tokio::test]
    async fn flags_dead_links_and_missing_fields_only() {
        let base = std::env::temp_dir().join(format!("check-test-{}", std::process::id()));
        let content = base.join("content");
        let guide = base.join("guide");
        std::fs::create_dir_all(&content).unwrap();
        std::fs::create_dir_all(&guide).unwrap();

        // (a) resolvable target in content
        write(
            &content,
            "existing-page.md",
            "---\ntype: topic\ntitle: E\nslug: existing-page\n---\nbody\n",
        );
        // (c) resolvable target only via the guide mount
        write(
            &guide,
            "setup-guide.md",
            "---\ntype: guide\ntitle: S\nslug: setup-guide\n---\nbody\n",
        );
        // linker: one good link, one guide link, one dead link, one category facet (ignored)
        write(
            &content,
            "linker.md",
            "---\ntype: topic\ntitle: L\nslug: linker\n---\nSee [[Existing Page]], [[Setup Guide]], [[No Such Page]], and [[/category/governance]].\n",
        );
        // (d) page missing the required `slug` field
        write(
            &content,
            "bad.md",
            "---\ntype: topic\ntitle: B\n---\nbody\n",
        );

        let report = run_check(&CheckOpts {
            content_dir: content.clone(),
            guide_dir: Some(guide.clone()),
            guide_dir_2: None,
            blueprints_dir: None,
        })
        .await
        .unwrap();

        // Exactly one dead link: linker -> no-such-page. The good link, the guide
        // link, and the /category/ facet must NOT be flagged.
        assert_eq!(
            report.dead_links.len(),
            1,
            "dead_links: {:?}",
            report.dead_links
        );
        assert_eq!(report.dead_links[0].target, "no-such-page");
        assert_eq!(report.dead_links[0].page, "linker");

        // Exactly one missing-fields finding: bad (missing slug).
        assert_eq!(
            report.missing_fields.len(),
            1,
            "missing_fields: {:?}",
            report.missing_fields
        );
        assert_eq!(report.missing_fields[0].page, "bad");
        assert_eq!(report.missing_fields[0].missing, vec!["slug".to_string()]);

        std::fs::remove_dir_all(&base).ok();
    }

    #[tokio::test]
    async fn clean_corpus_has_no_findings() {
        let base = std::env::temp_dir().join(format!("check-clean-{}", std::process::id()));
        let content = base.join("content");
        std::fs::create_dir_all(&content).unwrap();
        write(
            &content,
            "a.md",
            "---\ntype: topic\ntitle: A\nslug: a\n---\nlink to [[B]]\n",
        );
        write(
            &content,
            "b.md",
            "---\ntype: topic\ntitle: B\nslug: b\n---\nlink to [[A]]\n",
        );
        let report = run_check(&CheckOpts {
            content_dir: content.clone(),
            guide_dir: None,
            guide_dir_2: None,
            blueprints_dir: None,
        })
        .await
        .unwrap();
        assert_eq!(report.pages_checked, 2);
        assert!(report.dead_links.is_empty());
        assert!(report.missing_fields.is_empty());
        std::fs::remove_dir_all(&base).ok();
    }
}
