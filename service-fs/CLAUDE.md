# CLAUDE.md — service-fs

> **State:** Active  —  **Last updated:** 2026-04-25
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

**Activation drift surfaced — do not propagate.** The existing
scaffold at `src/main.rs` is `#![no_std] #![no_main]` with a
hand-rolled `_start` entrypoint and a panic handler that loops —
i.e., a bare-metal seL4 unikernel framing. That contradicts the
ratified architecture (2026-04-25) on two counts:

1. `~/Foundry/conventions/three-ring-architecture.md` §"MCP boundary
   at Ring 1": Ring 1 services are MCP-server processes; "each
   service exposes a stable wire protocol, not a Rust API."
2. `~/Foundry/conventions/zero-container-runtime.md`: every Foundry
   deployment runs as "a Linux binary under systemd on a plain VM
   or bare-metal host." A bare-metal seL4 unikernel is not that
   shape.

The unikernel scaffold is likely earlier work that belongs in a
future seL4-related project (the registry already carries
`vendor-sel4-kernel` and `moonshot-sel4-vmm` as scaffold work for
that lineage). **Operator decision 2026-04-25:** leave the scaffold
file untouched in the activation commit; the eventual MCP-server
scaffold for `service-fs` is paused pending Master Claude's
ratification of the rewrite plan (see cluster outbox message
`ring1-scaffold-runtime-model-drift`).

Activation snapshot:
- Per-project `CLAUDE.md` (this file) — present.
- Per-project `NEXT.md` — present.
- Registry row — Active.
- Code state — single 26-line stub from prior bare-metal framing;
  zero working Ring 1 functionality.

## Build and test

No build step yet — pending the ratified MCP-server scaffold. The
existing `Cargo.toml` declares no dependencies and the
`#![no_std] #![no_main]` binary will not link as part of a hosted
workspace member without removing the bare-metal attributes. Do
not attempt `cargo check` inside this directory until the rewrite
is ratified.

## File layout

```
service-fs/
├── Cargo.toml             — pre-rewrite stub; "Bare-Metal Unikernel"
│                            description; zero dependencies
├── Cargo.lock             — pre-rewrite stub
├── .cargo/config.toml     — pre-rewrite stub
└── src/main.rs            — 26-line no_std/no_main scaffold
                             (KEEP UNTOUCHED until rewrite ratified)
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
- **Do not modify `src/main.rs` until Master Claude ratifies the
  unikernel-vs-MCP rewrite plan.** The file is documentation of
  existing-scaffold drift, not active scaffolding.

## Dependencies on other projects

Consumed by:
- `service-people` (Ring 1, this cluster) — writes identity records.
- `service-email` (Ring 1, this cluster) — writes archived messages.
- `service-input` (Ring 1, this cluster) — writes ingested documents.
- `service-extraction` (Ring 2, `project-slm` cluster) — reads
  ledger contents to produce structured graphs.

## What not to do

- Do not "fix" the seL4 scaffold by adding bare-metal logic. The
  scaffold's runtime model conflicts with the ratified Ring 1
  architecture; further bare-metal work cements drift.
- Do not begin the MCP-server rewrite until Master ratifies the
  plan. The cluster outbox message
  `ring1-scaffold-runtime-model-drift` is the gate.

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
