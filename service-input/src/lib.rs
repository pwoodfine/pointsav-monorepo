// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `service-input` — Ring 1 generic document ingest.
//!
//! Per `~/Foundry/conventions/three-ring-architecture.md`,
//! `service-input` is a per-tenant boundary-ingest service that
//! accepts files of supported formats at the per-tenant boundary,
//! dispatches them to format-specific parsers, normalises the
//! parsed payload, and writes the result through `service-fs`
//! (Ring 1 WORM ledger; per the L2 `LedgerBackend` trait ratified
//! in `~/Foundry/conventions/worm-ledger-design.md`).
//!
//! ADR-07: zero AI in Ring 1. Parsing is deterministic. Format
//! detection is by extension and magic-byte sniffing only.
//!
//! This commit lands the crate scaffold per `service-input/NEXT.md`
//! Right-now: trait + dispatch table + format-detection skeleton.
//! Format-specific parsers are wired in subsequent commits as they
//! attach (PDF: oxidize-pdf; DOCX: docx-rust; XLSX: calamine;
//! Markdown: pulldown-cmark per `~/Foundry/SLM-STACK.md` §3.4).
//!
//! End-to-end shape (post-parser-wiring):
//!
//! ```text
//!   bytes + filename hint
//!         │
//!         ▼
//!   Format detection (extension → magic bytes)
//!         │
//!         ▼
//!   Parser::parse(bytes) → ParsedDocument
//!         │
//!         ▼
//!   service-fs L3 wire: POST /v1/append with the ParsedDocument
//!   (per-tenant moduleId header)
//! ```

pub mod docx;
pub use docx::DocxParser;
pub mod markdown;
pub use markdown::MarkdownParser;
pub mod pdf;
pub use pdf::PdfParser;

use serde::{Deserialize, Serialize};

/// Supported ingest formats. Expansion is demand-driven, not
/// completeness-driven — add a variant only when a customer use
/// case surfaces it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Format {
    Pdf,
    Docx,
    Xlsx,
    Markdown,
}

/// Normalised parsed-document representation. The shape Ring 2
/// (`service-extraction`) expects to read back from `service-fs`.
///
/// Today this is a minimal envelope; richer structure (per-page
/// segments, table extraction, embedded-image references) lands
/// as parsers attach. Backwards-compatibility on the schema is
/// doctrinal — Ring 2's deterministic parser combinators rely on
/// stable field names.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ParsedDocument {
    /// Source format the parser produced this from.
    pub format: Format,
    /// Caller-supplied identifier (e.g., source-document id).
    /// Forwarded to `service-fs`'s `payload_id` field on append.
    pub source_id: String,
    /// Plain-text body extracted from the source. Always present;
    /// empty string for formats with no extractable text.
    pub text: String,
    /// Per-format structured metadata (page count, sheet count,
    /// markdown headings, etc.). serde_json::Value to keep the
    /// schema flexible during the parser-wiring phase.
    pub metadata: serde_json::Value,
}

/// Parser errors. Today minimal; expansion lands with each parser.
#[derive(Debug)]
pub enum ParseError {
    /// Caller supplied bytes that do not match the declared format
    /// (e.g., a `.pdf` file that fails the PDF magic-byte check).
    FormatMismatch {
        declared: Format,
        reason: String,
    },
    /// Caller supplied an unsupported format (no Parser registered
    /// for it).
    UnsupportedFormat(Format),
    /// Format detection found no match for the supplied bytes +
    /// filename hint.
    FormatUndetected,
    /// Parser-internal error from the underlying parsing crate.
    /// String today; structured per-parser errors land with each
    /// parser.
    ParserInternal(String),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::FormatMismatch { declared, reason } => {
                write!(f, "format mismatch: declared {declared:?}; {reason}")
            }
            ParseError::UnsupportedFormat(fmt) => {
                write!(f, "unsupported format: {fmt:?} (no Parser registered)")
            }
            ParseError::FormatUndetected => {
                write!(f, "format detection failed (no extension match; no magic-byte match)")
            }
            ParseError::ParserInternal(msg) => write!(f, "parser internal error: {msg}"),
        }
    }
}

impl std::error::Error for ParseError {}

/// The L2 contract every format-specific parser implements.
///
/// Object-safe: methods take `&self` and return concrete types so
/// the dispatcher can hold `Box<dyn Parser + Send + Sync>` per
/// format. This is the same shape as `service-fs`'s
/// `LedgerBackend` trait — pluggable backends behind a stable
/// trait surface, swap at startup, no wire-layer changes.
pub trait Parser {
    /// Parse the supplied bytes into a normalised `ParsedDocument`.
    /// `source_id` is forwarded into the result envelope.
    fn parse(&self, source_id: &str, bytes: &[u8]) -> Result<ParsedDocument, ParseError>;
}

/// Dispatcher — owns a per-format parser registry. Operates as a
/// builder during construction, then becomes immutable for the
/// daemon's lifetime.
pub struct Dispatcher {
    pdf: Option<Box<dyn Parser + Send + Sync>>,
    docx: Option<Box<dyn Parser + Send + Sync>>,
    xlsx: Option<Box<dyn Parser + Send + Sync>>,
    markdown: Option<Box<dyn Parser + Send + Sync>>,
}

impl Default for Dispatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl Dispatcher {
    pub fn new() -> Self {
        Self {
            pdf: None,
            docx: None,
            xlsx: None,
            markdown: None,
        }
    }

    pub fn with_pdf(mut self, parser: Box<dyn Parser + Send + Sync>) -> Self {
        self.pdf = Some(parser);
        self
    }

    pub fn with_docx(mut self, parser: Box<dyn Parser + Send + Sync>) -> Self {
        self.docx = Some(parser);
        self
    }

    pub fn with_xlsx(mut self, parser: Box<dyn Parser + Send + Sync>) -> Self {
        self.xlsx = Some(parser);
        self
    }

    pub fn with_markdown(mut self, parser: Box<dyn Parser + Send + Sync>) -> Self {
        self.markdown = Some(parser);
        self
    }

    /// Dispatch to the parser registered for `format`.
    pub fn dispatch(
        &self,
        format: Format,
        source_id: &str,
        bytes: &[u8],
    ) -> Result<ParsedDocument, ParseError> {
        let parser = match format {
            Format::Pdf => self.pdf.as_deref(),
            Format::Docx => self.docx.as_deref(),
            Format::Xlsx => self.xlsx.as_deref(),
            Format::Markdown => self.markdown.as_deref(),
        };
        match parser {
            Some(p) => p.parse(source_id, bytes),
            None => Err(ParseError::UnsupportedFormat(format)),
        }
    }

    /// Convenience — detect format from filename + magic bytes,
    /// then dispatch.
    pub fn dispatch_with_detection(
        &self,
        filename: &str,
        source_id: &str,
        bytes: &[u8],
    ) -> Result<ParsedDocument, ParseError> {
        let format = detect_format(filename, bytes).ok_or(ParseError::FormatUndetected)?;
        self.dispatch(format, source_id, bytes)
    }
}

/// Detect a `Format` from a filename + magic-byte hint.
/// Extension match first (cheap, usually correct); magic-byte
/// fallback when extension is absent or ambiguous.
///
/// Per ADR-07: deterministic; no AI; no model inference.
pub fn detect_format(filename: &str, bytes: &[u8]) -> Option<Format> {
    if let Some(format) = detect_by_extension(filename) {
        return Some(format);
    }
    detect_by_magic(bytes)
}

fn detect_by_extension(filename: &str) -> Option<Format> {
    let lower = filename.to_ascii_lowercase();
    if lower.ends_with(".pdf") {
        Some(Format::Pdf)
    } else if lower.ends_with(".docx") {
        Some(Format::Docx)
    } else if lower.ends_with(".xlsx") {
        Some(Format::Xlsx)
    } else if lower.ends_with(".md") || lower.ends_with(".markdown") {
        Some(Format::Markdown)
    } else {
        None
    }
}

fn detect_by_magic(bytes: &[u8]) -> Option<Format> {
    // PDF: starts with "%PDF-"
    if bytes.starts_with(b"%PDF-") {
        return Some(Format::Pdf);
    }
    // DOCX and XLSX are both ZIP-format containers (Open Packaging
    // Conventions); ZIP starts with "PK\x03\x04". Distinguishing
    // DOCX vs XLSX without inspecting container internals is
    // ambiguous from magic bytes alone — this fallback returns
    // None for ZIP, deferring to the extension path. A more
    // thorough detection (read the [Content_Types].xml entry) lands
    // when the parsers are wired and we have the unzip surface
    // available.
    if bytes.starts_with(b"PK\x03\x04") {
        return None;
    }
    // Markdown has no reliable magic. Defer to extension match;
    // pure-text fallback is a future heuristic.
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_extension_pdf() {
        assert_eq!(detect_format("foo.pdf", b""), Some(Format::Pdf));
        assert_eq!(detect_format("Foo.PDF", b""), Some(Format::Pdf));
    }

    #[test]
    fn detect_extension_docx() {
        assert_eq!(detect_format("foo.docx", b""), Some(Format::Docx));
    }

    #[test]
    fn detect_extension_xlsx() {
        assert_eq!(detect_format("foo.xlsx", b""), Some(Format::Xlsx));
    }

    #[test]
    fn detect_extension_markdown() {
        assert_eq!(detect_format("foo.md", b""), Some(Format::Markdown));
        assert_eq!(detect_format("foo.markdown", b""), Some(Format::Markdown));
    }

    #[test]
    fn detect_magic_pdf_no_extension() {
        let bytes = b"%PDF-1.7\nsome content";
        assert_eq!(detect_format("untitled", bytes), Some(Format::Pdf));
    }

    #[test]
    fn detect_unknown_returns_none() {
        assert_eq!(detect_format("foo.bin", b"\x00\x01\x02"), None);
    }

    #[test]
    fn detect_zip_is_ambiguous_without_extension() {
        let bytes = b"PK\x03\x04rest of zip";
        // No extension hint and ZIP magic alone cannot distinguish
        // DOCX vs XLSX — detection returns None.
        assert_eq!(detect_format("untitled", bytes), None);
        // With an extension, the extension wins.
        assert_eq!(detect_format("untitled.docx", bytes), Some(Format::Docx));
    }

    /// A test parser that just records what it was called with —
    /// stand-in for the real format-specific parsers landing in
    /// later commits.
    struct EchoParser;

    impl Parser for EchoParser {
        fn parse(&self, source_id: &str, bytes: &[u8]) -> Result<ParsedDocument, ParseError> {
            Ok(ParsedDocument {
                format: Format::Markdown, // unused in this test
                source_id: source_id.to_string(),
                text: String::from_utf8_lossy(bytes).to_string(),
                metadata: serde_json::json!({}),
            })
        }
    }

    #[test]
    fn dispatch_to_registered_parser() {
        let d = Dispatcher::new().with_markdown(Box::new(EchoParser));
        let r = d
            .dispatch(Format::Markdown, "doc1", b"# hello")
            .unwrap();
        assert_eq!(r.source_id, "doc1");
        assert_eq!(r.text, "# hello");
    }

    #[test]
    fn dispatch_unsupported_format() {
        let d = Dispatcher::new();
        match d.dispatch(Format::Pdf, "doc1", b"%PDF-1.7") {
            Err(ParseError::UnsupportedFormat(Format::Pdf)) => {}
            other => panic!("expected UnsupportedFormat, got {other:?}"),
        }
    }

    #[test]
    fn dispatch_with_detection_uses_extension() {
        let d = Dispatcher::new().with_markdown(Box::new(EchoParser));
        let r = d
            .dispatch_with_detection("doc.md", "doc1", b"# hello")
            .unwrap();
        assert_eq!(r.text, "# hello");
    }

    #[test]
    fn dispatch_with_detection_undetected() {
        let d = Dispatcher::new();
        match d.dispatch_with_detection("untitled", "doc1", b"\x00\x01") {
            Err(ParseError::FormatUndetected) => {}
            other => panic!("expected FormatUndetected, got {other:?}"),
        }
    }

    #[test]
    fn dispatcher_routes_pdf_and_markdown_independently() {
        // Both parsers registered; each format routes to its parser.
        let d = Dispatcher::new()
            .with_pdf(Box::new(PdfParser))
            .with_markdown(Box::new(MarkdownParser));

        // Markdown parses successfully.
        let r = d
            .dispatch(Format::Markdown, "md1", b"# Hello\n\nBody text.")
            .unwrap();
        assert_eq!(r.format, Format::Markdown);
        assert!(r.text.contains("Hello"));

        // PDF with invalid bytes returns ParserInternal — the parser is registered
        // and ran, but oxidize-pdf rejected the bytes as an invalid PDF header.
        match d.dispatch(Format::Pdf, "bad", b"not a real pdf") {
            Err(ParseError::ParserInternal(_)) => {}
            other => panic!("expected ParserInternal for invalid PDF bytes, got {other:?}"),
        }

        // DOCX has no parser registered — returns UnsupportedFormat.
        match d.dispatch(Format::Docx, "doc1", b"PK\x03\x04") {
            Err(ParseError::UnsupportedFormat(Format::Docx)) => {}
            other => panic!("expected UnsupportedFormat for DOCX, got {other:?}"),
        }
    }
}
