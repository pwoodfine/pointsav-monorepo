# NEXT.md — project-design (Totebox)

> Totebox Session — starts in `/srv/foundry/clones/project-design/`
> **Scope: this archive only.** Cross-repo and workspace-level items live at `~/Foundry/NEXT.md`.

Last updated: 2026-06-20

---

## Active (Phase 2 complete — Phase 4 next)

v0.3.0 plan at `/home/jennifer/.claude/plans/no-make-a-plan-abundant-forest.md`.

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
