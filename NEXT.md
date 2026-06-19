# NEXT.md — project-console
# NEXT.md — project-gis (Totebox)

> Totebox Session — starts in `/srv/foundry/clones/project-console`
> Phase 10 complete 2026-06-16. Phase 11 (F7 BIM) blocked on project-bim Phase 1.
Hot open items. ≤200 lines. Backlog at `.agent/next-backlog.md`.
> **Scope: this archive only.** Cross-repo and workspace-level items live at `~/Foundry/NEXT.md`.

Last updated: 2026-06-19

---

## Phase 11 — F7 BIM cartridge (blocked)

- [ ] `app-console-bim` activation — blocked on project-bim Phase 1 service (no ETA from project-bim)

---

## Carry-forward diagnostics
- [x] **local-slm.service `--parallel 2`** — operator approved 2026-06-19; applied to
      threads.conf drop-in; daemon-reload + restart; service active; two slots now available
      [2026-06-19 command@claude-code]
- [ ] **yoyo-batch ML libs** — trl/peft/transformers/accelerate/bitsandbytes not installed in
      training venv on GPU VM; LoRA training has never produced a real adapter; install needed
      before next training cycle; yoyo-batch TERMINATED (us-central1-a STOCKOUT); restart
      requires operator approval [2026-06-16 operator]
- [ ] **Post-overnight build verification** — run next session:
      PKS T1 count (expect > 692), park_ride coverage (expect > 0/7,045),
      EU seismic cluster count (still 0 — ESHM20 tarball issue pre-existing),
      layer11 freshness (120 MB, Jun 18 22:18 ✓), layer12 EU flood (151 KB present).
      Commands: `python3 -c "import json; d=json.load(open('/srv/foundry/deployments/gateway-orchestration-gis-1/www/data/archetype-pks.geojson')); ..."` per plan.
- [ ] **GFWED wildfire — Night 6 verification** — GFWED variable name bug fixed (`:FWI` → `:GPM.LATE.v5_FWI`).
      Next `build-aec-flood.sh` run should produce layer15-wildfire-global.pmtiles.
      Verify: `ls -lh /srv/foundry/deployments/gateway-orchestration-gis-1/www/tiles/layer15*.pmtiles`
      [2026-06-19 totebox@claude-code]
- [ ] **EU seismic fallback** — `maps.efehr.org` is NXDOMAIN (subdomain removed upstream).
      Parent `efehr.org` resolves (129.132.116.17). Investigate:
      (a) `git clone --depth 1 https://gitlab.seismo.ethz.ch/efehr/eshm20.git` to see if
          actual hazard shapefiles are in the repo (vs the tarball's metadata-only GeoJSON);
      (b) GSHAP GeoTIFF from gfz.de as fallback (coarser 1999 data; already documented in
          sample-eshm20-api.py fallback section).
      [2026-06-19 totebox@claude-code]
- [ ] **FEMA US SFHA (layer12-fema-sfha-us.pmtiles)** — Not refreshed in Night 5 (clusters.geojson
      missing). Check why FEMA REST step was skipped; old Jun 17 tile (2.8 MB) still deployed.
      [2026-06-19 totebox@claude-code]
- [ ] **F-series tracking** — F1–F7 content repair requests sent to project-editorial 2026-06-14;
      track responses; update artifact-registry.md Status column when returned
      [2026-06-16 totebox]

## Blocked — Command Session (route via outbox)

- [ ] **Performance — nginx gzip + cache-control on foundry-prod** — Two nginx changes must be
      applied on foundry-prod via SSH (cannot be done from Totebox scope). Exact diffs in outbox
      msg `project-gis-20260619-perf-nginx-prod`. Expected impact: maplibre-gl.js 784 KB → ~200 KB;
      clusters-meta.json 19 MB → ~2.1 MB; repeat visits near-instant for cached assets.
      [2026-06-19 totebox@claude-code]
- [ ] **Stage 6 READY** — commits ahead of origin (pending from both yesterday and today):
      - `f06fff1e` fix(gis): numpy 2.x compat
      - `ce6bdca1` fix(gis): OGR_GEOJSON_MAX_OBJ_SIZE 0 for large IT flood GeoJSON
      - `b203609d` docs(gis): NEXT.md updated — Night 5 flood build
      - `d7602bc7` fix(gis): GFWED NetCDF variable name + gitignore + briefs README fix
      - (+ this session's commit once landed)
      Outbox msg queued. [2026-06-19 totebox@claude-code]
- [ ] **push-to-prod.sh gis** — after Stage 6; will deploy preload hints + new HTML.
      [2026-06-17 totebox@claude-code]
- [ ] **check --strict gate** — F2/F3 dead links at project-editorial must resolve first
      [2026-06-17 command@claude-code]

These require operator access to iMac / vault-privategit-source-1 to diagnose:

- [ ] os-console exits immediately after MBA error — run binary on MBA, capture full stderr, check binary age vs last known-good build
- [ ] Port 9093 "Address already in use" on iMac — non-blocking; identify which process holds the bind (`lsof -i :9093`) before next os-console launch
- [ ] local-console.service on GCE VM — verify `systemctl status local-console.service` on vault-privategit-source-1; gate: operator must open GCE firewall port 2222 first

---

## Phase H1 — seL4 substrate + VirtIO clipboard (unblocked after H0 Alpine)

- [ ] Fill in `moonshot-sel4-vmm` (~300 lines): `_start()`, seL4 ABI wrappers, `microkit_msginfo_t`,
      `notified()` + `protected()` callbacks — blocks all seL4 unikernel work
- [ ] Boot os-console as single seL4 PD in QEMU: `moonshot-toolkit build examples/os-console-sel4.toml`
- [ ] VirtIO clipboard in `moonshot-hypervisor` (non-optional): arboard host-side + VirtIO clipboard
      protocol guest-side; SMB operators require paste from host apps into cartridges
- [ ] VirtIO serial PD (~200 lines): ratatui output via VirtIO console; keyboard input
- [ ] smoltcp network PD (~400 lines MIT, vendorable): HTTP to test Totebox; replaces reqwest

**Blocked on:** H0 Alpine/QEMU guest validation (see BRIEF-os-console-hypervisor.md §10 Phase H0).
Outbox to project-data sent 2026-06-19 to start parallel os-totebox + os-orchestration seL4 work.

---

## Drafts-outbound

- [ ] project-editorial pickup pending — TOPIC-geometric-protection, TOPIC-os-console-totebox-browser,
      TOPIC-sel4-unikernel-substrate, TOPIC-three-binary-architecture (EN+ES = 8 files) + 2 GUIDEs
      staged to drafts-outbound 2026-06-19
- [ ] project-editorial pickup pending — editorial/research drafts routed 2026-06-19 (outbox sent)
- [ ] project-design pickup pending — DESIGN-* drafts routed 2026-06-19 (outbox sent)
- [x] **Stage 6 complete — 13 commits total** — 8 commits (088b8e21→4886129d) + 5 commits
      (1fe42506→12076cf1) on canonical; includes Doorman Tier A fallback (f1879462),
      LoRA r=32/alpha=64 + sigmoid_norm DPO (60e88399), batch-extract endpoint, drain-hold fix,
      repair-ledger.py, DOC_sweep quarantine gate, entity_filter.rs hardening
      [2026-06-19 command@claude-code]
- [x] **Doorman Tier A fallback (f1879462)** — `/v1/extract` now falls back to Tier A when
      Tier B circuit open; canonical but binary rebuild pending (in-flight 2026-06-19)
      [2026-06-19 command@claude-code]
- [x] **service-content rebuilt** — binary from 631574ee (prompt v3 + entity_filter.rs);
      local-content.service active; entity_count=12,080 [2026-06-19 command@claude-code]
- [x] **OOV cleanup** — 531 pre-OLMo3 entities + 84 noise-name entities deleted;
      615 total removed; DataGraph healthier post-cleanup [2026-06-19 totebox@project-intelligence]
- [x] **Phase 7 Tier A test** — 12/14 tests passed (prompt v3); two remaining are semantic
      edge cases (GCP zone context + Doorman entity classification) [2026-06-19 totebox@project-intelligence]
- [x] **yoyo-batch /data/weights/adapters** — directory created; June 14 adapter rsync'd;
      1,043 pairs queued; training will succeed on next cycle when VM restarts
      [2026-06-19 totebox@project-intelligence]
- [x] **LoRA target_modules fix** — OLMo 2 names: att_proj/ff_proj/ff_out/attn_out; startup
      assertion added; real LoRA training now possible [2026-06-16 totebox@project-intelligence]
- [x] **Bug 1: SHA-on-202-ACK** — repair-ledger.py (52746a3c) ran; stale SHA entries cleared;
      ~400 files will re-enrich automatically when Tier B restores [2026-06-16 totebox@project-intelligence]
- [x] **Doorman batch-extract endpoint** — POST /v1/batch/extract; Semaphore(4) Tier A /
      Semaphore(1) Tier B; CONTENT_BATCH_SIZE env var; commit e5c0ee4f [2026-06-16 command@claude-code]
- [x] **redrive-quarantine.py** — 737 quarantined briefs → queue; queue_quarantine=0
      [2026-06-16 command@claude-code]
- [x] **NEXT.md contamination repaired** — project-gis content replaced with correct
      project-intelligence state [2026-06-19 command@claude-code]
- [x] **build-aec-flood.sh OGR_GEOJSON_MAX_OBJ_SIZE fix** — Step 12 EU merge failed with
      "GeoJSON object too complex"; fixed via `--config OGR_GEOJSON_MAX_OBJ_SIZE 0` on ogr2ogr;
      layer12 produced (151 KB) (ce6bdca1) [2026-06-19 totebox@claude-code]
- [x] **build-aec-flood.sh numpy 2.x / USGS_TIF fix** — replaced gdal_calc.py + gdal_polygonize.py
      with pure GDAL Python API; USGS_TIF unbound variable guarded (f06fff1e) [2026-06-18 totebox@claude-code]
- [x] **AEC flood build — Night 5** — layer11 (120 MB global AQUEDUCT), layer12 (151 KB EU
      regulatory), FEMA US done; wildfire GFWED failed (12 months); layer15/16 skipped
      [2026-06-19 totebox@claude-code]
- [x] **overnight-aec-builds.sh path fix** — ingest-osm-parking.py was called from wrong dir;
      now uses `../pointsav-monorepo/app-orchestration-gis/ingest-osm-parking.py` [2026-06-17 totebox@claude-code]
- [x] **build-aec-seismic.sh EU join fix** — Step 8 `or` condition split into two separate `if`
      guards; EU vector join was skipping all clusters [2026-06-17 totebox@claude-code]
- [x] **build-aec-flood.sh AQUEDUCT threshold fix** — lowered from 100MB to 85MB; S3 file is
      92MB, causing every clean download to fail validation [2026-06-17 totebox@claude-code]
- [x] **PKS Phase 5b** — 7,045 clusters (T1=692/T2=2,665/T3=3,188); MX=177; false-US removed
      [2026-06-16 totebox]
## Completed (Sessions 84+)

- [x] **Performance — preload hints + preconnect** — `<link rel="preconnect">` for openfreemap.org
      + `<link rel="preload">` for maplibre-gl.js/pmtiles.js/CSS added to both deployment
      www/index.html and archive source app-orchestration-gis/www/index.html. Ships with next
      push-to-prod.sh gis. [2026-06-19 totebox@claude-code]
- [x] **Post-overnight build verification** — 2026-06-19 session: PKS T1=692 ✓, T2=2,670, T3=3,709;
      park_ride=22,514 ✓; layer10 (2.1 MB) ✓, layer11 (120 MB) ✓, layer12-EU (151 KB) ✓;
      flood_hazard=855 hits in PRO clusters-meta ✓; wildfire FAILED (variable name bug now fixed);
      EU seismic 0 (ESHM20 blocked, not script). [2026-06-19 totebox@claude-code]
- [x] **GFWED variable name fix** — NetCDF variable is `GPM.LATE.v5_FWI` not `FWI`; fixed
      in build-aec-flood.sh lines 504–505 (2026-06-19 totebox@claude-code)
- [x] **Log file cleanup** — *.log added to .gitignore (root + app-orchestration-gis);
      all remaining logs now gitignored [2026-06-19 totebox@claude-code]
- [x] **Briefs README contamination** — README.md was showing project-knowledge content;
      restored to GIS briefs (pks-fable-analysis + gis-nightly-rebuild-aec) [2026-06-19 totebox@claude-code]
- [x] **NEXT.md contamination repair (M-17)** — project-intelligence session wrote intelligence
      items to project-gis NEXT.md; restored to correct GIS state [2026-06-19 totebox@claude-code]
- [x] **build-aec-flood.sh OGR_GEOJSON_MAX_OBJ_SIZE fix** (ce6bdca1) [2026-06-19 totebox@claude-code]
- [x] **build-aec-flood.sh numpy 2.x / USGS_TIF fix** (f06fff1e) [2026-06-18 totebox@claude-code]
- [x] **AEC flood build — Night 5** — layer11 ✓, layer12-EU ✓; wildfire GFWED failed [2026-06-19 totebox@claude-code]
- [x] **overnight-aec-builds.sh path fix** (2026-06-17 totebox@claude-code)
- [x] **build-aec-seismic.sh EU join fix** — Step 8 OR→two-if (2026-06-17 totebox@claude-code)
- [x] **build-aec-flood.sh AQUEDUCT threshold fix** — 100MB→85MB (2026-06-17 totebox@claude-code)
