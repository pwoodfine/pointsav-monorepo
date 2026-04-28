---
mailbox: drafts-outbound
owner: task-project-data
location: ~/Foundry/clones/project-data/.claude/drafts-outbound/
schema: foundry-drafts-outbound-v1
---

# Drafts outbound — project-data cluster

Editorial-draft input port per the Reverse-Funnel Editorial Pattern
(Doctrine claim #35; `~/Foundry/conventions/cluster-wiki-draft-pipeline.md`).

This Task Claude stages **bulk** editorial drafts here when a
cluster milestone warrants TOPIC / GUIDE / per-project README
content. project-language Task sweeps this directory at session
start (via `bin/draft-sweep.sh`), refines each draft to register +
applies banned-vocab grammar + BCSC discipline + bilingual pair +
citation-registry resolution, and hands the refined `.md` + `.es.md`
off to the destination repo via the standard handoff mechanism.

## Discipline reminders

- **Bulk, not refined.** Don't try to hit Bloomberg-standard
  register; project-language enforces that downstream. Repetition,
  inline URLs, free-form structure are all fine here.
- **No bilingual pair.** project-language generates `.es.md` per
  DOCTRINE §XII. Author the English bulk only.
- **Inline citations OK as URLs.** project-language resolves to
  `[citation-id]` via `~/Foundry/citations.yaml`.
- **Frontmatter required.** Every draft carries `foundry-draft-v1`
  frontmatter — see `cluster-wiki-draft-pipeline.md` §2.
- **Apprenticeship event emission.** When staging a draft, emit a
  JSONL `draft-created` event to
  `~/Foundry/data/training-corpus/apprenticeship/prose-edit/pointsav/`
  per `apprenticeship-substrate.md` §7A.

## Trigger events (project-data cluster)

See cluster manifest `wiki_draft_triggers:` field for the
authoritative list. Discretion is mine; project-language can
request more via outbox if a milestone passes uncovered.

## Filename convention

`<protocol>-<slug>.draft.md` — e.g.
`topic-worm-ledger-architecture.draft.md`,
`guide-fs-anchor-emitter.draft.md`. The `.draft.md` suffix
distinguishes pending drafts from published refined output.

## See also

- `~/Foundry/conventions/cluster-wiki-draft-pipeline.md` — full pipeline
- `~/Foundry/conventions/reverse-funnel-editorial-pattern.md` — Doctrine claim #35
- `~/Foundry/conventions/language-protocol-substrate.md` §2 — PROSE family templates
- `~/Foundry/conventions/apprenticeship-substrate.md` §7A — JSONL event format
- `~/Foundry/CLAUDE.md` §11 — three input ports + layer-rule discipline
