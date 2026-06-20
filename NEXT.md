# NEXT.md — project-design (Totebox)

> Totebox Session — starts in `/srv/foundry/clones/project-design/`
> **Scope: this archive only.** Cross-repo and workspace-level items live at `~/Foundry/NEXT.md`.

Last updated: 2026-06-20
Last updated: 2026-06-19 (Session 26 — drain dispatch fix + Opus audit improvements)

---

## Active (Phase 2 complete — Phase 4 next)

v0.3.0 plan at `/home/jennifer/.claude/plans/no-make-a-plan-abundant-forest.md`.
- [ ] **Stage 6 + Doorman rebuild** — new outbox message needed: commits `c0448b81`→`75849f60`
      (6 commits including drain dispatch fix). After rebuild, add systemd overrides:
      `SLM_DRAIN_CONCURRENCY=4` and `SLM_QUEUE_DRAIN_INTERVAL_SEC=1` to local-doorman.service
      (saves ~63h on 1,128-item backlog). Command scope.
      [2026-06-19 totebox@project-intelligence]
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
