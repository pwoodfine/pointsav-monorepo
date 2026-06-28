# NEXT.md — project-totebox (Totebox)

> Totebox Session — starts in `/srv/foundry/clones/project-totebox`
> **Scope: this archive only.** Cross-repo and workspace-level items live at `~/Foundry/NEXT.md`.

Last updated: 2026-06-23

---

## Hot — active this session (2026-06-23)
## Hot — next session (2026-06-27, jennifer ingestion)

- [ ] **INFRA LESSON — Tier A queue unbounded saturation**: queue_pending hit 234 with zero jennifer entities ingested. Root cause: (1) 84,434 project-intelligence BRIEFs being processed by service-content jennifer instance clog Tier A (CPU OLMo at 17–60 min each = 65–235 hrs to drain); (2) no backpressure gate — service-content accepts CORPUS indefinitely regardless of queue depth; (3) processed_ledgers.jsonl absent, meaning all CORPUS files re-queued on every service-content restart. Required fixes (implement before next bulk ingestion): [2026-06-27 totebox@claude-code]
  - [ ] **Backpressure gate**: service-content should reject new Tier A queue submissions when queue_pending > THRESHOLD (e.g. 50) — return 429 with `{"queue_full":true, "queue_pending":N}`. New CORPUS files accumulate on disk (CORPUS_DIR); get processed when queue drains. Prevents cascading saturation from bulk ingestion runs. [2026-06-27 totebox@claude-code]
  - [ ] **Tier B on-recovery sweep (item D)**: `tier_progress.jsonl` per-document ledger — tracks tier_a_done / tier_b_done per worm_id. On Tier B recovery: batch re-submit docs with tier_a_done=true + tier_b_done=false. Currently: Tier A fallback is write-once (processed_ledgers.jsonl marks done after ANY tier completes). High-risk change — implement processed_ledgers.jsonl backward compat (bare-line old format + JSON new format) before touching dedup logic. [2026-06-27 totebox@claude-code]
  - [x] ~~**processed_ledgers.jsonl recovery**~~: EXISTS at /var/lib/local-content/graph/ — 63,978 entries, bare filename format (old format). Was a permissions issue (dir owned by local-content user, mathew-owned shell can't read). NOT missing. Dedup is working. [2026-06-27 totebox@claude-code]
  - [ ] **jennifer research ingestion path**: service-input emit_corpus_dir mode now implemented (this session). Deploy as: `SERVICE_INPUT_EMIT_CORPUS_DIR=/home/mathew/deployments/woodfine-fleet-deployment/cluster-totebox-jennifer/service-content/ledgers SERVICE_INPUT_DOMAIN_ID=projects ./service-input`. This bypasses the routing mismatch (service-fs→service-research/source is UNWATCHED by jennifer extraction). [2026-06-27 totebox@claude-code]
- [ ] **service-input direct CORPUS mode deployed**: binary rebuilt after this session's changes (emit_corpus_dir + domain_id + YAML soft relaxation). Nightly script needs SERVICE_INPUT_EMIT_CORPUS_DIR env var wired before next run. [2026-06-27 totebox@claude-code]
- [ ] **entity_filter field_missing counter**: deployed this session (service-content). Next logs after rebuild should show `drop=field_missing:N` revealing true drop cause — the 0/1 kept with drop=all-zeros was a diagnostic gap, not a real filter. [2026-06-27 totebox@claude-code]

---

## Hot — next session (2026-06-27T07:00Z)

- [ ] **D11 — service-extraction full pipeline tests (scope B)** — output contract + queue drain + redrive + poison; `cargo test -p service-extraction` must be green. [2026-06-23 totebox@claude-code]
- [ ] **Corpus merge** — engineering + apprenticeship → merged/; corpus-manifest.py + export-sft.py --source=all [2026-06-23 totebox@claude-code]
- [ ] **P5 wiring** — deploy-gate.sh + lora-scaled-dropin.sh written; activate when GPU adapter ready [2026-06-23 totebox@claude-code]
- [ ] **Phase D witness run** — capped SFT + delta probe + extract→graph proof; triggers when Tier B returns [2026-06-23 totebox@claude-code]
- [ ] **Stage 6 → Command** — commit all 2026-06-23 session code [2026-06-23 totebox@claude-code]

---

## Foundation build (BRIEF-flow-build-plan, 2026-06-21/22/23)

P1–P10 code complete on cluster/project-totebox. D10 + model label fixed 2026-06-23.

- [x] **lbug ABI fixed + tests green** — `cargo test -p service-content` = 54/54 green. [2026-06-22 totebox@project-totebox]
- [x] **Stage 6 PROMOTED** — foundation + graph migration code on canonical origin/main. [2026-06-22 command via promote.sh]
- [x] **Additive graph migration** — entity_aliases, er_review_queue, RelatedTo write-path, in-batch ER wired. [2026-06-22 totebox@project-totebox]
- [x] **query_context canonical resolution** — alias-aware read path; 54/54 tests green. [2026-06-22 totebox@project-totebox]
- [x] **D9 closed** — created_at first-write-wins; fill-rate telemetry logged. [2026-06-22 totebox@project-totebox]
- [x] **D8 closed** — additionalProperties:false on extraction JSON schema. [2026-06-22 totebox@project-totebox]
- [x] **P8 closed** — redrive-quarantine.py fixed to target queue-poison/. [2026-06-22 totebox@project-totebox]
- [x] **D10 closed** — SLM_DEFAULT_MODULE_ID=woodfine drop-in applied via zz-foundation.conf; Doorman restarted. [2026-06-23 totebox@claude-code]
- [x] **GAP-4 label corrected** — SLM_LOCAL_MODEL=OLMo-3-7B-Instruct via zz-foundation.conf drop-in. [2026-06-23 totebox@claude-code]
- [ ] **Activation (Command/sudo)** — run `service-slm/scripts/activate-foundation.sh`; restart local-content.service. [2026-06-22 totebox@project-totebox]
- [ ] **GPU training** — when yoyo-batch L4 returns: run-sft → run-dpo simpo → eval gate → promote. [2026-06-22 totebox@project-totebox]
- [ ] **Later stages** — GraphStore PK cutover (high blast radius, deferred); OWL2/reasoner/SHACL; always-on training loop. [2026-06-22 totebox@project-totebox]

---

## Flow Quality Audit (BRIEF-flow-quality-audit, 2026-06-20)

14 confirmed FAIL. P1–P10 fixes code-complete. D10 fixed 2026-06-23.

- [x] **P1–P4, P6–P10** — ALL FIXED in code (see BRIEF carry-forward). [2026-06-22 totebox@project-totebox]
- [ ] **P5** — deploy-gate.sh + lora-scaled-dropin.sh written; systemd activation pending GPU adapter. [2026-06-23 totebox@claude-code]
- [ ] **GAP-4** — base-model fork RESOLVED in code (base-registry.yaml); activation still Command. [2026-06-22 totebox@project-totebox]
- [ ] **Phase D witness runs (deferred — STOCKOUT ~43h)** — capped train + delta-probe; needs yoyo-batch L4. [2026-06-20 totebox@project-totebox]

---

## SLM / DataGraph open items

- [ ] **SFT-first follow-ups** — file-grounded prompts; wire SFT stage into lora-update.sh; DPO-format fix in verdict.rs; verify SFTTrainer/SFTConfig API on yoyo-batch trl 1.5.1. [2026-06-20 totebox@project-intelligence]
- [ ] **Entity vectors all null** — role_vector/location_vector never populated; grammar-constrained extraction path code-complete but drain sends plain prompts; fix after Tier B basic enrichment stable. [2026-06-19 totebox@project-intelligence]
- [ ] **Phase 4b reconciliation pass** — 1,281 sweep-ledger entries written before Tier B online; gated on yoyo-batch provisioned in us-central1-a. [2026-06-15 command@claude-code]
- [ ] **Phase 5b adapter pull verification** — pull wired in nightly-run.sh; verify after first successful yoyo-batch cycle. [2026-06-19 totebox@project-intelligence]
- [ ] **Remove dead config** — SERVICE_CONTENT_TIER_A_FALLBACK_ENABLED=false in live systemd unit; Command scope. [2026-06-19 totebox@project-intelligence]
- [ ] **down_for_secs in TierBInfo** — health_down_secs committed; deploy pending Stage 6 + slm-doorman-server rebuild. [2026-06-19 totebox@project-intelligence]

---

## Blocked — Command Session

- [ ] **Activation (Command/sudo)** — run activate-foundation.sh; restart local-content.service. [2026-06-22 totebox@project-totebox]
- [ ] **Stage 6 + Doorman rebuild** — outbox msg-id project-intelligence-20260620-session26c-stage6-prompt-fix; 8 commits. [2026-06-20 totebox@project-intelligence]

---

## TOPIC/GUIDE drafts pending

- [ ] **TOPIC/GUIDE/JOURNAL** — stage to .agent/drafts-outbound/ → project-editorial. [2026-06-22 totebox@project-totebox]
