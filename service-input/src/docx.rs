// SPDX-License-Identifier: Apache-2.0 OR MIT

//! DOCX parser via `docx-rust`.
//!
//! Implements the `Parser` trait over DOCX (Open Packaging Convention
//! ZIP container with `word/document.xml`). `docx-rust` exposes a
//! `from_reader` API that accepts any `Read + Seek`, so no temp-file
//! shim is needed — the input bytes are wrapped in a `Cursor` and
//! passed directly.
//!
//! ADR-07 compliant — deterministic, zero AI.

use std::io::Cursor;

use docx_rust::DocxFile;

use crate::{Format, ParsedDocument, ParseError, Parser};

pub struct DocxParser;

impl Parser for DocxParser {
    fn parse(&self, source_id: &str, bytes: &[u8]) -> Result<ParsedDocument, ParseError> {
        // DOCX is a ZIP-format container; all ZIP signatures start with "PK"
        // (0x50, 0x4B). Reject non-ZIP bytes early so callers get
        // FormatMismatch for obviously wrong input rather than a
        // parser-internal error from deep inside the ZIP parser.
        if !bytes.starts_with(b"PK") {
            return Err(ParseError::FormatMismatch {
                declared: Format::Docx,
                reason: format!(
                    "expected ZIP magic PK at offset 0, got {:?}",
                    &bytes[..bytes.len().min(2)]
                ),
            });
        }

        let cursor = Cursor::new(bytes);
        let docx_file = DocxFile::from_reader(cursor).map_err(|e| {
            ParseError::ParserInternal(format!("DocxFile::from_reader: {e}"))
        })?;

        let docx = docx_file.parse().map_err(|e| {
            ParseError::ParserInternal(format!("DocxFile::parse: {e}"))
        })?;

        let text = docx.document.body.text();
        let paragraph_count = docx
            .document
            .body
            .content
            .iter()
            .filter(|c| {
                matches!(c, docx_rust::document::BodyContent::Paragraph(_))
            })
            .count();

        Ok(ParsedDocument {
            format: Format::Docx,
            source_id: source_id.to_string(),
            text,
            metadata: serde_json::json!({
                "paragraph_count": paragraph_count,
                "parser": "docx-rust",
            }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Parser;

    #[test]
    fn non_zip_bytes_return_format_mismatch() {
        let p = DocxParser;
        match p.parse("doc1", b"not a zip file") {
            Err(ParseError::FormatMismatch {
                declared: Format::Docx,
                ..
            }) => {}
            other => panic!("expected FormatMismatch for Docx, got {other:?}"),
        }
    }

    #[test]
    fn zip_with_invalid_docx_returns_parser_internal() {
        let p = DocxParser;
        // Valid ZIP magic but not a real DOCX (no word/ directory).
        // Construct a minimal empty ZIP (end-of-central-directory record only).
        let empty_zip: &[u8] = &[
            0x50, 0x4B, 0x05, 0x06, // End of central directory signature
            0x00, 0x00, // Disk number
            0x00, 0x00, // Disk with start of central directory
            0x00, 0x00, // Number of entries on this disk
            0x00, 0x00, // Total entries
            0x00, 0x00, 0x00, 0x00, // Size of central directory
            0x00, 0x00, 0x00, 0x00, // Offset of central directory
            0x00, 0x00, // Comment length
        ];
        match p.parse("doc1", empty_zip) {
            Err(ParseError::ParserInternal(_)) => {}
            other => panic!("expected ParserInternal for invalid DOCX, got {other:?}"),
        }
    }
}
