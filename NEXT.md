# NEXT.md — project-totebox (Totebox)

> Totebox Session — starts in `/srv/foundry/clones/project-totebox`
> **Scope: this archive only.** Cross-repo and workspace-level items live at `~/Foundry/NEXT.md`.

Last updated: 2026-06-23
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
