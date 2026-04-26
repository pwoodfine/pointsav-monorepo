// SPDX-License-Identifier: Apache-2.0 OR MIT

//! PDF parser via [oxidize-pdf] — pure-Rust PDF parsing, no
//! external runtimes, no AI inference (per ADR-07).
//!
//! `oxidize-pdf` (v2.x) currently only opens PDFs by file path —
//! its `PdfReader::open` does not accept an in-memory byte slice.
//! v0.1.x of `PdfParser` shims around that by writing the input
//! bytes to a uniquely-named temporary file under
//! `std::env::temp_dir()`, calling `PdfReader::open`, then
//! deleting the temp file in a Drop guard. When `oxidize-pdf`
//! adds a bytes-based open API (or we migrate to a different
//! crate that already does), this shim collapses to a direct
//! call without changing the `Parser` trait surface.
//!
//! Format detection is the dispatcher's responsibility (per
//! `crate::detect_format` — extension first, magic-byte fallback).
//! By the time `PdfParser::parse` is called, the bytes have
//! been declared as PDF; this parser does not re-validate the
//! magic header (extension + dispatcher have already filtered).
//! It does, however, surface the underlying parser's error if the
//! bytes turn out to be malformed.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::{Format, ParseError, ParsedDocument, Parser};

pub struct PdfParser;

impl Default for PdfParser {
    fn default() -> Self {
        Self::new()
    }
}

impl PdfParser {
    pub fn new() -> Self {
        PdfParser
    }
}

impl Parser for PdfParser {
    fn parse(&self, source_id: &str, bytes: &[u8]) -> Result<ParsedDocument, ParseError> {
        // Write bytes to a temp file with a unique name. The Drop
        // guard cleans up even on early-return / panic.
        let tmp = TempPdfFile::new(bytes)
            .map_err(|e| ParseError::ParserInternal(format!("temp file write failed: {e}")))?;

        // oxidize-pdf 2.x file-path open
        let reader = oxidize_pdf::parser::PdfReader::open(tmp.path())
            .map_err(|e| ParseError::ParserInternal(format!("PdfReader::open: {e}")))?;
        let doc = oxidize_pdf::parser::PdfDocument::new(reader);

        let pages = doc
            .extract_text()
            .map_err(|e| ParseError::ParserInternal(format!("extract_text: {e}")))?;

        let page_count = pages.len();
        let text = pages
            .iter()
            .map(|p| p.text.as_str())
            .collect::<Vec<_>>()
            .join("\n\n");

        Ok(ParsedDocument {
            format: Format::Pdf,
            source_id: source_id.to_string(),
            text,
            metadata: serde_json::json!({
                "page_count": page_count,
                "parser": "oxidize-pdf",
            }),
        })
    }
}

/// RAII temp file. Created on `new`, deleted on `Drop`. Names are
/// process-id + monotonic counter to avoid collisions across
/// concurrent parses.
struct TempPdfFile {
    path: PathBuf,
}

static TEMP_CTR: AtomicU64 = AtomicU64::new(0);

impl TempPdfFile {
    fn new(bytes: &[u8]) -> std::io::Result<Self> {
        let n = TEMP_CTR.fetch_add(1, Ordering::SeqCst);
        let path = std::env::temp_dir().join(format!(
            "service-input-pdf-{}-{}.pdf",
            std::process::id(),
            n
        ));
        std::fs::write(&path, bytes)?;
        Ok(Self { path })
    }

    fn path(&self) -> &std::path::Path {
        &self.path
    }
}

impl Drop for TempPdfFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Invalid bytes (not a PDF) should produce a `ParserInternal`
    /// error rather than panic. Exercises the temp-file write +
    /// oxidize-pdf error propagation path without needing a
    /// known-good PDF fixture.
    #[test]
    fn invalid_bytes_yields_parser_internal_error() {
        let p = PdfParser::new();
        let result = p.parse("doc1", b"not a pdf at all");
        match result {
            Err(ParseError::ParserInternal(msg)) => {
                assert!(
                    !msg.is_empty(),
                    "ParserInternal message should not be empty"
                );
            }
            Err(other) => panic!(
                "expected ParserInternal, got {other:?}"
            ),
            Ok(doc) => panic!(
                "expected error for non-PDF bytes; got OK: {doc:?}"
            ),
        }
    }

    /// Even a malformed PDF that starts with the PDF magic header
    /// should not panic — it should return a structured error.
    #[test]
    fn malformed_pdf_with_magic_header_does_not_panic() {
        let p = PdfParser::new();
        // Valid magic but no actual PDF structure
        let result = p.parse("doc1", b"%PDF-1.7\nbut not really a pdf\n");
        match result {
            Err(ParseError::ParserInternal(_)) => {}
            other => panic!("expected ParserInternal, got {other:?}"),
        }
    }

    /// Happy-path test against a real minimal PDF fixture. Confirms
    /// that text extraction succeeds and page_count is correct.
    /// The fixture is a hand-crafted 1-page PDF with Helvetica text
    /// ("Hello World") in a BT...ET content stream.
    #[test]
    fn happy_path_minimal_pdf_parses() {
        let bytes = include_bytes!("../tests/fixtures/minimal.pdf");
        let parser = PdfParser::new();
        let doc = parser
            .parse("fixture-1", bytes)
            .expect("minimal.pdf must parse without error");
        assert!(
            !doc.text.is_empty(),
            "extracted text must be non-empty for a PDF with a content stream"
        );
        assert!(
            doc.metadata["page_count"].as_u64().unwrap_or(0) >= 1,
            "page_count must be >= 1"
        );
    }
}
