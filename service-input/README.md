# service-input

[ 🇪🇸 Leer este documento en Español ](./README.es.md)

Ring 1 boundary-ingest service for generic document intake. Accepts
files of supported formats (PDF, DOCX, XLSX, Markdown) at the
per-tenant boundary, normalises them, and writes the parsed payload
through `service-fs` into the WORM Immutable Ledger. Downstream Ring
2 consumers (`service-extraction`, `service-content`,
`service-search`) read from the ledger; they never touch the raw
document.

## Position in the architecture

- **Ring:** 1 (Boundary Ingest, per-tenant) — see
  `~/Foundry/conventions/three-ring-architecture.md`.
- **Writes to:** `service-fs` (WORM ledger).
- **Read by:** `service-extraction` (Ring 2) via MCP wire protocol.
- **Tenancy:** per-tenant; one process per `moduleId`.

## Format coverage (initial)

Per `~/Foundry/SLM-STACK.md` §3.4:

| Format | Crate |
|---|---|
| PDF (`.pdf`) | `oxidize-pdf` |
| Word (`.docx`) | `docx-rust` |
| Excel (`.xlsx`) | `calamine` |
| Markdown (`.md`) | `pulldown-cmark` |

Additional parsers are added as customer-facing formats appear; the
crate is structured so each parser is a pluggable adapter behind a
common ingest trait.

## Hard rules

- **ADR-07: zero AI in Ring 1.** Parsing is deterministic; no LLM
  inference, no embedding model, no AI-assisted normalisation.
- **WORM via `service-fs`.** This crate never persists directly to
  disk; every write goes through the `service-fs` MCP interface so
  the append-only invariant is enforced at one boundary.

## State

Reserved-folder. Created 2026-04-25 by Task Claude on the
`project-data` cluster. No code yet — the next session activates
the project per `~/Foundry/CLAUDE.md` §9 and adds the initial
parser-dispatcher skeleton.

## Licence

Refer to the repo `LICENSE` file. Component-level licence assignment
is governed by `pointsav/factory-release-engineering`'s
`LICENSE-MATRIX.md`.
