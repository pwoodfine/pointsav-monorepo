---
artifact: journal-notes
schema: foundry-draft-v1
title: "Auditing an AI Knowledge Flow with a Research-then-Audit Agent Swarm"
slug: journal-knowledge-flow-audit-swarm
artifact_type: JOURNAL
status: refined
language: en
bilingual_pair_required: false
bcsc_class: internal
forbidden_terms_cleared: true
route_to: command
refined_by: project-editorial
refined_on: 2026-06-28
source_draft: clones/project-totebox/.agent/drafts-outbound/JOURNAL-NOTES-flow-quality-audit-20260620.draft.md
created: 2026-06-20
updated: 2026-06-28
domain: documentation
target_venue: TBD
submission_status: not-submitted
relates_to_topics:
  - flow-quality-architecture
  - topic-tiered-entity-extraction-architecture
research_trail:
  sources_cited: true
  claims_verified: true
  verification_method: code review at file:line + refute-2 + read-only live baseline
  last_checked: 2026-06-28
---

# Auditing an AI Knowledge Flow with a Research-then-Audit Agent Swarm

## Premise

The Totebox owns an end-to-end flow that converts prose into a knowledge graph and
into LoRA adapters for a local model. The operator asked a direct question: are the
LoRA training and the ontological DataGraph at the highest possible quality, and if
not, what — up to a rewrite — would get them there. Rather than answer from a single
reading, we ran a two-stage agent swarm: first establish the gold-standard
methodology from primary sources with adversarial verification, then audit the real
flow against that standard with competing teams and a judge panel.

## What the research stage changed

The most useful result of the research stage was that it overturned our own seed
hypothesis. We had assumed the SFT script's LLaMA-style adapter target modules were
wrong for an OLMo base and silently trained a no-op. The literature said the
opposite: every OLMo family integrated into Hugging Face transformers uses the
LLaMA-style names; the "OLMo-specific" `att_proj/ff_proj` names live only in a
legacy codebase and match nothing on a modern base. The bug was real, but it was in
the **other** script. Had we skipped the verified-research step, we would have
"fixed" the correct script and broken it.

The research stage also disciplined us against in-code folklore: a comment claiming
a specific rank choice buys 10–20 points of F1 did not survive a source check, and
a set of library defaults could not be trusted because the libraries were not even
installed on the host. Both went onto a do-not-trust list before the audit began.

## What the audit stage found

The competing audit confirmed fifteen of sixteen dimensions as failing, each
grounded at a file and line and each re-checked by two adversarial passes. The
shape of the failure was consistent across both halves of the flow: the parts are
individually plausible, but the **loop is never closed**. On the training side, the
preference path aborts before it can train; the one adapter that exists was promoted
with no evaluation gate; and even a perfect adapter would be inert because the model
it targets is not the model being served — a fork we traced four ways. On the graph
side, entity resolution is a single normalisation step, so the same organisation
fragments into four live records; the relationship table is declared and never
written; and the default query scope points at an empty namespace, so the graph
context the model is supposed to receive is, in practice, nothing.

A completeness critic earned its place by finding what the dimension auditors
missed — the four-way base-model fork, a preference-optimisation library silently
degrading to a weaker objective, and an evaluation script still reading an abandoned
predecessor clone. It also corrected one of our own baseline observations, locating
a corpus channel we had reported as empty.

## The verdict and the restraint

Three judges independently and unanimously ranked incremental hardening first over
partial rewrite and greenfield. The reasoning is worth recording: most of the
highest-severity defects are one-constant or additive, reversible fixes — a target
module list, a learning rate, an assertion, a default scope. The genuinely
structural work, an alias table and provenance and typed edges, is real and
necessary, but it belongs behind an abstraction seam and behind a green evaluation,
never as an opening move on live data. The cheapest correct move and the most
ambitious one pointed in the same direction; the discipline was in the sequencing.

## What we could not witness

The live witness runs — a capped training pass, a base-versus-adapter probe, a
clean-pair count on the real corpus — were gated on a GPU trainer that capacity
exhaustion kept us from booting. Because every defect was already confirmed from the
source and adversarially verified, the witness runs are confirmatory rather than
load-bearing, and they are recorded as follow-ups for when capacity returns.

## From audit to build

A second round of agents then graded the live outputs directly — measured, not
inferred — and returned the same verdict the source review predicted: the graph is a
keyword index of orphan name-strings (eighty-nine percent of nodes carry no
attributes, no edges, no provenance), and the training corpus is a single-task,
length-confounded, mostly-truncated pile whose adapter could not be served anyway.
Neither output is usable today.

The point of saying so precisely is that it makes the path forward concrete. The
target is not a patched version of what exists; it is two co-evolving loops behind
one boundary — a formal, reasoning, self-growing ontology on one side, and a
continuously training, self-improving model on the other, sharing one pinned base on
owned hardware. The audit's reversible fixes are the foundation that has to land
first; the sophisticated-graph and always-on-training layers build on top of green
gates. The architecture is in the companion topic; the sequenced build and the
decisions it still needs are in the flow build-plan brief.

---

## Addendum — 2026-06-28: EQ fixes shipped; flow confirmed live

The EQ fix series (five implementation sessions across 2026-06-28) converted the
audit's paper verdict into live throughput. This addendum records what changed and
what was verified.

**Fixes shipped (in order):**

- **EQ0** — backpressure gate called `/healthz` instead of `/readyz`, making it
  silently non-functional. Fixed in `service-content/src/main.rs` (commit `199a262d`).
- **EQ1 + EQ2** — Doorman Tier A fallback had no grammar constraint (schema violations
  on ~50% of completions), no explicit temperature (default ~0.8, hallucinations),
  and a 512-token output cap (entity lists truncated). Fixed: Tier A now receives
  `GrammarConstraint::JsonSchema`, `temperature: 0.0`, `max_tokens: 1024`,
  `cache_prompt: true`. Grammar constraint also cloned correctly so Tier A and Tier B
  no longer share a moved value (commit `da56ebf2`).
- **EQ4** — raw web-scrape text (nav links, inline URLs, Unicode box-drawing chars)
  sent directly to the model. `preprocess_corpus_text()` strips HTML artifacts before
  any extraction call (commit `720e20d8`).
- **EQ5** — entities in the second half of long articles were missed because only the
  first 2,000 characters were sent. `chunk_for_gliner()` now splits at sentence
  boundaries into ≤2,000-character chunks and deduplicates by
  `(lower(entity_name), classification)` key (commit `720e20d8`).
- **GlinerOutcome enum** — `call_tier_0_gliner()` previously returned
  `Option<Vec<...>>`, conflating "service down" with "service up, empty result."
  Replaced with a three-variant enum: `Found` (entities extracted), `Empty` (GLiNER
  reachable, no entities — mark done immediately), `Unavailable` (service down —
  check backpressure, then Tier A fallback). This single change drained 3,440
  CSV/engineering files at ~1/sec vs. the previous indefinite deferral (commit
  `b7e5c7a8`).

**Measured outcomes (live, 2026-06-28):**

- `drop=field_missing:0` across 82 jennifer-module extractions (was non-zero on ~50%)
- 206 entities written to DataGraph today; total +106 since session start (11,982 → 12,088)
- 3,440+ files drained via `GlinerOutcome::Empty` in a single pass at ~1/sec
- Write→read round-trip confirmed: entity written via `/v1/graph/mutate`, read back
  via `/v1/graph/context` immediately, without restart (CHECKPOINT fix working)
- Three services cooperating: GLiNER (:9085, 769 MB, 0 restarts) → service-content
  → Doorman (:9080, 4.2 MB, 0 restarts) → DataGraph (:9081, 166 MB, 0 restarts)

**Stage 6 status:** commits `720e20d8`, `da56ebf2`, `b7e5c7a8` on
`cluster/project-totebox` awaiting Command Session canonical merge via `bin/promote.sh`.
