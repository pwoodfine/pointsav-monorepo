# NEXT.md — service-input

> Last updated: 2026-04-26
> Read at session start. Update before session end so the next
> session knows where to pick up.

---

## Right now

- **service-people pre-framework subdirectory inventory.** Five
  subdirectories exist inside `service-people/` that pre-date the
  project framework: `sovereign-acs-engine/`, `spatial-crm/`,
  `spatial-ledger/`, `substrate/`, `tools/`. Inventory each —
  determine whether it is keep-as-is, rename, retire, or relocate —
  and record the decision in `service-people/NEXT.md` (and the
  registry if relocation changes project count).

## Queue
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

- 2026-04-26: **service-input happy-path PDF test fixture** added.
  Generated `tests/fixtures/minimal.pdf` — a hand-crafted 614-byte
  PDF 1.4 document with correct xref table, one page, a Helvetica
  Type1 font, and a BT...ET content stream containing "Hello World".
  Added `happy_path_minimal_pdf_parses` test to `src/pdf.rs` using
  `include_bytes!` — asserts `text` is non-empty and `metadata
  ["page_count"] >= 1`. **30 tests pass clean** (29 prior + 1 new).
- 2026-04-26: **service-input MCP server interface** wired. New
  `service-input/src/mcp.rs` and `service-input/src/http.rs`. MCP
  handler mounted at `POST /mcp` (JSON-RPC 2.0 Streamable HTTP per 2026
  spec). Tool `document.ingest` (arguments: `filename`, `source_id`,
  `bytes_base64`) — base64-decodes bytes, calls `detect_format`, dispatches
  to the right parser, calls `FsClient::submit`, returns `{ cursor,
  source_id, format }`. Also exposes `initialize`, `tools/list`,
  `resources/list` per MCP spec. `X-Foundry-Module-ID` enforcement on
  all requests (JSON-RPC error, not 403, per MCP client contract). New
  `service-input/src/main.rs` daemon entrypoint reads `INPUT_MODULE_ID`
  (required), `INPUT_FS_URL` (required), `INPUT_BIND_ADDR` (default
  0.0.0.0:9200). Deps added to main [dependencies]: axum 0.7, tokio
  rt-multi-thread+macros+net, tracing, tracing-subscriber, base64 0.22.
  Dev-deps: service-fs path dep moved from [dev-dependencies] to
  [dev-dependencies] with tower 0.4. **5 MCP tests pass** (initialize,
  tools/list, tools/call with transport error, unknown module_id, unknown
  format). **29 unit + integration tests pass clean** (24 prior + 5 MCP).
- 2026-04-26: **service-input → service-fs HTTP client** wired. New
  `service-input/src/fs_client.rs` — `FsClient { base_url, module_id }`
  with `submit(&self, doc: &ParsedDocument) -> Result<u64, FsClientError>`.
  POSTs `{ payload_id: doc.source_id, payload: serde_json::to_value(doc) }`
  to `POST /v1/append` with `X-Foundry-Module-ID` header. Blocking I/O
  via ureq 3.3 (json feature). `FsClientError` distinguishes
  `Serialization` / `Transport` / `StatusError { status }` /
  `ResponseParse`. Integration test (`submit_appends_to_service_fs_and_
  returns_cursor_ge_1`) spins up a real service-fs axum server on port 0
  using a background thread + dedicated tokio runtime; asserts cursor ≥ 1.
  Re-exported as `service_input::FsClient` + `FsClientError`. Dev-deps
  added: `service-fs = { path = "../service-fs" }`, `tokio = { version =
  "1", features = ["rt-multi-thread"] }`, `axum = "0.7"`. **24 unit +
  integration tests pass clean** (23 prior + 1 FsClient integration).
- 2026-04-26: **XlsxParser via calamine 0.34** wired. New
  `service-input/src/xlsx.rs` — `XlsxParser` implementing the
  `Parser` trait. Uses `open_workbook_from_rs(Cursor::new(bytes))` —
  no temp-file shim (calamine's reader API). Magic-byte check rejects
  non-ZIP input with `FormatMismatch`. Text extracted by iterating all
  sheets via `Reader::worksheet_range`, all rows, all cells; cells
  space-separated per row, rows newline-separated; `Data::Empty`
  cells skipped; unreadable sheets produce an inline `[sheet 'X'
  error: ...]` line rather than aborting. Metadata: `sheet_count`,
  `sheets` (name array), `parser: "calamine"`. Re-exported as
  `service_input::XlsxParser` from `lib.rs`. **23 unit tests pass
  clean** (21 prior + 2 XlsxParser).
- 2026-04-26: **DocxParser via docx-rust 0.1.11** wired. New
  `service-input/src/docx.rs` — `DocxParser` implementing the
  `Parser` trait. Uses `DocxFile::from_reader(Cursor::new(bytes))`
  (no temp-file shim — docx-rust's reader API accepts any
  `Read + Seek`). Magic-byte check: rejects non-ZIP input with
  `FormatMismatch` early; ZIP-but-invalid-DOCX returns
  `ParserInternal` from the underlying parser. Text extracted via
  `document.body.text()`; `paragraph_count` + `parser: "docx-rust"`
  in metadata. Re-exported as `service_input::DocxParser` from
  `lib.rs`. **21 unit tests pass clean** (19 prior + 2 DocxParser).
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
