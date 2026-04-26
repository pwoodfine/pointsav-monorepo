# NEXT.md — service-fs

> Last updated: 2026-04-26
> Read at session start. Update before session end so the next
> session knows where to pick up.

---

## Right now

- **ADR-07 audit-log sub-ledger** per
  `~/Foundry/conventions/worm-ledger-design.md` §5 step 4. The
  `/v1/entries` handler currently logs reads only to `tracing::info!`.
  Persist every read-call to its own append-only sub-ledger at
  `<root>/<moduleId>/audit-log/` — a separate `LedgerBackend`
  instance (can be `PosixTileLedger` or `InMemoryLedger` behind the
  same trait). Each record: `{moduleId, request_id, since_cursor,
  entries_returned, timestamp_unix}`. The audit ledger is itself WORM
  via the same trait surface. Wire it into `AppState` and `http.rs`'s
  `entries()` handler; add a unit test that confirms at least one
  audit record per read call.

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

- 2026-04-26: **Step 3 checkpoint signing** per
  worm-ledger-design.md §5 step 3 (commit `b285259`). Deps added:
  `ed25519-dalek 2` + `base64 0.22`. `ledger.rs`: `signed_note_body`
  (C2SP signed-note format: `origin\ntree_size\nbase64(root_hash)\n\n`);
  `sign_checkpoint_body` (in-place signer for both backends);
  `verify_checkpoint_signature` (public function — Customer independent
  verification per Doctrine claim #28); `load_signing_key` (raw 32-byte
  seed file); `LedgerError::InvalidKey` + `LedgerError::SigningError`.
  `InMemoryLedger`: `open_with_signing_key` ctor + `checkpoint()` signs
  when key present. `PosixTileLedger`: `open()` gains optional
  `signing_key_path` param + `checkpoint()` signs when key present.
  `main.rs`: reads `FS_SIGNING_KEY` env (optional path to 32-byte seed)
  and passes to `PosixTileLedger::open`. `http.rs`: `ApiError` mapping
  covers new error variants. **22 unit tests pass clean** (18 prior +
  3 ledger signing tests + 1 posix_tile signing test).
- 2026-04-26: **L1 PosixTileLedger backend** per
  `~/Foundry/conventions/worm-ledger-design.md` §5 step 2. New
  `service-fs/src/posix_tile.rs` — persistent newline-delimited
  JSON log at `<root>/<moduleId>/log.jsonl`, D4 atomic-write
  discipline (write `.tmp` → fsync → rename → chmod 0o444),
  reload-on-open with chain integrity verification (returns
  `ChainTampered` if any record's stored hash diverges from the
  recomputed value). `LedgerBackend` trait grew by three methods:
  `checkpoint() -> Checkpoint` (linear-chain tip; `signature`
  field `None` today, populated in step 3); `verify_inclusion`
  + `verify_consistency` (chain segments as v0.1.x proofs;
  Merkle-tree upgrade is a follow-up that keeps the trait
  surface unchanged). Both `InMemoryLedger` and `PosixTileLedger`
  implement the full trait. `main.rs` swapped to construct
  `PosixTileLedger` by default. `http.rs` got `/v1/checkpoint`
  endpoint + extended `ApiError` to map all `LedgerError`
  variants to the right HTTP status (400/403/404/409/500). 7
  new tests on `PosixTileLedger`: durability across restart,
  checkpoint-after-restart consistency, chain extension after
  restart, tamper detection on reload, file-mode 0o444
  enforcement, empty-ledger checkpoint, verify_inclusion after
  restart. 11 trait-surface tests on `InMemoryLedger` cover
  checkpoint advance, inclusion success+failure, consistency
  success+failure, chain-origin stability. Total **18 unit
  tests pass clean**.
- 2026-04-26: **L2 trait extraction** per
  `~/Foundry/conventions/worm-ledger-design.md` §5 step 1. Factored
  `WormLedger` struct into `LedgerBackend` trait + `InMemoryLedger`
  impl. Trait carries today's three methods (append / read_since /
  root); the convention's checkpoint + verify_* methods land in
  step 2 with the POSIX backend (where they have real semantics).
  `http.rs` `AppState` now holds `Box<dyn LedgerBackend + Send +
  Sync>` so the wire layer is backend-agnostic. `main.rs`
  constructs `InMemoryLedger` and boxes it. 3 existing unit tests
  ported to run against the trait surface (via
  `make_ledger() -> Box<dyn LedgerBackend>`) so the same suite
  exercises the future `PosixTileLedger`. cargo check + cargo test
  pass clean.
- 2026-04-26: workspace v0.1.7 ratified
  `~/Foundry/conventions/worm-ledger-design.md` (`6c0b79a`); D1–D9
  ratified explicitly with rationale per decision; D10 separately
  tracked. Workspace v0.1.6 ratified DOCTRINE §IX External WORM
  standards alignment subsection (`ecee9fb`). service-fs/SECURITY.md
  + ARCHITECTURE.md headers upgraded from "proposed" to "ratified".
- 2026-04-26: research synthesis `service-fs/RESEARCH.md`
  committed — ~600 lines synthesising Foundry-side material
  (DOCTRINE §IX, MEMO §6.3 + §7, three-ring + zero-container
  conventions), industry standards (SEC 17a-4(f) 2022 amendment,
  eIDAS qualified preservation 2026/01, SOC 2 TSC), and modern
  verifiable-log architecture (Trillian-Tessera, Sigstore Rekor
  v2, RFC 9162 v2 tile-based CT). Proposed four-layer design
  with C2SP tlog-tiles + signed-note checkpoints; dual-target
  Linux daemon + seL4 Microkit unikernel; per-tenant moduleId
  enforcement at the WORM layer; integration with DOCTRINE
  Invention #7 monthly Rekor anchoring. Ten ratification
  decisions surfaced to Master via outbox
  `worm-ledger-design-convention-proposal`; recommended that the
  design land as workspace-tier convention
  `~/Foundry/conventions/worm-ledger-design.md`.
- 2026-04-26: Tokio MCP-server skeleton landed (commit
  `af73232`) —
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
