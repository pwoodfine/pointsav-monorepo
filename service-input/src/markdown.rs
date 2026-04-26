// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Markdown parser via `pulldown-cmark`.
//!
//! Implements the `Parser` trait over pure-text Markdown input.
//! No temp-file shim needed (pulldown-cmark accepts `&str` directly).
//! ADR-07 compliant — deterministic, zero AI.

use pulldown_cmark::{Event, Options, Parser as CmarkParser, Tag, TagEnd};

use crate::{Format, ParsedDocument, ParseError, Parser};

pub struct MarkdownParser;

impl Parser for MarkdownParser {
    fn parse(&self, source_id: &str, bytes: &[u8]) -> Result<ParsedDocument, ParseError> {
        let input = std::str::from_utf8(bytes).map_err(|e| ParseError::FormatMismatch {
            declared: Format::Markdown,
            reason: format!("input is not valid UTF-8: {e}"),
        })?;

        let opts = Options::ENABLE_TABLES
            | Options::ENABLE_STRIKETHROUGH
            | Options::ENABLE_TASKLISTS;
        let cmark = CmarkParser::new_ext(input, opts);

        let mut text_buf = String::new();
        let mut headings: Vec<String> = Vec::new();
        let mut heading_buf: Option<String> = None;

        for event in cmark {
            match event {
                Event::Text(s) => {
                    if let Some(h) = heading_buf.as_mut() {
                        h.push_str(&s);
                    }
                    text_buf.push_str(&s);
                }
                Event::Code(s) => {
                    text_buf.push_str(&s);
                }
                Event::SoftBreak | Event::HardBreak => {
                    text_buf.push('\n');
                }
                Event::Start(Tag::Heading { .. }) => {
                    heading_buf = Some(String::new());
                }
                Event::End(TagEnd::Heading(_)) => {
                    if let Some(h) = heading_buf.take() {
                        if !h.is_empty() {
                            headings.push(h);
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(ParsedDocument {
            format: Format::Markdown,
            source_id: source_id.to_string(),
            text: text_buf,
            metadata: serde_json::json!({
                "headings": headings,
                "parser": "pulldown-cmark",
            }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Parser;

    #[test]
    fn markdown_text_is_extracted() {
        let p = MarkdownParser;
        let doc = p.parse("doc1", b"Hello **world**. This is a test.").unwrap();
        assert_eq!(doc.format, Format::Markdown);
        assert_eq!(doc.source_id, "doc1");
        assert!(doc.text.contains("Hello"), "plain text should be present");
        assert!(doc.text.contains("world"), "bold text should be present");
        assert_eq!(doc.metadata["parser"], "pulldown-cmark");
    }

    #[test]
    fn markdown_headings_extracted_into_metadata() {
        let p = MarkdownParser;
        let md = b"# Title\n\nSome body.\n\n## Subtitle\n\nMore text.";
        let doc = p.parse("doc1", md).unwrap();
        let headings = doc.metadata["headings"].as_array().unwrap();
        assert_eq!(headings.len(), 2, "two headings should be extracted");
        assert_eq!(headings[0].as_str().unwrap(), "Title");
        assert_eq!(headings[1].as_str().unwrap(), "Subtitle");
    }

    #[test]
    fn markdown_body_text_present() {
        let p = MarkdownParser;
        let md = b"# Heading\n\nThis paragraph has content.";
        let doc = p.parse("doc1", md).unwrap();
        assert!(doc.text.contains("This paragraph has content."));
    }

    #[test]
    fn markdown_invalid_utf8_returns_format_mismatch() {
        let p = MarkdownParser;
        let bad = &[0x80u8, 0x81, 0x82]; // invalid UTF-8
        match p.parse("doc1", bad) {
            Err(ParseError::FormatMismatch {
                declared: Format::Markdown,
                ..
            }) => {}
            other => panic!("expected FormatMismatch for Markdown, got {other:?}"),
        }
    }

    #[test]
    fn markdown_empty_input_is_valid() {
        let p = MarkdownParser;
        let doc = p.parse("empty", b"").unwrap();
        assert!(doc.text.is_empty());
        assert_eq!(
            doc.metadata["headings"].as_array().unwrap().len(),
            0
        );
    }
}
