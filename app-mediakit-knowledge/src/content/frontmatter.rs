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
    #[serde(default)]
    pub cites: Vec<String>,
    /// A reference list (`id`, `text`, `url`) rendered as footnotes; the body's
    /// `[^id]` markers link to them (see `render::render_doc`).
    #[serde(default)]
    pub references: Vec<Reference>,
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

/// Parse a raw content file into frontmatter + body. Malformed YAML yields
/// default frontmatter (never an error) so one bad file cannot break a walk.
pub fn parse(text: &str) -> ParsedDoc {
    let (yaml, body) = split(text);
    let frontmatter = match yaml {
        Some(y) => serde_yaml::from_str(y).unwrap_or_default(),
        None => Frontmatter::default(),
    };
    ParsedDoc {
        frontmatter,
        body_md: body.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
