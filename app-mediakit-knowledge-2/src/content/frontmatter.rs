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
    #[serde(default, alias = "type")]
    pub content_type: Option<String>,
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
    fn malformed_yaml_falls_back_to_default() {
        let doc = parse("---\n:::not valid yaml:::\n---\nBody.\n");
        assert!(doc.frontmatter.title.is_none());
        assert_eq!(doc.body_md.trim(), "Body.");
    }

    #[test]
    fn type_alias_maps_to_content_type() {
        let doc = parse("---\ntype: topic\n---\nx");
        assert_eq!(doc.frontmatter.content_type.as_deref(), Some("topic"));
    }
}
