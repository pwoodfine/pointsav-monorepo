# NEXT.md — service-fs

> Last updated: 2026-04-26
> Read at session start. Update before session end so the next
> session knows where to pick up.

---

## Right now

- Swap the `WormLedger` storage backend from in-memory `Vec<Entry>`
  (placeholder, but invariants enforced and tests pass) to
  hash-addressed segment files in immutable directories rooted at
  `FS_LEDGER_ROOT`. The API surface (`open`, `append`,
  `read_since`, `root`) is the contract that survives the swap;
  callers and HTTP handlers should not need to change. Add a
  reload-from-disk path to `WormLedger::open` so the daemon can
  restart without losing state.

## Queue

- Layer the MCP-server interface on top of the existing JSON-over-
  HTTP routes per `~/Foundry/conventions/three-ring-architecture.md`
  §"MCP boundary at Ring 1": MCP resources for ledger reads
  (`/v1/entries`), MCP tools for append (`/v1/append`). Reference
  the Anthropic/Cloudflare 2026 MCP spec; the JSON shapes already
  match closely. The Tokio + axum surface stays; MCP is a layered
  protocol on top.
- Persist the ADR-07 audit log (currently just `tracing::info!`
  on every read) to its own append-only file alongside the ledger
  segments. Format: one JSON record per line with moduleId,
  request-id, since-cursor, entry count, timestamp.
- Re-add `service-fs` to root `Cargo.toml` `[workspace.members]`.
  Blocked on: pre-existing Layer 1 audit issue —
  `cargo check --workspace` currently fails on `openssl-sys`
  system-dep missing from a sibling member, unrelated to
  service-fs. Tracked at repo tier in
  `.claude/rules/cleanup-log.md` 2026-04-18 entry.
- systemd unit file (`infrastructure/service-fs/service-fs.service`
  shape, modelled on
  `infrastructure/local-slm/local-slm.service` v0.0.11):
  workspace-tier work, but coordinate via Master outbox so the
  unit file matches the daemon's env-var contract
  (`FS_BIND_ADDR`, `FS_MODULE_ID`, `FS_LEDGER_ROOT`).
- Round-trip test fixture: hit `/v1/append`, then `/v1/entries`,
  assert payload identity. Belongs as an integration test in
  `tests/` (not unit — exercises the full HTTP + ledger stack).
- `discovery-queue/` cleanup — registry has it as Not-a-project,
  noted as "gitignore + move to `service-fs/data/`" since the
  pre-framework era. With service-fs now a real hosted service,
  decide whether the runtime-data destination still makes sense
  (probably yes once the segment-file storage lands; deferred
  until then).

## Blocked

- Re-add to workspace `[members]` — Blocked on: pre-existing
  `openssl-sys` Layer 1 audit issue in a sibling member.

## Deferred

- Outbound replication / federation — Deferred: the Compounding
  Substrate's federated marketplace pattern operates at adapter
  layer (Ring 3 LoRA exchange), not at Ring 1 raw-ledger layer.
  Cross-tenant ledger replication is structurally out of scope.
- Streaming chunked append for very large payloads — Deferred:
  current shape buffers the JSON value in-memory before append.
  Real workload threshold has not been hit.

## Recently done

- 2026-04-26: Tokio MCP-server skeleton landed (this commit) —
  `Cargo.toml` (axum + tokio + serde + tracing + anyhow); `src/`
  with `main.rs` (env-driven entrypoint), `http.rs` (axum router
  with /healthz, /readyz, /v1/contract, /v1/append, /v1/entries
  + per-tenant moduleId enforcement + ApiError type), `ledger.rs`
  (WormLedger primitive, append-only invariant, 3 unit tests
  passing); bilingual READMEs (the project never had them);
  CLAUDE.md drift section closed; reference shape was
  slm-doorman-server (project-slm cluster `78031c4`).
- 2026-04-26: seL4 scaffold relocated from `service-fs/` to
  `vendor-sel4-fs/` per Master Decision 2 (commit `7519390`).
  Four files moved via `git mv` (main.rs, .cargo/config.toml,
  Cargo.toml, Cargo.lock); package name updated in transit;
  registry row added for `vendor-sel4-fs` as Reserved-folder.
- 2026-04-26: workspace `[exclude]` updated to keep service-fs
  out of `[members]` per Master Decision 3 until rewrite passes
  clean. Rewrite passes clean; re-add blocked on the unrelated
  Layer 1 audit issue (above).
- 2026-04-25: project activated per `~/Foundry/CLAUDE.md` §9
  (commit `ee209e3`); drift surfaced 2026-04-26 (00:10 UTC
  outbox); Master ratified 2026-04-26 07:20 UTC (inbox).
