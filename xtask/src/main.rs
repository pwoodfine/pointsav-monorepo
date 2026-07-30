//! xtask — build-time content gate (cargo xtask check-content).
//!
//! Usage:
//!   cargo xtask check-content <path1> [path2] ...
//!
//! Walks all Markdown files in the given mount paths, collects every slug,
//! then checks all [[wikilinks]] against the slug set. Reports dead links
//! and missing required frontmatter fields. Exits 1 if any dead links or
//! missing required fields are found.
//!
//! This is the Phase 5 hard-promote gate (L18 + L29): an unresolved [[slug]]
//! across the mount set BLOCKS promote. The gate must pass before
//! `wikilink-missing` render is removed from the live sites.
//!
//! Implementation note: this xtask duplicates the dead-link + frontmatter
//! logic from `app-mediakit-knowledge/src/check.rs` inline rather than
//! importing it, to avoid a circular or cross-crate dependency at build time.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    // First arg is the binary name; remaining args are mount paths.
    let subcommand = args.get(1).map(|s| s.as_str());
    let paths: Vec<PathBuf> = if subcommand == Some("check-content") {
        args[2..].iter().map(PathBuf::from).collect()
    } else if args.len() > 1 && subcommand != Some("check-content") {
        // Support bare invocation: cargo xtask <path1> [path2] ...
        args[1..].iter().map(PathBuf::from).collect()
    } else {
        eprintln!("Usage: cargo xtask check-content <path1> [path2] ...");
        eprintln!("       cargo xtask <path1> [path2] ...");
        std::process::exit(2);
    };

    if paths.is_empty() {
        eprintln!("error: at least one content path is required");
        std::process::exit(2);
    }

    // Validate that all paths exist.
    for p in &paths {
        if !p.is_dir() {
            eprintln!("error: not a directory: {}", p.display());
            std::process::exit(2);
        }
    }

    let (dead_links, missing_fields, total) = check_content(&paths);

    println!("checked {} articles across {} mount(s)", total, paths.len());
    for (source, target) in &dead_links {
        println!("DEAD LINK   {} -> [[{}]]", source, target);
    }
    for (slug, field) in &missing_fields {
        println!("MISSING     {}: {}", slug, field);
    }
    println!(
        "{} dead link(s), {} article(s) with missing required frontmatter fields",
        dead_links.len(),
        missing_fields.len()
    );

    if !dead_links.is_empty() || !missing_fields.is_empty() {
        std::process::exit(1);
    }
}

/// Run the content gate over a set of mount paths.
///
/// Returns `(dead_links, missing_fields, total_articles)`.
///
/// `dead_links` is a list of `(source_slug, target_slug)` pairs.
/// `missing_fields` is a list of `(slug, field_name)` pairs.
#[allow(clippy::type_complexity)]
fn check_content(paths: &[PathBuf]) -> (Vec<(String, String)>, Vec<(String, String)>, usize) {
    // Collect all article files across all mounts (skipping infrastructure
    // directories such as `.agent/`, `.git/`, which hold rule docs whose
    // `[[slug]]` / `[[Display Text]]` are literal syntax examples, not links).
    let mut all_files: Vec<PathBuf> = Vec::new();
    for base in paths {
        collect_md_files(base, &mut all_files);
    }

    // Parse every article once: its reporting slug (bare stem), its declared
    // frontmatter `slug:` (if any), and its body.
    struct Article {
        report_slug: String,
        body: String,
        fields: Option<BTreeMap<String, String>>,
    }
    let mut articles: Vec<Article> = Vec::new();
    // The resolution set is what `[[wikilinks]]` are matched against. The live
    // renderer resolves a *bare* slug (e.g. `genesis-protocol`), NOT the
    // category-prefixed path (`architecture/genesis-protocol`). So the set is
    // built from each article's bare filename stem AND its frontmatter `slug:`.
    let mut slug_set: HashSet<String> = HashSet::new();

    for path in &all_files {
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        let (yaml, body) = split_frontmatter(&text);
        let fields = yaml.and_then(serde_yaml_simple_parse);

        // Bare stem: filename without `.md`, and without a trailing `.es`
        // language suffix (the EN and ES siblings share one canonical slug).
        let stem = file_stem_slug(path);
        slug_set.insert(normalise_slug(&stem));
        if let Some(s) = fields.as_ref().and_then(|m| m.get("slug")) {
            slug_set.insert(normalise_slug(s));
        }

        let report_slug = fields
            .as_ref()
            .and_then(|m| m.get("slug").cloned())
            .unwrap_or_else(|| stem.clone());
        articles.push(Article {
            report_slug,
            body,
            fields,
        });
    }

    let mut dead_links: Vec<(String, String)> = Vec::new();
    let mut missing_fields: Vec<(String, String)> = Vec::new();
    let total = articles.len();

    // Required frontmatter fields for the built-in `topic` and `guide` blueprints.
    let required_topic = ["title", "slug"];
    let required_guide = ["title", "slug"];

    for art in &articles {
        // Dead-link gate.
        for target in parse_wikilinks(&art.body) {
            if is_non_page_target(&target) {
                continue;
            }
            // Match on the bare slug: take the last path segment, then
            // normalise (lowercase, spaces→hyphens) to mirror render.rs.
            let bare = target.rsplit('/').next().unwrap_or(&target);
            let normalised = normalise_slug(bare);
            if !slug_set.contains(&normalised) {
                dead_links.push((art.report_slug.clone(), target));
            }
        }

        // Blueprint frontmatter validation.
        let content_type_val: &str = art
            .fields
            .as_ref()
            .and_then(|m| m.get("type").or_else(|| m.get("content_type")))
            .map(|s| s.as_str())
            .unwrap_or("topic");

        if let Some(map) = &art.fields {
            let required = match content_type_val {
                "guide" => &required_guide[..],
                _ => &required_topic[..],
            };
            for &field in required {
                if !map.contains_key(field) {
                    missing_fields.push((art.report_slug.clone(), field.to_string()));
                }
            }
        }

        // Sprint N: section schema gate — warn when required level-2 headings are absent.
        // Only applies to `topic` and `guide` content types; other blueprints are exempt.
        let h2_headings: Vec<&str> = art
            .body
            .lines()
            .filter(|l| l.starts_with("## "))
            .map(|l| l.trim_start_matches('#').trim())
            .collect();
        match content_type_val {
            "topic" if h2_headings.is_empty() => {
                missing_fields.push((
                    art.report_slug.clone(),
                    "section:topic-requires-at-least-one-h2".to_string(),
                ));
            }
            "guide" => {
                let guide_sections = ["Steps", "Prerequisites", "Procedure"];
                let has_guide_section = h2_headings
                    .iter()
                    .any(|h| guide_sections.iter().any(|s| h.eq_ignore_ascii_case(s)));
                if !has_guide_section {
                    missing_fields.push((
                        art.report_slug.clone(),
                        "section:guide-missing-Steps/Prerequisites/Procedure".to_string(),
                    ));
                }
            }
            _ => {}
        }
    }

    (dead_links, missing_fields, total)
}

/// Bare slug for a Markdown file: filename without the `.md` extension and
/// without a trailing `.es` language suffix. `architecture/genesis-protocol.md`
/// and `architecture/genesis-protocol.es.md` both yield `genesis-protocol`.
fn file_stem_slug(path: &Path) -> String {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    let stem = name.strip_suffix(".md").unwrap_or(name);
    stem.strip_suffix(".es").unwrap_or(stem).to_string()
}

/// Directories that hold operational / rule / VCS files rather than published
/// wiki content. Their Markdown contains `[[slug]]`-style syntax examples that
/// are not real wikilinks and must not be scanned.
fn is_skipped_dir(name: &str) -> bool {
    matches!(
        name,
        ".agent" | ".git" | ".claude" | "node_modules" | "target"
    ) || name.starts_with('.')
}

/// Recursively collect all published `.md` files under `dir`, skipping
/// infrastructure directories (see `is_skipped_dir`).
fn collect_md_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let skip = path
                .file_name()
                .and_then(|n| n.to_str())
                .map(is_skipped_dir)
                .unwrap_or(false);
            if !skip {
                collect_md_files(&path, out);
            }
        } else if path.extension().map(|e| e == "md").unwrap_or(false) {
            out.push(path);
        }
    }
}

/// Split raw Markdown text into `(Option<yaml_block>, body)`.
fn split_frontmatter(text: &str) -> (Option<&str>, String) {
    if let Some(rest) = text.strip_prefix("---\n") {
        if let Some(end) = rest.find("\n---\n") {
            return (
                Some(&rest[..end]),
                rest[end + "\n---\n".len()..].to_string(),
            );
        }
    }
    (None, text.to_string())
}

/// Minimal YAML parser: extract key: value pairs from a flat YAML block.
/// Does not handle nested structures — only the top-level scalar fields
/// needed for frontmatter validation.
fn serde_yaml_simple_parse(yaml: &str) -> Option<BTreeMap<String, String>> {
    let mut map = BTreeMap::new();
    for line in yaml.lines() {
        if let Some((key, val)) = line.split_once(':') {
            let key = key.trim().to_string();
            let val = val.trim().trim_matches('"').trim_matches('\'').to_string();
            if !key.is_empty() && !key.starts_with('-') && !key.starts_with('#') {
                map.insert(key, val);
            }
        }
    }
    if map.is_empty() {
        None
    } else {
        Some(map)
    }
}

/// Blank out fenced code blocks (``` / ~~~) and inline code spans (backtick
/// runs) so their contents are not scanned for wikilinks. Comrak does not parse
/// wikilink syntax inside code, so `[[wikilink]]` written as a `` `[[wikilink]]` ``
/// example in prose is literal text on the live site, not a link.
fn strip_code(body: &str) -> String {
    let mut out = String::with_capacity(body.len());
    let mut in_fence = false;
    for line in body.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            out.push('\n');
            continue;
        }
        if in_fence {
            out.push('\n');
            continue;
        }
        // Strip inline code spans: a run of N backticks closes on the next run
        // of N backticks. Everything between (and the ticks) is dropped.
        let bytes = line.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'`' {
                let mut n = 0;
                while i + n < bytes.len() && bytes[i + n] == b'`' {
                    n += 1;
                }
                // Find a closing run of exactly n backticks.
                let mut j = i + n;
                let mut closed = false;
                while j < bytes.len() {
                    if bytes[j] == b'`' {
                        let mut m = 0;
                        while j + m < bytes.len() && bytes[j + m] == b'`' {
                            m += 1;
                        }
                        if m == n {
                            j += m;
                            closed = true;
                            break;
                        }
                        j += m;
                    } else {
                        j += 1;
                    }
                }
                if closed {
                    i = j;
                    continue;
                }
                // Unterminated run — keep the rest of the line as-is.
                out.push_str(&line[i..]);
                break;
            }
            out.push(bytes[i] as char);
            i += 1;
        }
        out.push('\n');
    }
    out
}

/// Extract `[[wikilink]]` targets from Markdown body text (code-stripped).
fn parse_wikilinks(body: &str) -> Vec<String> {
    let stripped = strip_code(body);
    let mut targets = Vec::new();
    let mut rest = stripped.as_str();
    while let Some(start) = rest.find("[[") {
        rest = &rest[start + 2..];
        if let Some(end) = rest.find("]]") {
            let inner = rest[..end].trim();
            // Wikilink target is BEFORE the pipe: `[[target|Display Text]]`
            // (comrak `wikilinks_title_after_pipe = true`; mirrors render.rs).
            let target = if let Some((slug, _)) = inner.split_once('|') {
                slug.trim()
            } else {
                inner
            };
            targets.push(target.to_string());
            rest = &rest[end + 2..];
        } else {
            break;
        }
    }
    targets
}

/// Return true for wikilink targets that are not wiki article slugs.
fn is_non_page_target(target: &str) -> bool {
    target.starts_with("/category/")
        || target.contains("://")
        || target.starts_with("http")
        || target.starts_with('#')
}

/// Normalise a wikilink target to a slug: lowercase, spaces→hyphens.
/// Mirrors the normalisation applied in `render.rs`.
fn normalise_slug(target: &str) -> String {
    target
        .to_lowercase()
        .replace(' ', "-")
        .trim_matches('/')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_wikilinks() {
        let body = "See [[Genesis Protocol]] and [[ppn-architecture-overview]].";
        let links = parse_wikilinks(body);
        assert_eq!(links, vec!["Genesis Protocol", "ppn-architecture-overview"]);
    }

    #[test]
    fn parses_display_text_wikilinks() {
        // Convention is `[[target|Display Text]]` — target before the pipe.
        let body =
            "See [[sovereign-mesh|Sovereign Mesh]] and [[yoyo-compute-substrate|GPU compute]].";
        let links = parse_wikilinks(body);
        assert_eq!(links, vec!["sovereign-mesh", "yoyo-compute-substrate"]);
    }

    #[test]
    fn skips_wikilinks_in_code() {
        // Inline code span and fenced block must not yield wikilink targets.
        let body =
            "Use `[[wikilink]]` syntax. Real: [[genesis-protocol]].\n```\n[[in-fence]]\n```\nDone.";
        let links = parse_wikilinks(body);
        assert_eq!(links, vec!["genesis-protocol"]);
    }

    #[test]
    fn keeps_links_with_accented_prose() {
        // UTF-8 (Spanish) prose around an ASCII-slug wikilink must still resolve.
        let body = "El [[app-mediakit-knowledge|wiki de ingeniería]] acepta `[[slug]]`.";
        let links = parse_wikilinks(body);
        assert_eq!(links, vec!["app-mediakit-knowledge"]);
    }

    #[test]
    fn file_stem_strips_md_and_es() {
        assert_eq!(
            file_stem_slug(Path::new("a/b/genesis-protocol.md")),
            "genesis-protocol"
        );
        assert_eq!(
            file_stem_slug(Path::new("a/b/genesis-protocol.es.md")),
            "genesis-protocol"
        );
    }

    #[test]
    fn resolves_bare_slug_against_category_pathed_file() {
        // Article lives at architecture/genesis-protocol.md with bare frontmatter
        // slug; a bare [[genesis-protocol]] link from another category must resolve.
        let tmp = std::env::temp_dir().join(format!("xtask-bare-{}", std::process::id()));
        std::fs::create_dir_all(tmp.join("architecture")).unwrap();
        std::fs::create_dir_all(tmp.join("substrate")).unwrap();
        std::fs::write(
            tmp.join("architecture/genesis-protocol.md"),
            "---\ntitle: Genesis\nslug: genesis-protocol\n---\nBody.\n",
        )
        .unwrap();
        std::fs::write(
            tmp.join("substrate/other.md"),
            "---\ntitle: Other\nslug: other\n---\nSee [[genesis-protocol]] and [[Genesis Protocol|the protocol]].\n",
        )
        .unwrap();
        let (dead, _missing, total) = check_content(std::slice::from_ref(&tmp));
        assert_eq!(total, 2);
        assert!(
            dead.is_empty(),
            "bare + display-text links should resolve: {dead:?}"
        );
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn skips_agent_rule_dirs() {
        // `.agent/rules/*` syntax-example placeholders must not be scanned.
        let tmp = std::env::temp_dir().join(format!("xtask-skip-{}", std::process::id()));
        std::fs::create_dir_all(tmp.join(".agent/rules")).unwrap();
        std::fs::write(
            tmp.join(".agent/rules/naming.md"),
            "Use [[slug]] or [[unknown-slug]] as an example.\n",
        )
        .unwrap();
        std::fs::write(
            tmp.join("real.md"),
            "---\ntitle: Real\nslug: real\n---\nBody.\n",
        )
        .unwrap();
        let (dead, _missing, total) = check_content(std::slice::from_ref(&tmp));
        assert_eq!(
            total, 1,
            "only the real article is counted, not .agent/ docs"
        );
        assert!(
            dead.is_empty(),
            "placeholder examples must not count: {dead:?}"
        );
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn skips_non_page_targets() {
        assert!(is_non_page_target("/category/architecture"));
        assert!(is_non_page_target("https://example.com"));
        assert!(!is_non_page_target("genesis-protocol"));
    }

    #[test]
    fn normalises_slug() {
        assert_eq!(normalise_slug("Genesis Protocol"), "genesis-protocol");
        assert_eq!(normalise_slug("PPn Architecture"), "ppn-architecture");
    }

    #[test]
    fn simple_yaml_parse() {
        let yaml = "title: Test Article\nslug: test-article\ncategory: architecture\n";
        let map = serde_yaml_simple_parse(yaml).unwrap();
        assert_eq!(map.get("title").map(|s| s.as_str()), Some("Test Article"));
        assert_eq!(map.get("slug").map(|s| s.as_str()), Some("test-article"));
    }

    #[test]
    fn check_content_catches_dead_links() {
        let tmp = std::env::temp_dir().join(format!("xtask-test-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(
            tmp.join("existing.md"),
            "---\ntitle: Existing\nslug: existing\n---\n\n## Section\n\nBody.\n",
        )
        .unwrap();
        std::fs::write(
            tmp.join("linker.md"),
            "---\ntitle: Linker\nslug: linker\n---\n\n## Section\n\nSee [[existing]] and [[missing-page]].\n",
        )
        .unwrap();
        let (dead, missing, total) = check_content(std::slice::from_ref(&tmp));
        assert_eq!(total, 2);
        assert_eq!(dead.len(), 1);
        assert_eq!(dead[0].1, "missing-page");
        assert!(missing.is_empty());
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn check_content_catches_missing_required_fields() {
        let tmp = std::env::temp_dir().join(format!("xtask-miss-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        // Missing `slug` field.
        std::fs::write(
            tmp.join("bad.md"),
            "---\ntitle: Bad Article\n---\n\n## Section\n\nBody.\n",
        )
        .unwrap();
        let (dead, missing, _) = check_content(std::slice::from_ref(&tmp));
        assert!(dead.is_empty());
        assert_eq!(missing.len(), 1);
        assert_eq!(missing[0].1, "slug");
        std::fs::remove_dir_all(&tmp).ok();
    }
}
