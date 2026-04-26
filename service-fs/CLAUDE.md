# CLAUDE.md — service-fs

> **State:** Active  —  **Last updated:** 2026-04-26
> **Version:** 0.0.1  (per `~/Foundry/CLAUDE.md` §7 and DOCTRINE.md §VIII)
> **Registry row:** `pointsav-monorepo/.claude/rules/project-registry.md`
>
> When state changes, update this header AND the registry row in the
> same commit. Drift between the two is a documentation defect.
>
> Per-commit: bump PATCH; tag `vservice-fs-MAJOR.MINOR.PATCH`
> annotated and SSH-signed; commit message ends with
> `Version: M.m.P` trailer; `CHANGELOG.md` records one line per PATCH.

---

## What this project is

Ring 1 boundary-ingest service: the WORM (Write-Once-Read-Many)
Immutable Ledger that all other Ring 1 services (`service-people`,
`service-email`, `service-input`) write through. Per
`~/Foundry/conventions/three-ring-architecture.md`, every persisted
artefact in the per-tenant data plane lands in `service-fs` first;
Ring 2 (`service-extraction`, in the `project-slm` cluster) reads
from it as an MCP client.

## Current state

**Tokio MCP-server skeleton landed 2026-04-26 (this commit).**
Drift closed.

History: an earlier seL4-unikernel scaffold (`#![no_std]
#![no_main]` with a hand-rolled `_start` entrypoint) was at
`src/main.rs` — surfaced as drift in the cluster outbox
`ring1-scaffold-runtime-model-drift` on 2026-04-26 because it
contradicted the ratified `three-ring-architecture.md` (Ring 1 =
MCP-server processes) and `zero-container-runtime.md` (every
deployment is a Linux binary under systemd). Master ratified
three decisions the same day:

1. **Decision 1** — replace with hosted Tokio MCP-server
   skeleton. Done in this commit.
2. **Decision 2** — relocate the seL4 scaffold to
   `vendor-sel4-fs/` (Reserved-folder; joins the seL4 lineage
   alongside `vendor-sel4-kernel` and `moonshot-sel4-vmm`).
   Done in commit `7519390`.
3. **Decision 3** — hold workspace membership until the rewrite
   compiles clean. Rewrite compiles (`cargo check`) and the 3
   ledger tests pass; re-add to workspace `[members]` is blocked
   on a separate Layer 1 audit issue (workspace-level
   `cargo check --workspace` surfaces an `openssl-sys` system-
   dep missing from a sibling member, unrelated to service-fs).
   Service-fs sits in workspace `[exclude]` for now; tracked in
   NEXT.md.

Code state: minimal but real. The Tokio runtime is up, axum
router exposes `/healthz`, `/readyz`, `/v1/contract`,
`/v1/append`, and `/v1/entries`. The WORM ledger primitive in
`src/ledger.rs` is in-memory (placeholder for hash-addressed
segment files in immutable directories — first NEXT.md item).
Three unit tests enforce the append-only / monotonic-cursor
invariant at the ledger API surface.

## Build and test

```
cargo check    # standalone (service-fs is workspace-excluded)
cargo test     # runs the 3 ledger invariant tests in src/ledger.rs
```

Workspace-level commands (`cargo check --workspace` from repo
root) skip service-fs because it's currently in workspace
`[exclude]`. Re-adding to `[members]` requires the unrelated
`openssl-sys` Layer 1 audit issue to be closed at repo tier.

## File layout

```
service-fs/
├── Cargo.toml             — Tokio + axum + serde + tracing + anyhow
├── Cargo.lock             — checked in (binary crate; reproducible
│                            build target)
├── README.md              — English overview (bilingual pair added
│                            2026-04-26 alongside the rewrite)
├── README.es.md           — Spanish overview (bilingual pair)
├── CLAUDE.md              — this file
├── NEXT.md                — work queue
└── src/
    ├── main.rs            — Tokio entrypoint; reads FS_BIND_ADDR,
    │                        FS_MODULE_ID, FS_LEDGER_ROOT from env;
    │                        spins axum on the bind addr
    ├── http.rs            — axum router + endpoint handlers;
    │                        per-tenant moduleId enforcement on
    │                        /v1/append and /v1/entries; ApiError
    │                        type wraps internal errors with HTTP
    │                        status + JSON body
    └── ledger.rs          — WormLedger primitive; append-only
                             invariant enforced at API surface;
                             3 unit tests; in-memory storage
                             placeholder pending the segment-file
                             swap (first NEXT.md item)
```

## Hard constraints — do not violate

- **ADR-07 zero-AI in Ring 1.** No AI model calls, no LLM
  inference, no embedding-model use anywhere in this crate. Any
  enrichment that needs intelligence happens in Ring 2 / Ring 3
  via wire protocol, not in-process here.
- **Append-only invariant.** When the rewrite scaffold lands, no
  code path may delete, rewrite, or mutate a previously-persisted
  ledger entry. Tests must enforce this at the API surface.
- **Per-tenant boundary.** Each `service-fs` instance serves one
  `moduleId` (per `three-ring-architecture.md` §moduleId
  discipline). Cross-tenant reads/writes are not in scope for this
  service.
- **Do not re-introduce bare-metal framing.** `#![no_std]`,
  `#![no_main]`, hand-rolled `_start`, target overrides to
  `x86_64-unknown-none` — none belong in service-fs. That lineage
  is `vendor-sel4-fs/` and the seL4 family of crates.

## Dependencies on other projects

Consumed by:
- `service-people` (Ring 1, this cluster) — writes identity records.
- `service-email` (Ring 1, this cluster) — writes archived messages.
- `service-input` (Ring 1, this cluster) — writes ingested documents.
- `service-extraction` (Ring 2, `project-slm` cluster) — reads
  ledger contents to produce structured graphs.

## What not to do

- Do not pull in AI/ML inference dependencies (candle, anthropic
  client, openai client, etc.). ADR-07 keeps Ring 1 zero-AI;
  even diagnostic-only inclusion of an inference dep is the kind
  of slow drift that becomes load-bearing.
- Do not write directly to disk paths outside `FS_LEDGER_ROOT`.
  All persistence is rooted under the operator-supplied ledger
  directory.
- Do not silently broaden the moduleId enforcement (e.g., "if
  header is missing, fall back to FS_MODULE_ID"). Explicit
  rejection is the per-tenant-boundary discipline; soft fallbacks
  hide misrouted clients.

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
and surface the conflict** — do not silently override. Conflict
resolution is an architectural decision, not an implementation
choice.
