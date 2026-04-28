---
queue: cluster-scope sub-agent briefs
owner: task-project-data
location: ~/Foundry/clones/project-data/.claude/
schema: foundry-sub-agent-queue-v1
---

# Sub-agent queue — project-data cluster (cluster-scope)

Cluster-scope sub-agent briefs ratified by Master per v0.1.30 §1A.
Master-scope briefs go to `~/Foundry/.claude/sub-agent-queue.md`.

Each brief is dispatched by this Task session via the `Agent` tool
with `subagent_type: "general-purpose"`, `model: "sonnet"`,
foreground + serial when writing (git-index race per §1A.2).
Per §1A.6: parent Task reviews + commits the result; never
delegates the commit decision.

After dispatch + commit, the brief moves to the **Completed** section
at the bottom of this file (preserves audit trail).

---

## Pending

### PD.4 — sovereign-acs-engine → people-acs-engine directory rename

- **Status:** ratified by Master v0.1.33 (2026-04-27); dispatch green-lit by Master v0.1.43 (2026-04-28)
- **Cluster:** project-data
- **Branch:** cluster/project-data
- **Tier:** Sonnet (mechanical edits)
- **Foreground / serial:** required (writes git index)
- **Cap:** report under 200 words

**Context (self-contained):**

The directory `service-people/sovereign-acs-engine/` is a Rust
binary that does email-regex + UUIDv5 deterministic identity
anchoring. The Cargo `name` field was already updated from
`sovereign-acs-engine` → `people-acs-engine` per the Do-Not-Use
"sovereign" prefix discipline, but the **directory name** and
several in-repo references still use the old name. Close that
gap.

**Steps (execute in order; commit ALL in one commit at the end —
but do NOT run the commit yourself; parent Task does):**

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
7. **Out-of-cluster reference (DO NOT touch):** The string
   `sovereign-acs-engine` also appears in
   `tool-acs-miner/src/main.rs:32` (eprintln Usage). `tool-*`
   is outside cluster project-data scope per
   `~/Foundry/clones/project-data/.claude/manifest.md`. **Leave
   it as-is.** Will be surfaced as an outbox item to Master for
   routing to the appropriate Root or other Task session.
8. **Do NOT touch** the cluster cleanup-log mentions
   (`.claude/rules/cleanup-log.md` lines around 265, 297, 839)
   or the cluster manifest line ~86 — those are historical notes
   about the rename being queued; leave as historical.
9. **Do NOT commit yourself.** Parent Task runs the commit via
   `~/Foundry/bin/commit-as-next.sh` after reviewing the
   diff. Stage all changes (`git add` the rename + the 5 file
   edits) but stop before commit.
10. Verify `cargo check --workspace` passes clean (the directory
    is not a workspace member, but the workspace lockfile may
    update — leave any Cargo.lock changes staged for parent to
    commit too).

**Deliverable (cap 200 words):**

- One-line confirmation each step completed (or noted as skipped/
  failed)
- `cargo check --workspace` final status
- One outbox-suggestion line for Master regarding the
  out-of-cluster `tool-acs-miner` reference

**Anti-slop:** if any step fails or the directory rename hits
unexpected file references not listed above, STOP and report —
do not improvise.

---

## Completed

*(none yet)*
