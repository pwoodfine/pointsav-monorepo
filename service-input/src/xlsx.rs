// SPDX-License-Identifier: Apache-2.0 OR MIT

//! XLSX parser via `calamine`.
//!
//! Implements the `Parser` trait over XLSX (Excel Open XML Format).
//! calamine exposes `open_workbook_from_rs` which accepts any
//! `Read + Seek`, so no temp-file shim is needed — the input bytes
//! are wrapped in a `Cursor` and passed directly.
//!
//! Text is extracted by iterating all sheets, all rows, all cells,
//! concatenating cell `Display` values space-separated per row, with
//! rows newline-separated. Non-text cells (numbers, dates, booleans)
//! are stringified via their `Display` impl.
//!
//! ADR-07 compliant — deterministic, zero AI.

use std::io::Cursor;

use calamine::{open_workbook_from_rs, Data, Reader, Xlsx};

use crate::{Format, ParsedDocument, ParseError, Parser};

pub struct XlsxParser;

impl Parser for XlsxParser {
    fn parse(&self, source_id: &str, bytes: &[u8]) -> Result<ParsedDocument, ParseError> {
        // XLSX is a ZIP-format container (OOXML). Reject non-ZIP bytes
        // early so the caller gets FormatMismatch rather than a
        // deep-parser internal error.
        if !bytes.starts_with(b"PK") {
            return Err(ParseError::FormatMismatch {
                declared: Format::Xlsx,
                reason: format!(
                    "expected ZIP magic PK at offset 0, got {:?}",
                    &bytes[..bytes.len().min(2)]
                ),
            });
        }

        let cursor = Cursor::new(bytes);
        let mut workbook: Xlsx<_> = open_workbook_from_rs(cursor)
            .map_err(|e| ParseError::ParserInternal(format!("calamine open: {e}")))?;

        let sheet_names: Vec<String> = workbook.sheet_names().to_vec();
        let sheet_count = sheet_names.len();
        let mut text_buf = String::new();

        for sheet_name in &sheet_names {
            match workbook.worksheet_range(sheet_name) {
                Ok(range) => {
                    for row in range.rows() {
                        let row_text: Vec<String> = row
                            .iter()
                            .filter_map(|cell| match cell {
                                Data::Empty => None,
                                other => Some(other.to_string()),
                            })
                            .collect();
                        if !row_text.is_empty() {
                            if !text_buf.is_empty() {
                                text_buf.push('\n');
                            }
                            text_buf.push_str(&row_text.join(" "));
                        }
                    }
                }
                Err(e) => {
                    // A single unreadable sheet is logged in the text
                    // buffer; we do not abort the whole parse so the
                    // caller gets partial text from other sheets.
                    if !text_buf.is_empty() {
                        text_buf.push('\n');
                    }
                    text_buf.push_str(&format!("[sheet '{sheet_name}' error: {e}]"));
                }
            }
        }

        Ok(ParsedDocument {
            format: Format::Xlsx,
            source_id: source_id.to_string(),
            text: text_buf,
            metadata: serde_json::json!({
                "sheet_count": sheet_count,
                "sheets": sheet_names,
                "parser": "calamine",
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
        let p = XlsxParser;
        match p.parse("doc1", b"not a zip file") {
            Err(ParseError::FormatMismatch {
                declared: Format::Xlsx,
                ..
            }) => {}
            other => panic!("expected FormatMismatch for Xlsx, got {other:?}"),
        }
    }

    #[test]
    fn zip_with_invalid_xlsx_returns_parser_internal() {
        let p = XlsxParser;
        // Valid ZIP magic but not a real XLSX (no xl/ directory).
        let empty_zip: &[u8] = &[
            0x50, 0x4B, 0x05, 0x06, // End of central directory signature
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        match p.parse("doc1", empty_zip) {
            Err(ParseError::ParserInternal(_)) => {}
            other => panic!("expected ParserInternal for invalid XLSX, got {other:?}"),
        }
    }
}
