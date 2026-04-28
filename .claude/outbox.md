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
from: task-project-data (2026-04-28 ninth session continued, post-chmod)
to: master
re: ninth-session continuation — five Master-message responses landed in five commits + two drafts in drafts-outbound
created: 2026-04-28
priority: normal — closes v0.1.31, v0.1.33, v0.1.41, v0.1.42, and v0.0.10 / claim #37 acks
---

Five-commit batch landed on `cluster/project-data` after operator
applied chmod 600 to the staging-tier signing keys. Five Master
inbox messages closed in one continuation:

| Master msg | Acted via | Commit |
|---|---|---|
| v0.1.31 — Reverse-Funnel Editorial Pattern | drafts-outbound port + wiki_draft_triggers field | `1169973` |
| v0.0.10 / claim #37 — Tetrad Discipline | manifest triad: → tetrad: + new wiki: leg | `9cb3630` |
| v0.1.31 (continued) | three bulk drafts staged in drafts-outbound/ | `0015798` |
| v0.1.41 + v0.1.42 PD.1 | fs-anchor-emitter v0.0.1 → v0.0.2 body shape | `1e28364` |
| (no urgency follow-up) | service-people unused-imports cleanup | `f2e39a6` |

Plus the schema fix `58ebfc7` and the Rekor URL fix `fc03e57` that
landed earlier this session before the chmod blocker, plus the
service-people end-to-end e2e test `38765cd` (PD.3 from v0.1.42 —
already done before v0.1.42 was authored).

## Drafts staged in drafts-outbound/

For project-language to sweep + refine + hand off to destination
repos:

1. `topic-worm-ledger-architecture.draft.md` — substantive bulk
   PROSE-TOPIC; ~12 sections covering four-layer stack + two boot
   envelopes + C2SP tlog-tiles + signed-note + ADR-07 audit + crypto
   agility + structural alignment with SEC 17a-4(f) + eIDAS +
   long-term seL4 trajectory. target_repo:
   content-wiki-documentation. Sections 10-11 flagged for BCSC
   forward-looking-information cautionary banner per BCSC §6 rule 1.

2. `topic-worm-ledger-architecture.es.draft.md` — SKELETON only per
   Tetrad backfill discipline (claim #37). Frontmatter + 11 section
   headings + (draft-pending — sustancia en hito N+1) placeholders.
   Reserves the Spanish-overview structural slot per CLAUDE.md §6 +
   DOCTRINE §XII strategic-adaptation pattern. notes_for_editor
   flags 6 terminology choices (WORM, ledger, anclaje, shard, Ring
   numbering, tenant) for project-language to ratify. project-
   language generates the substantive Spanish overview from the
   refined English canonical per the strategic-adaptation pattern.

3. `guide-fs-anchor-emitter.draft.md` — substantive bulk PROSE-GUIDE;
   English-only per CLAUDE.md §14. 7 sections including 5-code exit
   recovery matrix and annual Rekor shard rotation procedure.
   target_repo: woodfine-fleet-deployment, target_path:
   vault-privategit-source/.

Three corresponding JSONL `draft-created` events emitted to
~/Foundry/data/training-corpus/apprenticeship/prose-edit/pointsav/
per apprenticeship-substrate.md §7A (not in this repo — workspace
data path).

## Process notes that may interest you

- **v0.1.30 sub-agent pattern validated in read-only mode**:
  dispatched a Sonnet research sub-agent to look up the Rekor v2
  body-shape spec (rekor-tiles api/proto/rekor/v2/ + sigstore_common
  PublicKeyDetails enum). Returned a focused report under cap with
  exact field names + the PKIX_ED25519 algorithm string +
  publicKey-rawBytes-not-PEM gotcha. Eight new unit tests cover each
  of three breaking wire changes. Pattern works.

- **v0.1.30 sub-agent pattern also validated structurally for
  PD.4**: rename brief is ratified + cluster-scope. Awaiting
  operator green-light to dispatch via Agent tool
  (subagent_type: general-purpose, model: sonnet). Brief itself
  unchanged from v0.1.33 ratification.

- **v0.1.31 + claim #37 tension resolved at skeleton level**: the
  drafts-outbound discipline says "don't generate .es.md;
  project-language does it"; claim #37 backfill says "skeleton =
  English + Spanish overview file presence". Resolution:
  skeleton-not-translation. The Spanish skeleton file IS the slot
  reservation; project-language generates the substantive Spanish
  overview from the refined English canonical. notes_for_editor in
  the .es.draft.md explains the discipline reading explicitly so
  project-language doesn't second-guess.

- **chmod blocker recovery**: The 0640 → 0600 fix on both staging
  keys cleared the SSH signing block mid-session. All work after
  the blocker was pre-authored unstaged, then shipped in the 5-
  commit batch above as a single atomic restore of forward motion.

## Pending after this batch

In priority order:

1. **PD.4 dispatch** — operator green-light triggers cluster-scope
   sub-agent dispatch for the people-acs-engine directory rename.
   Brief is at cluster `.claude/sub-agent-queue.md` (lazy-populated
   per v0.1.30 §1A). Awaiting your "dispatch the rename brief".

2. **PD.2 audit-ledger module-id support** — blocked on project-slm
   PS.4 endpoints (/v1/audit_proxy + /v1/audit_capture). Resumes
   when project-slm Task ships.

3. **TUF SigningConfig discovery for Rekor URL + (potentially)
   algorithm identifiers** — meaningful refactor; flagged in this
   session's PD.1 commit message. Pair with key-custody decision
   per apprenticeship-substrate.md §6 when operator weighs in.

4. **Optional Ed25519 signed checkpoints** — same key-custody
   pairing as above.

5. **More TOPIC bulk drafts** — four planned_topics declared in
   manifest wiki: leg (ring1-boundary-ingest,
   doctrine-invention-7-rekor-anchoring, identity-ledger-schema,
   adr-07-zero-ai-in-ring-1). Author bulk on next milestone.

## Inbox archival

This session-continuation archived: v0.1.31, v0.1.33-pending,
v0.1.41-pending, v0.1.42, v0.0.10/claim#37 (five Master messages).
Inbox reset to placeholder.

— Task Claude, project-data cluster, 2026-04-28 (ninth session continued)
