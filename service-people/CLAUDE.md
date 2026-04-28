# CLAUDE.md — service-people

> **State:** Active  —  **Last updated:** 2026-04-25
> **Version:** 0.0.1  (per `~/Foundry/CLAUDE.md` §7 and DOCTRINE.md §VIII)
> **Registry row:** `pointsav-monorepo/.claude/rules/project-registry.md`
>
> When state changes, update this header AND the registry row in the
> same commit. Drift between the two is a documentation defect.
>
> Per-commit: bump PATCH; tag `vservice-people-MAJOR.MINOR.PATCH`
> annotated and SSH-signed; commit message ends with
> `Version: M.m.P` trailer; `CHANGELOG.md` records one line per PATCH.

---

## What this project is

Ring 1 boundary-ingest service: the per-tenant Identity Ledger.
Manages the canonical identity records (people, organisations,
roles, communication endpoints) that downstream services attach
events and documents to. Per
`~/Foundry/conventions/three-ring-architecture.md`, identity records
are persisted through `service-fs` (WORM) and read by Ring 2
services as MCP clients.

## Current state

Inventoried 2026-04-26. Pre-framework subdirectories assessed;
per-item decisions in `NEXT.md` Recently done section.

- `Cargo.toml` — minimal stub, no dependencies
- `src/lib.rs` — 3-line `system_status()` placeholder
- `service-people.py` — pre-framework Python; retire-pending
- `ledger_personnel.json` — placeholder seed contacts; retire-pending
- `people-acs-engine/` — Rust binary: email regex + UUIDv5
  Anchor/Claim JSONL; informs Identity Ledger schema; keep
- `spatial-ledger/` — Rust binary: batch ledger-writer from
  `discovery-queue/` → `substrate/ledger_personnel.jsonl`; keep
  until MCP + service-fs pipeline replaces it
- `spatial-crm/` — Rust binary: cross-ring entity extractor
  (writes to `service-slm/transient-queues`); retire-pending
- `substrate/` — runtime data directory; `*.jsonl` gitignored 2026-04-26
- `scripts/` — `extract-people-ledger.sh` (moved from `tools/`)

Next step (NEXT.md Right-now): define Identity Ledger schema,
building on the Anchor/Claim pattern in `people-acs-engine/`.

No drift flags at activation time: the existing scaffold is
near-empty Rust + adjacent prior-work artefacts, not bare-metal
unikernel framing (contrast `service-fs`'s drift). The runtime
model can move forward as a hosted MCP server per the ratified
architecture without a doctrine-level conflict.

## Build and test

`Cargo.toml` has no dependencies; `cargo check` inside this
directory will build the trivial `lib.rs` stub but exercises
nothing. Defer running until the schema and MCP surface are
defined.

## File layout

```
service-people/
├── Cargo.toml              — minimal stub (no dependencies yet)
├── README.md, README.es.md — bilingual overview
├── CLAUDE.md, NEXT.md
├── src/lib.rs              — 3-line system_status() placeholder
├── service-people.py       — pre-framework Python; retire-pending
├── ledger_personnel.json   — placeholder seed contacts; retire-pending
├── people-acs-engine/      — Rust binary; email-regex + UUIDv5
│                             Anchor/Claim; keep
├── spatial-ledger/         — Rust binary; discovery-queue → substrate JSONL
├── spatial-crm/            — Rust binary; retire-pending (cross-ring)
├── substrate/              — runtime data (*.jsonl gitignored)
└── scripts/
    └── extract-people-ledger.sh  — SSH+rsync substrate export
```

## Hard constraints — do not violate

- **ADR-07: zero AI in Ring 1.** No LLM-assisted entity
  resolution, no embedding-based identity matching, no AI-driven
  schema inference. Identity matching is deterministic
  (canonical-key based).
- **WORM via `service-fs`.** Identity records are persisted
  through `service-fs`'s MCP append surface. This crate does not
  write to disk directly.
- **Per-tenant boundary.** One process per `moduleId`. Cross-tenant
  identity sharing is out of scope for Ring 1; if it ever lands,
  it lives in Ring 2 / Ring 3.
- **Schema stability is doctrinal.** Once the Identity Ledger
  schema is published in a version, breaking changes require a
  MAJOR bump and migration plan — downstream Ring 2 services
  depend on it.

## Dependencies on other projects

- Writes to: `service-fs` (Ring 1, this cluster) — every identity
  record lands in the WORM ledger.
- Read by: `service-extraction` (Ring 2, `project-slm` cluster) —
  resolves contact/organisation references in extracted documents.
- Read by: `service-email` (Ring 1, this cluster) — attaches
  message senders/recipients to canonical identities.

## What not to do

- Do not begin schema work without first inventorying the existing
  sub-directories. They predate the framework; some may be
  abandoned, some may carry the right schema thinking. Decide
  per-subdirectory before reusing or replacing.
- Do not import AI/ML inference dependencies. ADR-07 applies.
- Do not duplicate Identity Ledger persistence inside this crate
  when the WORM ledger is `service-fs`. One persistence boundary,
  one append-only invariant.

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
