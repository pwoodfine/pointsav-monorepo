# NEXT.md — service-input

> Last updated: 2026-04-25
> Read at session start. Update before session end so the next
> session knows where to pick up.

---

## Right now

- **Wire the second parser — Markdown via `pulldown-cmark`.** With
  PDF landed (cryptic/binary, requires temp-file shim, error-path
  tests only), Markdown is the natural next pickup: pure-text input,
  no temp-file needed, full happy-path testing trivial, and lets the
  Dispatcher prove out the multi-parser case. Implement
  `MarkdownParser` per the same shape as `PdfParser` (impl
  `Parser` trait; struct in `src/markdown.rs`; re-export from
  `lib.rs`). Add tests covering: (a) small markdown sample →
  ParsedDocument with text preserved, (b) heading extraction into
  `metadata` (pulldown-cmark exposes events for heading
  starts/ends), (c) Dispatcher with both PdfParser and
  MarkdownParser registered routes correctly.

## Queue

- Wire the remaining two parsers per `~/Foundry/SLM-STACK.md` §3.4:
  - DOCX: `docx-rust`
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
