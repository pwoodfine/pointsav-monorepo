---
mailbox: inbox
owner: task-project-data
location: ~/Foundry/clones/project-data/.claude/
schema: foundry-mailbox-v1
---

# Inbox — Task Claude on project-data cluster

Messages addressed to whoever opens the next Task Claude session in
this cluster. Read top to bottom at session start. Act on items in
order; archive to `inbox-archive.md` after acting.

If this inbox accumulates more than 5 pending items, post a NOTAM
(per Doctrine §VI) and flag in Master's inbox.

---

---
from: task-project-data (2026-04-27 eighth session end)
to: task-project-data (next session)
re: orientation — eighth session complete; branch state + pickup order
created: 2026-04-27
---

## Branch state

Branch `cluster/project-data`. Last commit: `6262d10`
(fs-anchor-emitter — Doctrine Invention #7 Task half).

Seven tasks completed across this session and the preceding context
that was compacted (#14–#20). All 20 tasks from v0.1.23 + v0.1.26
inbox pickups are done.

## What landed this session

- **Task #20** — `service-fs/anchor-emitter/` Rust binary crate
  (commit `6262d10`). Sigstore hashedrekord POST to rekor.sigstore.dev;
  ephemeral Ed25519 keypair per run; SPKI DER manual encoding; tlog
  entry written back to service-fs `/v1/append`. 6 unit tests pass.
- **service-people MCP server** — `POST /mcp` with `identity.lookup` +
  `identity.append` tools (commit `8c4eb7e`). `src/mcp.rs` + `src/http.rs`
  + `src/main.rs` + `src/fs_client.rs` + `src/people_store.rs`. 20 tests
  pass. Was written earlier in this session (before context compaction)
  but not yet committed; landed at session end.

Earlier in the session (before context compaction):
- **Task #14** — workspace [members] doc cleanup (stale blocked items,
  registry note); operator had already committed the re-add.
- **Tasks #15–#18** — people-acs-engine rename, email-splitter rename,
  ingress-harvester + master-harvester-rs archival.
- **Task #19** — service-people canonical person-record schema
  (`src/person.rs`; 8 tests; chrono + serde + uuid deps).

## Outbox

Session-end summary sent to Master. Key points surfaced:
- fs-anchor-emitter done (was v0.1.26 "next-session" item)
- §5.10 zero-container drift in service-slm — out of scope for this
  cluster; flagged to Master for project-slm routing
- system-security panic_impl — no-pressure future item

## Next pickup order

1. **service-people FsClient end-to-end integration test** — spin up
   service-fs with a temp ledger root, call `identity.append` via the
   MCP endpoint, verify the record is readable via service-fs
   `/v1/entries`. Closes the Ring 1 pipeline from identity input to
   persisted WORM record.
2. **system-security panic_impl** (no urgency) — feature-gate
   `#[panic_handler]` behind `#[cfg(not(test))]` or move bare-metal
   code to a non-tested target to clear `cargo test --workspace`
   lang-item conflict.

## Where to look first

- This inbox (archived after reading)
- `service-people/NEXT.md` — Right-now is MCP server interface
- `service-fs/NEXT.md` — systemd unit env-var surface note
- `~/Foundry/CLAUDE.md` §8 — commit flow (commit-as-next.sh)
- `~/Foundry/conventions/worm-ledger-design.md` — WORM design spec

## Model tier note

This session completed implementation work (a Rust binary crate, no
architectural decision-making required). Sonnet 4.6 is the right tier
for the next session — MCP server wiring for service-people is
implementation, not deep-think.

— Task Claude, project-data cluster, 2026-04-27
