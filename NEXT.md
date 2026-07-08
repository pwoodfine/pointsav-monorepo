# NEXT.md — project-totebox (Totebox)

> Totebox Session — starts in `/srv/foundry/clones/project-totebox`
> **Scope: this archive only.** Cross-repo and workspace-level items live at `~/Foundry/NEXT.md`.

Last updated: 2026-06-23
## Hot — full Tier A/B live test, gate-script hardening, 4 carry-forwards closed (2026-07-04)

Full detail throughout `.agent/briefs/BRIEF-flow-quality-audit.md` (multiple dated entries this
session — search "2026-07-04"). Condensed:

- [x] ~~**Rank-mismatch crash fixed + live-verified**~~ — `SFT_LORA_ALPHA` realigned 8→32 in both
  training scripts to match the on-disk checkpoint (`apprenticeship-pointsav-incremental/
  checkpoint-49`, saved at r=16/alpha=32) and current alpha≥r LoRA guidance. Confirmed against a
  real nightly-cycle stint: training resumed cleanly, completed with `rc=0`, wrote the first
  receipt since 2026-07-03. Commit `53b25c3b` + result `34fc9511`.
- [x] ~~**GAP-4 model mislabel — actually fixed this time**~~ — 2026-06-23's fix only touched the
  non-authoritative `zz-foundation.conf` drop-in; the real culprit was
  `/etc/local-doorman/local-doorman.env`'s `EnvironmentFile=` (wins over `Environment=` drop-ins at
  process spawn on this host). Edited directly, restarted, verified via `/proc/<pid>/environ`.
- [x] ~~**Quality-gated the retrained adapter**~~ — both `deploy-gate.sh` (10/20 delta, 4 of the
  10 nulls were host-contention empties) and `score-gate.sh` (0% format-compliance) FAIL — expected
  and not alarming, this adapter has only completed ~7% of one epoch so far.
- [x] ~~**Gate scripts hardened**~~ (commit `4631ad7b`) — `deploy-gate.sh` now separates
  `both_empty_count` (contention) from genuine `null_count` (real no-op); `_gate-common.sh` warns
  (non-blocking) on high host load before either gate starts; `score-gate.sh` now appends every
  gate attempt (pass or fail) to `data/adapters/registry.yaml`, reusing `eval-adapter.sh`'s existing
  schema. `score-gate.sh`'s missing executable bit (committed as `100644`, should be `100755`) also
  fixed.
- [x] ~~**queue_poison root-caused**~~ — 2,767 files (not 656 — that reading was already stale).
  22.5% instant-poisoned by the 16 KiB oversized-payload gate (legitimate large diffs); 77.5%
  retry-exhausted (5 attempts) against a mostly-down Tier B, likely compounded by an "optimistic
  health" window that reopens on every `local-doorman.service` restart. Command-scope fix
  (raise `SLM_QUEUE_MAX_PAYLOAD_BYTES`; investigate the restart/retry-window interaction); not
  fixed this session — production dispatch logic shared by 7+ archives.
- [x] ~~**Holdout-schema reconciliation**~~ — `score-gate.sh` was stuck on the stale 76-row
  `data/corpus/eval/holdout.jsonl`; `eval-adapter.sh` was already using the fresh 388-row
  `holdout-v1.jsonl`. `score-gate.sh` now defaults to the same fresh file; both schemas still
  supported. Caught and fixed a self-introduced bug in the same pass: the new registry-write had no
  `--dry-run` guard and had polluted the registry with fake scores — fixed, registry cleaned.
- [x] ~~**`eval-adapter.sh` soft-deprecated**~~ — header comment added pointing to `score-gate.sh`
  (now equal-or-better on every axis); not deleted, since `activate-foundation.sh`/`eval-prepare.sh`/
  `lora-update.sh` still reference it in comments.
- [x] ~~**D10 status re-confirmed**~~ — F2 fix (`be0a3ca5`) is 184 commits behind `origin/main`,
  unpromoted; live binary predates the fix by 3+ days. Still broken in production, still
  Command-scope, no new action needed beyond existing Stage 6 tracking.

## Hot — Tier A quality audit, drain un-pause, adapter→Tier A promotion research (2026-07-03)

Full detail throughout `.agent/briefs/BRIEF-flow-quality-audit.md` (multiple dated
entries this session). Condensed:

- [x] ~~**Tier A quality-tested, code bugs found+fixed**~~ — commit `be0a3ca5`: LoRA
  rank-mismatch guard (see section below), strengthened graph-context injection
  framing in `router.rs` + closed a confirmed zero-test-coverage gap on that path, added
  `SLM_LOCAL_MODEL` startup observability logging. `cargo test`: 196/196 (slm-doorman),
  51+5/56 (slm-doorman-server). Live-tested: GLiNER extraction 4/4 correct; confirmed
  `ask_local` still hallucinates on ungrounded entity queries (D10) — the router.rs fix
  is committed but `local-doorman.service`'s binary hasn't been rebuilt/redeployed
  (Stage 6 + Command scope, not done this session).
- [x] ~~**`SLM_DRAIN_PAUSED` un-paused (operator-approved) — surfaced a real, repeatable
  systemd bug in the process**~~ — had been `true` for 8+ days, freezing apprenticeship
  corpus growth. First attempt (flip the `zz-foundation.conf` drop-in + restart) silently
  did NOT take effect — confirmed via direct `/proc/<pid>/environ` read. Root cause: this
  host's `EnvironmentFile=` (`/etc/local-doorman/local-doorman.env`) wins over a
  textually-later drop-in `Environment=` directive at actual process spawn, even though
  `systemctl show -p Environment` reports the opposite (textbook) resolution — the exact
  same class of bug as the `SLM_LOCAL_MODEL` mislabel found earlier. **Fixed by editing
  the actually-winning file directly** (`local-doorman.env`), verified live. Drain now
  runs but correctly holds on `all Tier B nodes offline` (Tier B/yoyo-batch genuinely
  down, not a new problem) — so real corpus growth still depends on Tier B coming back.
  **Worth remembering generally**: any future env-var change on this specific host
  should be verified via direct `/proc/<pid>/environ` read post-restart, not trusted from
  `systemctl show` alone.
- [x] ~~**`corpus-threshold.py` + `eval-adapter.sh` fixed and verified live**~~ — commits
  `667abf9a`/`15e9b1a1`: `corpus-threshold.py` now returns a real exit code (was always
  0); `eval-adapter.sh`'s 5 mechanical bugs fixed (wrong hardcoded base model, unnecessary
  `~/training-venv` dependency that doesn't exist on this VM, vestigial `yq` check,
  mislabeled `corpus_pairs` metric) and the abandoned duplicate `bin/eval-adapter.sh`
  deleted. Verified end-to-end: generated the first-ever real holdout set (388 pairs) and
  ran a live dry-run against the on-disk adapter — reached the pass@5 step, correctly
  FAILed (expected, no adapter loaded into Tier A yet). Follow-up text-bug fix: commit
  `62879862` (printed next-step said a non-existent `--lora-adapters` CLI flag).
- [~] **Adapter→Tier A promotion path — researched, documented, NOT implemented**
  (commit `688ff975`). Operator question: "is the adapter the whole point of Tier B, do
  we need a smoother transition?" Answer: yes to both, and the right building blocks
  already exist — `deploy-gate.sh`+`score-gate.sh`+`_gate-common.sh` (built by this same
  archive on 2026-07-02, commit `e66048e5` — see the SLM-production audit section below;
  today's session re-verified them directly, they weren't newly discovered) already
  implement a safe, tested, production-decoupled hot-swap (scratch `llama-server` on
  port 8090, `--lora --lora-init-without-apply` + `POST /lora-adapters`), and already
  correctly FAILed a synthetic test adapter. **Recommended next step, not done this
  session**: retire `eval-adapter.sh`'s own (cruder) scoring in favor of these two,
  keeping only its registry-write role; reconcile the two different holdout-file schemas
  (`eval-prepare.py`'s fresh 388-row `prompt`/`expected` vs. `score-gate.sh`'s stale
  76-row `instruction`/`brief_id`/`task_type`). **Why nothing was promoted**: the one
  real on-disk adapter (`apprenticeship-pointsav-incremental`) is trained against the
  wrong base (`OLMo-2-1124-7B-Instruct` vs. canonical `OLMo-3-7B-Instruct`) — needs
  retraining before any of this matters for it. Full scaffolding-completeness inventory
  and exact next-step instructions in the BRIEF so this doesn't need re-researching.
  [2026-07-03 totebox@claude-code]

## Hot — nightly yoyo-batch cycle: automated training has been silently failing for weeks (2026-07-02)

- [~] **`~/Foundry/bin/yoyo-daily-cycle.sh` Phase 6 passed `--queue-done`/`--engineering-corpus`
  flags to `run-dpo-training.py` that have never existed** (confirmed via
  `git log -S'"--queue-done"'` / `-S'"--engineering-corpus"'` on this repo — zero hits,
  ever). Every substantial cycle log back to 2026-06-20 shows the identical
  `run-dpo-training.py: error: unrecognized arguments` / `rc=2` failure — the automated
  nightly path had never successfully triggered training in at least 2 weeks. All real
  training progress this cycle (Runs 14-18) came from manual `test-mode.sh` invocations,
  not this automated path — consistent with this finding, not contradicted by it.
  Flagged high-priority + time-sensitive (msg-id
  `command-20260702-time-sensitive-yoyo-daily-cycle-sh-phase`), ~20 min before the
  UTC-midnight budget reset that would trigger the identical failure again.
  **Command fixed same-day, commit `1ec3951`** (landed minutes before the reset): syncs
  `export-sft.py` to the remote VM, runs `--source=all` to merge corpora, then calls
  `run-dpo-training.py --corpus <dir> --mode sft`. Correctly left `run-sft-training.py`
  (the *other* training-method path, which already had working
  `--queue-done`/`--engineering-corpus` args) untouched. Not smoke-testable against the
  live remote VM at fix time (stopped between cycles) — logic-verified only.
  **Live-tested 2026-07-03T00:38Z (this archive watched it happen)**: STOCKOUT cleared on
  attempt 3, Phase 6 ran the new code path for real — **progress, not yet success**. New,
  more specific failure: `[export-sft] ERROR: task-type dir not found:
  .../data/training-corpus/apprenticeship/git-commit`. Root cause found and reported
  (msg-id `command-20260703-tonight-s-cycle-ran-your-fix-new-more-pr`): the fix's rsync
  step still copies `data/apprenticeship/queue-done/` (flat `<brief_id>.brief.jsonl`
  dispatch records, 4604 files) to the remote VM, but `export-sft.py`'s
  `load_apprenticeship()` (default `--task-type=git-commit`) expects a **different,
  per-task-type-subdirectoried source**: `data/training-corpus/apprenticeship/<task_type>/
  shadow-*.jsonl` — confirmed real and populated locally (`.../apprenticeship/git-commit/`
  has 2,263 files; 10 other task_type siblings exist too). The rsync source needs to
  change from `queue-done/` to `training-corpus/apprenticeship/` (preserving its
  subdirectory structure) — still Command-scope (`yoyo-daily-cycle.sh` is workspace-root).
  Cycle failed cleanly otherwise — no crash, no stranded VM, no receipt written (correctly
  didn't touch production).
  **Watched live all night (2026-07-02/03), 6 separate stints**: 00:38Z, 01:02Z, 01:46Z,
  02:51Z, 03:35Z, 03:58Z UTC — every single one hit the identical rsync-source error
  (STOCKOUT accounted for most of the gaps between stints; every time the VM actually
  came up, the failure was the same). Sent a follow-up status-check to Command
  (msg-id `command-20260703-status-check-same-export-sft-rsync-sourc`) at the 5th
  occurrence, no reply as of the 6th. Today's budget (7200s) is now down to ~938s — one
  more short attempt possible tonight, otherwise resumes at the next UTC-midnight reset.
  Cost across all 6 stockout-and-fail stints tonight: ~$0.76 total (0.140+0.125+0.126+
  0.124+0.127+0.113, VM-on time only, no training compute — every attempt correctly
  aborted before any GPU work began).
  **Wrapped up watching 2026-07-03T~04:00Z**, at operator request — live monitor stopped.
  No reply from Command on the rsync-source fix as of wrap-up.
  **Exact patch sent 2026-07-03 (msg-id `command-20260703-exact-patch-for-the-rsync-source-bug-3-l`)**,
  read directly from the live `~/Foundry/bin/yoyo-daily-cycle.sh` rather than inferred from
  logs — 3 line changes, all in the Phase 6 dpo branch:
  - **Line 543** (root cause): `_QUEUE_DONE_SRC="${FOUNDRY_ROOT}/data/apprenticeship/queue-done"`
    → `_QUEUE_DONE_SRC="${FOUNDRY_ROOT}/data/training-corpus/apprenticeship"`.
  - **Line 544** (local existence check, flat→recursive): `ls "${_QUEUE_DONE_SRC}"/*.brief.jsonl`
    → `find "${_QUEUE_DONE_SRC}" -name 'shadow-*.jsonl' -print -quit | grep -q .`.
  - **Line 570** (remote post-rsync count, same flat→recursive fix): `find
    '${REMOTE_QUEUE_DONE}' -name '*.brief.jsonl'` → `-name 'shadow-*.jsonl'`.
  Rsync itself (line 545-549) needs no change — already recursive, will correctly mirror
  the per-task-type subdirectory tree once the source path is right. Asked the operator
  how to proceed (send patch / apply directly with sign-off / keep waiting) — no response
  in time, defaulted to sending the patch (lowest-friction, keeps the Command-scope
  boundary intact). Next session: check `.agent/inbox.md` for a reply, or re-check
  `/srv/foundry/data/yoyo-cycle-logs/` for a cycle reaching `Phase 6: starting SFT
  training` (not `export-sft.py merge failed`) as the signal this is fixed.
  **Attempted to apply the 3-line patch directly (operator sign-off given) — blocked by
  the permission classifier**: correctly flagged the race risk of a Totebox session
  editing a shared workspace-root file with no worktree isolation while Command might be
  concurrently working the same file. Asked the operator how to proceed given that new
  information; decision: revert to patch-only, don't cross the boundary. File confirmed
  unchanged (`git status`/`git diff` clean on `bin/yoyo-daily-cycle.sh`). The patch stands
  as sent — resolution is Command's to apply.
  **Command applied it, commit `344ebe6`** (msg-id
  `command-20260703-applied-rsync-source-fix-landed-commit-3`) — verified all 3 changes
  against the live file before applying (matched exactly), also fixed two log-message
  strings still referencing the old `*.brief.jsonl` pattern for consistency.
  **Not yet live-verified**: checked at 2026-07-03T05:52Z — no cycle has run since the fix
  landed (04:43Z); today's day-budget was already exhausted (6922s/7200s) from last
  night's 6 stints, service just idling. Next real attempt is the next UTC-midnight
  budget reset (~18h out). Replied to Command acknowledging this
  (msg-id `command-20260703-ack-can-t-verify-tonight-today-s-budget-`).
  **Update 2026-07-03 — confirmed live-verified, then a NEW bug found+fixed.** The
  04:58:27Z cycle (first to run after commit `344ebe6` landed) DID reach `Phase 6:
  starting SFT training` — the rsync-source fix works. But it immediately crashed on
  `resume_from_checkpoint` with a LoRA rank-mismatch (`size mismatch ... torch.Size([16,
  4096])` vs `torch.Size([32, 4096])` on all 32 layers): the checkpoint was saved at
  r=16 (via `run-sft-training.py`'s corrected R1 hyperparams) but `run-dpo-training.py`'s
  internal SFT-fallback path (which is what actually runs nightly — see the
  marker-routing item below) was still hardcoded to the old r=32/alpha=64. **Fixed same
  session** (commit `be0a3ca5`): split `SFT_LORA_R`/`SFT_LORA_ALPHA` constants, added a
  fail-loud rank-compatibility guard at all 3 resume sites in both training scripts.
  0/82 cycles produced an adapter that day as a result of this bug; the fix auto-deploys
  next cycle since `yoyo-daily-cycle.sh` rsyncs the training scripts fresh from this
  archive's `service-slm/scripts/` on every run — no Command action needed for this part.
  **Two more structural gaps found the same day** (both Command-scope,
  `bin/yoyo-daily-cycle.sh`): Phase 6 never actually checks corpus-threshold.py's
  floor/quality decision before training (just checks marker-file existence — and 42
  stale markers from 2026-05-08 mean it's been training regardless of readiness), and
  `eval-adapter.sh` is never invoked anywhere in the cycle (confirmed via
  `grep -c "eval-adapter"` → 0), which is the literal reason `data/adapters/registry.yaml`
  has always been empty. Exact patch for both (Gate 6 + new Phase 6c + marker archival)
  sent to Command (msg-id `command-20260703-exact-patch-for-yoyo-daily-cycle-sh-corp`) —
  **not yet applied as of shutdown** (no new commit on `yoyo-daily-cycle.sh` since
  `344ebe6`). Full detail: `.agent/briefs/BRIEF-flow-quality-audit.md`.
  **Checked 2026-07-04**: `cycle-20260704-000945.log` reached `Phase 6: starting SFT
  training` (rsync-source fix holds) but crashed again — a *different* rank mismatch
  than the one F1 fixed: checkpoint-49 was saved at r=16/alpha=32, but F1's own
  `SFT_LORA_ALPHA=8` didn't match that. F1's guard caught it correctly (fail-closed,
  clear message, no raw crash) — fixed same day by realigning `SFT_LORA_ALPHA`
  (both scripts) 8→32 to match the checkpoint. Live GPU re-test to confirm this holds
  is pending — see `.agent/briefs/BRIEF-flow-quality-audit.md` 2026-07-04 entry.
  **Still open**: whether Command applied the Gate-6/Phase-6c patch (look for a new
  commit on `~/Foundry/bin/yoyo-daily-cycle.sh` past `344ebe6`, or a cycle log reaching
  `Phase 6c: PASS`/`Phase 6c: FAIL`). [2026-07-02/03/04 totebox@claude-code]
- [ ] **Live GPU re-test of the 2026-07-04 rank-alpha realignment** — `test-mode.sh` run
  queued to confirm Phase 6 completes past the rank-mismatch point against the real
  remote checkpoint (code fix only verified via `py_compile` so far, not a live run).
  [2026-07-04 totebox@claude-code]
- [x] ~~**GAP-4 mislabel — actually fixed 2026-07-04**~~ — the 2026-06-23 fix only ever
  touched the non-authoritative `zz-foundation.conf` drop-in; `/etc/local-doorman/
  local-doorman.env`'s `EnvironmentFile=` (which wins over `Environment=` drop-ins at
  process spawn) still had the wrong value. Edited directly, restarted, verified via
  `/proc/<pid>/environ` + a fresh `ask_local` call: `model=OLMo-3-7B-Instruct`.
  [2026-07-04 totebox@claude-code]
- [x] ~~**Resolved 2026-07-03**: the corpus-threshold/Phase-6 gate mismatch flagged
  below is a real, un-intentional bug, not independent-by-design~~ — confirmed by
  reading `yoyo-daily-cycle.sh`'s Phase 6 gate chain directly: it's a 5-condition
  `if/elif` (marker exists, authorization tag, no double-spend receipt, budget>0, ML
  libs installed) that never consults `corpus-threshold.py`'s floor decision at all.
  Training has been attempting every night purely because stale marker files exist,
  regardless of corpus readiness. Fix (Gate 6, exact patch) sent to Command alongside
  the eval-adapter.sh-never-wired fix above — see that entry. [2026-07-03 totebox@claude-code]

## Housekeeping — drift cleanup + NEXT.md hygiene (2026-07-02)

- [x] ~~**Fixed**: `.agent/session-start.md` had 100% project-workplace content~~ — rewritten
  with correct project-totebox mission/branch/gotchas/handoff. [2026-07-02 totebox@claude-code]
- [x] ~~**Fixed**: `.agent/rules/brief-discipline.md` title said "project-design"~~ — corrected
  to project-totebox (same copy-templating bug class seen in other archive clones).
  [2026-07-02 totebox@claude-code]
- [x] ~~**Fixed**: `.agent/briefs/README.md` Active-BRIEFs table misfiling~~ — moved
  `BRIEF-sel4-unikernel.md` (project-console's os-console, not this archive's work) to
  Reference; moved `BRIEF-os-totebox-ppn-build-out.md` (its own frontmatter says
  `status: superseded`) to a new Superseded table. [2026-07-02 totebox@claude-code]
- [x] ~~**Fixed**: `BRIEF-slm-tier-split-architecture.md` `owner:` field~~ — `project-data` →
  `project-totebox` (pre-merge name; content itself was always correctly in-scope).
  [2026-07-02 totebox@claude-code]
- [x] ~~**NEXT.md self-consolidation**~~ — struck several stale duplicate checkboxes (Run 17
  null-delta finding, P0-1/P0-2/P0-4 "(verified)" duplicates, R1 "possibly wrong" duplicate,
  Doorman two-OLMo-instances rejected-decision item) that were already resolved elsewhere in
  this same file but left unchecked. [2026-07-02 totebox@claude-code]
- [ ] **Flagged, not fixed**: root `/srv/foundry/clones/project-totebox/README.md` predates this
  archive (committed 2026-05-25, org-chart file-naming content for a different deployment) and
  gives no os-totebox orientation. Not touched this session — rewriting a pre-existing shared
  monorepo-root file needs its own scope decision, not a drift-cleanup side effect.
  [2026-07-02 totebox@claude-code]
- [ ] **Flagged, not fixed**: 7 active BRIEFs vs. this archive's own `brief-discipline.md`
  soft cap of 5 (after moving 2 out to Reference/Superseded this session, down from 9).
  Candidate for further consolidation in a future session — not forced here.
  [2026-07-02 totebox@claude-code]

## Hot — live flow smoke test found + fixed a real bug (2026-07-02)

- [x] ~~**KoGNER entity-hint sampling picked up graph noise, degrading GLiNER extraction to
  zero**~~ — found via an actual live end-to-end test (wrote a real test corpus document
  into the live ingestion directory, watched it drain, confirmed the graph never got the
  expected entities). Root cause: `service-content/src/entity_hints.rs`'s
  `init_entity_hints()` samples up to 3 example names per classification from raw graph
  data with no quality filter, then appends them to GLiNER's label descriptions as
  "concrete examples." It had picked up pre-existing noise — the literal string
  `"Person"` as a "Person" example, and phrases like `"no named person identified"`,
  `"different location"`, `"candidate locations"` — corrupting the label semantics GLiNER
  uses for zero-shot matching. Live-verified: a test document that GLiNER reliably
  extracts 4 correct entities from (direct `/v1/extract` calls, twice, same text) came
  back with **zero entities** through the real pipeline once entity_hints were included.
  Fixed (commit `edd22edf`): `is_valid_hint_candidate()` rejects self-referential
  label-name matches and lowercase-initial common-noun phrases, reuses
  `entity_filter::is_noise_entity_name` for defense-in-depth; also factored the bucketing
  logic into a pure `build_hints()` fn to fix a latent test-isolation race the new tests
  exposed. 97/97 tests green (was 92), clippy clean. **Cannot deploy from this
  session** — `bin/deploy-binary.sh` has an explicit Command-only scope guard, and
  requires Stage-6-promoted HEAD first. Flagged to Command via outbox (msg-id
  `command-20260702-stage-6-deploy-request-entity-hints-rs-f`), folded into the existing
  157-commit Stage 6 backlog. [2026-07-02 totebox@claude-code]
- [ ] **Note for the eventual graph-noise-cleanup pass** (already tracked elsewhere in this
  file): the specific noise entries this bug surfaced (`"Person"`, `"no named person
  identified"`, `"different location"`, `"candidate locations"`, similar generic-phrase
  entries under Location/Project) are good concrete targets. This fix only stops them
  from being used as hints going forward — it doesn't clean the existing graph.
  [2026-07-02 totebox@claude-code]
- [x] ~~**Left in place**: `CORPUS_smoketest_20260702.json`~~ — the test file used to
  surface the bug above, sitting in the live `service-content/ledgers/` directory.
  Matches existing precedent (`CORPUS_aaa_test_flow_*.json` and similar fixtures already
  there from earlier sessions) — left rather than removed, per established practice in
  this archive. [2026-07-02 totebox@claude-code]

## Hot — SLM-production + content-quality trajectory audit (2026-07-02)

Second Fable audit (9 agents) this session, distinct scope from the first. Full verdicts +
17-item refined roadmap in `.agent/briefs/BRIEF-flow-quality-audit.md` §"SLM-production +
content-quality trajectory audit". Condensed:

- [x] ~~**NEW P0 — `/v1/draft/generate` UTF-8 panic**~~ — **fixed**, commit `9f96bafa`.
  `truncate_at_char_boundary()` replaces the fixed-byte-offset slice; 3 new unit tests
  (including one reproducing the exact multi-byte-at-boundary shape that panicked the old
  code); 86/86 tests green, clippy clean. [2026-07-02 totebox@claude-code]
- [~] **NEW P0 — signal-inflow restoration**: training corpus is ~100% static. Three
  sub-parts, different status each:
  - 3a. **service-input deploy**: nothing more for Totebox to do — sysadmin package already
    sent to Command (msg-id `command-20260701-action-required-service-input-never-depl`),
    confirmed still `status: pending` in Command's inbox as of 2026-07-02.
  - 3b. **Engineering-capture gap — ROOT-CAUSED 2026-07-02** (corrects the prior finding
    below). The "zero entries after 2026-06-01T18:30:00Z" claim was itself slightly wrong:
    that timestamp belongs to `queue-done/test-shadow-001.brief.jsonl` — a synthetic test
    fixture, not a real commit capture (confirmed by filename + compact-JSON formatting
    that doesn't match the real emitter's output). The **actual** last genuine
    `shadow-capture` entry is `2026-05-29T01:55:37Z` (verified via `grep` across all 872
    `shadow-capture`-tagged files in `queue-done/`). Root cause found via git-log
    archaeology on the **workspace** repo (`~/Foundry`, not this archive): commit
    `dacffb1` (2026-05-29T16:31:55Z, "ops(workspace): JOURNAL artifact type, J2 citations,
    capture-edit production swap...") replaced the 541-line `bin/capture-edit.py` — the
    §7C Brief Queue Substrate script that wrote `task_type: "shadow-capture"` to a
    file-backed queue, with role/scope auto-detection (master/task/root by CWD) AND
    secret-sanitization (redacted private keys, AWS keys, API-style tokens, high-entropy
    bearer tokens) — with a 47-line bash script (byte-identical to this archive's own
    `service-slm/scripts/git-post-commit-hook.sh`) that POSTs directly to `/v1/shadow`
    with `task_type: "git-commit"` and **no sanitization logic at all** (confirmed via
    grep — zero redact/sanitize/REDACTED hits in either current hook script). Two
    distinct findings, not one:
    1. Whether "git-commit" is an intentional rename/simplification of "shadow-capture"
       or an accidental loss of a distinct training-signal category is a design question
       for whoever owns `bin/capture-edit.py` — **Command Session scope, not this
       archive's to fix** (workspace-root file).
    2. **Security-relevant regression, flagged separately**: the current live hook sends
       raw, unsanitized git diffs to the Doorman on every commit across all clusters —
       the secret-redaction logic that existed in the deleted Python script is gone.
       This is true regardless of the task_type naming question and should be
       prioritized independently.
    Both flagged to Command via outbox this session — not actionable from this Totebox
    session (the file lives at `~/Foundry/bin/capture-edit.py`).
  - 3c. **Apprenticeship verdict wiring — done**. `service-slm/scripts/
    list-pending-apprenticeships.py` (commit `b75d6d45`) makes the queue visible; does not
    cast verdicts itself (human-reviewed only, per SYS-ADR-07 + model-collapse evidence).
    Live run confirms the scale of the gap precisely: 4,604 total shadow attempts, only 10
    ever verdicted (0.2%).
  [2026-07-02 totebox@claude-code]
- [~] **NEW P0 — scored promotion gate**: re-ranked from P1 to P0. Run 18's delta-count gate
  is direction-free (proves change, not improvement) — auto-promotion is unbuildable on it.
  **Built and committed** (`e66048e5`): `service-slm/scripts/score-gate.sh` + `_gate-common.sh`
  (shared scratch-server infra factored out of `deploy-gate.sh`, which was refactored onto
  it — confirmed behavior-preserving via dry-run before/after). Scores diff-parse,
  git-apply-check, and envelope-format-compliance against the 76-row curated holdout set
  (stratified by task_type); entity-F1 correctly reported as `"available": false` (blocked
  on the still-unbuilt `service-gliner/eval/` P1 item) rather than faked.
  **Live validation — logic confirmed, full run blocked by VM contention, not a scoring
  bug**: first live attempt (12 probes) surfaced and fixed a real bug — the inherited
  128-token probe budget truncated every diff completion before its closing fence, so
  every "pass" was actually a truncation artifact (completion_len clustered at ~410 chars).
  Fixed with a score-gate.sh-specific 768-token / 480s-timeout override. Second live
  attempt (6 probes) confirmed the fix works (completions reaching 4000+ chars) and that
  the scoring logic itself is sound — the one probe that completed inside the timeout
  scored non-degenerately (envelope-format passed, diff-parse correctly failed, a
  believable mixed result). The other 5/6 probes returned empty at exactly the timeout
  boundary — this VM's CPU contention (production Tier A queue) exceeded even the bumped
  480s window. **Not yet obtained**: a full clean multi-probe live run — needs either a
  quieter CPU window or the P1 "sanctioned full-run mode" infra pulled forward. Not
  claiming a pass or fail on the Run 18 adapter from this round — only that the instrument
  itself is now built correctly and partially proven live. [2026-07-02 totebox@claude-code]
- [ ] **binary-targets.yaml audit (Command broadcast, actioned but with a real gap)**:
  this archive has no `.agent/binary-targets.yaml` yet. Nothing added/changed a `[[bin]]`
  target this session, so the shutdown-checklist trigger didn't fire — but the archive's
  existing services (`service-content`, others) have never been inventoried against the
  schema. Needs its own pass: `bin/binary-registry-report.sh --archive project-totebox`
  first, then create the file per `conventions/soft-distribution-pipeline.md` §3.
  [2026-07-02 totebox@claude-code]
- [ ] **NEW P1 — context-assembler v2**: verified the entire existing P1 DataGraph roadmap
  (edges, ER, embeddings, spans) is write-side only — `format_entity_block` (http.rs:700-719)
  and Doorman's `GraphContextClient` both render a flat name/classification/role/location/
  contact block, never edges/confidence/source_doc/temporal order. Completing P1 as speced
  today changes generated content **zero**. [2026-07-02 totebox@claude-code]
- [ ] **Sequencing correction**: entity resolution before/with co-occurrence edges (was:
  edges Stage 1, ER later) — DEG-RAG evidence says edges on fragmented duplicate nodes degrade
  generation. [2026-07-02 totebox@claude-code]
- [ ] **R1 re-sequenced**: run AFTER the scored gate, not before/first — a direction-free gate
  can't rank two passing adapters. [2026-07-02 totebox@claude-code]
- [x] ~~**Corrected**: proofreader is fully live~~ — verified via `journalctl -u
  local-proofreader.service` (real completed request, `degraded=[]`). Earlier session
  background (drawn from a stale README) was wrong; corrected in the BRIEF.
  [2026-07-02 totebox@claude-code]
- [x] ~~**Corrected**: `corpus-threshold.py --force` daily-bypass claim (from the first Fable
  audit) is now stale~~ — `local-corpus-threshold.timer` is masked; live path
  (`yoyo-daily-cycle.sh:429`) passes no `--force`. New real defect found instead:
  `corpus-threshold.py:91` schema misread zeroes all 1,410 engineering rows.
  [2026-07-02 totebox@claude-code]
- [x] ~~**Outbox to project-console**~~ — sent, msg-id `command-20260702-proofreader-corrections-live-pipeline-st`.
  [2026-07-02 totebox@claude-code]
- [x] ~~**Note to Command Session**~~ — sent, msg-id `command-20260702-stale-doc-infrastructure-local-proofread`.
  [2026-07-02 totebox@claude-code]
- [ ] **Operator decision needed**: Loop B's self-rewarding OLMo-judge design conflicts with
  model-collapse evidence under SYS-ADR-07 (no external judge can curate) — needs a BRIEF-level
  amendment, not a footnote. Also: from-base-each-cycle vs. incremental `--resume` (the nightly
  cycle already does the latter) is an unresolved conflict, not a lockable default.
  [2026-07-02 totebox@claude-code]

## Hot — 100x roadmap Phase 1 implementation (2026-07-01, in progress)

- [x] ~~**P0-4 DONE + DEPLOYED + LIVE-VALIDATED**~~ — `service-content/src/graph.rs` MERGE now
  coalesces role/location/contact_vector (new non-empty wins, empty preserves existing) and
  source_doc (first-write-wins). Commit `b3ae1936`. 3 new unit tests + 83/83 suite green,
  clippy clean. Rebuilt release binary, backed up old `/usr/local/bin/service-content`,
  restarted `local-content.service`, confirmed healthy. **Live HTTP smoke test via the real
  `/v1/graph/mutate` + `/v1/graph/context` API confirms the fix**: wrote an entity with
  role_vector+location_vector, re-mutated with null vectors (simulating a GLiNER re-mention),
  read back — both vectors survived (would have been blanked to null before this fix). Test
  entity lives in isolated `module_id=test-p0-4`, does not affect real data.
  [2026-07-01 totebox@claude-code]
- [x] ~~**P0-1 (deploy-gate.sh rewrite) DONE + VALIDATED**~~ — commits `bfc165fe`, `a0f75826`.
  Scratch-server scale-toggle protocol replaces the broken PEFT-directory-path approach. 3
  bugs found+fixed while validating (missing `requests` in conversion venv, `/lora-adapters`
  readiness race extended 60s→180s, missing `--no-jinja` crashed the scratch server). Validated
  zero-GPU-cost against the real Run 17 adapter (14/20 delta, honest FAIL for that undertrained
  adapter) before spending GPU on Run 18. [2026-07-01/02 totebox@claude-code]
- [x] ~~**P0-2 (trainer config) DONE + VALIDATED**~~ — commit `b133383d`. eval_strategy
  steps→epoch, save_steps 5→25, eval split capped ~64 rows, packing=True, MAX_LENGTH 512→2048,
  fail-closed truncation pre-check added. Live-confirmed in Run 18: checkpoint-94 vs prior
  checkpoint-21 (~4.5x more optimizer steps in the same capped window).
  [2026-07-01/02 totebox@claude-code]
- [x] ~~**Run 18 — combined validation**~~ — **FIRST-EVER REAL PASS**: 16/20 probes non-trivial
  delta (threshold 15), 4/20 null (all the same known terse-prompt base-model pattern, not
  adapter-related). `passed:true` in `/srv/foundry/data/adapters/deploy-gate-result.json`. Full
  writeup: `.agent/briefs/BRIEF-flow-quality-audit.md` §"Phase 1 implementation". Delayed ~35min
  mid-run by the production nightly cycle legitimately holding the VM lock — waited it out
  rather than force a collision (lock worked as designed). [2026-07-02 totebox@claude-code]
- [ ] **R1 (LoRA alpha/r ratio) experiment** — now unblocked, gate is trustworthy. Compare
  `r=16/alpha=8` (0.5 ratio) against `alpha>=r` per current literature. [2026-07-02 totebox@claude-code]
- [ ] **P5 promotion decision** — deliberately NOT made. Smoke-scale PASS proves pipeline
  correctness, not production readiness. Needs a larger/uncapped run + explicit operator
  sign-off before any `--lora-scaled` activation. [2026-07-02 totebox@claude-code]
- [ ] **P1 roadmap items** (full detail in BRIEF) — not yet implemented, next session's scope
  pending operator go-ahead per item. [2026-07-02 totebox@claude-code]

Last updated: 2026-07-02

---

## Hot — active this session (2026-06-23)

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

- **v0.2.0** — multi-module rewrite (Phase A routes, Phase B SSE, Phase C edit overlay, Phase D AI bridge); binary deployed 2026-06-20 sha256 1883110e; canonical commit 8c540cd4
- **DESIGN-BUNDLE ratified** — namespace component.document.legal.* confirmed 2026-06-20
# NEXT.md — project-editorial (Totebox)

> **Scope: this archive only.** Cross-repo and workspace-level items live at `~/Foundry/NEXT.md`.
> Hot open items. ≤200 lines. Backlog at `.agent/next-backlog.md`.

Last updated: 2026-06-20 (gate clean — 0 dead links, 0 MISSING sections)

**→ project-editorial (supplemental dispatch 2026-05-17 — Opus army synthesis):**
- [x] 11 NEW TOPIC drafts for `content-wiki-projects/topics/bim/`
  - topic-bim-building-width-method + zone-depths-per-use-type (Agent 1)
  - topic-bim-floor-plate-methodology + tile-system + tile-combinations + leasing-efficiencies (Agent 2)
  - topic-bim-key-plans-index + private-office + medical + business + professional-office (Agent 3)
- All structured as living documents (Future research sections for iteration)

---

## Active (Totebox scope)

- [ ] **Stage 6 pending** — Command: promote all media-knowledge-* sub-clones
  - media-knowledge-projects: 7fa466b, 3e3579b, bef1c2e, 58dbe9b, 45ea336, f4aa1ef, b2a92d4
  - media-knowledge-corporate: ac6379f, 981809f
  - media-knowledge-documentation: f1451e9, a971310, 281bc0d, d5bdae9, c6ecf4e, 8fa30e9, a88b9c7, 3cb31b4, 028832e, dcd40d7, 63e68c5
  [2026-06-20 totebox@claude-code]

## Blocked — Command Session (route via outbox)

- [ ] **Trademark Phase 1a** — factory-release-engineering (TRADEMARK.md, tokens/legal-tokens-*.yaml, readmes/footer-*.md, policies/DISCLAIMER.md, README.md, PLAYBOOK.md); outbox message sent [2026-06-19 totebox@claude-code]
- [ ] **Trademark Phase 4** — woodfine-fleet-deployment GUIDEs (~80 files), workspace governance docs (CLAUDE.md, AGENT.md, conventions/); admin-tier [2026-06-19 totebox@claude-code]

---

## Completed (2026-06-20)

- [x] **wiki repo migration** — ~40 MCorp research/BIM articles moved from media-knowledge-documentation to media-knowledge-projects with topic- prefix naming and correct archetype terminology (PRO=Retail Centres, VWH=Urban Fringe, PKS=Commuter); wikilink slugs updated; gate clean 0/0 after migration [2026-06-20 totebox@claude-code]
- [x] **gate clean** — 0 dead links, 0 MISSING sections across 788 articles (all 3 wikis) [2026-06-20 totebox@claude-code]
- [x] **Dead link sweep (F2/F3)** — 29 dead links resolved: stub articles created (service-vm-fleet, service-vm-tenant, location-intelligence-archetypes), wikilink fixes, cross-wiki link removal; commit 8fa30e9 [2026-06-20 totebox@claude-code]
- [x] **M9 media-knowledge-documentation parity sweep** — ES articles expanded to full parity across architecture/, substrate/, reference/, applications/; commits 281bc0d, d5bdae9, c6ecf4e, a88b9c7 [2026-06-20 totebox@claude-code]
- [x] **TOPIC intake — Phase B** — 9 TOPICs editorial clearing + EN+ES committed to media-knowledge-documentation [2026-06-20 totebox@claude-code]
- [x] **Inbox actioning** — trademark pivot messages actioned; zero content-wiki edits needed [2026-06-20 totebox@claude-code]
- [x] **Non-TOPIC routing** — outbox messages sent to project-design, project-documents, project-data, Command [2026-06-20 totebox@claude-code]

## Completed (2026-06-19)

- [x] **Trademark Phase 3 — TOPIC/GUIDE content wikis** — MCorp™ + Capability Geometry™ applied across all three sub-clones (documentation, projects, corporate); 3 commits (3e3579b, ac6379f, f1451e9); body text editorial pass done; formal legal disclaimers preserved; copyright lines corrected to Woodfine Capital Projects Inc. [2026-06-19 totebox@claude-code]
- [x] **NEXT.md contamination cleanup** — removed project-gis, project-console, project-intelligence, project-workplace, project-design content [2026-06-19 totebox@claude-code]
- [x] **M7 snapshot dating** — corrected 7,594 → 6,493 in index.md; methodology-example note in dedup article; commit 4649f95 [2026-06-19 totebox@claude-code]
- [x] **M9 EN/ES parity sweep** — all 53 ES articles in media-knowledge-projects at 84%+; 5 commit passes (f7a9be5, 6310748, 1c5d2db, ba4c412, 7fa466b) [2026-06-19 totebox@claude-code]
# NEXT.md — project-console

> **Scope: this archive only (pointsav-monorepo Totebox).**
> Cross-repo and workspace-level items live at `~/Foundry/NEXT.md`.
> Out-of-scope items route to outbox, not this file.

Last updated: 2026-06-19 [Jennifer Woodfine / claude-code]

---

## Phase H1 — seL4 unikernel substrate — COMPLETE 2026-06-19

| Item | Status | Notes |
|---|---|---|
| vendor-sel4-kernel AArch64 build | COMPLETE | `build/aarch64-qemu/kernel.elf` (910K, AArch64 ELF) |
| moonshot-sel4-vmm `#![no_std]` PD runtime | COMPLETE | `lib.rs`, `syscall.rs`, `debug.rs`, `types.rs`; seL4 ABI wrappers; cfg-gated AArch64 asm |
| `console_hello.c` bare-metal PD + TOML spec | COMPLETE | `moonshot-toolkit/examples/console_hello.c`; `os-console-hello.toml` |
| moonshot-toolkit image build | COMPLETE | `build/system-image.bin` (1.1M elfloader ELF) built via separate target-dir to avoid cargo-lock contention |
| QEMU boot verification | **GATE PASSED** | `Hello from os-console seL4 PD` on serial; QEMU `-m 1G` required (DTB reports 1 GiB; 512M causes Data Abort) |
| Phase H1 commit | COMMITTED | All Phase H1 files staged and committed |

[2026-06-19 totebox@claude-code]

## Phase H2 — seL4 substrate continuation (multi-day, see BRIEF-sel4-unikernel.md)

### H2a — Rust rootserver — GATE PASSED 2026-06-19

| Item | Status | Notes |
|---|---|---|
| `CompileRustPd` step in moonshot-toolkit | COMPLETE | `spec.rs` `rust_bin: Option<String>`; `plan.rs` `CompileRustPd` variant; `main.rs` `compile_rust_pd()` — cargo build → `aarch64-unknown-none --release` |
| `moonshot-sel4-vmm/src/bin/console_main.rs` | COMPLETE | Pure Rust `_start()` → `vmm::write_bytes(BANNER)` → `vmm::spin()`; no C |
| `moonshot-toolkit/examples/os-console-rust.toml` | COMPLETE | `rust_bin = "console_main"` spec |
| QEMU boot verification | **GATE PASSED** | "Hello from moonshot-sel4-vmm (Rust)" on serial; chardev file: `-chardev file,id=s0,path=/tmp/sel4-serial.log -serial chardev:s0 -m 1G` |

### H2b — Two PDs + seL4 IPC (Day 2, ~6-10 hours)
- [ ] `moonshot-sel4-vmm/src/bootstrap.rs` — rootserver CSpace/VSpace setup (~150 lines)
- [ ] counter-pd + receiver-pd (C or Rust)
- [ ] `moonshot-toolkit/examples/os-console-ipc.toml` — 3-PD spec
- **Gate:** "IPC received: N" printed by receiver-pd via rootserver-distributed endpoint cap.

### H2c — UART MMIO from user space (Day 3, ~4-6 hours)
- [ ] Rootserver maps PL011 UART page (0x09000000) into console-pd VSpace
- [ ] Direct MMIO write to UART DR/FR registers (no SysDebugPutChar)
- **Gate:** "Hello via MMIO UART" from PD-direct register write.

### H3 — VirtIO serial + ratatui (Week 2, 2-3 days)
- [ ] VirtIO MMIO serial driver (QEMU virt 0x0a000000+; virtqueue rings)
- [ ] ratatui backend — TestBackend → buffer → VirtIO write per line
- **Gate:** ratatui layout (borders + 2 panes) visible in QEMU serial output.

---

## Phase 9 — Operations — COMPLETE 2026-06-14

| Item | Commit | What shipped |
|---|---|---|
| 1 — Graceful SIGTERM | `3e20be12` | `AtomicBool` + ctrlc handler; `request_shutdown()`; terminal restored on `systemctl stop` |
| 2 — fail2ban port 2222 | `5efb513d` | `infrastructure/fail2ban/jail.local` + filter; 5-retry, 1h ban |
| 3 — Prometheus metrics | `3e20be12` | `os_console_up` / `os_console_uptime_seconds` / `os_console_info` on loopback :9299; `metrics_port` config field |
| 4 — Multi-tab ContentCartridge | `a27860b3` | `TabSnapshot` + `Vec<tabs>`; Ctrl-T open, Ctrl-W close, Ctrl-Tab cycle; max 4 tabs; tab bar on >1 tabs |

---

## Stage 6 pending (Command scope — route via outbox)

All Phase 8+9+10+T0 commits + 2026-06-19 need `bin/promote.sh` from Command Session:

| SHA | Subject |
|---|---|
| `6f21f580` | feat(release): Phase B — CI matrix, rustls-tls, TerminalCaps |
| `d9261705` | ops(session): Phase B complete |
| `d58960b4` | ops(brief): mark Phase B complete |
| `5c36ce66` | ops(monorepo): remove .agent/ from git index |
| `5efb513d` | ops(fail2ban): port 2222 brute-force protection |
| `3e20be12` | feat(sigterm+metrics): SIGTERM + Prometheus |
| `a27860b3` | feat(tabs): multi-tab ContentCartridge |
| `2c21e142` | ops(phase9): mark complete — NEXT.md + BRIEF |
| `469b7147` | test(tabs): 9 unit tests for tab management |
| `bc95acfa`..`fc4d0978` | Phase 10 commits (F2 People, reconnect watchdog, session persistence) |
| `5dab352e`..`91eb2148` | T0 pairing + tunnel fixes |
| `c9084667` | feat(content): pdfium-render optional — pdf feature flag |
| `3816794d` | docs(briefs): BRIEF-macos-binary-mac-pro |
| `0e8cfef5` | docs(sel4): BRIEF-sel4-unikernel + H2a/b/c/H3 roadmap; strip M-17 contamination from NEXT.md |
| `e25b6ad7` | feat(sel4): Phase H1b — CompileRustPd build step in moonshot-toolkit + AArch64 panic handler |
| H2a completion | feat(sel4): Phase H2a — Rust PD gate passed; console_main.rs + os-console-rust.toml |
| `2e0b47c5` | feat(sel4): Phase H8 — HTTP GET to Doorman /healthz; ARP reply + raw TCP; gate PASSED |

## darwin-x86_64 binary pending (waiting on Jennifer)

- [ ] Jennifer builds on Mac Pro: `cargo build --release --bin os-console`
- [ ] Jennifer scps binary to `mathew@34.53.65.203:/tmp/darwin-x86_64-0.2.4`
- [ ] Deploy: scp to foundry-prod + chmod (instructions in BRIEF-macos-binary-mac-pro.md)
- [ ] Then: `curl -fsSL https://software.pointsav.com/releases/os-console/install.sh | bash` on Mac Pro

---

## Operator-gated items

- [ ] GCE firewall: open port 2222 inbound
- [ ] Deploy `local-console.service` systemd unit + enable
- [ ] `pairing-server` systemd unit on GCE VM
- [ ] Peter SSH key: `proofctl user add peter --tenant woodfine --role editor`
- [ ] Tag `v0.1.0` on pointsav-monorepo (triggers GitHub Actions release build)
- [ ] Branch rename `cluster/project-proofreader → cluster/project-console` on GitHub

---

## Phase 10 — next coding sprint (in-scope when ready)

| Item | What |
|---|---|
| F2 People cartridge | `app-console-people` lib + `PeopleCartridge`; read-only from `service-people :9091` |
| Chassis reconnect watchdog | retry MBA connection on drop; backoff; indicator in status bar |
| `/audit` log viewer | tail `service-input` ledger; search; export |
| Tab labels from state | improve `tab_label()` to pull actual query/title text live |

---

## Standing deferred

- F7 BIM cartridge — gated on `app-console-bim` activation
- F10 mesh cartridge — gated on `app-console-mesh` activation; Phase 1 scope when ready: poll `service-vm-fleet :9203` GET /v1/nodes → read-only table (node ID | hostname | ip | status | last_heartbeat | preferred role); no writes
- F11 → :9202 endpoint — currently polls :9201; will connect to `service-ppn-pairing :9202` when project-infrastructure deploys it (PPN Phase 1)
- Phase 12 (AI marginalia) — gated on SYS-ADR-07/10/19 review
- **os-totebox Phase 2** — Veriexec strict=1, wm0 NIC fix, SSH via SLIRP validated; Stage 6 complete 2026-06-14 (canonical commit 090a090c)
- **service-vm-tenant v0.1.0** — Bearer auth + quota + WORM audit; 11 tests; Stage 6 complete 2026-06-14
- **service-vm-fleet + service-vm-host** — PPN fleet controller + heartbeat agent; Stage 6 complete 2026-06-14
- **moonshot-toolkit v0.3.1** — Rust-only seL4 build orchestrator; TOML spec → bootable image; QEMU gate passed 2026-05-29
- **moonshot-sel4-vmm Phase H1** — `#![no_std]` PD runtime; QEMU gate passed 2026-06-19
- **wiki leg** — 9 TOPICs on canonical media-knowledge-documentation; confirmed 2026-06-19
- [x] **Trademark Phase 3 — TOPIC/GUIDE content wikis** — MCorp™ + Capability Geometry™ applied across all three sub-clones; 3 commits (3e3579b, ac6379f, f1451e9) [2026-06-19 totebox@claude-code]
- [x] **M7 snapshot dating** — corrected 7,594 → 6,493 in index.md; commit 4649f95 [2026-06-19 totebox@claude-code]
- [x] **M9 EN/ES parity sweep** — all 53 ES articles in media-knowledge-projects at 84%+; 5 passes (7fa466b) [2026-06-19 totebox@claude-code]
# NEXT.md — project-gis (Totebox)

Hot open items. ≤200 lines. Backlog at `.agent/next-backlog.md`.
> **Scope: this archive only.** Cross-repo and workspace-level items live at `~/Foundry/NEXT.md`.

Last updated: 2026-06-25

---

## Active (Totebox scope)

### Map UX/tech audit — follow-through
- [x] **Mobile sneak-peek + tagline round PUBLISHED TO PROD 2026-06-22 (session 3).** Peek stops at search bar
      (PEEK_PX_OVERVIEW 152); NA↔EU tabs always tappable at peek; sheet snaps to peek on region switch; tagline
      "Spatial measures of retail, industrial, and transit / anchor clustering" — mixed case, no italic, two-line
      break. Commits f852fc7b / 1c4b1fdb / 3f9b61c2 / 47bf530d / 2daf1153 / 3087fe56. git-source ↔ deployment diff = 0.
      [2026-06-22 totebox@claude-code]
- [x] **PUBLISHED TO PROD 2026-06-22** — all the above shipped live to gis.woodfinegroup.com via `push-to-prod.sh gis`
      (operator-authorized; 738KB transferred, nginx reloaded; verified 200 + new tagline/retail-clusters/Zero-Cookies/
      unified-bubbles + /research 200). Pre-push snapshot committed at cf04e0c5 (index.html + research pages + css; note:
      commit-as-next hook landed them in a clippy-labelled commit — content intact). Open for Command: nginx gzip+cache;
      Stage 6 canonical promote; review GIS www gitignore policy + the commit-as-next hook sweeping staged files. [2026-06-22 totebox@claude-code]
- [x] **Bubble unification across all 3 modes (2026-06-22, localhost, screenshot-verified).** Operator: "they all need
      to be the same; Retail doesn't look good." Dropped Retail's `mcBump` proportional sizing (merged blobs) + the
      confidence-wash (hollow noise dots); standardized Retail/UF/Commuter on ONE clean spec: per-tier radius
      T1 4→10 / T2 2.8→7 / T3 2→5, flat opacity 0.90/0.82/0.74, tapered white stroke 1.5/1/0.5 (shared
      `BUBBLE_OPACITY`/`BUBBLE_STROKE_WIDTH`). All three now identical + crisp; Top-400 recess + click preserved.
      Tradeoff: dropped F23 member-count + F13 conf cues (re-addable as a stroke treatment via a data change). [2026-06-22 totebox@claude-code]
- [x] **Declutter + accuracy + privacy round (2026-06-22, localhost, screenshot-verified).** (1) Show-all gentle
      de-clutter (paint-only: white halos + circle-sort-key + unified blue ramp + ~18% low-zoom size taper)
      applied identically to Retail/UF/Commuter — every dot still rendered (audit-defensible), now consistent + legible;
      Top-400 recess/overlay preserved (also fixed `_restoreNodesIfClear` to restore the staged opacity expr).
      **Correction (operator, same day): the zoom-graded alpha 0.78 dimming was REVERTED** — it made base bubbles read
      as permanently shaded ("shading always up"); base bubbles now full-strength in every mode, shading EXCLUSIVELY
      from the Top-400 recess (verified base-solid vs Top-400-recessed+amber).
      (2) Copy: sublabel "retail co-location clusters"→"retail clusters"; tagline → "Where retail, industrial, and
      transit anchors co-locate across North America and Europe" (no period; was wrongly "large-format retail…thirteen
      countries"); swept the false "thirteen countries" from modal + JSON-LD + welcome lead. (3) Zero-Cookie posture:
      verbatim "Digital Infrastructure & Privacy Posture" in Disclaimer modal + "∅ Zero Cookies · no tracking" welcome
      badge (GDPR/CCPA/PIPEDA proof). [2026-06-22 totebox@claude-code]
- [ ] **Command: add canonical Data Policy to factory-release-engineering** — detailed outbox sent
      (msg re: Zero-Cookie/Zero-State Telemetry/no-PII; UI to link once created). Editorial: reconcile JOURNAL/TOPIC
      13↔18 country count. [2026-06-22 totebox@claude-code]
- [x] **Bubble/Top-400/onboarding build-out (2026-06-22) — localhost deployment, 6 phases, screenshot-verified.**
      (1) Unified bubble tier ramp to one Blues scale across Retail/Urban Fringe/Commuter (killed the amber-tier
      ↔ Top-400 collision). (2) Top-400 Regional Markets now activatable OVER any mode — bubbles recess to 0.45
      (archetype opacity stored/restored), amber stars `moveLayer`'d on top, amber legend row. (3) Dropped
      "★ Regional Markets" label; toggle renamed "Top 400 Regional Markets". (4) Branded first-run welcome card
      (navy gradient + amber underline, tier swatch chips, "Explore the map →"; mobile bottom-sheet). (5) `/research`
      overhaul: sticky map-matched topbar (← Map + section tabs + breadcrumb + prev/next) across all 5 pages,
      tokens synced to map, consolidated into `lib/research-mobile.css`. (6) Compacted overview tiers → 3-up chips,
      peek 214→176, trimmed BentoBox copy. [2026-06-22 totebox@claude-code]
- [x] **Compare badge fix + 2-page print — PUSHED TO PROD + STAGE 6 DONE 2026-06-24.** (a) `removeFromCompare()` badge count fix. (b) 2-page print: `preserveDrawingBuffer: true`, `calcPrintZoom`, `flyTo→idle→toDataURL`. Stage 6: targeted `_www-sync-2` branch → canonical `acc90c2e` (`app-orchestration-gis/www/index.html`, 50 insertions). [2026-06-24 totebox@claude-code]
- [ ] **Operator visual sign-off (build-out)** — review localhost desktop + mobile + /research; flag tweaks. [2026-06-22]
- [x] **Auto-implementation run (2026-06-21) — all front-end-feasible findings on deployment localhost.**
      Opus wording review → `~/sandbox/gis-visual-audit/COPY-SPEC.md`, then 5 verified batches on
      `deployments/gateway-orchestration-gis-1/www/index.html` (JS node --check clean; 200; screenshot-verified):
      B1 cartography (F2 zoom-staged de-clutter, F8 ordered CVD-safe blue ramp consistent across dots/legend/pills,
      F22 demote POIs, F23 proportional symbols; F18 skipped-overlays mutually exclusive);
      B2 (F17 chain search counts/sections/aria, F19 first-run coachmark, F13 low-conf uncertainty render, F20 tagline re-added);
      B3 (F26 ARIA + live region + crawlable Top-400 list + focus-visible, F29 rings already geodesic, F25 MAUP/spend caveat, F32 type/contrast);
      B4 wording (tiers → Regional/District/Local everywhere incl. pills [now stacked]; ~20 BentoBox rewrites + 6 tooltips; headline copy);
      B5 stretch (F24 print one-pager, F5 scorecard summary line, F15 compare tray; F9 = no 404s found).
      Plus earlier: F1/F7/F14/F21/F27/F28/F30/F33 + Top-400 shading fix. [2026-06-21 totebox@claude-code]
- [ ] **Operator visual sign-off** — review localhost (desktop + mobile); flag any tweaks (e.g. stacked pills vs compact). [2026-06-21]
- [ ] **Deferred: after-screenshot verify** — VM browser was starved by an unrelated orphaned
      headless-chrome pool (ports 9400-9403); retry `~/sandbox/wiki-harness/gshot.mjs` against
      `http://127.0.0.1:8900/` when the pool clears. [2026-06-20 totebox@claude-code]
- [x] **Deferred: deployment→git-source drift reconciliation — RESOLVED 2026-06-23.** Canonical `app-orchestration-gis/www/index.html` synced to 4421 lines (deployment version, with og:/twitter:/JSON-LD SEO tags). lib/ + research pages added. git-source now matches deployment. [2026-06-23 totebox@claude-code]
- [ ] **Engineering sub-BRIEF: delivery re-architecture** — F3/F4/F9/F10/F31 (PMTiles migration of
      clusters-meta+archetypes, N+1 catchment pack, inline-JS extraction, basemap self-host, metro-404).
      `.agent/briefs/BRIEF-gis-delivery-rearchitecture.md`. Biggest diligence risk. [2026-06-20 totebox@claude-code]
- [ ] **Engineering sub-BRIEF: white-space & cannibalization model** — F16; quantify Union-Find ring
      overlap; surface white-space on chain select. `.agent/briefs/BRIEF-gis-whitespace-cannibalization-model.md`.
      [2026-06-20 totebox@claude-code]
- [ ] **Track audit drafts at gateways** — 7 drafts routed (3 DESIGN-RESEARCH→project-design,
      4 TOPIC+1 TEXT→project-editorial); update artifact-registry when refined/committed.
      [2026-06-20 totebox@claude-code]

### GIS Reports — 4-page print report
- [x] **Print enhancements B1–B6 COMMITTED 050c581f (2026-06-24).** B1 ring zoom fix (calcPrintZoom uses ring_km*2.4); B2 Overpass table → Category|Retailers 2-col no distance; B3 dual Wikipedia fetch (local + metro anchor, first-sentence each when both shown, full extract for standalone); B4 metro context line + co-location metro scope + Top-400 rank column; B5 AEC Climate & Hazard block (hides until pipeline adds AEC to compact schema); B6 satellite anchor overlay (white-haloed category circles on canvas + HTML legend). [2026-06-24 totebox@claude-code]
- [x] **Phase 2 pipeline rebuild DONE (2026-06-24).** build-clusters.py + build-tiles.py ran; clusters-meta.json (13.8 MB, 6117 clusters) + layer1/2/3 PMTiles rebuilt in deployment. rm_type distribution: 2677 standalone / 2005 satellite / 1220 metro / 215 unresolved. Spot-check: Airdrie=satellite_regional/Calgary Metro ✓, Calgary T1=metro ✓. [2026-06-24 totebox@claude-code]
- [x] **PUSHED TO PROD 2026-06-25** — `push-to-prod.sh gis` (operator-authorized); 640 MB transferred (tiles unchanged by checksum); nginx reloaded OK. Commits shipped: 2cc885b0 / c2c8cb93 / 8bbf24a8 / 9e0894e2 (print p3 polish round). [2026-06-25 totebox@claude-code]
- [x] **Print p3 polish round DONE 2026-06-25.** (1) `metaToClusterProps` now returns raw `members` array — fixes category bubble coloring. (2) Bubble row: colored dot (brand color) if category present at site, grey outline if absent — deployed on p3. (3) Wikipedia headings redesigned as "ABOUT AIRDRIE" / "ABOUT CALGARY" section labels with navy left-border accent + Wikidata subtitle inline. (4) maxChars bumped 400→550 local / 480 metro for richer context. (5) Retailer dedup via Set (no more duplicate Dollarama / Canada Post). (6) p2 map page kept clean — no legend. Screenshot-verified via Playwright headless capture. [2026-06-25 totebox@claude-code]
- [x] **Print p3 polish round 2 DONE 2026-06-25.** (1) Population column right-aligned in trade area table (`.prt-mini-table td:nth-child(2)`). (2) Province abbreviations expanded to full names (Alberta vs AB) in all page titles, co-location sibling rows, and Wikipedia title lookups — uses existing `PROV_ABBR` map; `locationFull` promoted to top of `printOnePager()`. (3) Section spacing bumped: `.prt-section-head` padding-top 8→10px + margin 10→12px; `#print-rm-colocs-heading` padding-top 8→10px; `#print-retail-subheading` padding-top 8→10px; `#print-anchor-matrix` margin 8→10px bottom, 4→6px. (4) Co-location sibling rows now show expanded market name in small text below tier label. (5) Top-400 rank appended to page 3 note line if this cluster is ranked. git-source synced. [2026-06-25 totebox@claude-code]
- [ ] **Operator visual review** — print a T3 satellite cluster (e.g. Airdrie AB) to verify: full province name "Alberta" in all titles + co-location rows; population column right-aligned under "Population" header; trade area table visible; section spacing; Top-400 rank if applicable. [2026-06-25 totebox@claude-code]
- [ ] **BRIEF-gis-reports.md created 2026-06-24** — 4-page print report: p1=stats, p2=vector map (existing), p3=retailer table (Overpass OSM) + Regional Market context, p4=ESRI satellite aerial. Open questions: ESRI TOS (OQ1), Overpass hosting threshold (OQ2), RM data load strategy (OQ6). Implementation: Phase 1 = retailer table, Phase 2 = satellite render. [2026-06-24 totebox@claude-code]
- [ ] **Phase 3 report enhancements** — see BRIEF-gis-reports.md §Phase 3 for what tenant reps want: drive-time isochrones, age/income demographics, "Prepared by" branding block. [2026-06-24 totebox@claude-code]

### AEC / data pipeline — manual pickup (see BRIEF-gis-aec-climate-layers.md for full runbook)

**Coverage as of 2026-06-25:** wildfire ✅ (99.7%), flood ⚠️ (13%), all other AEC fields 0/6117.
No automated overnight runs — pick one task at a time from below.

- [ ] **Task A — Köppen class (< 2 min, data on disk)** — write `build-koppen-join.py`, run against
      `work/aec/koppen_geiger.tif`. Then immediately run `build-ashrae-zone.py` to derive `ashrae_zone`.
      Full script template in BRIEF §Task A. [2026-06-25 totebox@claude-code]
- [ ] **Task B — Ecoregion name + biome (10–15 min, data on disk)** — write `build-ecoregion-join.py`,
      run against `work/aec/ecoregions-global.geojson`. Full script in BRIEF §Task B.
      [2026-06-25 totebox@claude-code]
- [ ] **Task C — Seismic + wetland (20–40 min, mostly data on disk)** — run `build-aec-seismic.sh`.
      Check `work/aec/eshm20-eu.tar.gz` first (`ls -lh`; `tar -tzf` to verify integrity). US data
      already downloaded; CA NRCan is live. EU tarball should allow ESHM20 step to proceed.
      [2026-06-25 totebox@claude-code]
- [ ] **Task D — Flood completeness (1–2 hr, ≥10 GB free required)** — re-run `build-aec-flood.sh`.
      Bumps flood coverage from 13%→~80% (FEMA REST + WRI Aqueduct + EU WFS). Check `df -h` first.
      [2026-06-25 totebox@claude-code]
- [ ] **Task E — Temperature / HDD / CDD (30 min + ~35 MB download)** — write `build-temperature-join.py`
      sourcing WorldClim v2.1 10-min monthly rasters. Unlocks NECB zone (CA) and tightens ASHRAE.
      [2026-06-25 totebox@claude-code]
- [ ] **Task H — NECB zone for Canada (< 1 min, depends on Task E)** — write `build-necb-zone.py`;
      derives from `hdd18` field set in Task E. [2026-06-25 totebox@claude-code]
- [ ] **Task F — Solar GHI (30 min + ~3 GB download)** — write `build-solar-join.py` against
      Global Solar Atlas bulk GeoTIFF. [2026-06-25 totebox@claude-code]
- [ ] **Task G — Wind speed (30 min + ~2 GB download)** — write `build-wind-join.py` against
      Global Wind Atlas 250m v3 GeoTIFF. [2026-06-25 totebox@claude-code]
- [ ] **After each task** — run coverage audit (`python3 -c "..."` in BRIEF), push to prod. [2026-06-25]
- [ ] **GFWED wildfire — Night 6 verification DONE** — layer15-wildfire-global.pmtiles now 2.3 MB
      (Jun 25). wildfire_hazard already 6101/6117 in clusters-meta.json. No re-run needed. [2026-06-25]
- [ ] **EU seismic fallback** — `maps.efehr.org` NXDOMAIN. Try `work/aec/eshm20-eu.tar.gz` first
      (may already be complete); if corrupt, clone from GitLab. [2026-06-19 totebox@claude-code]
- [ ] **FEMA US SFHA (layer12-fema-sfha-us.pmtiles)** — subsumed into Task D (flood completeness
      re-run). Current tile is 2.8 MB Jun 24 — unclear if valid; Task D will rebuild. [2026-06-19]
- [ ] **F-series tracking** — F1–F7 content repair requests at project-editorial (2026-06-14);
      track responses; update artifact-registry Status when returned. [2026-06-16 totebox]

### Three-tier market hierarchy — Phase 2 (2026-06-24)
- [x] **Phase 0+3 COMMITTED (24826c4c, a8ca1637)** — `normalizeMarketName()` + Bento three-tier display + search country filter + font fix. All tracked in `app-orchestration-gis/www/index.html`.
- [x] **Phase 2A/2B boundary files BUILT** — `ca_csd_statcan.geojson` (34 MB, 1414 city-type CSDs) + `metro_markets.geojson` (1.1 MB, 156 CA CMAs + 935 US CBSAs). Build scripts committed in `build-settlements.py`. Airdrie T3 cluster gets Nominatim override → "Airdrie" (override added to `ca_places_nominatim.json`).
- [x] **Phase 2C/2D pipeline code WRITTEN** — `region_engine.py` (StatCan PIP + `resolve_market_full()`), `build-clusters.py` (emit `rm_type`/`metro_id`/`metro_name_val`), `build-tiles.py` (pass-through new fields). Changes in gitignored `pointsav-monorepo/` working copy — VERIFIED via end-to-end engine test. Awaits pipeline rebuild to reflect in `clusters-meta.json`.
- [x] **Pipeline rebuild DONE (2026-06-24)** — clusters-meta.json rebuilt with rm_type/metro_id/metro_name; Airdrie=satellite_regional/Calgary Metro ✓; Calgary T1=metro ✓. [2026-06-24 totebox@claude-code]
- [ ] **Phase 2E UI verify** — Bento Box three-tier display code committed; clusters-meta.json now has live data — verify visually after push-to-prod. [2026-06-24 totebox@claude-code]

### Canonical gap — pipeline scripts (Phase 5)
- [ ] **Canonical has 21 pipeline commits NOT on this cluster** — since divergence, `origin/main` gained `config.py`, `taxonomy.py`, AEC build scripts etc. via other archive promotes. These live at `app-orchestration-gis/` in canonical but are NOT in this cluster's working tree. Pull into cluster before next pipeline work: `git fetch origin && git cherry-pick <range>` or rebuild from canonical. [2026-06-23 totebox@claude-code]
- [ ] **Rust clippy commits superseded — do NOT promote** — cluster commits `8d0036f5`/`dfdd6fd7`/`58911091` (system-security), `913ef5bf`/`d6476e09`/`0ea26e61` (service-fs), `bb2d818b`/`d2974e58`/`fe17d688`/`d57b10c9` (service-content), `b1028659`/`006b6e20` (app-console-content), `6f8e6724` (app-privategit-marketplace) are superseded by canonical's axum-based rewrites of those crates. These commits must never be cherry-picked to canonical — they will conflict with the rewritten versions. [2026-06-23 totebox@claude-code]

## Blocked — Command Session (route via outbox)
- [x] ~~**Design: entity_types.csv schema**~~ — `service-content/ontology/entity_types.csv`, columns: `label, description_projects, description_corporate, description_documentation, coa_link`. [2026-06-30 totebox@claude-code]
- [x] ~~**Implement: GLiNER loads entity types from CSV at startup**~~ — `_load_domain_labels()` replaces hardcoded DOMAIN_LABELS dict; falls back to it (`_FALLBACK_DOMAIN_LABELS`) if the CSV is missing/malformed. [2026-06-30 totebox@claude-code]
- [x] ~~**Implement: entity_filter.rs ALLOWED_CLASSIFICATIONS reads from ontology**~~ — `init_ontology_classifications()` + `is_allowed_classification()`; all 4 call sites migrated (DPO validator, ingest gate, /v1/graph/cleanup, raw_entities_to_graph); falls back to compile-time const. Commit 8731c7af, 79/79 tests. [2026-06-30 totebox@claude-code]
- [ ] **DEPLOY ONLY — not a code gap**: live `local-content.service` still points `SERVICE_CONTENT_ONTOLOGY_DIR` at the orphaned `project-intelligence` clone (pre-2026-06-20-merge), so none of the above is visible in production until that env var + a binary rebuild ships. Flagged to Command — outbox msg-id `command-20260630-stage-6-backlog-consolidated-121-commits`. [2026-06-30 totebox@claude-code]

---

## Hot — next session (2026-06-28 EQ session deferred items)

- [ ] **Stage 6 (GLiNER + RESCUE + CHECKPOINT)**: promote commit `720e20d8` → `self-service-promote.sh` — NOTE 2026-06-30: this archive's pairings.yaml `self_service` grant is `build-deploy`, not `build-deploy-stage6lite`; `self-service-promote.sh` hard-fails here. All Stage 6 must go through Command. Outbox msg-id `project-totebox-20260628-stage6-gliner-tier0` (consolidated into `command-20260630-stage-6-backlog-consolidated-121-commits`). [2026-06-28 totebox@claude-code]
- [x] ~~**EQ5 — article chunking**~~ — `chunk_for_gliner()` (sentence-boundary, 150-char overlap) already covered the GLiNER path; 2026-06-30 extended the same chunker to the Tier A OLMo synchronous fallback (`call_tier_a_extract_chunked`, 6000-char window) which was still sending whole documents in one call. Commit 8731c7af. [2026-06-30 totebox@claude-code]
- [x] ~~**Grammar on Doorman path**~~ — verified 2026-06-30: already fixed same commit as EQ5 (`da56ebf2`/`c873d174`, 2026-06-28). `slm-doorman-server/src/http.rs` `extract()` handler applies `GrammarConstraint::JsonSchema` on BOTH the Tier B request (line ~686) and the Tier A fallback request (line ~740). Only remaining issue was a stale doc comment directly above the function claiming "Tier A uses no grammar constraint" — contradicted the code 6 lines below it. Fixed comment 2026-06-30. TIER-0 RESCUE remains as defense-in-depth, not the sole mitigation. [2026-06-30 totebox@claude-code]
- [ ] **CHECKPOINT fix verification**: CRE checkpoint test file (`CORPUS_CRE_checkpoint_*.json`) queued in drain. After drain processes it, verify entity count increases AND entity is queryable via HTTP. Confirms lbug 0.16 cross-thread fix. [2026-06-28 totebox@claude-code]
- [x] ~~**CSV structured-data files**~~ — `is_csv_structured_data()` detects 2+ `Entity Name:` marker lines, skips the wasted GLiNER round-trip, routes straight to Tier A. Commit 8731c7af. [2026-06-30 totebox@claude-code]

- [ ] **M-17 root fix — CLAUDE.md + manifest identity** — project-gis CLAUDE.md header says
      "project-intelligence"; manifest shows project-proforma. Foreign sessions overwrite NEXT.md /
      session-context.md / NEXT (contaminated 3× on 2026-06-20 alone). Outbox sent. [2026-06-20 totebox@claude-code]
- [ ] **Performance — nginx gzip + cache-control on foundry-prod** — diffs in outbox
      `project-gis-20260619-perf-nginx-prod`. maplibre-gl.js 784KB→~200KB. [2026-06-19 totebox@claude-code]
- [x] **Stage 6 canonical sync DONE 2026-06-23** — targeted `_www-sync` branch (commit ab182536) pushed to canonical; `app-orchestration-gis/www/` now at 4421 lines + lib/ + research pages. promote.sh bypassed due to ADD/ADD conflict bug on pre-existing path. [2026-06-23 totebox@claude-code]
- [ ] **check --strict gate** — F2/F3 dead links at project-editorial must resolve first. [2026-06-17 command@claude-code]
## Fix-2 finding: GLiNER batch endpoint (2026-06-28)

`/v1/batch-extract` endpoint added to `service-gliner/main.py` and committed.
Uses `model.inference(texts, labels)` which accepts `List[str]`.

**CPU result:** 5-text batch took 19m 23s vs ~1s sequential. PyTorch attention
mechanism scales as `O(batch × seq_len²)` on CPU — no parallelism benefit without
GPU CUDA cores. Batch is NOT used by service-content on this VM.

**batch endpoint kept as infrastructure** — will be used when:
1. GLiNER bi-encoder (gliner_bi-edge-v2.0) deployed — bi-encoder pre-computes
   label embeddings offline, so batch cost is near-constant regardless of label count
2. GPU node available — CUDA batching gives linear scaling

**Correct CPU throughput path:** `CONTENT_DRAIN_THREADS` env var (already wired).
Set to 4 in local-content.service to run 4 parallel drain workers, each making
sequential /v1/extract calls to separate uvicorn workers on GLiNER.
But GLiNER runs single-worker by default — need `--workers 4` in local-gliner.service
first (or run 4 separate GLiNER processes).

- [ ] **Enable multi-worker GLiNER**: add `--workers 4` to local-gliner.service ExecStart;
  requires `if __name__ == '__main__': ...` guard already in main.py [2026-06-28 totebox@claude-code]
- [ ] **CONTENT_DRAIN_THREADS=4**: set in local-content.service after GLiNER multi-worker active [2026-06-28 totebox@claude-code]
- [ ] **Plan: GLiNER bi-encoder** (gliner_bi-edge-v2.0): evaluate after multi-worker baseline;
  replaces medium-v2.1; requires different inference call in service-gliner [2026-06-28 totebox@claude-code]

---

## Hot — done (2026-06-28, GLiNER Tier 0)

## Completed (recent)

- [x] **Map UX/tech audit (2026-06-20)** — 8-persona browser-in-the-loop swarm; BRIEF F1–F34;
      research + synthesis + 10 follow-through docs generated. Commit 53bf62ed +. [2026-06-20 totebox@claude-code]
- [x] **Performance — preload hints + preconnect** — index.html; ships with push-to-prod. [2026-06-19]
- [x] **Night 5 build verification + GFWED variable fix + log gitignore**. [2026-06-19]
- [x] **build-aec-flood.sh OGR_GEOJSON_MAX_OBJ_SIZE + numpy 2.x fixes; AEC flood Night 5**. [2026-06-19]
- [ ] **TOPIC/GUIDE/JOURNAL** — stage to .agent/drafts-outbound/ → project-editorial. [2026-06-22 totebox@project-totebox]
## Currently open

### software.pointsav.com — Binary Library repositioning + visual redesign COMPLETE [2026-07-07 totebox@claude-code]

The `-2` rewrite program (`BRIEF-software-ng-rewrite.md`, P0–P8 complete, still live/shipped —
not reverted) is redirected per operator decision: software.pointsav.com is being repositioned
around a "Binary Library" concept (cursor.com/marketplace-inspired) — see
`.agent/briefs/BRIEF-binary-library-repositioning.md` for the full research, audited CURSOR
simulation, PointSav-transposed design (mockups at
`app-privategit-marketplace-2/docs/mockups/*.html`), product/licensing recommendation, and phased
build roadmap.

**Catalog-scope decision — RESOLVED 2026-07-07:** operator approved the recommended two-shelf
model (Commercial os-*-only shelf unchanged + a new Open Source/Community shelf, populated only
as crates are actually relicensed) — this partially reverses the os-*-only-**public**-catalog rule
from `BRIEF-software-hyperscaler-audit.md` for a clearly-separated second shelf, not by opening
the Commercial shelf itself.

**Phase 1 (catalog schema) — COMPLETE.** `LicenseTier::OpenSource` + `shelf()` two-shelf model
added to `app-privategit-marketplace-2`, with a loud-failure catalog-validation guard.

**Phase 2 (relicensing) — governance COMPLETE via Command Session** (see
`BRIEF-software-licensing-structure.md`, Command-scope): `tool-wallet` → Apache-2.0 landed in
`factory-release-engineering` + `conventions/software-distribution-substrate.md`, plus a much
broader per-product tier review (`os-orchestration` bug fix to Proprietary/permanent,
`os-totebox`/`os-privategit` engine → FSL, 5 `moonshot-*` → FSL). project-software's own
in-repo follow-up (SPDX header + `Cargo.toml` for `tool-wallet`) also done same session.

**Phase 3 (two-shelf catalog UI) — COMPLETE.** `/software` now renders both shelves from the
Phase 1 schema; fixed a real bug where the old 2-way tier partition would have silently misfiled
an `open-source` product under the "FSL" heading. `shelf` field added to `/v1/products` JSON.
74/74 tests passing, clippy/fmt clean.

**Phase 4 (nav + hero copy) — COMPLETE.** "Binary Library" nav item added (anchors into
`/software#open-source`, not a new route or a `Products` rename); hero copy extended (not
replaced) with a "The Binary Library" eyebrow + "components of an orchestration, not an app
store" line. 76/76 tests passing, clippy/fmt clean.

**Post-Phase-4 visual redesign — COMPLETE, same day.** Operator flagged the rendered result as
visually wrong — verified directly against `home.pointsav.com`/`documentation.pointsav.com`'s
real live CSS (not assumption): the near-black footer and gold accent used throughout this crate
(inherited from the prior ng-rewrite program) matched **neither** real family site, and
`pointsav-design-system`'s own registered footer token is itself stale/wrong (claims to be "live
on documentation.pointsav.com," verified false). Corrected `tokens.rs` to the real values (navy
`#164679` — confirmed correct — as the only accent, light-grey `#f8f9fa` footer, no gold), footer
restructured to home.pointsav.com's exact Site/Network two-column pattern (real URLs), and
dropped `"Playfair Display"` site-wide for `Georgia` (Playfair is specifically the *wiki's*
display font, not the family's real headline font). `/software` also restructured toward denser,
CURSOR-mockup-inspired card-grid + shelf-rail layout — real PointSav colors, not CURSOR's.
Full provenance and reasoning for every token value is in `tokens.rs`'s doc comment. 76/76 tests
passing, clippy/fmt clean. Incidentally fixed a real pre-existing bug: the footer's "Pricing"
link pointed at `/licensing` instead of `/pricing`.

**Still open, explicitly deferred, not assumed:** page consolidation (merging
Products/Pricing/Licensing into one page, per the operator's "we don't need all these pages??")
— the redesign's added density may already address the underlying concern; revisit only if the
operator still wants it after seeing this live.

**Still no live catalog entries** in either new shelf — Phases 1–4 built the capability;
depositing tool-wallet's actual binary (SHA256, `install.sh`, `RELEASES_DIR`, `products.yaml`
entry) is separate follow-up work, not yet scheduled. **Next: Phase 5 — full cross-viewport
operator visual sign-off, required before Phase 6 (production cutover). Nothing further should
proceed without it.**

### software distribution — Stage 6 NOT blocked, queued for auto-promote [2026-07-02 totebox@claude-code]

**RESOLVED — was a false alarm.** Command investigated the prior high-priority escalation
(msg-id `command-20260701-escalation-high-project-software-stage-6`) and found the
"3146 → 3454 → 3482 growing" framing conflated *commits behind* (irrelevant unrelated
history from other archives' promotes) with *commits ahead* (the actual work needing
promotion). The real number is **30 commits ahead of origin/main** — same size/pattern as
every other archive's routine promote. See msg-id
`command-20260701-resolved-project-software-stage-6-escala`.

**30 commits queued for promotion**, including:
- `427bfff7` — storefront HTML fix (Knowledge Wiki product card was missing)
- `67627b33` — os-privategit README correction
- `c3399e2e` — BETA catalog listings
- ~10 clippy/fmt pre-promote-gate fixes; ~15 feature commits (VM fleet polling, PPN
  heartbeat, wallet fd-lock, token revocation, original scaffold)
- All cargo gates green

- [ ] Command will run the promote automatically once this Totebox session goes idle — no
  action needed from this archive; do not re-attempt `self-service-promote.sh`
- [ ] After canonical promotion: app-privategit-source, app-privategit-marketplace, tool-wallet get binary-ledger entries + RELEASES_DIR entries for self-hosting

### storefront `/products` page — static HTML, not catalog-driven [2026-07-01 totebox@claude-code]

Discovered `app-privategit-marketplace/static/software.html` is baked into the binary via
`include_str!` at `src/main.rs:102` — it never reads `products.yaml` at request time.
Only `/v1/products` (JSON API) reflects catalog edits. The entire page is placeholder
content: every "Install →" link is `href="#"` except the one card fixed this session, and
there's a literal "Sample listing" disclaimer at the bottom.

- [ ] Decide: render `/products` dynamically from products.yaml, or commit to hand-maintaining
  cards per product (current state) — flagged to Command, not yet decided
- [x] Added real Knowledge Wiki card with working href (commit `427bfff7`) — only fix so far
- [ ] `os-network-admin` and `soft-orchestration-command` cards still stale/placeholder despite
  being live BETA products

### foundry-prod sync path — partially resolved [2026-07-01 totebox@claude-code]

`software.pointsav.com` resolves to foundry-prod (34.168.19.68), a different machine from
this session's host (foundry-workspace, 34.53.65.203). Found `~/Foundry/bin/push-to-prod.sh
software` (Command-only, manual) which rsyncs the two marketplace binaries + RELEASES_DIR,
but on read it has no reference to `/var/lib/local-software/catalog/products.yaml` — may
leave prod's catalog stale even after a successful push. Flagged informationally to Command:
msg-id `command-20260701-fyi-found-push-to-prod-sh-software-targe`.

- [ ] Confirm whether products.yaml needs adding to push-to-prod.sh's `target_software()`
- [ ] Nothing deposited in project-software sessions to date is confirmed live for real customers until this is resolved
- [ ] **Clobber risk (2026-07-02):** `app-privategit-source` + `app-privategit-marketplace` were
  just manually added to the live `/var/lib/local-software/catalog/products.yaml` (see below) —
  this is exactly the file `push-to-prod.sh`'s `target_software()` does NOT currently sync. If
  Command fixes that gap by adding a catalog rsync, confirm the source-of-truth direction
  (workspace → prod) doesn't silently overwrite or drop these two live-added entries.

### software distribution — BETA catalog pending [2026-07-02 totebox@claude-code]

- [x] `app-mediakit-knowledge` QCOW2 (Format B) — deposited 2026-07-01: SHA verified, RELEASES_DIR slot at app-mediakit-knowledge/0.1.0/, MANIFEST.json written, products.yaml updated, ACK sent to project-knowledge
- [x] Self-produced catalog entries — `app-privategit-source` (sha `36ecb701…`, v0.1.0) and `app-privategit-marketplace` (sha `e53b629a…`, v0.0.3) deposited 2026-07-02: RELEASES_DIR + MANIFEST.json + live products.yaml + monorepo products.yaml (commit `037e51da`); both verified 200 OK, visible in `/v1/products`. `tool-wallet` does NOT need a catalog entry — binary taxonomy rule (§6 of the BRIEF) says tool binaries are NEVER distributed; prior note listing it as pending was itself stale. `os-privategit` still needs RELEASES_DIR + catalog entry once engineering produces a binary.
- [x] `install.sh` — **authored + deployed 2026-07-02** (commit `80e08c18`): one generic bare-binary
  template, instantiated for the 4 live bare-binary products (`os-network-admin`,
  `soft-orchestration-command`, `app-privategit-source`, `app-privategit-marketplace`);
  deployed live at `RELEASES_DIR/<product>/install.sh`, verified serving via
  `/releases/<product>/install.sh`. Templates tracked in
  `app-privategit-source/scripts/install-templates/`. Not yet done: OS-image and
  QCOW2-class products still have `href="#"` placeholder cards and no install.sh — deferred,
  no real consumer today.
  **Gap found + fixed:** the `/releases/:product/:version/MANIFEST` route does not resolve
  `"latest"` the way the binary-download route does — a script hardcoded to check
  `.../latest/MANIFEST` would silently skip SHA verification forever. Fixed by resolving the
  redirect's concrete version first, then checking that version's MANIFEST. Verified
  end-to-end against `app-privategit-source` (SHA confirmed matching).
  **Still not live for real customers:** tested against the real public URL
  (`https://software.pointsav.com`) and got 404 — confirms the foundry-prod sync gap above;
  these scripts work correctly against foundry-workspace but the deposits haven't reached
  the actual customer-facing host yet.
- [ ] `os-privategit` engineering — scaffold only (lib.rs stub, no main.rs); README rewritten to be product-accurate 2026-07-01 but binary not yet built. A 288 KB informal binary exists in RELEASES_DIR from 2026-05-31 (commit 03741cb9, 401-gated, unledgered) — provenance/purpose unclear
- [x] Product page template (S136, 2026-07-03) — `GET /software/:product_id` in `app-privategit-marketplace-2`, BETA badge, tier badge, single-row platform table, curl install, version, client-side-fetched SHA256 (MANIFEST endpoint, degrades to a visible link), optional `guide_url`. Platform table is a known simplification (single row from the existing free-text `platform` field + `linux-x86_64` slug convention) — real multi-platform data needs a schema change (`platforms: Vec<PlatformArtifact>` on `Installer`), tracked as a follow-up below, not blocking.
- [ ] **Follow-up from S136:** `Installer.platform` is free-text and doesn't model true multi-platform products (some ship macOS/Win/Linux, some are Linux-only) — a `platforms: Vec<PlatformArtifact>` schema change would let the product-detail page render a real multi-row table instead of one synthesized row. Needs `products.yaml` data for every live entry; not urgent.
- [ ] **`os-console` v0.1.0 BETA submission (2026-07-01, msg-id `command-20260701-binary-submission-os-console-v0-1-0-beta`)** —
  tracked, not actioned. Binary itself is "pending Command Session pipeline (build-soft.sh,
  expected after Stage 6 promotion)" — nothing to deposit yet. Revisit once Command confirms
  the binary is built.
- [ ] **`app-mediakit-knowledge` Format A (bare Linux x86_64 binary)** — confirmed via mailbox
  search (2026-07-02) that no message specifically requesting this deposit has landed in this
  archive's inbox. project-knowledge's own tracking says the bare binary is blocked on **their**
  Stage 6 rebuild — not on project-software. No action needed here until they submit it.

### software.pointsav.com — ground-up rewrite, P0-P7 complete, P8 pending [2026-07-02 totebox@claude-code]

`BRIEF-software-ng-rewrite.md` (active, primary tracker) — full ground-up rewrite of
`app-privategit-source` → `app-privategit-source-2` and `app-privategit-marketplace` →
`app-privategit-marketplace-2`, `tool-wallet` distributed as-is (third binary, taxonomy
exception, documented). `os-privategit` confirmed out of scope.

- [x] P0-P6 complete: scaffold, characterization harness, full route port (source-2: 38 tests,
  0 → 38; marketplace-2: 33 tests, 11 → 33), Sovereign Editorial chrome, dynamic catalog
  rendering, license/payment flow with the reviewable pricing-unit fix (Checkpoint 2, adversarial
  review caught and fixed 2 real issues before acceptance).
- [x] P7 (Checkpoint 3a — simulated parity): 48-fixture diff sweep, 0 unexplained differences,
  found + fixed a dropped securities-disclosure footer citation. Footer/disclaimer reworked to
  match the live wiki/home-site pattern (self-contained `/page/disclaimer`, "Important
  information" accordion) — commits `24f2dca6`, `ea987edc`.
- [ ] **Checkpoint 3b (real on-chain USDC transaction) DEFERRED — operator decision, 2026-07-02.**
  Real-transaction testing is not feasible for some time; the site needs to launch regardless.
  In place of a pre-launch gate: the first real transaction through `app-privategit-marketplace-2`
  is now flagged distinctly (marker file + `tracing::warn!` "FIRST-LIVE-TRANSACTION" log line,
  commit `ea987edc`) for close manual review after the fact, rather than a synthetic pre-test.
  **When that first real transaction occurs in production, review it closely** (license key,
  receipt, catalog match) before considering coin-acceptance fully validated.
- [ ] **Still open before P8:** operator visual sign-off across viewports (P7's other stated
  requirement — needs an actual human, not something automatable).
- [x] `/page/privacy`, `/page/accessibility`, `/page/contact` (2026-07-03) — built, mirroring
  the existing `/page/disclaimer` pattern exactly. Closes the highest-priority audit finding
  (`/page/contact` returning HTTP 0, flagged twice: original audit + operator dogfood-test
  escalation). No fabricated legal/contact claims — `open.source@pointsav.com` is the only
  real contact channel documented anywhere in the workspace; no retention policy, phone
  number, or mailing address exists to cite, so none was invented.
- [x] **Outbox handoff sent to Command** (msg-id `command-20260702-promote-queue-project-software-app-priva`,
  priority high) — full P8 readiness state, the Checkpoint 3b deferral note (make sure whoever
  watches production logs post-swap knows to check for "FIRST-LIVE-TRANSACTION"), and the
  staging-mirror snag below.
- [x] **Detailed follow-up sent** (msg-id `command-20260702-detailed-software-pointsav-com-ground-up`,
  priority high) — full phase-by-phase detail: all three checkpoint outcomes (including
  Checkpoint 2's adversarial review catching 2 real issues before acceptance), test counts,
  every commit hash, deliberate deviations from the old crates, the footer/disclaimer rework,
  and explicit action items for Command.
- [ ] **`self-service-promote.sh` hit a non-fast-forward rejection** pushing to
  `origin-staging-j`'s `main` — that shared jwoodfine staging fork's `main` had been advanced by
  another archive's concurrent push (last was a project-knowledge chrome commit) since this
  branch's last sync point. **Do not re-attempt `self-service-promote.sh` or force-push** —
  same guidance as the earlier 30-commit escalation above; this is a cross-archive contention
  question for Command to arbitrate, not a Totebox-scope fix. `cluster/project-software` HEAD
  `8de58179` is fully committed and durable locally regardless.
- [ ] **Next: P8 (swap + retire old crates).** Command Session / Stage 6 scope — processes the
  canonical merge (once the staging-mirror contention above is resolved) and the actual
  production swap (systemd services on ports 9201/9202).

### software.pointsav.com — hyperscaler-comparison audit sprint, COMPLETE, awaiting operator review [2026-07-02 totebox@claude-code]

`BRIEF-software-hyperscaler-audit.md` (new, active) — operator judged the ground-up rewrite demo
(port 9303) "way too complicated" vs. both `home.pointsav.com` and general hyperscaler standard.
Ran a two-pass research sprint (3 Explore agents for direct codebase re-verification, then a
4-way parallel Opus workflow — internal audit + 3 external research streams — synthesized by a
Fable pass) per explicit operator instruction, **before writing any new code**.

- [x] Confirmed technically: `home.pointsav.com`'s own chrome is a flat 3-link editorial nav with
  unratified placeholder tokens — not itself a hyperscaler-caliber reference to clone.
  Confirmed: paid-tier download+license flow spans 3 services / 6 manual handoffs, 4 of them raw
  JSON with no UI, license key never rendered to a human. Confirmed: $0/BETA pricing already
  fully bypasses the payment flow today — does NOT need to be $1.00, question closed technically.
  Confirmed: `products.yaml`'s `licenses:` list conflates 2 actual license-model rows
  (Apache-2.0/FSL, should be per-product attributes) with 5 unrelated free products mislabeled as
  "licenses" — likely a root cause of the perceived complexity.
- [x] External research delivered: hyperscaler/dev-tool download-UX comparables (AWS, Docker Hub,
  HashiCorp, GitHub Releases, npm/Homebrew), pricing/licensing comparables (Sentry, HashiCorp,
  GitLab, Docker, Canonical, Red Hat) + a BETA-to-paid decision framework, and nav/IA comparables
  (Stripe, Vercel, HashiCorp, Docker, GitHub) with a concrete `os-*/tool-*/app-*/soft-*`-keyed nav
  proposal.
- [x] Full findings + prioritized 7-item gap list written to `BRIEF-software-hyperscaler-audit.md`.
  **No UI/route code touched this session** — confirmed via `git status --short` before/after.
- [x] **Operator reviewed and approved a follow-up implementation plan** — see the new section
  immediately below. Superseded the `os-*/tool-*/app-*/soft-*` 4-family nav proposal (item 3) once
  the ratified three-path model was checked — public storefront sells os-* only.

### software.pointsav.com — storefront cleanup implementation, IN PROGRESS [2026-07-03 totebox@claude-code]

Implementation of the approved follow-up plan (BRIEF addenda: AI-Adoption Challenge Pass,
Positioning Pivot, Licensing Corrections — all appended to `BRIEF-software-hyperscaler-audit.md`,
commit `4a167935`). Checked directly against `factory-release-engineering/LICENSE-MATRIX.md` (top
governance authority for licensing) and `.agent/briefs/BRIEF-software-distribution-substrate.md`
(operator-ratified 2026-05-22 business-model spec) — both corrected several assumptions from the
audit phase; see BRIEF for full detail.

- [x] **Phase 0 (guardrails)**: baseline snapshot — 71/71 tests green (33 marketplace-2 + 38
  source-2), both crates confirmed to build and boot cleanly with pre-change data. Saved to
  `/tmp/software-baseline/` (scratch, not committed).
- [x] **Phase 1 (catalog rebuild)**, commit `62c53d98`: `products.yaml` rebuilt around the
  authoritative 8-product os-* tier table (4 PointSav Commercial/$1, 4 FSL/$19 — corrects a
  research-agent table that had `os-privategit`/`os-interface` swapped). `os-interface` renamed to
  `os-orchestration` per operator confirmation (cross-checked against Command's own 2026-06-30
  binary-catalog-bootstrap message, which independently lists `os-orchestration`). `tool-wallet`
  and 3 misplaced app-* entries removed from the public catalog. **All 8 products ship at
  `price_usdc: 0`** — an active BETA gate, not an oversight: `.agent/inbox.md` carries explicit,
  current (2026-07-01/02) Command/operator instructions that os-console, os-mediakit, and the
  orchestration-command binary must stay free during BETA, and none has been lifted. Flipping a
  specific product later is a one-line data change. 71/71 tests still green, clippy/fmt clean.
  **Live-host catalog file NOT touched** — this folds into P8 readiness per the approved plan
  (ships together with the pending swap, not before it); the production
  `/var/lib/local-software/catalog/products.yaml` update happens as part of that cutover.
- [x] **Naming conflict — RESOLVED by Command 2026-07-06**: canonical name is
  `app-orchestration-command` (msg-id `command-20260706-decision-orchestration-command-naming-re`,
  replying to `command-20260703-re-orchestration-command-naming-soft-vs-`). Fits the
  already-licensed `app-orchestration-*` family (FSL-1.1-ALv2, $19 tier) per LICENSE-MATRIX.md
  §4.3 — no governance PR needed. Command confirmed no catalog entry exists anywhere yet
  (canonical or any clone) — this was genuinely deferred, not a rename of something live; the
  `os-orchestration-command` operator instruction and the `soft-orchestration-command` slug are
  both superseded. **Use `app-orchestration-command` whenever this product is actually added to
  `products.yaml`.** Note: a stray pre-built binary + install.sh template already exist under
  the old `soft-orchestration-command` slug (RELEASES_DIR, `install-templates/`) from a 2026-06-30
  BETA deposit that was never actually catalogued — left as-is per Command's guidance (no rename
  of the existing artifact required); rename only applies going forward at real catalog-add time.
- [ ] **os-network-admin vs os-infrastructure relative pricing** — flagged for future
  reconsideration (both currently ratified at FSL/$19; `os-network-admin` reads more like a thin
  control-surface app than a peer substrate — see BRIEF for the size/maturity evidence). Not
  resolved in this program; needs a `factory-release-engineering` PR + legal review if changed.
- [x] **Phase 2 (checkout/order flow + real token minting)**, commit `230c3e01`: replaced the
  4-raw-JSON-hop paid flow with `GET /checkout/:product_id` (invoice) → `GET /order?product=&
  tx_hash=` (303 to canonical URL) → `GET /order/:tx_hash` (status/entitlement page, built on a
  new `resolve_license()` shared with the existing JSON endpoint) → `GET /order/:tx_hash/download`
  (mints a real Ed25519 token). **Closes a previously-undiscovered gap**: no code anywhere ever
  actually minted a valid download-auth token before this — the old `generate_license_key` was a
  cosmetic hex string structurally unrelated to what `app-privategit-source-2` verifies. Uses the
  time-limited-URL mechanism `BRIEF-software-distribution-substrate.md` already specifies
  (`channel_expiry` = today) rather than the single-use/revocation-list scheme originally
  sketched in planning — simpler, and needs **zero changes to `app-privategit-source-2`**.
  `product_id` is carried explicitly end-to-end (checkout → order) rather than relying on
  price-based inference, which is now genuinely ambiguous since multiple products can share a
  tier price. New `SIGNING_KEY_SECRET` env var (marketplace-2), mirroring source-2's
  `VERIFY_KEY_PUB`; no production key provisioned — that's a deployment-time step for whoever
  owns key management at cutover. 49 marketplace-2 (+16) + 38 source-2 tests pass, clippy/fmt
  clean. **End-to-end verified live** across two separate scratch processes with a real keypair
  (not just in-process tests): full checkout→order→download round trip, plus negative paths
  (pending/not-found, a tampered token rejected 401 by source-2, a mismatched `?product=` claim
  rejected 400 by the marketplace).
- [x] **Phase 3 (nav restructuring)**, commit `a5bc5183`: nav stays a flat 3-link list
  (Products/Licensing/Documentation) — corrected the 4-family (`os-*/tool-*/app-*/soft-*`)
  dropdown proposed in the original audit, which assumed all four families sell here; the
  ratified three-path model says os-* only. `catalog_markup` now groups by `license_tier`
  (PointSav Commercial / FSL) instead of free/paid status, so grouping stays meaningful once
  real pricing mixes with still-BETA products. Removed a dead `#downloads` anchor (nav + footer)
  left over from the old free/paid section ids. Live-checked against the real 8-product catalog:
  4 correctly under Commercial, 4 under FSL. 50 tests pass, clippy/fmt clean. **Automated checks
  only — full cross-viewport operator visual sign-off still pending**, same caveat as P7.
- [x] **Phase 4 (pricing page + licensing.html rewrite)**, commit `1be33071`: new `GET /pricing`
  (catalog-driven, live per-tier product counts, buy-to-own framing, explicit "free during BETA"
  note so tier prices aren't overstated as current charges, BC tax line, AGPL source link — all
  previously missing). `static/licensing.html` rewritten wholesale (1211 lines → real content):
  dropped the fictional multi-chain wallet-connect flow, live HST 13% calculator, and five fake
  priced products that existed nowhere in `products.yaml`; replaced with the actual PointSav
  Commercial/FSL grant text. Added a compile-time (`include_str!`) regression test guarding
  against ever regressing to that fictional content again. "Pricing" added to the flat nav.
  55 tests pass (+5), clippy/fmt clean. Live-checked against the real 8-product catalog.
- [x] **Phase 6 (Docs nav link) — verified already satisfied, no code change needed.** Checked
  `surface.rs::nav_links()` directly: `Documentation` has been its own flat top-level `NavLink`
  since before this cleanup program started — never nested or dropdown-only. The original audit's
  gap-list item #7 ("presently folded into or absent") was written against the never-built
  4-family dropdown *proposal*, not the actual (always-flat) nav code. Nothing to do here.
- [x] **Phase 5 (tx-log.jsonl + FSL clock)**, commit `03c184bb`: `append_tx_log` writes one JSONL
  row per confirmed sale matching the ratified BRIEF's own schema, wired into `resolve_license`'s
  fresh-confirmation branch only (never the receipt-cache-replay branch, which isn't a new sale).
  `Installer.fsl_conversion_date` field added (optional, populated manually per release). New
  `xtask fsl-clock <products.yaml>` subcommand lists FSL conversion dates soonest-first; live-run
  against the real catalog correctly shows all 4 FSL products as undated. 96 tests pass across
  all three crates, clippy/fmt clean.

## software.pointsav.com storefront cleanup — ALL 6 PHASES COMPLETE [2026-07-03 totebox@claude-code]

Every phase of the approved implementation plan is done and committed (commits `62c53d98`
through `03c184bb`, plus this file's own tracking commits). Summary for anyone picking this up:

- **Data model**: `products.yaml` rebuilt around the authoritative 8-product os-* tier table
  (`LICENSE-MATRIX.md`), `tool-wallet`/app-* removed from the public catalog, all products at
  active `price_usdc: 0` BETA gate per current Command/operator instructions.
- **Paid flow**: real `/checkout` → `/order` → `/order/.../download` flow with genuine Ed25519
  token minting (a real, previously-undiscovered gap this closes) — verified end-to-end live
  with a real keypair across two separate processes, not just unit tests.
- **Nav/IA**: flat nav (Products/Pricing/Licensing/Documentation), catalog grouped by license
  tier. **Docs link needed no work** — it was already flat, contrary to the original audit's
  premise.
- **Pricing/legal content**: new catalog-driven `/pricing` page; `static/licensing.html` rewritten
  wholesale from 1211 lines of fictional wallet-connect/tax/fake-product content to real license
  terms.
- **Compliance**: `tx-log.jsonl` (CRA record) and FSL conversion-date tracking now exist.

**Still open, explicitly out of scope for this program** (per the approved plan):
- One of two naming/pricing questions flagged to Command/project-data remains open: whether
  `os-network-admin` should really share `os-infrastructure`'s FSL/$19 tier. (The other —
  `soft-orchestration-command`'s correct name — was resolved by Command 2026-07-06:
  `app-orchestration-command`. See the naming-conflict item above.)
- Live production catalog sync — this folds into P8 (still pending) rather than shipping ahead
  of it; `/var/lib/local-software/catalog/products.yaml` has not been touched.
- Full cross-viewport operator visual sign-off on the nav/catalog changes (automated checks only
  so far, same caveat P7 already established).
- The pay-per-instantiation pricing idea and provenance-tier pricing option — both captured in
  `BRIEF-software-hyperscaler-audit.md` as named future directions, deliberately not built.

- [x] **Outbox handoff sent to Command** (msg-id `command-20260703-software-pointsav-com-storefront-cleanup`,
  priority high) — requests Stage 6 canonical merge + the P8 production swap. Flags three real
  blockers Command needs to own: (1) `cluster/project-software` is ~71 commits ahead of
  `origin/main`, which itself just advanced independently — needs rebase/merge reconciliation,
  same staging-mirror contention already logged above; (2) live catalog sync to
  `/var/lib/local-software/catalog/products.yaml` hasn't happened, deliberately (folds into
  cutover); (3) a real production `SIGNING_KEY_SECRET` needs provisioning for
  `app-privategit-marketplace-2` (only a test key exists in code). Also reiterates the two
  already-flagged open questions (orchestration-command naming, os-network-admin tier) and the
  outstanding visual sign-off — none of those three block the swap itself.
- [x] **Command replied and fixed the staging-mirror contention** (msg-id
  `command-20260703-self-service-promote-fixed-please-retry`): root cause was every self-service
  archive pushing to the same shared `main` ref (collision), compounded by `set -e` silently
  dropping the promote-queue write on a failed push — confirmed zero prior queue entries for
  project-software ever existed. Fixed: each archive now pushes to its own ref; queue-write is
  unconditional. Also confirmed no action needed from us on `SIGNING_KEY_SECRET` provisioning
  (Command handles it at P8 cutover) and flagged an unrelated naming reorg
  (`local-software-marketplace`/`-source` → `local-pointsav-marketplace`/`local-pointsav-release`
  on foundry-prod, already handled in `push-to-prod.sh`/`software-units.yaml`, nothing for us to
  change).
- [x] **`bin/self-service-promote.sh` retried successfully** — pushed `cluster/project-software`
  to both staging mirrors as its own ref (no collision), promote-queue entry confirmed durable
  (`staging_push_failed: 0`), `HEAD` at queue time `c763441e` (tip of the full storefront cleanup
  + this tracking). Confirmation sent back to Command (msg-id
  `command-20260703-re-self-service-promote-sh-fixed-retried`). **Nothing further from this
  archive until the canonical merge lands** — next step is Command processing the promote-queue.

### Live-site audit findings + fixes — 2026-07-04 [totebox@claude-code]

Full audit of `https://software.pointsav.com` after confirming the P8 cutover had landed
(commits `62c53d98`..`03c184bb` from the storefront-cleanup entry above). Full detail:
`BRIEF-software-ng-rewrite.md`'s 2026-07-04 entries.

- [x] **Trademark footer fixed** (`e84a5760`) — was asserting 6 fabricated marks never in
  `TRADEMARK.md`; now correct. 4 legal pages deduplicated to a shared helper.
- [x] **Security headers added** (`5c775712`) — HSTS/X-Content-Type-Options/X-Frame-Options/
  Referrer-Policy on both `marketplace-2`/`source-2`; CSP on `marketplace-2` only.
- [x] **`xtask deposit` subcommand built** (`707839f2`) — root-cause fix for "every release was
  hand-deposited"; writes binary + manifests + surgically updates `products.yaml`. Does not
  touch foundry-prod.
- [x] **Partially resolved [2026-07-05 totebox@claude-code]:** confirmed only 2/8 products
  (os-console, os-network-admin) ever had a real deposited release; the other 6 never did.
  Operator decision: removed the 6 unbuilt products from `products.yaml` rather than build them
  (stops the 404 symptom immediately). **Still open:** if any of those 6 should return to the
  catalog, a real `RELEASES_DIR` build must be deposited first — `xtask deposit` is ready for
  that once a binary exists. Also fixed a related prod/source drift: someone had hotfixed
  os-console/os-network-admin's edition+path directly on foundry-prod, bypassing Stage 6; source
  now matches. See `BRIEF-software-ng-rewrite.md`'s 2026-07-05 entry.
- [ ] **`v1_products`'s `download_url` formula bug** — built from `path` alone, missing the
  platform segment the real `/releases/:product/:version/:platform` route requires. Deliberately
  not fixed — depends on the platform-slug policy question below.
- [ ] **Platform-slug convention decision needed** — `order_download`/`product_detail.rs`
  hardcode `"linux-x86_64"`, `os-network-admin`'s install script uses `"x86_64"`, the two
  self-hosting infra install scripts use full Rust target triples. No safe default exists until
  someone picks a go-forward convention; `xtask deposit --platform` is required with no default
  for exactly this reason.
- [x] **`tool-wallet` atomic-write/backup gap — reconciled [2026-07-05 totebox@claude-code]:**
  queued as part of the Stage 6 reconciliation below (`0285c772`). Not yet confirmed merged to
  canonical — see next entry.

### Stage 6 reconciliation — 83-commit block re-diagnosed and split into 2 clean branches — 2026-07-05 [totebox@claude-code]

Command's `promote.sh` reported a blocked cherry-pick (83 commits, conflict on the oldest
scaffold-era commit). Re-diagnosed from scratch rather than trusting the specific claims — real
scope was much smaller. Full detail: `BRIEF-software-ng-rewrite.md`'s 2026-07-05 entry,
`command-20260705-stage-6-reconciliation-done-corrected-di`.

- [x] **Corrected: `xtask/src/deposit.rs` was never actually missing from canonical** — already
  landed byte-identical under a different commit hash. No action.
- [x] **Corrected: canonical's `xtask/src/main.rs` is ahead of this branch, not behind** — has
  project-knowledge's `check-content` gate, which this branch's xtask lacks entirely. Left xtask
  untouched; promoting our version would have regressed another archive's feature.
- [x] **Reconciled onto `scratch-stage6-reconcile` (0285c772), off `origin/main`, tests/fmt/clippy
  clean:** old-crate `app-privategit-marketplace`/`app-privategit-source` dead-code cleanup
  (neither is the deployed binary post-P8-cutover), the `tool-wallet` atomic-lock fix above, 5 new
  `install-templates/*.sh` files, `os-privategit`'s scaffold-to-real-docs cleanup.
- [x] **products.yaml drift fix isolated onto its own branch** (`scratch-products-yaml-fix`,
  424b2746) so it isn't gated on the above — see entry above.
- [ ] **Not yet confirmed merged to canonical** — both branches are in `promote-queue.jsonl`,
  mirror push failed (expected, ref-collision-by-design), Command needs to `promote.sh` directly
  from the local Totebox clone's branches (deleted locally after queuing; recoverable from
  `424b2746`/`0285c772`). Verify against `origin/main` before assuming either has landed.
- [ ] **Deliberately NOT reconciled — low value, flagged not silently dropped:**
  `app-privategit-source`'s revocation-list feature and old-`app-privategit-marketplace`'s
  cleanup both target crates that aren't the deployed binaries (`-2` versions are, and `source-2`
  already has its own independent revocation implementation) — queued anyway per operator's
  "go further" choice, but landing them fixes no live customer-facing gap.

### NEXT.md duplication — RESOLVED, consolidated to monorepo root [2026-07-02 totebox@claude-code]

This file was previously duplicated: an actively-maintained copy at the archive root
and this stale copy (last touched 2026-05-16). Consolidated per `repo-layout.md`'s stated
convention (NEXT.md belongs at the monorepo root) — archive-root copy reduced to a
one-line pointer at `../NEXT.md`. This section documents the merge; no open action.

### VM stability — crash prevention [2026-05-16 task@claude-code]

Root causes identified and addressed after 2× daily crash pattern (GCP host maintenance + cgroup OOM).

- [x] **LadybugDB buffer pool blowup** — `SystemConfig::default()` allocated 12.8 GB (80% RAM). Fixed: explicit `buffer_pool_size` from env var `SERVICE_CONTENT_LBUG_BUFFER_POOL_MB` (default 64 MB). Deployed `7672e76f`. Dropin: `MemoryMax=3G`, pool=2048 MB.
- [x] **local-slm MemoryMax reverts to 3G on daemon-reload** — created `/etc/systemd/system/local-slm.service.d/memory.conf` with `MemoryMax=6G`. Verified `6442450944` bytes after reload.
- [x] **vm.swappiness=10** — set via `/etc/sysctl.d/99-foundry-inference.conf`. Prevents inference workload swap.
- [x] **Retry storm on circuit-open extract** — added `Retry-After: 300` header to `/v1/extract` when `yoyo-circuit-open`. Deployed `31397dad`.
- [x] **GCP host maintenance — MIGRATE confirmed** — `onHostMaintenance=MIGRATE`, `automaticRestart=True`, `preemptible=False`. VM already correctly configured. Crashes were OOM-only, not host maintenance.
- [x] **journald cap** — `/etc/systemd/journald.conf.d/foundry-cap.conf` created with `SystemMaxUse=2G`; journald restarted. Done session 3 (2026-05-16).
- [ ] **Delete unused 7B-Think weights** — `/var/lib/local-slm/weights/` has wrong 7B variant (4.5 GB). Recover disk space once 7B → OLMo 2 1B is confirmed stable.

### service-content — ontology CSVs + Domains.json [2026-05-16 task@claude-code]

**DONE** — commit `7e55e530` (Jennifer Woodfine):
- `topics_documentation.csv`: 167 documentation wiki articles registered (168 total rows).
- `guides_documentation.csv`: 38 additional GUIDEs registered (44 unique fleet guides total).
- `Domains.json`: `"Sovereign Telemetry"` → `"Verified System Telemetry"` (Do-Not-Use §5).
- **Known gap:** ~30 topic titles are slug-derived (fallback) rather than H1-extracted. Low-priority editorial cleanup only.
- **Stage 6** already complete on session start — `main == origin/main`. No promotion action needed this session.
- **Yo-Yo 1-hr test:** DONE. Watchdog fired at T+1hr (2026-05-16T17:33:40Z) but `stop-yoyo.sh` failed — `SCRIPT_DIR: unbound variable` in watchdog subshell. VM stopped manually; bug fixed in `2a4c8ade` (SCRIPT_DIR defined at line 40 of `start-yoyo.sh`).

### service-slm / service-content — Sprint 0a prerequisites [2026-05-14 task@claude-code]

**Sprint 0a SHIPPED** — `POST /v1/messages` live on workspace VM (`fdd1a223` + `7cd9ca61`).

- [x] **Add `graph_context_enabled: Option<bool>` to `ComputeRequest`** — done; shim sets `Some(false)` (`slm-core/src/lib.rs:116`, `http.rs:1308`)
- [x] **Decide opus → Tier C path** — Path A shipped (2026-05-16): `claude-opus-*` routes `tier_hint: External`, `tier_c_label: "editorial-refinement"` (`31397dad`). Requires `has_external=true` at runtime (Tier C env config). Currently returns 503 (unconfigured) which is correct failsafe.
- [x] **Reconcile apprenticeship flag drift** — `compute/systemd/slm-doorman.service:37` updated to `true` (2026-05-15)

**Sprint 0b (next):**
- [ ] **Real per-token SSE streaming** in `http.rs::anthropic_sse_body()` (~60 LOC). Currently buffers full response then emits 6 events at once.
- [ ] **On-demand Yo-Yo lazy-start** in `router.rs` — start Yo-Yo VM when Tier B request arrives and VM is stopped.
- [ ] **Wire `SLM_TIER_C_ANTHROPIC_*` env** for opus → Tier C passthrough (routing is wired in `31397dad`; ExternalTierClient needs API key + endpoint env vars set in `local-doorman.env`).

### service-content — Ring 2/Ring 3 decoupling [2026-05-14 task@claude-code]

Current `main.rs` is the **legacy watcher** that `service-content/ARCHITECTURE.md` designates
deprecated. Ring 2 ingest halts completely when Ring 3 (Doorman) is unavailable — the Community
Tier principle is aspirational, not real. See `.agent/plans/service-content-architecture-2026.md`.

- [x] **Sprint 1 — deterministic Source node write** — done (2026-05-15, `889bc993`). Source node written before Doorman call; graph grows regardless of Tier B reachability.
- [ ] **Persistent extraction queue** (replace per-boot retry)
  `processed_ledgers: Vec<String>` resets on restart. 114 deferred files retry every boot.
  Fix: disk-backed set (sidecar JSONL or SQLite) + Yo-Yo-up notification trigger.
- [x] **Validate `module_id`; reject `__` prefix** — done (2026-05-15, `889bc993`). Rejects `__`-prefixed overrides.
- [ ] **Wire `RelatedTo` edges in graph store**
  `graph.rs:66-72` declares `RelatedTo` table; it is never populated anywhere. Graph is
  node-only. Everything in ARCHITECTURE.md §8 about linked nodes is unmet.
- [x] **Fix `main.rs:293` unwrap** — done (2026-05-15, `889bc993`).
- [ ] **Move `/v1/draft/generate` to Doorman** (Ring violation — Ring 2 generating text via Ring 3).

### service-slm — audit ledger completeness [2026-05-14 task@claude-code]

- [x] **`ExtractionAuditEntry` missing fields** — done (2026-05-15, `889bc993`). `model`, `cost_usd`, `sanitised_outbound` added.
- [x] **Add `"graph-query"` to `AUDIT_CAPTURE_VALID_EVENT_TYPES`** — done (2026-05-15, `889bc993`).

### Leapfrog compound loop — close the flywheel [2026-05-14 task@claude-code]

The compound moat (apprenticeship → LoRA → sovereign model) requires these steps in order.
See `.agent/plans/leapfrog-2026.md` for full strategic analysis.

- [x] **1. Git post-commit hook** — done (2026-05-15). `service-slm/scripts/capture-edit.sh` (54 LOC). Reads `.git/foundry-brief-id`; POSTs diff to `/v1/shadow`. Install: `ln -sf ... .git/hooks/post-commit`. Agent session writes brief_id to file before committing; clears at session end.
- [ ] **2. Eval harness** — held-out eval set + regression test for Tier A and Tier B tasks.
  Must exist BEFORE first LoRA training run (no way to measure improvement otherwise).
- [x] **3. Corpus quality gate** — shipped (2026-05-16, `31c389b7`): MIN_BRIEF_BODY_CHARS=50, MIN_DIFF_CHARS=20, PII patterns (API keys, SSH private keys). 422 on rejection.
- [ ] **4. Ratify `conventions/permissible-model-substrate.md`** — BCSC posture, OLMo-only
  rule, upgrade procedure as policy. Excludes Qwen/DeepSeek/Yi/GLM (PRC-headquartered).
- [ ] **5. Tier A upgrade** — `OLMo-2-1124-7B-Instruct-Q4_K_M.gguf`, `MemoryMax=6G`.
  Current 1B cannot produce reliable flat-schema tool-call args (blocks haiku-tier shim).
  Requires weights download to `/var/lib/local-slm/weights/` + unit file update + redeploy.
- [ ] **6. First LoRA training run** — on Yo-Yo #1 after steps 1–3 complete.
- [ ] **7. mistralrs-server migration** — at LoRA milestone; enables hot-swap adapters at runtime.

### app-mediakit-knowledge — Phase 4 continuation

**CLOSED (2026-05-15).** Steps 4.1–4.8 all confirmed shipped in source:
`src/mcp.rs` (Step 4.6, `POST /mcp` default-off), `src/git_protocol.rs` (Step 4.7
smart-HTTP). Project-root `NEXT.md` already says "Phase 4 COMPLETE". CLAUDE.md and
project NEXT.md are authoritative.

Remaining open item: **Deploy** — rebuild release binary, restart
`local-knowledge-documentation.service` and `local-knowledge-projects.service`.
This requires operator presence on the workspace VM; no code work needed.

### Leapfrog 2030 Architecture & Multi-Yo-Yo Roadmap
- **Software layer complete** (180/180 tests as of 2026-05-15). See `service-slm/NEXT.md`.
- **Yo-Yo #1 VM live** — `yoyo-tier-b-1` in `europe-west4-a` (relocated from `us-central1-a` via Mode 2 stockout cascade; confirmed 2026-05-15). L4, image `slm-yoyo-20260507-061137`. Doorman wired; nginx TLS + bearer auth verified working.
- **Idle monitor fixed** (`890b3f6`) — was returning HTTP 411 (missing `Content-Length: 0`
  on GCP POST); fixed with `.body("")`. The SA (Editor role) can stop instances without
  additional IAM grant — step 2 below is no longer required.
- **VM currently TERMINATED** — stopped 2026-05-16 (manually, after 1-hr watchdog failed due to SCRIPT_DIR bug; bug fixed in `2a4c8ade`). No Instance Schedule active; operator must start manually when weights are ready.
- **Remaining operator steps:**
  1. Upload OLMo 3 32B-Think Q4 weights (~20 GB) to `/data/weights/olmo-3-32b-think-q4.gguf`
     on the Yo-Yo VM via `gcloud compute scp`. This is the only blocker for full
     nightly drain cycle. Once loaded, VM starts at 02:00 UTC, vLLM serves, drain
     worker routes briefs to Tier B, idle monitor stops VM after 30 min idle.
  2. ~~Grant `roles/compute.instanceAdmin.v1`~~ — not needed; Editor role sufficient.
  3. Run smoke test per `service-slm/docs/deploy/deploy-yoyo-tier-b.md` §8.
  4. Re-enable apprenticeship: set `SLM_APPRENTICESHIP_ENABLED=true` in `local-doorman.env`.
- Runbook: `service-slm/docs/deploy/deploy-yoyo-tier-b.md`.

### Layout hygiene — defect closures queued

Rule source: `.agent/rules/repo-layout.md` (introduced 2026-04-23).
Each item below is a separate commit via `tool-commit-as-next.sh`.

*(queue empty — Tier-2 project-root scripts closed 2026-04-23;
see Recently closed below and `cleanup-log.md`)*

### Awaiting cross-repo handoff

Entries lodged in `.agent/rules/handoffs-outbound.md`. Pattern is
passive — nothing moves until Master Claude or a Root Claude in
the destination repo picks up the entry and commits the add-side.
Source files remain in place here until the destination has
committed; only then does a follow-up Root Claude session commit
the source-remove.

- **`guide-operations.md` → `content-wiki-documentation`** — see
  outbox for destination path and rationale.
- **`USER_GUIDE_2026-03-30_V2.md` → `content-wiki-documentation`**
  (with `_V2` dropped in transit) — see outbox.

### Framework follow-ups

- **BIM project activations** — three of four BIM projects are still
  Reserved-folder. Follow the `app-console-bookkeeper` pilot pattern
  (framework §8): `app-console-bim`, `app-orchestration-bim`,
  `app-workplace-bim`, `service-bim` (the fourth, which triggered
  the taxonomy expansion).
- **`service-bookkeeper` forward reference** — the
  `app-console-bookkeeper` view reads "Awaiting service-bookkeeper
  sync" but that service is not in the registry. Decide: register
  as Reserved-folder, redirect to `service-fs/data/`, or correct
  the reference.
- **HTML-plugin vs Rust-crate `Type`-column refinement.**
  `app-console-*` and `app-network-*` projects contain both
  patterns; the registry's `Type` column does not distinguish.
  Surfaced during bookkeeper activation.
- **`BIM.zip` triage** — verified 2026-05-07: no zip artefact present on disk; item closed.

### Rename series

*(queue empty — all five rename-series items closed 2026-04-23;
see Recently closed below and `cleanup-log.md` Completed
migrations)*

### Structural defects

- **lbug 0.16.1 prebuilt packaging regression** [2026-05-13] — The prebuilt `liblbug.a`
  (both `compat` and `perf` Linux x86_64 variants) shipped without the companion
  `libfastpfor.a`, causing undefined `__fastpack*` symbols at link time. Workaround:
  build from source via `LBUG_SHARED=1`. Resolution options:
  (a) pin `lbug` to the last version with a self-contained static prebuilt (was working
  with lbug as of 2026-05-08 binary), or
  (b) add a `build.rs` env override to force shared-lib path by default.
  Upstream: report packaging regression to lbug crate maintainers.

- ~~**`start-yoyo.sh` Mode 2 Doorman env bug**~~ — **CLOSED (2026-05-15).** `update_doorman_env` already called at line 421 in Mode 2 path (confirmed in code). Both Mode 1 (line 388) and Mode 2 (line 421) call it unconditionally.

- ~~**`start-yoyo.sh` watchdog `SCRIPT_DIR` unbound variable**~~ — **CLOSED (2026-05-16).** `SCRIPT_DIR` was used at line 469 in the `--runtime` watchdog subshell but never defined. 1-hr watchdog fired but `stop-yoyo.sh` call failed; VM left running. Fix: `SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"` added after `set -uo pipefail`. Commit `2a4c8ade` (Peter Woodfine).

- **Workspace `Cargo.toml` unification** — per 2026-04-18 audit,
  workspace declares only 8 of ~70+ crates as members. Other crates
  are treated as standalone workspaces (hence 23 stray
  `Cargo.lock` files). Unifying would consolidate targets and
  resolve profile inheritance.
- **Large binaries** — tracked artefacts that should move to
  build-time fetch:
  - `app-mediakit-telemetry/assets/GeoLite2-City.mmdb` (63.5 MB)
    — **still tracked**. Next candidate for fetch-at-build
    treatment. Paths reclassified 2026-04-23.
  - `service-slm/router-trainer/engine/llamafile` (35 MB) —
    **untracked since 2026-04-23** via `git rm --cached` + new
    `.gitignore` pattern. Physical file remains at path for the
    Python workflow. History still contains the blob; shrinking
    the repo requires `git-filter-repo`, separate task.
  - `service-slm/router-trainer/engine/weights/qwen2.5-coder-1.5b.gguf`
    (15 MB) — already covered by existing `**/weights/*` +
    `*.gguf` ignore patterns. Same history-blob caveat applies.
  - ISO / IMG artefacts in `os-infrastructure/`,
    `os-network-admin/`, `os-totebox/` (tracking status TBD).

### Conformance and activations

*(queue empty — see Recently closed 2026-05-07 below)*

### Stashes parked in this repo

- `stash@{0}` — 2026-04-22 — "task21 WIP before worktree removal"
  (on `audit-layer-1-findings`; engineering work on `slm-memory-kv`
  crate, renames, untracked research doc). Restore with
  `git stash pop` when ready to resume.
- `stash@{1}` — pre-existing — "On service-extraction-v04: main:
  registry + BIM untracked — parked before task [21] resume".

## Recently closed (2026-05-07)

- **Reverse-Flow Substrate project registrations (Doctrine claim #52)** — six new
  Reserved-folder projects created with bilingual READMEs and registry rows in one
  commit each: `service-market`, `service-exchange`, `app-orchestration-market`,
  `app-orchestration-exchange`, `app-console-market`, `app-console-exchange`.
- **`app-orchestration-gis` registry drift** — directory created; Reserved-folder row
  added to registry. Deployed instance `gateway-orchestration-gis-1` was missing from
  the project registry.
- **`.gitignore` deduplication** — "Asymmetric Storage Protocol: Enforce Tier-1
  Quarantine" block was duplicated 4× (lines 4–18). Normalised to a single copy.
- **`service-extraction/CLAUDE.md`** — CLAUDE.md created; describes the 149-line
  filesystem-watching router accurately (replaces the stale v0.2/v0.4 framing in README).
- **`app-workplace-memo` activation** — CLAUDE.md + NEXT.md added; registry row
  promoted from Scaffold-coded → Active per framework §8.
- **`app-workplace-proforma/CLAUDE.md`** — local-only file committed to git; header
  updated to standard CLAUDE.md format.

## Recently closed (2026-04-23)

- Repo-layout rule introduced — `.agent/rules/repo-layout.md`
  codifies allowed files at the monorepo root and at each project
  directory root; names the sibling repos
  (`content-wiki-documentation`, `pointsav-design-system`, etc.)
  where cross-cutting content belongs. Anchor for the "Layout
  hygiene" queue above.
- `force_build.sh` relocated — root → `vendor-sel4-kernel/scripts/`.
  Zero runtime callers; script uses absolute paths so no content
  edits were needed. Repo root is now one file lighter against the
  new rule.
- `os-infrastructure/build_iso/forge_iso.sh` renamed to
  `compile_binary.sh` — resolves filename collision with the
  sibling ISO-assembly script at the project root. In-file header
  updated. Zero external callers. New open question logged in
  `cleanup-log.md`: the compile and assembly scripts are not wired
  together.
- `app-console-content/src/{pointsav-surveyor.sh,surveyor.py}`
  relocated to `app-console-content/scripts/`. Both files moved as
  100% renames. Shell wrapper is relative (`$(dirname "$0")`),
  Python script uses absolute paths — neither needed content
  edits. Throttle open-question row in `cleanup-log.md` updated
  with a code-reference pointer to the new path; the operator
  decision on `MAX_DAILY_VERIFICATIONS = 10` remains open.
- Handoff-outbound pattern introduced —
  `.agent/rules/handoffs-outbound.md` logs cross-repo file moves
  kept in place here until a Root Claude in the destination repo
  commits them. Two entries lodged (`guide-operations.md`,
  `USER_GUIDE_2026-03-30_V2.md`, both to
  `content-wiki-documentation`). Formalisation of the pattern in
  `~/Foundry/CLAUDE.md` §9 and §10 surfaced for Master Claude in
  `cleanup-log.md`.
- Tier-2 project-root scripts relocated — 18 files across 9
  projects moved to their respective `scripts/` subfolders in 9
  separate commits (`8f5cc48` through `faae141`). Every file
  registered as a 100% rename; no callers needed updating.
  Projects touched: `os-totebox`, `service-content`,
  `service-email`, `service-slm`, `tool-cognitive-forge`,
  `os-network-admin`, `vendor-phi3-mini`, `service-vpn`,
  `app-mediakit-telemetry`. Stray `tool-cognitive-forge/llama.log`
  surfaced as a separate housekeeping item.
- `service-parser/` removed — first rename-series closure.
  Directory contained only a README describing a superseded
  AI-routing framing; zero runtime references, never a workspace
  member, one commit in history. Nothing recyclable into
  `service-extraction` (which describes a different, deterministic
  Parser-Combinators approach). Rename-table row moved to
  Completed migrations; registry row removed (Defect count
  5 → 4, Total rows 100 → 99).
- `pointsav-pty-bridge` → `service-pty-bridge` — second
  rename-series closure. Directory renamed via `git mv` (4 files,
  all 100% renames); `Cargo.toml` `name` field updated in the
  same commit. Registry row moved from "Other / special" into
  the Service table; reclassified Defect → Scaffold-coded
  (Defect 4 → 3, Scaffold-coded 51 → 52). Zero external import
  references; not a workspace member; stray `Cargo.lock` left
  in place (resolves with workspace unification).
- Fifth (final) rename-series closure — Cognitive Forge term
  retired in one commit. `service-slm/cognitive-forge/` renamed
  to `service-slm/router/`; former top-level `tool-cognitive-forge/`
  moved to `service-slm/router-trainer/`. Rust runtime
  (`router/`) and Python distillation workflow
  (`router-trainer/`) now live together as producer/consumer.
  Cargo.toml `name` + `main.rs` usage string updated.
  `distill_knowledge.py` moved from non-canonical `src/` to
  `scripts/`. Three binary/log files untracked via `git rm
  --cached` + new `.gitignore` patterns (llamafile 35 MB,
  engine.log, llama.log) — physical files remain at new paths.
  Registry Scaffold-coded 54 → 53, Total 98 → 97. Closes the
  rename-series queue entirely (5 of 5) and the separate
  `llama.log` housekeeping item.
- `service-email-egress-{ews,imap}` wrappers flattened — fourth
  rename-series closure. Consolidation-to-`service-email-egress`
  plan reversed after sub-crate review: EWS and IMAP are two
  protocol adapters, not duplicates, and merging them would erase
  the architectural distinction. Instead, the redundant
  doubly-nested wrapper directories were flattened — 73 files
  promoted up one level. Registry reclassified both from
  Defect → Scaffold-coded; Defect count 2 → 0 (registry is now
  Defect-free). The 13 dir-name / Cargo-name mismatches from the
  2026-04-18 audit remain separate.
- `vendors-maxmind` reclassified to
  `app-mediakit-telemetry/assets/` — third rename-series closure.
  Data-only directory moved to the authoritative path already
  documented in the vendor's README; `.mmdb` (63.5 MB) + both
  READMEs travelled together; empty `vendors-maxmind/` removed.
  Open question "does it belong as a `vendor-*` crate at all?"
  closed (answer: no; non-workspace data directory).
  `repo-layout.md` extended to name `assets/` and `data/` as
  conventional subfolders. Registry Defect 3 → 2, Total rows
  99 → 98. In-transit edit to `USER_GUIDE_2026-03-30_V2.md`
  line 902 updates the path reference — travels with the pending
  cross-repo handoff. Separate `.mmdb` → build-time-fetch task
  remains open under Structural defects.

## Recently closed (2026-04-22)

- Audit cleanup — removed 2 `__MACOSX/` directories and 16 tracked
  `.DS_Store` / AppleDouble files from egress extraction-artefact
  scaffolding. `.DS_Store` added to `.gitignore`. Commit `0eeaeba`.
- Project registry bootstrap — 96-row inventory covering every
  top-level directory. Commit `fd7811f`.
- BIM-research project rows + cleanup-log bootstrap on `main` (drift
  closed) + taxonomy-expansion session entry. Commit `3cc8f4a`.
- `app-console-bookkeeper` activation pilot — Reserved-folder
  (mis-classified) → Active. Commit `27ad6d2`.

## Pointers

- Workspace-level open items: `~/Foundry/NEXT.md`
- Workspace changelog: `~/Foundry/CHANGELOG.md`
- Project registry: `.agent/rules/project-registry.md`
- Cleanup log: `.agent/rules/cleanup-log.md`
- Repo layout rule: `.agent/rules/repo-layout.md`
- Handoffs outbound: `.agent/rules/handoffs-outbound.md`
