# NEXT.md — service-fs

> Last updated: 2026-04-25
> Last updated: 2026-04-27
> Last updated: 2026-05-27 (session 5)
> Last updated: 2026-05-29 (session 6)
> Read at session start. Update before session end so the next
> session knows where to pick up.

---

## Right now

- Surface seL4-unikernel-vs-MCP scaffold drift to Master Claude via
  cluster outbox (`~/Foundry/clones/project-data/.claude/outbox.md`,
  subject `ring1-scaffold-runtime-model-drift`). Activation has
  landed; the rewrite waits on Master ratification.
- **systemd unit file** — `infrastructure/local-fs/` (workspace-tier;
  coordinate via Master outbox so the unit matches the daemon's
  env-var contract: `FS_BIND_ADDR`, `FS_MODULE_ID`, `FS_LEDGER_ROOT`,
  `FS_SIGNING_KEY`). Master owns this per the action matrix; Task role
  here is to confirm the env-var surface before Master authors the unit.
  The surface is stable as of 2026-04-26.

## Queue

- Once Master ratifies the rewrite plan: replace the no_std bare-
  metal scaffold with a hosted Ring 1 MCP-server skeleton — Tokio
  async runtime, per-tenant moduleId isolation, append-only API
  surface.
- Add the crate as a workspace member in the root `Cargo.toml` (it
  is not currently declared a member; see Layer 1 audit finding in
  `.claude/rules/cleanup-log.md` 2026-04-18 entry).
- Storage layout for the ledger — likely hash-addressed segment
  files in immutable directories. Decision deferred until the MCP
  API surface is fixed (the wire protocol drives the storage shape,
  not the other way around).
- Append-only invariant tests at the API surface — no path mutates
  a previously-persisted entry.
- ADR-07 audit hook: every Ring 2 caller's read is logged with
  moduleId + timestamp + opaque-cursor for downstream auditing.
- MCP server interface per Anthropic / Cloudflare 2026 reference —
  resources for "ledger reads", tools for "append".

## Blocked

- All scaffold-replacement work — Blocked on: Master Claude
  ratification of the rewrite plan in response to outbox message
  `ring1-scaffold-runtime-model-drift`.
Nothing pending in Totebox scope. Master owns the systemd timer
for `anchor-emitter/` (monthly Rekor anchoring, Doctrine Invention #7).

## Queue

- `discovery-queue/` cleanup — registry has it as Not-a-project;
  move to `service-fs/data/` deferred until segment-file storage lands.
  Operator decision pending.
- **criterion benchmarks** — measure append throughput (entries/sec,
  bytes/sec under sustained load), checkpoint latency (time to sign a
  checkpoint over N entries), and Rekor round-trip time (anchor-emitter
  invocation to tlog writeback confirmed). Route results to
  project-editorial as a JOURNAL-NOTES-j2 addendum once available.
  Required for J2 submission data section (J2 ASPLOS submission;
  J5 on HOLD pending J2).

## Deferred

- seL4 bare-metal file-system work — Deferred: belongs in a future
  seL4-related project alongside `vendor-sel4-kernel` /
  `moonshot-sel4-vmm`, not in this Ring 1 service. The current
  `src/main.rs` is the surviving artefact of that earlier framing
  and should be relocated when that project opens.

## Recently done

- 2026-04-25: project activated per `~/Foundry/CLAUDE.md` §9 — this
  CLAUDE.md, this NEXT.md, and the registry row created in one
  commit; runtime-model drift surfaced in CLAUDE.md "Current state"
  rather than silently propagated.
- 2026-05-19 (Master): **`local-fs.service` deployed** on `127.0.0.1:9100`.
  `FS_MODULE_ID=foundry-workspace`, `FS_LEDGER_ROOT` configured. Confirmed
  healthy in session 3. Systemd unit at `infrastructure/local-fs/local-fs.service`.

- 2026-04-27: **`anchor-emitter/` Rust binary** (Doctrine Invention #7).
  New standalone crate at `service-fs/anchor-emitter/` (own `[workspace]`
  to avoid openssl-sys workspace conflicts). Reads `FS_ENDPOINT` +
  `FS_MODULE_ID`; GETs `/v1/checkpoint`; wraps the checkpoint JSON as a
  Sigstore `hashedrekord` v0.0.1 entry (SHA-256 artifact hash +
  ephemeral Ed25519 signing keypair per run; SPKI PEM encoding — manual
  44-byte DER, no pkcs8 dep needed); POSTs to
  `rekor.sigstore.dev/api/v2/log/entries`; writes the returned tlog entry
  back via `POST /v1/append` with `payload_id: anchor-rekor-<unix-ts>`.
  Deps: reqwest 0.11 (rustls-tls + blocking + json), ed25519-dalek 2
  (rand_core feature), rand_core 0.6 (getrandom), sha2, hex, base64 0.22,
  serde + serde_json. Exit codes 0/1/2/3/4. **6 unit tests pass clean**
  (config env-var absent × 2, SPKI headers + OID correctness, connection-
  refused error paths × 2). Intended to be invoked by a monthly systemd
  timer (Master-tier); binary is the Task-scoped half of Invention #7.
- 2026-04-26: **MCP-server interface layer** per
  three-ring-architecture.md §"MCP boundary at Ring 1". New
  `service-fs/src/mcp.rs` implements JSON-RPC 2.0 (Streamable HTTP
  transport, single JSON response) on top of the existing axum
  surface. `POST /mcp` added to the router in `http.rs`. Capabilities
  exposed: tool `ledger.append` (arguments: payload_id, payload;
  returns cursor via `content[0].text`); resource `ledger://entries`
  (URI: `ledger://entries?since=N`; returns entries as JSON text in
  `contents[0].text`). `initialize` announces both `tools` and
  `resources` capabilities. `X-Foundry-Module-ID` per-tenant
  enforcement carries through MCP (mismatch → JSON-RPC error, not a
  bare 403, so MCP clients get a protocol-level error). `lib.rs`
  exports `mcp` module. **5 new tests** cover: `initialize` returns
  capabilities; `tools/list` includes `ledger.append`;
  `tools/call` ledger.append returns cursor; `resources/read`
  returns appended entry; wrong module_id → RPC error.
  **30 tests total** (28 unit + 2 integration).
- 2026-04-26: **Round-trip integration tests** in `tests/round_trip.rs`.
  Two `#[tokio::test]` cases: (1) `append_then_entries_returns_payload` —
  POST `/v1/append` then GET `/v1/entries?since=0`, asserts payload
  identity (cursor, payload_id, and JSON payload bytes all round-trip
  unchanged) and `next_cursor` matches the last entry's cursor;
  (2) `entries_since_excludes_boundary` — two appends, then GET
  with `since=c1`, confirms only the second entry is returned (boundary
  is exclusive). Added `[lib]` target (`src/lib.rs`) exposing
  `router`, `AppState`, `posix_tile_open` helper for integration-test
  use; `main.rs` updated to import from the library instead of
  declaring modules directly. **25 tests pass total** (23 unit + 2
  integration).
- 2026-04-26: **Step 4 ADR-07 audit-log sub-ledger** per
  worm-ledger-design.md §5 step 4. `AppState` gained
  `audit_ledger: Box<dyn LedgerBackend + Send + Sync>`. The
  `entries()` handler now appends a JSON record to `audit_ledger`
  after every read: `{module_id, request_id, since_cursor,
  entries_returned, timestamp_unix}`. Audit-ledger failures are
  logged via `warn!` but not propagated — a failing audit log must
  not reject a read request. `main.rs` opens a second
  `PosixTileLedger` at `<ledger_root>/<moduleId>/audit-log/` (same
  D4 atomic-write discipline; no signing key wired to the audit
  ledger). `Cargo.toml` dev-dep: `tower = "0.4"` for the HTTP
  handler test. New test `http::tests::audit_records_each_entries_call`
  calls `GET /v1/entries` through the axum router via
  `tower::ServiceExt::oneshot`, then reads the audit ledger and
  confirms exactly one record with the correct fields. **23 unit
  tests pass clean** (22 prior + 1 new).
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
