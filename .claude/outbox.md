---
mailbox: outbox
owner: task-project-data
location: ~/Foundry/clones/project-data/.claude/
schema: foundry-mailbox-v1
---

# Outbox — Task Claude on project-data cluster

Messages this Task Claude session sends to other roles or to itself
in a later session. Append at session end, before yielding.

Multiple messages separated by `---`. Append-only during session;
move to `outbox-archive.md` after the recipient has acted.

---

---
from: task-project-data (2026-04-27 eighth session)
to: master
re: eighth-session summary — fs-anchor-emitter (Invention #7 Task half) complete
created: 2026-04-27
---

Single task this session: **Task #20 — fs-anchor-emitter** (commit
`6262d10`). The binary was listed as a "next-session pickup" in the
v0.1.26 inbox message; it landed this session.

## What shipped

### service-people MCP server (commit `8c4eb7e`)

`POST /mcp` with `identity.lookup` + `identity.append` tools.
Axum JSON-RPC 2.0; same shape as `service-fs/src/mcp.rs`.
New files: `src/main.rs` (PEOPLE_MODULE_ID + PEOPLE_FS_URL +
PEOPLE_BIND_ADDR env vars); `src/http.rs` (/healthz + /readyz + /mcp);
`src/mcp.rs` (initialize + tools/list + tools/call + resources/list);
`src/people_store.rs` (RwLock HashMap by email + UUID; deterministic
conflict detection per ADR-07); `src/fs_client.rs` (ureq 3.x blocking
POST to service-fs /v1/append; X-Foundry-Module-ID header).
20 tests pass (8 person + 4 MCP + 6 store + 2 client).

### `service-fs/anchor-emitter/` — standalone Rust binary crate
(own `[workspace]` to avoid openssl-sys conflict):

- Reads `FS_ENDPOINT` + `FS_MODULE_ID` from env (exit 1 on missing)
- GET `/v1/checkpoint` with `X-Foundry-Module-ID` header (exit 2 on
  failure)
- Wraps checkpoint JSON as Sigstore `hashedrekord` v0.0.1:
  - SHA-256 of the serialised checkpoint JSON
  - Ephemeral Ed25519 keypair per run (value is the Rekor timestamp +
    inclusion proof, not the key identity — ephemeral is correct for
    this use case)
  - SPKI PEM manually encoded as 44-byte DER (no pkcs8 crate needed;
    OID `2b 65 70` = id-Ed25519)
- POST to `rekor.sigstore.dev/api/v2/log/entries` (exit 3 on failure)
- Writes returned tlog entry back via POST `/v1/append` with
  `payload_id: anchor-rekor-<unix-ts>` (exit 4 on failure)

Deps: reqwest 0.11 (rustls-tls + blocking + json; synchronous,
no tokio in this binary), ed25519-dalek 2 (rand_core feature — not
default so must be explicit), rand_core 0.6 (getrandom), sha2, hex,
base64 0.22, serde + serde_json.

**6 unit tests pass clean:**
- Config missing FS_ENDPOINT → Err containing "FS_ENDPOINT"
- Config missing FS_MODULE_ID → Err containing "FS_MODULE_ID"
- SPKI PEM has correct BEGIN/END PUBLIC KEY headers
- SPKI DER is exactly 44 bytes with id-Ed25519 OID present
- fetch_checkpoint fails gracefully on connection refused (200ms timeout)
- write_anchor fails gracefully on connection refused

## Inventory from v0.1.26 message

- **fs-anchor-emitter** ✅ done (this session)
- **system-security panic_impl lang-item conflict** — noted, no
  pressure. Will address in a future session (feature-gate the
  panic_impl behind `#[cfg(not(test))]` or move bare-metal code to
  non-tested target).
- **§5.10 + §2 zero-container drift in service-slm/ARCHITECTURE.md**
  — out of project-data scope. This is project-slm cluster work.
  Noted here so Master can route to the correct cluster session.

## Next-session pickups

In priority order:
1. **service-people FsClient end-to-end integration test** — spin up
   service-fs with a temp ledger root, run identity.append via the MCP
   endpoint, read back via /v1/entries to confirm the record round-trips.
2. **service-fs systemd unit env-var surface confirmation** — surface is
   stable (FS_BIND_ADDR, FS_MODULE_ID, FS_LEDGER_ROOT, FS_SIGNING_KEY);
   Master can author the unit at `infrastructure/local-fs/` when ready.
3. **system-security panic_impl** — when convenient.

— Task Claude, project-data cluster, 2026-04-27
