# NEXT.md — service-input

> Last updated: 2026-04-26
> Read at session start. Update before session end so the next
> session knows where to pick up.

---

## Right now

- **Wire the DOCX parser via `docx-rust`.** PDF and Markdown
  parsers are wired and 19 tests pass. DOCX is the next pickup:
  `service-email` attachment payloads are predominantly DOCX, so
  this unblocks the `service-email` → `service-input` integration
  path. Implement `DocxParser` following the same `Parser` trait
  shape as `PdfParser` and `MarkdownParser` (`struct DocxParser` in
  `src/docx.rs`; re-exported from `lib.rs` as
  `service_input::DocxParser`). DOCX is a ZIP-format container
  (Open Packaging Convention) containing `word/document.xml`;
  `docx-rust` provides a `DocxFile` API. The ZIP magic-byte
  detection in `detect_by_magic` can be refined to peek at the
  `[Content_Types].xml` entry once the docx crate's unzip surface
  is available — or leave as a follow-up behind the extension path.
  Add tests: (a) valid DOCX bytes → `ParsedDocument` with
  non-empty text, (b) invalid bytes → `ParserInternal` error.

## Queue

- Wire the remaining two parsers per `~/Foundry/SLM-STACK.md` §3.4:
  - XLSX: `calamine`
- Wire `service-fs` integration — once at least one parser is
  working, add a thin client that holds the parsed `ParsedDocument`
  and `POST /v1/append`s to the configured `service-fs` URL with
  the per-tenant `X-Foundry-Module-ID` header. Initially via
  `service-fs`'s JSON-over-HTTP wire (today's surface); when
  `service-fs`'s MCP-server interface lands per worm-ledger-design
  §5 step 5, swap to MCP-client semantics.
- MCP server interface — expose ingest as a tool; one moduleId per
  process (per `~/Foundry/conventions/three-ring-architecture.md`
  §moduleId discipline).
- Add `service-input` as a workspace member in the monorepo root
  `Cargo.toml` once the crate compiles. Coordinate with the open
  Layer 1 audit finding (`.claude/rules/cleanup-log.md` 2026-04-18
  entry) — workspace under-declaration is not a problem this
  session creates, but adding a new member is the natural moment
  to surface it again.
- Round-trip test fixture — small known-good document of each
  format, parse, write through a stub `service-fs`, read back, hash
  the payload. Confirms determinism end-to-end.

## Blocked

- Workspace `Cargo.toml` membership — Blocked on: monorepo's
  workspace under-declaration is a separate cleanup tracked at
  repo level (`.claude/rules/cleanup-log.md` 2026-04-18). The new
  crate can land standalone first; member declaration follows.

## Deferred

- Additional format parsers beyond the initial four — Deferred:
  format coverage is demand-driven, not completeness-driven. Add
  one only when a customer use case surfaces it.
- Streaming / chunked ingest for very large files — Deferred:
  parser crates above accept whole-buffer input; chunked variant
  waits for a real workload that exceeds memory.

## Recently done

- 2026-04-26: **MarkdownParser via pulldown-cmark 0.12** wired. New
  `service-input/src/markdown.rs` — `MarkdownParser` implementing
  the `Parser` trait. Pure-text input; no temp-file shim needed.
  pulldown-cmark events used to extract: all text runs (including
  bold/italic/code spans) into `text_buf`; heading text into a
  `headings` metadata list; line breaks as `\n`. Returns
  `ParsedDocument` with full text and `{headings, parser:
  "pulldown-cmark"}` metadata. Re-exported as
  `service_input::MarkdownParser` from `lib.rs`. Multi-parser
  integration test `dispatcher_routes_pdf_and_markdown_independently`
  added to `lib.rs` — confirms PDF and Markdown dispatch
  independently (Markdown returns text; PDF with invalid bytes
  returns `ParserInternal`). **19 unit tests pass clean**
  (11 dispatcher + 2 PdfParser + 5 MarkdownParser + 1 multi-parser).
- 2026-04-26: **PdfParser via oxidize-pdf 2.x** wired. New
  `service-input/src/pdf.rs` — `PdfParser` implementing the
  `Parser` trait. Shims around oxidize-pdf's file-path-only API
  by writing input bytes to a uniquely-named temp file under
  `std::env::temp_dir()` with an RAII Drop guard. Returns
  `ParsedDocument` with extracted text + per-page metadata
  (page_count, parser="oxidize-pdf"). Tests cover invalid-bytes
  + malformed-PDF error paths; happy-path test deferred until a
  known-good PDF fixture is checked in (no fixture today —
  generating one with a write-side library is a future
  refinement). Re-exported as `service_input::PdfParser` from
  `lib.rs`.
- 2026-04-26: **parser-dispatcher initial scaffold** landed.
  `Cargo.toml` (serde + serde_json today; format-specific parsers
  added as each is wired). `src/lib.rs` carries `Format` enum
  (Pdf / Docx / Xlsx / Markdown), `ParsedDocument` struct,
  `ParseError` enum, `Parser` trait (object-safe — `Box<dyn
  Parser + Send + Sync>` per format), `Dispatcher` (per-format
  registry with builder API + `dispatch` + `dispatch_with_detection`),
  and `detect_format` (extension-first; magic-byte fallback for
  PDF; ZIP-magic ambiguity between DOCX and XLSX deliberately
  defers to extension match). 11 unit tests cover detection +
  dispatch behaviour. cargo check + cargo test pass clean.
  service-input added to root `Cargo.toml` `[exclude]` alongside
  service-fs (same blocker: workspace-level openssl-sys breakage
  in a sibling member).
- 2026-04-25: project created (directory + bilingual READMEs +
  registry row Reserved-folder), then activated (this CLAUDE.md +
  NEXT.md + registry row → Active) per `~/Foundry/CLAUDE.md` §9 in
  two consecutive commits.
