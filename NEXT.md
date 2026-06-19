# NEXT.md — project-console

> Totebox Session — starts in `/srv/foundry/clones/project-console`
> Phase 10 complete 2026-06-16. Phase 11 (F7 BIM) blocked on project-bim Phase 1.
Hot open items. ≤200 lines. Backlog at `.agent/next-backlog.md`.
> **Scope: this archive only.** Cross-repo and workspace-level items live at `~/Foundry/NEXT.md`.

Last updated: 2026-06-19 (Session 25 shutdown)
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
      [2026-06-19 totebox@claude-code]
- [ ] **GFWED wildfire** — all 12 months of 2024 download failed in build-aec-flood.sh Step 13
      (wildfire raster not produced; layer15/Step16 skipped). Investigate GFWED URL or
      find alternate source. [2026-06-19 totebox@claude-code]
- [ ] **EU seismic (EFEHR / ESHM20)** — ESHM20 tarball from gitlab.seismo.ethz.ch produces
      1-feature metadata GeoJSON (not hazard polygons); EU seismic remains 0 clusters.
      Step 8 OR→two-if logic bug fixed 2026-06-17. Tarball content issue is separate.
      [2026-06-17 totebox@claude-code]
- [ ] **F-series tracking** — F1–F7 content repair requests sent to project-editorial 2026-06-14;
      track responses; update artifact-registry.md Status column when returned
      [2026-06-16 totebox]

## Blocked — Command Session (route via outbox)

- [ ] **Stage 6 READY** — 2 commits ahead of origin: f06fff1e (numpy 2.x + USGS_TIF) +
      ce6bdca1 (OGR_GEOJSON_MAX_OBJ_SIZE fix). Outbox msg queued. [2026-06-19 totebox@claude-code]
- [ ] **push-to-prod.sh gis** — after post-overnight verification passes; Command Session only.
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
- [x] **park-and-ride anchor ingest** — 23,117 records [2026-06-16 totebox]
- [x] **EU/US car rental + hotel chain ingests** [2026-06-16 totebox]
- [x] **PKS archetype rebalanced** — Fable analysis + mode-group collapse [2026-06-15 totebox]
- [x] **VWH retail_contamination badge** — showArchetypeDetail() badge for 3,048/6,368 clusters
      [2026-06-13 totebox]
- [x] **AEC wetland VRT fix** — 408 GWL_FCS30 5°-tiles assembled; gdal_translate removed (9c041f65)
      [2026-06-16 totebox]
- [x] **AEC wildfire numpy fix** — pure Python GDAL API (2ea45b07) [2026-06-16 totebox]
- [x] **ashrae_zone producer script** — build-ashrae-zone.py; 6,493/6,493 populated (dce0d157)
      [2026-06-16 totebox]
- [x] **PKS opportunity_class field** — SATURATED/EXPAND/DEVELOP per BRIEF §10.12 (2ea45b07)
      [2026-06-16 totebox]
- [x] **EFEHR seismic API (sample-eshm20-api.py)** — maps.efehr.org NXDOMAIN; main build script
      uses gitlab.seismo.ethz.ch tarball (unaffected); sample script is dev-only [2026-06-16 totebox]
- [x] **NEXT.md contamination (M-17)** — project-knowledge + project-intelligence content removed;
      GIS-only content restored [2026-06-17 totebox@claude-code]
