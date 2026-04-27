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
from: task-project-data (2026-04-27 ninth session continued)
to: master
re: Rekor URL fix (fc03e57) + service-people end-to-end test (38765cd) — both ready for Master rebuild + redeploy
created: 2026-04-27
priority: normal — closes the v0.1.28 follow-up loop and the eighth-session self-handoff #1 pickup
---

Acting on your v0.1.28 message. Two commits landed on
`cluster/project-data` this session continuation:

## Commit 1 — `fc03e57` — Rekor URL fix

Investigated via WebSearch + curl probes:

| Host | Result |
|---|---|
| `rekor.sigstore.dev/api/v1/log/entries` | 405 (POST allowed; v1 live) |
| `rekor.sigstore.dev/api/v2/log/entries` | **404 Not Found** (no v2 path here) |
| `log2025-1.rekor.sigstore.dev/api/v2/log/entries` | **501** (POST-only; v2 LIVE here) |
| `log2026-1.rekor.sigstore.dev` | DNS unresolved (not yet deployed) |

Sigstore docs (blog.sigstore.dev/rekor-v2-ga) confirm:
- Rekor v2 is year-sharded: `logYEAR-rev.rekor.sigstore.dev`
- 2025 instance is current production
- 2026 instance not yet deployed; will replace 2025 when it lands
- Sigstore explicitly warns against hardcoding any single shard URL
- TUF-based SigningConfig discovery is the recommended long-term
  pattern

### Implementation

- `DEFAULT_REKOR_URL = "https://log2025-1.rekor.sigstore.dev/api/v2/log/entries"`
- `REKOR_URL` env var override; operator sets it on the
  local-fs-anchor.service `[Service]` block to point at log2026-1
  when that host appears — no binary rebuild required
- Plumbed `rekor_url` through `Config` + `post_to_rekor` signature
- 8 unit tests pass clean (added 2: default points at log2025-1
  shard + env override works)
- `cargo build --release` clean

### Optional follow-up — TUF discovery

The long-term-correct pattern per Sigstore docs is to fetch the
active log shard URL from Sigstore's TUF repository
(SigningConfig). That's a meaningful refactor — adds a TUF client
dependency (`tough` crate, ~50 transitive deps), introduces a TUF
trust-root bootstrap problem (where does the binary get its
initial root.json from?), and changes startup behaviour (TUF fetch
on every run, or cached-with-staleness-window). I propose holding
this until the apprenticeship-substrate key-custody decision lands
(the same operator decision you flagged in v0.1.28); both are
substrate-level trust questions and benefit from being decided
together. Not blocking.

## Commit 2 — `38765cd` — service-people end-to-end integration test

Closes the eighth-session self-handoff #1 pickup item, ratified
GO in your v0.1.28 message.

`service-people/tests/end_to_end_fs_round_trip.rs` (new). Spins
up a real service-fs daemon (axum on ephemeral 127.0.0.1 port,
PosixTileLedger over a tempdir) + drives a service-people router
via `tower::ServiceExt::oneshot`. Three steps:

1. POST `/mcp` `tools/call` `identity.append` (Alice Anderson +
   organisation)
2. GET service-fs `/v1/entries?since=0` — assert payload
   round-trips byte-faithfully (id, name, primary_email,
   organisation, created_at)
3. POST `/mcp` `tools/call` `identity.lookup` (by email) —
   assert PeopleStore cache also has the record

Multi-threaded tokio runtime (`worker_threads = 4`) required
because FsClient is synchronous (ureq blocking) and is invoked
from inside an async axum handler — that blocking call needs a
worker thread distinct from the one driving service-fs's serve
loop.

dev-deps added:
- `service-fs = { path = "../service-fs" }` — lib surface only
  (router, AppState, posix_tile_open)
- `tower = { version = "0.4", features = ["util"] }` —
  ServiceExt::oneshot for in-process router driving

ADR-07 preserved end-to-end: deterministic identity matching only;
no AI in any code path exercised. Test passes clean. Existing 20
service-people unit tests unaffected.

## Ready for Master

For the Rekor fix:
1. `cd /srv/foundry/clones/project-data/service-fs/anchor-emitter
   && cargo build --release`
2. `sudo install -o root -g root -m 0755 target/release/fs-anchor-emitter /usr/local/bin/`
3. `systemctl start local-fs-anchor.service` (smoke)

After smoke succeeds, the armed timer fires correctly on
**2026-05-01** without further intervention. When `log2026-1`
appears (Sigstore plans late 2025 / early 2026), update the
service unit's `Environment=REKOR_URL=https://log2026-1...` and
`systemctl daemon-reload` — no rebuild.

For the e2e test: it runs in CI on `cargo test` from the
service-people directory. No Master action needed.

## Next-session pickup order

1. ✅ Schema fix (your v0.1.27 — done last session, ratified)
2. ✅ Rekor URL fix (this session — `fc03e57`)
3. ✅ service-people end-to-end test (this session — `38765cd`)
4. **TUF SigningConfig discovery for Rekor URL** — when key-custody
   pattern lands per `apprenticeship-substrate.md` §6
5. **Optional Ed25519 signed checkpoints** — when key-custody
   ratified
6. **system-security panic_impl** — when convenient

Project-data Phase 1A scope reaches natural completion: all four
Ring 1 services have MCP servers + canonical schemas + at least
one end-to-end test through service-fs (the WORM ledger backbone).

— Task Claude, project-data cluster, 2026-04-27 (ninth session continued)
