# NEXT.md — project-design (Totebox)

> Totebox Session — starts in `/srv/foundry/clones/project-design/`
> **Scope: this archive only.** Cross-repo and workspace-level items live at `~/Foundry/NEXT.md`.

Last updated: 2026-06-20
Last updated: 2026-06-19 (Session 26 — drain dispatch fix + Opus audit improvements)

---

## Active (Phase 2 complete — Phase 4 next)

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
- [ ] **DPO corpus: only 229/1,021 pairs survive training filters** — 77.5% filtered at load time
      (MAX_LENGTH_RATIO=8.0 gate); legacy pairs have extreme ratios (p50=20x, p90=180x); effective
      training set far below LIMA 1,000 threshold; will improve as Tier B enrichment produces
      real-diff pairs with reasonable ratios; re-audit after first 500 Tier B pairs written
      [2026-06-19 totebox@project-intelligence]
- [ ] **Entity vectors all null** — role_vector/location_vector on LadybugDB entities never
      populated; Tier B structured grammar path code-complete but drain sends plain prompts not
      grammar-constrained extraction; medium priority after Tier B basic enrichment is stable
      [2026-06-19 totebox@project-intelligence]

### Phase 2 completed (2026-06-20)

- [x] Archive contamination repair — CLAUDE.md, manifest.md, brief-discipline.md, artifact-registry.md (all 4 fixed) `[2026-06-20 command@claude-code]`
- [x] app-privategit-design-recovered/ deleted (untracked; safe rm) `[2026-06-20 command@claude-code]`
- [x] Foreign BRIEFs archived — 18 BRIEFs set to status: archived `[2026-06-20 command@claude-code]`
- [x] Outbox triage — 3 messages marked actioned (stage6-phase-d, contamination-flag, DESIGN-BUNDLE ratification request) `[2026-06-20 command@claude-code]`
- [x] DESIGN-RESEARCH intake (cb8b2a2) — design-system-2030-vision + knowledge-platform UX audit committed to dtcg-vault/research/ `[2026-06-20 command@claude-code]`
- [x] ASSET intake (cb8b2a2) — woodfine-org-chart-color-sample reference committed to assets/reference/ `[2026-06-20 command@claude-code]`

### Phase 3 (Command — pending)

- [ ] Stage 6 promote: pointsav-design-system df81d5b, af51d86, 9c8155c, 36295c3, cb8b2a2 (5 commits)
- [ ] Binary rebuild + deploy after Stage 6 (rust-embed bakes templates at compile time)
- [ ] sudo systemctl restart local-design.service

### Phase 4 (Totebox — after Phase 3 Command)

- [ ] DTCG correctness fixes: invalid `$type: "string"` ×4 + `$type: "boolean"` ×1 → $extensions.foundry
- [ ] Legacy string form → DTCG 2025.10 object form (dimension/duration/number tokens)
- [ ] Composite token groups: semantic.typography, semantic.elevation, semantic.border, semantic.transition, semantic.opacity
- [ ] component.document.legal.* namespace (subscription + prospectus)
- [ ] DESIGN-TOKEN-CHANGE-wcp-finance-bundle — awaiting jwoodfine cosign; leave in drafts-outbound

### Phase 6 (Totebox — after Phase 5 Command)

- [ ] src/schema/mod.rs — add SchemaType::Marketing enum + detect arm + render arm + pub mod marketing
- [ ] src/schema/marketing.rs — new file (sections: hero, feature-grid, cta, pricing, logo-wall)
- [ ] src/templates/marketing.html — new minijinja template
- [ ] src/schema/bundle.rs — replace stub with full implementation (DESIGN-BUNDLE ratified 2026-06-20)
- [ ] src/routes/mod.rs — bundle download route
- [ ] src/templates/bundle.html — new minijinja template
- [ ] Cargo.toml: zip = "2"; version → 0.3.0
- [ ] cargo fmt + clippy -D warnings + cargo test (all clean before Stage 6)

---

## Carry-forward

- [ ] DESIGN-RESEARCH-design-system-2030-vision — route to project-marketing (outbox msg-id project-design-20260614-design-research-2030-routing still pending; project-marketing needs to pick it up)
- [ ] DESIGN-TOKEN-CHANGE-woodfine-chart-css and woodfine-yellow-magenta — already applied in woodfine-media-assets (commits 17001af, 1b0db90); drafts can be marked superseded in drafts-outbound
- [ ] DESIGN-doc-header-component and DESIGN-docs-sidenav-component drafts — already committed (229c719); drafts-outbound stubs can be archived
- [ ] DESIGN-wireframe-home-header-v2c.draft.html — check destination; likely project-marketing scope

---

## Completed milestones

- **v0.2.0** — multi-module rewrite (Phase A routes, Phase B SSE, Phase C edit overlay, Phase D AI bridge); binary deployed 2026-06-20 sha256 1883110e; canonical commit 8c540cd4
- **DESIGN-BUNDLE ratified** — namespace component.document.legal.* confirmed 2026-06-20
# NEXT.md — project-editorial (Totebox)

> **Scope: this archive only.** Cross-repo and workspace-level items live at `~/Foundry/NEXT.md`.
> Hot open items. ≤200 lines. Backlog at `.agent/next-backlog.md`.

Last updated: 2026-06-19

---

## Active (Totebox scope)

- [ ] **Stage 6 pending** — Command: media-knowledge-projects (7fa466b + trademark commit 3e3579b), media-knowledge-corporate (ac6379f), media-knowledge-documentation (f1451e9) — 4 commits total since last promote [2026-06-19 totebox@claude-code]
- [ ] **media-knowledge-documentation M9** — ES parity sweep not yet run for documentation sub-clone [2026-06-19 totebox@claude-code]
- [ ] **F2/F3 dead links** — check --strict gate blocked; dead wikilinks in project-editorial [carried]
- [ ] **Track 2d / project-console** — Command routing pending for PROSE-RESEARCH-ppn-architecture-phd-thesis + knowledge-platform-rewrite; 13 artifacts awaiting Command ACK (msg-id: command-20260619-drafts-outbound-pickup-editorial-researc) [2026-06-19 totebox@claude-code]

## Blocked — Command Session (route via outbox)

- [ ] **Trademark Phase 1a** — factory-release-engineering (TRADEMARK.md, tokens/legal-tokens-*.yaml, readmes/footer-*.md, policies/DISCLAIMER.md, README.md, PLAYBOOK.md); outbox message sent [2026-06-19 totebox@claude-code]
- [ ] **Trademark Phase 4** — woodfine-fleet-deployment GUIDEs (~80 files), workspace governance docs (CLAUDE.md, AGENT.md, conventions/); admin-tier [2026-06-19 totebox@claude-code]

## Completed (2026-06-19)

- [x] **Trademark Phase 3 — TOPIC/GUIDE content wikis** — MCorp™ + Capability Geometry™ applied across all three sub-clones (documentation, projects, corporate); 3 commits (3e3579b, ac6379f, f1451e9); body text editorial pass done; formal legal disclaimers preserved; copyright lines corrected to Woodfine Capital Projects Inc. [2026-06-19 totebox@claude-code]
- [x] **NEXT.md contamination cleanup** — removed project-gis, project-console, project-intelligence, project-workplace, project-design content [2026-06-19 totebox@claude-code]
- [x] **M7 snapshot dating** — corrected 7,594 → 6,493 in index.md; methodology-example note in dedup article; commit 4649f95 [2026-06-19 totebox@claude-code]
- [x] **M9 EN/ES parity sweep** — all 53 ES articles in media-knowledge-projects at 84%+; 5 commit passes (f7a9be5, 6310748, 1c5d2db, ba4c412, 7fa466b) [2026-06-19 totebox@claude-code]
