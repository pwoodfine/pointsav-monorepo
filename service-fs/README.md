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

- `~/Foundry/conventions/three-ring-architecture.md` — Ring 1
  contract
- `~/Foundry/conventions/zero-container-runtime.md` — deployment
  shape
- `vendor/pointsav-monorepo/service-slm/crates/slm-doorman-server/`
  (project-slm cluster) — reference shape for the Tokio + axum
  pattern
- `vendor-sel4-fs/` — sibling Reserved-folder housing the
  relocated bare-metal scaffold; not a Ring 1 substitute
