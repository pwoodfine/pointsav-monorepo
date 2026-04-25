# NEXT.md — service-input

> Last updated: 2026-04-25
> Read at session start. Update before session end so the next
> session knows where to pick up.

---

## Right now

- Scaffold the initial Cargo crate skeleton — `Cargo.toml` with
  `[package]` block, `src/lib.rs` exposing the parser-dispatcher
  trait. No format parsers wired up yet; trait first, dispatch
  table second, parsers third.

## Queue

- Define the parser-dispatcher trait — input is a byte slice + a
  hint (filename or MIME), output is the normalised structured
  payload that `service-fs` will accept on its MCP `append`
  surface.
- Add format detection — extension first, magic-byte fallback. No
  AI; deterministic only (ADR-07).
- Wire the four initial parsers per `~/Foundry/SLM-STACK.md` §3.4:
  - PDF: `oxidize-pdf`
  - DOCX: `docx-rust`
  - XLSX: `calamine`
  - Markdown: `pulldown-cmark`
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

- 2026-04-25: project created (directory + bilingual READMEs +
  registry row Reserved-folder), then activated (this CLAUDE.md +
  NEXT.md + registry row → Active) per `~/Foundry/CLAUDE.md` §9 in
  two consecutive commits.
