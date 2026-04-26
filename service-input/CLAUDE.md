# CLAUDE.md — service-input

> **State:** Active  —  **Last updated:** 2026-04-25
> **Version:** 0.0.1  (per `~/Foundry/CLAUDE.md` §7 and DOCTRINE.md §VIII)
> **Registry row:** `pointsav-monorepo/.claude/rules/project-registry.md`
>
> When state changes, update this header AND the registry row in the
> same commit. Drift between the two is a documentation defect.
>
> Per-commit: bump PATCH; tag `vservice-input-MAJOR.MINOR.PATCH`
> annotated and SSH-signed; commit message ends with
> `Version: M.m.P` trailer; `CHANGELOG.md` records one line per PATCH.

---

## What this project is

Ring 1 boundary-ingest service for generic document intake. Accepts
files of supported formats at the per-tenant boundary, dispatches
them to format-specific parsers, normalises the parsed payload, and
writes through `service-fs` into the per-tenant WORM Immutable
Ledger. Sibling to `service-people` (identity ingest) and
`service-email` (Communications Ledger). Read downstream by
`service-extraction` (Ring 2) via MCP wire protocol.

## Current state

Newly created 2026-04-25; previously did not exist in the cluster
or the project registry. Activation transitions it directly from
Reserved-folder to Active because the parser dispatcher is the
entire next workstream and we want per-project `CLAUDE.md` /
`NEXT.md` discipline in place before any code lands.

No code yet. The directory contains only `README.md` and
`README.es.md` (bilingual per `~/Foundry/CLAUDE.md` §6). The next
session in this cluster lands the initial Cargo crate skeleton and
the parser-dispatcher trait. Cargo workspace membership is a
separate decision tracked in NEXT.md.

## Build and test

```
cargo check    # standalone (service-input is workspace-excluded
               # alongside service-fs while the openssl-sys Layer 1
               # audit issue lives in a sibling member)
cargo test     # runs the 11 unit tests covering format detection +
               # dispatcher behaviour
```

## File layout

```
service-input/
├── Cargo.toml            — crate manifest; serde + serde_json today;
│                           parser crates (oxidize-pdf, docx-rust,
│                           calamine, pulldown-cmark) added as each
│                           parser is wired
├── README.md, README.es.md — bilingual overview
├── CLAUDE.md, NEXT.md
└── src/
    └── lib.rs            — Format enum, ParsedDocument struct,
                            ParseError enum, Parser trait,
                            Dispatcher (per-format registry +
                            dispatch + dispatch_with_detection),
                            detect_format (extension-first, magic-
                            byte fallback). 11 unit tests cover
                            extension detection, magic-byte
                            detection, ambiguous ZIP cases,
                            dispatch + UnsupportedFormat behaviour.
                            No format-specific parsers wired yet.
```

## Hard constraints — do not violate

- **ADR-07: zero AI in Ring 1.** Parsing is deterministic. No
  LLM-assisted text extraction, no embedding-model normalisation,
  no AI-driven format detection. Format detection is by extension
  and magic-byte sniffing only.
- **WORM via `service-fs` only.** This crate does not persist to
  disk directly. Every parsed payload is written through
  `service-fs`'s MCP interface so the append-only invariant lives
  at one boundary.
- **Per-tenant boundary.** One process per `moduleId` (per
  `~/Foundry/conventions/three-ring-architecture.md`). No
  cross-tenant routing.
- **Format coverage starts narrow.** Initial four parsers per
  `SLM-STACK.md` §3.4: oxidize-pdf, docx-rust, calamine,
  pulldown-cmark. Expansion needs a NEXT.md item naming the
  customer use case driving it; not "for completeness."

## Dependencies on other projects

- Writes to: `service-fs` (Ring 1, this cluster) — every parsed
  payload goes here.
- Read by: `service-extraction` (Ring 2, `project-slm` cluster) —
  reads ledger entries via MCP.
- Future: customer-extension parsers may plug in as additional
  parser adapters behind the same trait, no fork of this crate
  needed.

## What not to do

- Do not import `anthropic`, `openai`, `candle-core` (for
  inference), or any other AI/ML inference dependency. Ring 1 is
  zero-AI by ADR-07.
- Do not write directly to disk. The WORM invariant lives in
  `service-fs`; bypassing it breaks append-only enforcement.
- Do not add a parser for a format until a customer use case
  surfaces it. Format coverage is driven by demand, not by
  speculative completeness.

---

## Inherited rules — do not duplicate, do not silently override

This project inherits rules from two parent scopes. Do NOT copy
their content into this file; reference them.

- **Repo-level:** `pointsav-monorepo/CLAUDE.md` (when added; the
  monorepo does not yet carry a repo-level `CLAUDE.md` — see
  `~/Foundry/NEXT.md` Stage 4) — prefix taxonomy, canonical names,
  ADR hard rules (SYS-ADR-07, -10, -19), Do-Not-Use vocabulary,
  bilingual README rule, BCSC / Sovereign Data Foundation
  disclosure.
- **Workspace-level:** `~/Foundry/CLAUDE.md` — identity store,
  commit flow (`bin/commit-as-next.sh`), promotion flow
  (`bin/promote.sh`), authoritative-document priority, rules of
  engagement.

If a rule at this level conflicts with an inherited rule, **stop
and surface the conflict** — do not silently override.
