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

## 2026-04-26 — from task-project-data (sixth session, Opus 4.7) to next session — model-tier handoff + task-list briefing

priority: high — read first; the operator was preparing to log back
in and explicitly asked for a save. The recommendation below is
that you (the next session) open at Sonnet 4.6, not Opus 4.7.

recommended_model: claude-sonnet-4-6

### Why a tier change is recommended

Per `~/Foundry/conventions/model-tier-discipline.md` §1 trigger
threshold: "the session has finished a deep-think pass and the
remaining work is implementation against an established plan."
That trigger fires here. The deep-think pass — RESEARCH.md
synthesis → `worm-ledger-design.md` ratified at workspace v0.1.7
(commit `6c0b79a`) → DOCTRINE §IX External WORM standards
ratified (commit `ecee9fb`) — is done. Every Right-now and Queue
item across the four Active Ring 1 projects in this cluster is
now implementation against ratified plans. None require
architectural judgment, doctrine interaction, or multi-source
synthesis.

### Terminal commands to switch

```bash
# 1. Verify clean state before exit (it should be — sixth session
#    only edited mailbox files, committed via bin/commit-as-next.sh)
git status

# 2. Exit current Claude Code session
#    Type 'exit', or press Ctrl-D

# 3. Re-enter at Sonnet
cd /srv/foundry/clones/project-data && claude --model claude-sonnet-4-6
```

### State of the branch (unchanged from fifth session)

- Branch: `cluster/project-data` (verify with
  `git branch --show-current`).
- HEAD at exit of fifth session: `9f9b824` (service-input PdfParser).
- HEAD after this sixth-session save commit: see `git log -1`.
- Working tree: clean after this commit (verify with `git status`).
- Push state: nothing pushed. Per v0.0.10 auto-mode safety brief,
  push is operator-authorised; do not push without explicit
  instruction.

### Task list — built this session (13 items, all Sonnet-suitable)

Sixth-session Opus session built a 13-item task list in the
Claude Code TaskList tool. Tasks do NOT persist across sessions
in Claude Code; the list below is the canonical record. Run
`TaskList` at session start if you want to re-create them; or
just work directly from the per-project `NEXT.md` files which
are the durable source of truth.

**Recommended pickup order (customer-first):**

1. **service-fs step 3 — checkpoint signing (Ed25519 + signed-note).**
   Right-now per `service-fs/NEXT.md`. Convention specifies the
   signed-note format byte-for-byte (`origin\ntree_size\nbase64(root_hash)\n\n`);
   add `FS_SIGNING_KEY` env var; load on `PosixTileLedger::open`;
   populate `Checkpoint::signature`; add `--verify-checkpoint`
   test path for independent verification per Doctrine claim #28;
   trait surface grows by `signing_key` on `open` per
   worm-ledger-design.md §2.
2. **service-input — MarkdownParser via pulldown-cmark.** Right-now
   per `service-input/NEXT.md`. Pattern-follow on PdfParser; pure-
   text input, no temp-file shim, full happy-path testing trivial.
3. **service-fs step 4 — ADR-07 audit-log sub-ledger.** Queue.
   Depends on (1) for signed audit checkpoints. Separate
   `LedgerBackend` instance at `<root>/<moduleId>/audit-log/`;
   per-call entries with `entries_returned`; itself WORM via the
   same trait surface.
4. **service-fs step 5 — MCP-server interface layer.** Queue. Layer
   MCP on top of existing JSON-over-HTTP routes per
   `three-ring-architecture.md` §"MCP boundary at Ring 1".
5. **service-fs round-trip integration test.** Queue. Hit
   `/v1/append` then `/v1/entries`, assert payload identity.
   Belongs in `tests/`.
6. **service-input — DOCX parser via docx-rust.** Queue.
7. **service-input — XLSX parser via calamine.** Queue.
8. **service-input → service-fs HTTP client integration.** Queue.
   Once at least one parser works.
9. **service-input — MCP server interface.** Queue. Depends on (4).
10. **service-input — happy-path PDF test fixture.** Deferred from
    PdfParser commit; needs known-good fixture (oxidize-pdf write
    API call, hand-crafted byte string, or checked-in binary).
11. **service-people — inventory pre-framework subdirectories.**
    Queue per cluster manifest. Five subdirs +
    `service-people.py` + `ledger_personnel.json`. Each:
    keep / rename / retire / relocate decision. First NEXT.md item
    before any schema work.
12. **service-email — inventory pre-framework subdirectories.**
    Queue per cluster manifest. Four subdirs.
13. **service-email — EWS auth rebase.** Queue. Operator-decided
    pattern conversion (rebase Graph OAuth onto EWS-based MSFT
    auth from sibling `service-email-egress-ews/`). Depends on
    (12).

### Where to look first

1. `service-fs/NEXT.md` and `service-input/NEXT.md` — Right-now items.
2. `~/Foundry/conventions/worm-ledger-design.md` (workspace v0.1.7,
   `6c0b79a`) — authoritative for service-fs work.
3. `~/Foundry/conventions/model-tier-discipline.md` — the convention
   that put you at Sonnet rather than Opus.
4. `service-fs/RESEARCH.md` — input draft for the convention; ten
   D1–D10 decisions.
5. `service-fs/ARCHITECTURE.md` + `service-fs/SECURITY.md` — durable
   per-project overviews; "ratified at workspace tier 2026-04-26".
6. Cluster manifest at `.claude/manifest.md` — Master backfilled
   `triad:` per Doctrine v0.0.4; three "leg-pending" forward-
   looking items, none immediate.
7. Cluster outbox at `.claude/outbox.md` — fourth + fifth session-
   end summaries to Master, both informational; no asks.

### Hard constraints (still in force)

- ADR-07 zero-AI in Ring 1 (no LLM inference, no embedding,
  no AI-assisted normalisation in any of these crates).
- Per-tenant moduleId isolation (header check today; capability
  isolation in Envelope B / seL4 long-term).
- Append-only invariant at the L2 trait surface and at the L1
  filesystem layer.
- Push only to staging-tier remotes (`origin-staging-j` /
  `origin-staging-p`); never to `origin` (canonical) per
  v0.0.10 auto-mode safety brief — applies even if you are
  operating in auto mode.
- Workspace `[members]` re-add for service-fs + service-input is
  BLOCKED on a pre-existing `openssl-sys` Layer 1 audit issue in
  a sibling member; this is repo-tier work, not Task-tier. Do
  not try to fix it inside this cluster.

### What stays at current tier (do NOT exit yet if applicable)

Nothing in flight requires Opus tier. The ratified convention
fully specifies the next several commits' worth of work. If the
Sonnet session encounters a genuinely architectural fork
mid-implementation, it should write its own tier-up
recommendation back to outbox and pause for operator decision.

After acting on this orientation note, archive it to
`inbox-archive.md` per the mailbox protocol.

---
