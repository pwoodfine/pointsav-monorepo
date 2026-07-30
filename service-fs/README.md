# service-fs

[ 🇪🇸 Leer este documento en Español ](./README.es.md)

Ring 1 boundary-ingest service: the per-tenant WORM
(Write-Once-Read-Many) Immutable Ledger that other Ring 1
services (`service-people`, `service-email`, `service-input`)
write through. Downstream Ring 2 consumers (`service-extraction`)
read from the ledger; they never touch the originating service
directly.

## Position in the architecture

- **Ring:** 1 (Boundary Ingest, per-tenant) — see
  `~/Foundry/conventions/three-ring-architecture.md`.
- **Tenancy:** one process per `moduleId`; per-tenant boundary
  enforced both by infrastructure (separate processes) and by
  request-time `X-Foundry-Module-ID` header check.
- **Runtime:** hosted Tokio + axum HTTP-server under systemd
  (per `~/Foundry/conventions/zero-container-runtime.md`).
- **Wire protocol:** JSON over HTTP today; MCP-server interface
  layered on top per the Ring 1 contract (next NEXT.md item after
  `cargo check` passes clean).

## Hard rules

- **ADR-07: zero AI in Ring 1.** No LLM inference, no
  embedding-based filtering, no AI-assisted normalisation.
- **Append-only invariant.** No public API mutates or deletes a
  previously-persisted entry. Tests in `src/ledger.rs` enforce
  the invariant at the API surface.
- **Per-tenant boundary.** `FS_MODULE_ID` env is required; cross-
  tenant requests are rejected with 403.

## State

Active since 2026-04-25 (`ee209e3`). Hosted Tokio MCP-server
skeleton landed 2026-04-26 per Master Decision 1, replacing the
prior bare-metal seL4 unikernel scaffold (relocated to
`vendor-sel4-fs/` per Master Decision 2). Workspace membership
held until `cargo check` passes clean (Master Decision 3).

## Endpoints

| Method | Path | Purpose |
|---|---|---|
| GET | `/healthz` | Liveness; always 200 |
| GET | `/readyz` | Readiness; 200 once the ledger is open |
| GET | `/v1/contract` | Service version, moduleId, ledger root |
| POST | `/v1/append` | Append a payload; returns assigned cursor |
| GET | `/v1/entries?since=N` | Ring 2 read surface; cursor-paged |

`X-Foundry-Module-ID` header is required on `/v1/append` and
`/v1/entries`; mismatch with `FS_MODULE_ID` returns 403.

## Standards & compliance posture

`service-fs` targets two external WORM standards plus the SOC 2
Trust Services Criteria most relevant to immutable storage:

- **SEC Rule 17a-4(f)** (US, broker-dealer electronic
  recordkeeping; 2022 amendment effective 2023-05-03) — WORM
  path, not the Audit-Trail alternative.
- **eIDAS qualified preservation service** (EU; Commission
  Implementing Regulation 2025/1946 in force 2026-01-06; ETSI
  TS 119 511 v1.2.1; ETSI EN 319 401 v3.2.1; CEN TS 18170:2025).
- **SOC 2 TSC** — CC6 (Logical Access), CC7 (System Operations),
  PI1 (Processing Integrity Inputs), PI4 (Processing Integrity
  Outputs).

Plus Foundry-internal: WORM legal compliance per MEMO §6.3 line
194; DARP per DOCTRINE §IX; ADR-07 zero-AI in Ring 1; Pillar 1
plain text only; Pillar 2 100-year readability; Invention #7
monthly Sigstore Rekor anchoring.

Full posture in `SECURITY.md`. What is NOT promised today (no
formal SOC 3 attestation, no eIDAS designation) is stated
explicitly there.

## Architecture

Four-layer stack — **L1** tile storage (POSIX today,
capability-mediated `moonshot-database` long-term); **L2** WORM
Ledger Rust trait (target-independent contract); **L3** wire
protocol (axum HTTP today, MCP-server layered on top); **L4**
monthly Sigstore Rekor anchoring (workspace-tier per Invention
#7).

Two boot envelopes share the same wire protocol and same tile
format: **Envelope A** Linux/BSD daemon under systemd (today);
**Envelope B** seL4 Microkit Protection Domain unikernel
(long-term Totebox Archive native).

Full overview in `ARCHITECTURE.md`. Full synthesis with
alternatives considered + ten ratification decisions for Master
in `RESEARCH.md`.

## Build and test

```
cargo check    # verify the skeleton type-checks against deps
cargo test     # run the append-only + cursor invariant tests in
               # src/ledger.rs
```

## Licence

Refer to the repo `LICENSE` file. Component-level licence
assignment is governed by `pointsav/factory-release-engineering`'s
`LICENSE-MATRIX.md`.

## See also

- `SECURITY.md` — compliance posture (SEC 17a-4(f), eIDAS, SOC 2)
- `ARCHITECTURE.md` — four-layer stack overview
- `RESEARCH.md` — full synthesis, alternatives, ratification
  decisions
- `CLAUDE.md` — operational state, hard constraints
- `NEXT.md` — work queue
- `~/Foundry/conventions/three-ring-architecture.md` — Ring 1
  contract
- `~/Foundry/conventions/zero-container-runtime.md` — deployment
  shape
- `~/Foundry/MEMO-2026-03-30-Development-Overview-V8.md` §6.3
  (service-fs role) + §7 (moonshot trajectory toward seL4
  unikernel + moonshot-database)
- `~/Foundry/DOCTRINE.md` §IX (SOC 2 / DARP posture); §II.7
  (Invention #7 Integrity Anchor)
- `vendor/pointsav-monorepo/service-slm/crates/slm-doorman-server/`
  (project-slm cluster) — reference shape for the Tokio + axum
  pattern
- `vendor-sel4-fs/` — sibling Reserved-folder housing the
  relocated bare-metal scaffold; not a Ring 1 substitute
