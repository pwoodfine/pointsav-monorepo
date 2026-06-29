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
