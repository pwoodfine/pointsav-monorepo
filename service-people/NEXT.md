# NEXT.md — service-people

> Last updated: 2026-04-27 (session 7)
> Read at session start. Update before session end so the next
> session knows where to pick up.

---

## Right now

- **End-to-end integration test with service-fs** — the current
  `src/fs_client.rs` uses ureq 3.x to POST to service-fs `/v1/append`.
  Next session: spin up both `service-fs` (on 127.0.0.1:9100) and
  `service-people` (on 127.0.0.1:9300) with matching `moduleId`; POST
  `/mcp` with `identity.append` tool call; confirm the Person record
  lands in the WORM ledger and can be read back via `GET /v1/entries`.
  This closes the Ring 1 pipeline from identity input to persisted WORM.
  Integration test: add a `tests/e2e_fs_integration.rs` that spins up
  both services and exercises the full append-lookup cycle.

## Queue

- **`people-acs-engine/` absorption.** The email-anchoring binary's
  logic (UUIDv5 anchor derivation, Claim JSONL) now lives in
  `src/person.rs`. Consider folding the binary into a `src/cmd/`
  submodule or retiring the standalone binary once the MCP tool
  surface covers the same use case.
- **Append integration with `service-fs`** — identity record writes
  flow through the WORM ledger via `FsClient` (same as
  `service-input/src/fs_client.rs`). This crate never persists
  directly.
- **Deterministic entity-resolution rules** — canonical-key matching
  only (ADR-07; no AI). Surfaces ambiguity to the operator (per
  ADR-10 / F12), does not silently merge.
- **`spatial-ledger/` retirement.** Superseded once the MCP +
  service-fs WORM append pipeline is live end-to-end.
- **`spatial-crm/` retirement.** Cross-ring coupling — writes directly
  to `service-slm/transient-queues`. Retire when service-extraction
  covers the regex extraction use-case.
- **`service-people.py` + `ledger_personnel.json` retirement.**
  Pre-framework Python script and placeholder seed data; retire once
  the Rust MCP service has real schema + real data.

## Deferred

- Cross-tenant identity sharing — Deferred: out of scope for Ring
  1 by `~/Foundry/conventions/three-ring-architecture.md`. If it
  ever lands, it lives in Ring 2 / Ring 3.
- Embedding-based fuzzy identity matching — Deferred (and
  doctrinally constrained): ADR-07 keeps Ring 1 zero-AI.
  Fuzzy matching, if needed, runs in Ring 2 with a deterministic
  read-only contract.

## Recently done

- 2026-04-27: **MCP server interface** (`src/mcp.rs`, `src/http.rs`,
  `src/main.rs`, `src/fs_client.rs`, `src/people_store.rs`). `POST /mcp`
  JSON-RPC 2.0 endpoint with `identity.append` (name + primary_email +
  optional aliases + organisation → Person → FsClient → service-fs
  `/v1/append` + local PeopleStore cache) and `identity.lookup` (email or
  UUID → Person). `PeopleStore`: in-process RwLock HashMap index by email
  + UUID; deterministic conflict detection (ADR-07). `FsClient`: ureq 3.x
  blocking POST with `X-Foundry-Module-ID` header. Deps: axum 0.7 + tokio
  + tracing + ureq 3.3 + anyhow. Env vars: `PEOPLE_MODULE_ID` (required),
  `PEOPLE_FS_URL` (required), `PEOPLE_BIND_ADDR` (default 127.0.0.1:9300).
  **12 new tests** pass (4 MCP protocol + 6 PeopleStore + 2 FsClient).
  Total: **20 tests** (8 person + 12 MCP/store/client).

- 2026-04-27: **canonical person-record schema** (`src/person.rs`).
  `Person` struct with `id` (UUIDv5 from primary_email, matching
  `people-acs-engine/` convention), `name`, `primary_email`,
  `email_aliases`, `organisation`, `created_at`, `updated_at`. Serde
  Serialize + Deserialize; chrono `DateTime<Utc>`; builder pattern
  (`with_alias`, `with_organisation`). Deps added to `Cargo.toml`:
  `serde`, `serde_json`, `chrono` (serde feature), `uuid` (v4+v5+serde).
  `src/lib.rs` re-exports `Person`. **8 unit tests pass clean**
  (deterministic ID, email normalisation, ACS-engine UUID parity,
  alias builder, organisation builder, JSON round-trip, key presence,
  null-organisation serialisation). `cargo test -p service-people` green.

- 2026-04-26: **pre-framework subdirectory inventory complete.**
  Five subdirectories + two root artefacts assessed; decisions:
  | Item | Decision |
  |---|---|
  | `people-acs-engine/` | **Keep** — deterministic email-anchoring via UUIDv5; well-structured Anchor/Claim JSONL schema; informs Identity Ledger design; eventually fold into service-people library. |
  | `spatial-ledger/` | **Keep** — batch ledger-writer that generates `substrate/ledger_personnel.jsonl` from `discovery-queue/`. Precursor to WORM append pipeline. Retire once MCP + service-fs integration is live. |
  | `spatial-crm/` | **Retire-pending** — cross-ring coupling (writes to `service-slm/transient-queues` directly, violating Ring 1 boundary). Regex extraction functionality superseded by service-extraction (Ring 2). Retire when service-extraction is wired. |
  | `substrate/` | **Runtime data container** — `ledger_personnel.jsonl` (9 real identity records from OpenStack ML) untracked from git and gitignored this session. Physical directory remains for the running service. |
  | `tools/` | **Relocated** — `extract-people-ledger.sh` moved to `scripts/` per repo-layout.md; `tools/` directory removed. Done this session. |
  | `service-people.py` | **Retire-pending** — pre-framework dual-timezone campaign-contact script; different domain from identity ledger. Retire alongside schema work. |
  | `ledger_personnel.json` | **Retire-pending** — placeholder seed contacts (WMC-001/002/003). Retire once real schema and data land. |

- 2026-04-25: project activated per `~/Foundry/CLAUDE.md` §9 —
  this CLAUDE.md, this NEXT.md, and the registry row created in
  one commit; existing sub-directories left in place for
  inventory in the next session.
