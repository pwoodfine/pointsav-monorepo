// SPDX-License-Identifier: FSL-1.1-ALv2
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

//! YAML frontmatter parsing.
//!
//! A content file is an optional `---`-delimited YAML block followed by a
//! Markdown body. The schema mirrors the `foundry-doc-v1` fields the content
//! repos actually use; unknown keys are ignored so content can carry
//! editorial metadata the engine does not consume.

use serde::Deserialize;

/// Parsed frontmatter. Every field is optional — a file with no frontmatter
/// yields `Frontmatter::default()`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Frontmatter {
    pub title: Option<String>,
    pub slug: Option<String>,
    pub category: Option<String>,
    pub status: Option<String>,
    #[serde(default)]
    pub content_type: Option<String>,
    // Separate from `content_type`: many files carry BOTH `type:` and
    // `content_type:`. Aliasing them to one field makes serde treat that as a
    // duplicate and fail the whole parse (dropping title/category). Keep `type`
    // as its own field so both can coexist.
    #[serde(default, rename = "type")]
    pub kind: Option<String>,
    pub short_description: Option<String>,
    pub last_edited: Option<String>,
    pub editor: Option<String>,
    pub bcsc_class: Option<String>,
    /// Slug of the bilingual counterpart (`paired_with`).
    pub paired_with: Option<String>,
    pub audience: Option<String>,
    pub language: Option<String>,
    pub hatnote: Option<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// Bracket-ID citation set (SPEC-journal-wiki-render-contract.md §1.3). Currently
    /// unused by any render path (P5b Phase 2 wires this up) — do not confuse with
    /// `references:` below, a separate, already-shipped footnote mechanism.
    #[serde(default)]
    pub cites: Vec<String>,
    /// A reference list (`id`, `text`, `url`) rendered as footnotes; the body's
    /// `[^id]` markers link to them (see `render::render_doc`).
    #[serde(default)]
    pub references: Vec<Reference>,

    // --- JOURNAL (`foundry-journal-v1`) fields — P5b Phase 1. Parsed but not yet
    // consumed by any render path; masthead/banner/route wiring is later P5b phases.
    /// Abstract as a frontmatter field (SPEC §2.3) — replaces a body `## Abstract`.
    #[serde(default, rename = "abstract")]
    pub abstract_text: Option<String>,
    /// Lifecycle state: `draft` | `under-review` | `accepted` | `published` | `archived`.
    pub state: Option<String>,
    /// SemVer version string (feeds the working-paper banner).
    pub version: Option<String>,
    #[serde(default)]
    pub authors: Vec<Author>,
    pub license: Option<String>,
    pub cite_as: Option<String>,
    pub preprint_posted_date: Option<String>,
    /// Publish gate (SPEC §6): must be `true` before `category: research` is public.
    #[serde(default)]
    pub forbidden_terms_cleared: bool,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub subject_codes: Vec<String>,
    /// `standard` (default, absent) | `geospatial` (SPEC §10 figure/caption handling).
    pub paper_class: Option<String>,
    pub doi: Option<String>,
}

/// One JOURNAL paper author (SPEC-journal-wiki-render-contract.md, masthead fields).
#[derive(Debug, Clone, Deserialize)]
pub struct Author {
    pub name: Option<String>,
    pub affiliation: Option<String>,
    pub email: Option<String>,
    pub orcid: Option<String>,
    #[serde(default)]
    pub credit_roles: Vec<String>,
}

/// One entry of a `references:` list.
#[derive(Debug, Clone, Deserialize)]
pub struct Reference {
    #[serde(default, deserialize_with = "scalar_to_string")]
    pub id: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub url: Option<String>,
}

/// Accept a YAML scalar id as either a string or an integer.
fn scalar_to_string<'de, D: serde::Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Scalar {
        S(String),
        I(i64),
    }
    Ok(match Scalar::deserialize(d)? {
        Scalar::S(s) => s,
        Scalar::I(i) => i.to_string(),
    })
}

/// A parsed content file: its frontmatter and the Markdown body after it.
#[derive(Debug, Clone)]
pub struct ParsedDoc {
    pub frontmatter: Frontmatter,
    pub body_md: String,
}

/// Split a raw file into (optional raw YAML, body). Frontmatter must be the
/// very first line as `---` and close with a line that is exactly `---`.
fn split(text: &str) -> (Option<&str>, &str) {
    let t = text.strip_prefix('\u{feff}').unwrap_or(text); // tolerate BOM
    let Some(rest) = t.strip_prefix("---\n").or_else(|| t.strip_prefix("---\r\n")) else {
        return (None, text);
    };
    // Find the closing delimiter line.
    let mut idx = 0;
    for line in rest.split_inclusive('\n') {
        let trimmed = line.trim_end_matches(['\n', '\r']);
        if trimmed == "---" {
            let yaml = &rest[..idx];
            let body = &rest[idx + line.len()..];
            return (Some(yaml), body);
        }
        idx += line.len();
    }
    // No closing delimiter — treat the whole thing as body.
    (None, text)
}

/// Strip HTML comments (`<!-- ... -->`, possibly multi-line) from a Markdown
/// body, skipping fenced code blocks so an example of comment syntax in a
/// code sample survives untouched. Applied once at parse time so internal
/// authoring notes never leak into rendered HTML, the search index, or the
/// auto-derived short-description fallback (`first_body_summary` in
/// `walk.rs`, which is comment-*opening*-line-aware but not
/// comment-*continuation*-line-aware — this is the actual fix for that gap).
fn strip_html_comments(md: &str) -> String {
    let mut out = String::with_capacity(md.len());
    let mut in_fence = false;
    let mut in_comment = false;
    for line in md.lines() {
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            in_fence = !in_fence;
            out.push_str(line);
            out.push('\n');
            continue;
        }
        if in_fence {
            out.push_str(line);
            out.push('\n');
            continue;
        }
        let mut rest = line;
        loop {
            if in_comment {
                match rest.find("-->") {
                    Some(end) => {
                        rest = &rest[end + 3..];
                        in_comment = false;
                    }
                    None => break,
                }
            } else {
                match rest.find("<!--") {
                    Some(start) => {
                        out.push_str(&rest[..start]);
                        rest = &rest[start + 4..];
                        in_comment = true;
                    }
                    None => {
                        out.push_str(rest);
                        break;
                    }
                }
            }
        }
        out.push('\n');
    }
    out
}

/// Parse a raw content file into frontmatter + body. Malformed YAML yields
/// default frontmatter (never an error) so one bad file cannot break a walk.
/// The body has HTML comments stripped (see `strip_html_comments`).
pub fn parse(text: &str) -> ParsedDoc {
    let (yaml, body) = split(text);
    let body = strip_html_comments(body);
    let frontmatter = match yaml {
        Some(y) => serde_yaml::from_str(y).unwrap_or_default(),
        None => Frontmatter::default(),
    };
    ParsedDoc {
        frontmatter,
        body_md: body,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_single_line_comment() {
        let doc = parse("---\ntitle: T\n---\n<!-- internal note -->\nReal content.\n");
        assert!(!doc.body_md.contains("internal note"));
        assert!(doc.body_md.contains("Real content."));
    }

    #[test]
    fn strips_multiline_comment_including_continuation_lines() {
        // This is the actual bug the audit found: a comment's *continuation*
        // line (not the opening `<!--` line) was leaking into
        // walk.rs::first_body_summary's auto-derived description, because
        // that function only skips lines that individually start with `<!--`.
        let doc = parse(
            "---\ntitle: T\n---\n<!--\nThis line does not start with <!-- itself.\nRendered by src/server.rs::home_chrome().\n-->\nReal content.\n",
        );
        assert!(!doc.body_md.contains("home_chrome"));
        assert!(!doc.body_md.contains("does not start with"));
        assert!(doc.body_md.contains("Real content."));
    }

    #[test]
    fn preserves_comment_syntax_inside_code_fence() {
        let doc = parse("---\ntitle: T\n---\nExample:\n\n```html\n<!-- this is a real example -->\n```\n");
        assert!(doc.body_md.contains("<!-- this is a real example -->"));
    }

    #[test]
    fn parses_frontmatter_and_body() {
        let src = "---\ntitle: \"Zero-container inference\"\nslug: zero-container-inference\ncategory: architecture\nstatus: stub\n---\nBody starts here.\n";
        let doc = parse(src);
        assert_eq!(doc.frontmatter.title.as_deref(), Some("Zero-container inference"));
        assert_eq!(doc.frontmatter.slug.as_deref(), Some("zero-container-inference"));
        assert_eq!(doc.frontmatter.category.as_deref(), Some("architecture"));
        assert_eq!(doc.body_md.trim(), "Body starts here.");
    }

    #[test]
    fn no_frontmatter_is_all_body() {
        let doc = parse("# Just markdown\n\nNo frontmatter.\n");
        assert!(doc.frontmatter.title.is_none());
        assert!(doc.body_md.contains("Just markdown"));
    }

    #[test]
    fn parses_references_with_int_and_string_ids() {
        let doc = parse(
            "---\ntitle: X\nreferences:\n  - id: 1\n    text: \"First\"\n    url: \"https://a\"\n  - id: rfc-9162\n    text: \"Second\"\n---\nbody\n",
        );
        assert_eq!(doc.frontmatter.references.len(), 2);
        assert_eq!(doc.frontmatter.references[0].id, "1"); // int coerced to string
        assert_eq!(doc.frontmatter.references[1].id, "rfc-9162");
        assert_eq!(doc.frontmatter.references[0].url.as_deref(), Some("https://a"));
    }

    #[test]
    fn malformed_yaml_falls_back_to_default() {
        let doc = parse("---\n:::not valid yaml:::\n---\nBody.\n");
        assert!(doc.frontmatter.title.is_none());
        assert_eq!(doc.body_md.trim(), "Body.");
    }

    #[test]
    fn type_and_content_type_coexist() {
        // A file carrying BOTH keys must still parse (this used to fail).
        let doc = parse("---\ntitle: T\ntype: topic\ncontent_type: guide\n---\nx");
        assert_eq!(doc.frontmatter.title.as_deref(), Some("T"));
        assert_eq!(doc.frontmatter.kind.as_deref(), Some("topic"));
        assert_eq!(doc.frontmatter.content_type.as_deref(), Some("guide"));
    }

    #[test]
    fn parses_journal_frontmatter() {
        // Deliberately one continuous string (no `\`-newline source continuation,
        // which strips leading whitespace on the next line and would flatten the
        // `authors:` list's indentation) — matches the style of the existing
        // `parses_references_with_int_and_string_ids` test above.
        let src = "---\ntitle: \"Capability Geometry\"\nslug: capability-geometry\ncategory: research\nstate: draft\nversion: 0.4.0\npaper_class: standard\nabstract: |\n  A short abstract spanning\n  two lines.\nlicense: CC BY 4.0\ncite_as: \"Woodfine (2026)\"\npreprint_posted_date: 2026-07-02\nforbidden_terms_cleared: true\ndoi: 10.0000/example\nkeywords: [capability, security]\nsubject_codes: [cs.OS]\ncites: [rfc-9162, c2sp-signed-note]\nauthors:\n  - name: J. Woodfine\n    affiliation: PointSav Digital Systems\n    email: corporate.secretary@woodfinegroup.com\n    orcid: \"0000-0000-0000-0000\"\n    credit_roles: [Writing, Conceptualization]\n---\nBody.\n";
        let doc = parse(src);
        let fm = &doc.frontmatter;
        assert_eq!(fm.state.as_deref(), Some("draft"));
        assert_eq!(fm.version.as_deref(), Some("0.4.0"));
        assert_eq!(fm.paper_class.as_deref(), Some("standard"));
        assert_eq!(fm.abstract_text.as_deref(), Some("A short abstract spanning\ntwo lines.\n"));
        assert_eq!(fm.license.as_deref(), Some("CC BY 4.0"));
        assert_eq!(fm.cite_as.as_deref(), Some("Woodfine (2026)"));
        assert_eq!(fm.preprint_posted_date.as_deref(), Some("2026-07-02"));
        assert!(fm.forbidden_terms_cleared);
        assert_eq!(fm.doi.as_deref(), Some("10.0000/example"));
        assert_eq!(fm.keywords, vec!["capability", "security"]);
        assert_eq!(fm.subject_codes, vec!["cs.OS"]);
        assert_eq!(fm.cites, vec!["rfc-9162", "c2sp-signed-note"]);
        assert_eq!(fm.authors.len(), 1);
        assert_eq!(fm.authors[0].name.as_deref(), Some("J. Woodfine"));
        assert_eq!(fm.authors[0].credit_roles, vec!["Writing", "Conceptualization"]);
    }

    #[test]
    fn journal_fields_default_absent() {
        // A non-JOURNAL doc (no journal fields at all) must still parse cleanly —
        // every new field is optional, matching this struct's existing convention.
        let doc = parse("---\ntitle: X\n---\nbody");
        let fm = &doc.frontmatter;
        assert!(fm.state.is_none());
        assert!(fm.paper_class.is_none());
        assert!(fm.abstract_text.is_none());
        assert!(!fm.forbidden_terms_cleared);
        assert!(fm.authors.is_empty());
        assert!(fm.keywords.is_empty());
    }
}
