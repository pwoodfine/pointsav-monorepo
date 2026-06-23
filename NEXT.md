# NEXT.md — project-system (Totebox)

> Totebox Session — starts in `/srv/foundry/clones/project-system`
> **Scope: this archive only.** Cross-repo and workspace-level items live at `~/Foundry/NEXT.md`.

Last updated: 2026-06-20

---

## Active
## Hot — pick up here next session

- [x] **NOTAM unreadable — resolved 2026-05-20** `[2026-05-20 totebox@claude-code]`
  - Fixed by Command: NOTAM.md now `-rw-r--r-- mathew:foundry` (world-readable). Outbox message actioned.

- [x] **Key Plans foundation — 4 operator decisions received 2026-05-20** `[2026-05-20 totebox@claude-code]`
  - All 4 decisions answered via inbox `command-20260520-bim-foundation-decisions`
  - Decision 1: descriptive display names (Index PDF style); codes (PO-1/M-1/B-1) are internal-only DTCG keys
  - Decision 2: **delete** inline BIM_TOKENS block from `building-width-calculator.html`; fetch from DTCG at render time
  - Decision 3: all 3 building types in scope now (Professional Centre + Retail Select + Tech Industrial + 12 common-area Key Plans)
  - Decision 4: type-prefixed tile codes (CO-A, RS-A, TI-A); Corridor Expander T = 300 SF; arithmetic gaps intentional by design; J/K/L/M as stub DTCG entries with `status: reserved`
  - **Now unblocked:** DTCG token standardisation, HTML BIM_TOKENS removal, Rust crate scaffold

- [x] **Deliverable 1: key-plans-registry.md — done 2026-05-21** `[2026-05-21 totebox@claude-code]`
  - Committed: d1ac026 in woodfine-bim-library (pwoodfine, main)
  - Output: `woodfine-bim-library/key-plans/key-plans-registry.md`
  - Also in `outputs/key-plans-registry.md` — pull via `fpull bim outputs/`

- [ ] **Apply Decision 1–4 to existing DTCG tokens + HTML** `[2026-05-21 totebox@claude-code]`
  - Standardise naming in all existing DTCG entries to Decision 1 convention
  - Delete BIM_TOKENS block from `building-width-calculator.html` (Decision 2)
  - Add stub entries for RS/TI tiles and J/K/L/M placeholders (Decisions 3 + 4)

### Foundation build DONE (BRIEF-flow-build-plan, 2026-06-21/22)
Autonomous foundation build — commits on cluster/project-totebox. P1–P10 of the audit
landed as code. Canonical base = OLMo-3-7B-Instruct.
- [x] **lbug ABI fixed + tests green** — `cargo test -p service-content` = 54/54 green. [2026-06-22 totebox@project-totebox]
- [x] **Stage 6 PROMOTED** — foundation + graph migration code on canonical origin/main. [2026-06-22 command via promote.sh]
- [x] **Additive graph migration** — entity_aliases, er_review_queue, RelatedTo write-path, in-batch ER wired. [2026-06-22 totebox@project-totebox]
- [x] **query_context canonical resolution** — alias-aware read path; 54/54 tests green. [2026-06-22 totebox@project-totebox]
- [x] **D9 closed** — created_at first-write-wins (no longer overwrites on re-upsert); fill-rate telemetry logged. [2026-06-22 totebox@project-totebox]
- [x] **D8 closed** — additionalProperties:false on extraction JSON schema. [2026-06-22 totebox@project-totebox]
- [x] **P8 closed** — redrive-quarantine.py fixed to target queue-poison/ (actual dead-letter dir). [2026-06-22 totebox@project-totebox]
- [x] **Stage 6 requested to Command** — outbox message sent; pending Command promote + local-content.service restart. [2026-06-22 totebox@project-totebox]
- [ ] **Activation (Command/sudo)** — run `service-slm/scripts/activate-foundation.sh`; restart local-content.service (init_schema now adds entity_aliases + er_review_queue — safe additive). [2026-06-22 totebox@project-totebox]
- [ ] **GPU training** — when yoyo-batch L4 returns: run-sft → run-dpo simpo → eval gate → promote. [2026-06-22 totebox@project-totebox]
- [ ] **Later stages** — GraphStore PK cutover (high blast radius, deferred); OWL2/reasoner/SHACL; always-on training loop. [2026-06-22 totebox@project-totebox]
- [ ] **D11** — service-extraction zero tests; own session. [2026-06-22 totebox@project-totebox]

### Flow Quality Audit (BRIEF-flow-quality-audit, 2026-06-20)
Two-stage swarm audit. 14 confirmed FAIL. P1–P10 fixes now code-complete on cluster branch.
- [x] **P1–P4, P6–P10** — ALL FIXED in code (see BRIEF carry-forward for full status). [2026-06-22 totebox@project-totebox]
- [ ] **P5** — deploy gate + `--lora-scaled` (Command/sudo + GPU hand-off). [2026-06-22 totebox@project-totebox]
- [ ] **GAP-4** — base-model fork RESOLVED in code (base-registry.yaml); activation still Command. [2026-06-22 totebox@project-totebox]
- [ ] **Phase D witness runs (deferred — STOCKOUT)** — capped train + delta-probe; needs yoyo-batch L4. [2026-06-20 totebox@project-totebox]
- [ ] **Drift: this NEXT.md is contaminated** (titled `project-system`, holds GIS/AEC items). Separate cleanup session. [2026-06-20 totebox@project-totebox]

v0.3.0 plan at `/home/jennifer/.claude/plans/no-make-a-plan-abundant-forest.md`.
- [ ] **Stage 6 + Doorman rebuild** — outbox updated (msg-id project-intelligence-20260620-session26c-stage6-prompt-fix);
      commits `c0448b81`→`0506d359` (8 commits). After rebuild, add systemd overrides:
      `SLM_DRAIN_CONCURRENCY=4` and `SLM_QUEUE_DRAIN_INTERVAL_SEC=1` to local-doorman.service.
      Command scope.
      [2026-06-20 totebox@project-intelligence]
- [x] **DPO corpus quality: 55% template-echo stubs** — root cause: `apprentice_prompt()` had
      redundant "## Required response shape" block with `<unified diff, OR empty if escalate=true>`
      placeholder inside code fence; OLMo echoed it literally. Fix: removed block entirely
      (system prompt already shows format). Commit `0506d359`. Expect real_diff rate 19%→50%.
      [2026-06-20 totebox@project-intelligence]
- [ ] **down_for_secs in TierBInfo** — `health_down_secs: Option<u64>` added to TierBInfo
      + `health_down_since_secs: Arc<AtomicU64>` wired in YoYoTierClient/run_health_probe;
      committed but deploy pending (Stage 6 + slm-doorman-server rebuild required)
      [2026-06-19 totebox@project-intelligence]
- [ ] **Phase 4b reconciliation pass** — 1,281 sweep-ledger entries written before Tier B online;
      DOC_sweep quarantine gate in place; Totebox sprint when Tier B restores; gated on
      yoyo-batch being provisioned in us-central1-a (operator approval required)
      [2026-06-15 command@claude-code]
- [x] **CLAUDE.md contamination** — confirmed clean (81 lines, correct project-intelligence
      SLM/Doorman/OLMo/LoRA/DataGraph content; no project-console text)
      [2026-06-19 totebox@project-intelligence]
- [ ] **Phase 5b — adapter pull verification** — pull wired in nightly-run.sh (Phase 5b block);
      pulls from yoyo-batch:/data/weights/adapters/apprenticeship-pointsav-wip/ at start of
      Phase 1 each cycle; verify after first successful yoyo-batch cycle:
      `ls /srv/foundry/data/adapters/apprenticeship-pointsav-incremental/`
      [2026-06-19 totebox@project-intelligence]
- [x] **Phase 6-D — enrichment spot-check** — 3 extractions confirmed; `tier_used: "tier_a_fallback"`;
      OLMo-2 Tier A returning clean entities (Person/Company/Location); f1879462 verified working
      [2026-06-19 totebox@project-intelligence]
- [ ] **Remove dead config** — `SERVICE_CONTENT_TIER_A_FALLBACK_ENABLED=false` confirmed
      absent from all codebase files; must be in live systemd unit only; Command scope
      (systemd override cleanup + daemon-reload); routed via outbox
      [2026-06-19 totebox@project-intelligence]
- [x] **Bug: semaphore leak on client disconnect** — fixed 2026-06-19; 120 s timeout wrapper
      (`EXTRACT_DEADLINE_SECS`) around entire routing block in `/v1/extract` handler;
      `DoormanError::RequestTimeout` returned on deadline → permit drops via RAII; bounds
      permit hold to 120 s even when hyper 0.14 keeps handler alive after client disconnect
      [2026-06-19 totebox@project-intelligence]
- [x] **Bug: DeferReason wildcard in http.rs** — fixed 2026-06-19; added `TierAFailed`,
      `ParseError`, `Timeout`, `AllTiersUnavailable` variants to `DeferReason` enum in
      slm-core; both extract + batch handler wildcards now have explicit arms;
      `DoormanError::RequestTimeout` added to error.rs + ApiError status mapping
      [2026-06-19 totebox@project-intelligence]
- [ ] **Known: queue saturates OLMo in Tier B degraded mode** — corpus queue runs 2 in-flight
      (matching OLMo --parallel 2); when Tier B down, queue uses Tier A leaving 0 slots for
      interactive /v1/extract; resolves automatically when yoyo-batch restores (queue → Tier B);
      workaround: limit queue to 1 in-flight via SLM_BATCH_CONCURRENCY=1 when Tier B down
      [2026-06-19 totebox@project-intelligence]
- [x] **DPO corpus: only ~168/1,021 pairs survive — task unlearnable as framed** — 2026-06-19
      four-agent Opus audit: prompt=bare commit subject (no file ctx), chosen=whole-repo diff,
      rejected=OLMo fragment (93x ratio). SFT-first pivot (commit `3ee7eaaa`): export-sft.py
      per-file split + canonical envelope → 2,585 clean SFT records (15x); run-dpo-training.py
      --mode sft + max_length=512 truncation fix. See BRIEF-training-pipeline-10x.
      [2026-06-20 totebox@project-intelligence]
- [ ] **SFT-first follow-ups** (BRIEF-training-pipeline-10x §Decisions open):
      (a) file-grounded prompts — git post-commit hook to capture SHA + pre-edit blobs (Rust/hook);
      (b) wire SFT stage into lora-update.sh/nightly before the preference stage;
      (c) DPO-format fix in verdict.rs (both sides canonical envelope) for the later pref phase;
      (d) verify SFTTrainer/SFTConfig API on yoyo-batch trl 1.5.1 before first real run.
      [2026-06-20 totebox@project-intelligence]
- [ ] **DataGraph NULL vectors — prompt/schema contradiction** — service-content/src/main.rs:55
      extraction prompt says "exactly two fields" while schema (main.rs:869-885) declares 5
      (incl. 3 vectors); prompt actively forbids vectors. Fix: add vectors to prompt + few-shot,
      or delete from schema. Plus: no entity resolution (Corp./Corp dupes). See BRIEF §DataGraph.
      [2026-06-20 totebox@project-intelligence]
- [ ] **Entity vectors all null** — role_vector/location_vector on LadybugDB entities never
      populated; Tier B structured grammar path code-complete but drain sends plain prompts not
      grammar-constrained extraction; medium priority after Tier B basic enrichment is stable
      [2026-06-19 totebox@project-intelligence]
### seL4 Phase H1 — moonshot-toolkit integration
- [x] **HTML print layout — resolved 2026-05-17** `[2026-05-17 totebox@claude-code]`
  - Root cause: `@page { size: landscape; margin: 0.3in }` + `slide { width: 10.4in }` triple-stacked margins; Chrome silently ignored
  - Fix: `@page { size: 11in 8.5in; margin: 0; }` + `slide { width: 11in; height: 8.5in; transform: none }` in all 3 preview HTMLs
  - PDF generator: `preview/build-pdf.mjs` (Playwright + Chromium); confirmed 792×612pt = 11×8.5in per page
  - Generate: `NODE_PATH=/home/jennifer/sandbox/working/ps-talking-points/node_modules node build-pdf.mjs <file.html>` or `all`
  - Do NOT use the browser print dialog — output varies by operator; use the script

- [x] `moonshot-toolkit` v0.3.1 — build pipeline functional; `os-console-hello.toml` spec exists; QEMU gate passed
- [x] `moonshot-sel4-vmm` Phase H1 — `#![no_std]` PD runtime complete (syscall, types, debug modules)
- [x] Confirm project-data PD target — `os-totebox` confirmed via `BRIEF-os-totebox-build-out` (owner: project-data) `[2026-06-20 totebox@project-system]`
- [x] Create `moonshot-toolkit/examples/os-totebox-hello.toml` + `totebox_hello.c` — committed `23b7026d5` `[2026-06-20 totebox@project-system]`
- [ ] NOTE: moonshot-toolkit + moonshot-sel4-vmm both declare `[workspace]` — cannot be monorepo workspace members; use `--manifest-path` for toolkit, path deps for vmm in PD crates
- [ ] Stage 6 pending: commit `23b7026d5` (os-totebox Phase H1 seL4 spec) — route to Command `[2026-06-20 totebox@project-system]`

### Clippy gate verification

### Phase 3 (Command — complete 2026-06-20)

- [x] Stage 6 promote: pointsav-design-system df81d5b..cb8b2a2 (5 commits) — canonical push successful
- [x] Vendor mirror pulled (cb8b2a2); sync-design-tokens.sh ran; research/ synced to vault
- [x] sudo systemctl restart local-design.service; healthz ok

### Phase 4 (Totebox — complete 2026-06-20)

- [x] DTCG correctness fixes: invalid `$type: "string"` ×4 (dtcg-bundle.json) + boolean×3 (main-page.dtcg.json) → $extensions.foundry (commit dc9eca1)
- [ ] Legacy string→object form migration (dimension/duration/number) — DEFERRED to v0.4.0; 64 dimension group headers + 100+ leaf values; too large for this phase
- [x] Composite token groups: semantic.typography + elevation + transition + opacity (commit de6fbab)
- [x] component.document.legal.* namespace (subscription + prospectus) (commit de6fbab)
- [ ] DESIGN-TOKEN-CHANGE-wcp-finance-bundle — awaiting jwoodfine cosign; leave in drafts-outbound

### Phase 5 (Command — after Phase 4 outbox pickup)

- [ ] Stage 6 promote: pointsav-design-system dc9eca1 + de6fbab (2 commits)
- [ ] Binary rebuild + deploy + sudo systemctl restart local-design.service
- [ ] Smoke test: composite token groups visible in token browser

### Phase 6 (Totebox — complete 2026-06-20)

- [x] src/schema/mod.rs — SchemaType::Marketing + detect/render dispatch (commit 5cbf6ced)
- [x] src/schema/marketing.rs — new: :::block-type parser, hero/feature-grid/cta/pricing/logo-wall (commit 5cbf6ced)
- [x] src/schema/bundle.rs — full implementation: identity header, member list, metadata dl (commit 5cbf6ced)
- [x] src/routes/browse.rs — bundle_download handler: in-memory ZIP via zip v2.4.2 (commit 5cbf6ced)
- [x] src/routes/mod.rs — /elements/:slug/download route (commit 5cbf6ced)
- [x] Cargo.toml: zip = "2.4.2"; version → 0.3.0 (commit 5cbf6ced)
- [x] cargo fmt ✓ + clippy -D warnings ✓ + cargo test ✓
- Note: marketing.html + bundle.html templates not needed — renderers produce HTML strings directly (pattern: component.rs, research.rs)

### Phase 7 (Command — Stage 6 + final deploy)

- [ ] Pick up Stage 6 outbox: project-design-20260620-stage6-v030-code
- [ ] promote.sh from clones/project-design (or direct sub-clone push if dirty tree blocks)
- [ ] cargo build --release -p app-privategit-design (must build with zip v2 dep)
- [ ] bin/deploy-binary.sh app-privategit-design + sudo systemctl restart local-design.service
- [ ] Smoke tests: /healthz ok; MARKETING + BUNDLE elements render correctly; /elements/:slug/download returns ZIP
- [ ] CHANGELOG.md v0.3.0 entry
- [ ] binary-ledger sha256 verify
- [x] `cargo clippy -p system-vm-fleet-types -- -D warnings` — CLEAN; carry-forward was stale `[2026-06-20 totebox@project-system]`
- [x] `cargo clippy -p os-console -- -D warnings` — CLEAN; carry-forward was stale `[2026-06-20 totebox@project-system]`

### Archive identity repair (ongoing)

- [ ] CLAUDE.md header still says "project-design — Archive Guide" — needs correction to project-system `[2026-06-19 command@claude-code]`
- [ ] `.agent/manifest.md` `cluster:` field says "project-design" — needs correction to project-system `[2026-06-19 command@claude-code]`
- [ ] `.agent/briefs/README.md` contains project-marketing content — needs rewrite `[2026-06-20 totebox@project-system]`

---

## Blocked — Command Session

- [ ] drafts-outbound contamination — 24 foreign files pending redistribution (outbox msg-id: project-system-20260614-drafts-outbound-contamination; attempts: 3)

- [ ] **Opus army synthesis — 5 operator decisions surfaced** `[2026-05-17 totebox@claude-code]`
  - Source: `.agent/plans/agent-{1,2,3}-*-report.md`
  1. **Academic Small area** — 105 m² (V3 Master Summary, authoritative) vs 87.7 m² in `woodfine-bim-library/tokens/bim/professional-office-subtypes.dtcg.json`. Token file needs update commit.
  2. **Civic zone depths** — still synthesised; no DISCOVERY sketch exists. Field-research pass needed.
  3. **Professional Office Z2/Z3** — V12 carries TBD placeholders (3.0/3.0). Confirm or specify.
  4. **Business Building Width option** — A/A (32.29 m, widest) is currently in HTML; operator may prefer C/C (27.27 m, balanced). Confirm.
  5. **End-cap tile sizing** — tokens say E-1/E-2 = 2,700 SF; V12 Methodology end-cap diagrams show 3,500–5,500 SF. Token file fix needed.

---

## Completed milestones

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
