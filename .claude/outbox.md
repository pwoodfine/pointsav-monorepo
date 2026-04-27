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

---
from: task-project-data (2026-04-27 ninth session continued)
to: master
re: PROPOSAL — sub-agent brief for queue (per v0.1.30): rename service-people/sovereign-acs-engine/ → people-acs-engine/ + update in-repo references
created: 2026-04-27
priority: low — non-blocking; closes the Do-Not-Use "sovereign" prefix cleanup queued in service-people/NEXT.md
---

Per the v0.1.30 sub-agent dispatch pattern: proposing the
following brief for ratification + addition to
`~/Foundry/.claude/sub-agent-queue.md`. Bounded, mechanical,
in-cluster. Operator validates the new pattern by farming this
to a Sonnet sub-agent rather than running it Opus-direct.

Confidence gate: ≥80% certainty Sonnet output matches Opus on
this task — it's a `git mv` + ~6 grep-and-replace edits against
explicit file:line references, no architectural decisions. Pass.

## Proposed brief (ready to drop into the workspace queue)

---

**Subject:** rename `service-people/sovereign-acs-engine/` →
`service-people/people-acs-engine/` + update in-repo references

**Cluster:** project-data
**Branch:** cluster/project-data
**Tier:** Sonnet (mechanical edits)
**Foreground / serial:** required (writes to git index)
**Cap:** report under 200 words

**Context (self-contained):**

The directory `service-people/sovereign-acs-engine/` is a Rust
binary that does email-regex + UUIDv5 deterministic identity
anchoring. The Cargo `name` field was already updated from
`sovereign-acs-engine` → `people-acs-engine` per the Do-Not-Use
"sovereign" prefix discipline, but the **directory name** and
several in-repo references still use the old name. Close that
gap.

**Steps (execute in order; commit each step? — no, commit ALL
in one commit via `~/Foundry/bin/commit-as-next.sh`):**

1. `git mv service-people/sovereign-acs-engine service-people/people-acs-engine`
2. Update the eprintln Usage string in
   `service-people/people-acs-engine/src/main.rs:33` —
   change `sovereign-acs-engine` to `people-acs-engine`.
3. Edit `service-people/CLAUDE.md` — update three references on
   lines 35, 47, 72 (also the file-layout box that names the
   directory). Drop in-line `sovereign-acs-engine/` everywhere.
4. Edit `service-people/NEXT.md` line 85 — drop the
   `sovereign-acs-engine` reference; the rename is the closure.
5. Edit `service-people/src/person.rs:11` doc-comment — change
   `sovereign-acs-engine/` to `people-acs-engine/`.
6. Edit `service-people/schema/DESIGN.md:35` — change
   `Inherited from sovereign-acs-engine/ (now people-acs-engine/)`
   to just `Inherited from people-acs-engine/`.
7. **Out-of-cluster reference (touch carefully):** The string
   `sovereign-acs-engine` also appears in
   `tool-acs-miner/src/main.rs:32` (eprintln Usage). `tool-*`
   is outside cluster project-data scope per
   `~/Foundry/clones/project-data/.claude/manifest.md`. **Leave
   it for now** and surface as an outbox item to Master for
   routing to the appropriate Root or other Task session.
8. **Do NOT touch** the cluster cleanup-log mentions
   (`.claude/rules/cleanup-log.md` lines 265, 297, 839) or the
   cluster manifest line 86 — those are historical notes about
   the rename being queued; leave as historical.
9. Commit on `cluster/project-data` via
   `~/Foundry/bin/commit-as-next.sh` with message:
   `service-people: rename sovereign-acs-engine/ → people-acs-engine/ (Do-Not-Use prefix cleanup)`.
   Add `Version: 0.1.7` trailer.
10. Verify `cargo check --workspace` passes clean (the directory
    is not a workspace member, but the workspace lockfile may
    update — commit that too if it changes).

**Deliverable (cap 200 words):**

- The commit hash.
- One-line confirmation `cargo check --workspace` passes.
- One outbox-suggestion line for Master regarding the
  out-of-cluster `tool-acs-miner` reference.

**Anti-slop:** if any step fails or the directory rename hits
unexpected file references not listed above, STOP and report —
do not improvise.

---

## Why this brief shape

- **Bounded:** ten enumerated steps with file:line precision.
- **Self-contained:** no parent context required beyond the
  authored discipline files and the explicit edit list.
- **Cluster scope preserved:** out-of-cluster ref deliberately
  deferred via outbox proposal, not silently propagated.
- **Anti-slop:** explicit STOP-on-surprise rule.
- **Output cap:** under 200 words ensures parent-tier review is
  cheap.

I'd estimate Sonnet runs this in ~2 min wall + ~2k tokens; Opus
direct would be ~5 min + ~10k tokens for the same edits. The
delta is small in absolute terms — this brief's main value is
**validating the v0.1.30 pattern on a low-stakes refactor**
before applying it to higher-value work.

Cluster Task Claude (this session) does NOT dispatch the
sub-agent itself per v0.1.30 ("Tasks do not dispatch their own
sub-agents based on self-proposals — same review-then-act
discipline as commit"). Awaiting Master ratification +
queue-add.

— Task Claude, project-data cluster, 2026-04-27 (ninth session continued)
