# NEXT.md — project-system (Totebox)

> Totebox Session — starts in `/srv/foundry/clones/project-system`
> **Scope: this archive only.** Cross-repo and workspace-level items live at `~/Foundry/NEXT.md`.

Last updated: 2026-06-20

---

## Active

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

---

## Completed milestones

- **v0.2.0** — multi-module rewrite (Phase A routes, Phase B SSE, Phase C edit overlay, Phase D AI bridge); binary deployed 2026-06-20 sha256 1883110e; canonical commit 8c540cd4
- **DESIGN-BUNDLE ratified** — namespace component.document.legal.* confirmed 2026-06-20
# NEXT.md — project-editorial (Totebox)

> **Scope: this archive only.** Cross-repo and workspace-level items live at `~/Foundry/NEXT.md`.
> Hot open items. ≤200 lines. Backlog at `.agent/next-backlog.md`.

Last updated: 2026-06-20 (gate clean — 0 dead links, 0 MISSING sections)

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

Last updated: 2026-06-20

---

## Active (Totebox scope)

- [ ] **GFWED wildfire — Night 6 verification** — GFWED variable name bug fixed (`:FWI` → `:GPM.LATE.v5_FWI`).
      Next `build-aec-flood.sh` run should produce layer15-wildfire-global.pmtiles.
      Verify: `ls -lh /srv/foundry/deployments/gateway-orchestration-gis-1/www/tiles/layer15*.pmtiles`
      [2026-06-19 totebox@claude-code]
- [ ] **EU seismic fallback** — `maps.efehr.org` is NXDOMAIN (subdomain removed upstream).
      Parent `efehr.org` resolves (129.132.116.17). Investigate:
      (a) `git clone --depth 1 https://gitlab.seismo.ethz.ch/efehr/eshm20.git` to see if
          actual hazard shapefiles are in the repo (vs tarball's metadata-only GeoJSON);
      (b) GSHAP GeoTIFF from gfz.de as fallback (coarser 1999 data; documented in
          sample-eshm20-api.py fallback section).
      [2026-06-19 totebox@claude-code]
- [ ] **FEMA US SFHA (layer12-fema-sfha-us.pmtiles)** — Not refreshed in Night 5 (clusters.geojson
      missing). Check why FEMA REST step was skipped; old Jun 17 tile (2.8 MB) still deployed.
      [2026-06-19 totebox@claude-code]
- [ ] **F-series tracking** — F1–F7 content repair requests sent to project-editorial 2026-06-14;
      track responses; update artifact-registry.md Status column when returned.
      [2026-06-16 totebox]

## Blocked — Command Session (route via outbox)

- [ ] **Performance — nginx gzip + cache-control on foundry-prod** — Two nginx changes must be
      applied on foundry-prod via SSH. Exact diffs in outbox msg
      `project-gis-20260619-perf-nginx-prod`. Expected impact: maplibre-gl.js 784 KB → ~200 KB;
      clusters-meta.json 19 MB → ~2.1 MB; repeat visits near-instant for cached assets.
      [2026-06-19 totebox@claude-code]
- [ ] **Stage 6 READY** — 5 commits ahead of origin:
      - `b1f2514d` fix(gis): numpy 2.x compat
      - `b881c640` fix(gis): OGR_GEOJSON_MAX_OBJ_SIZE 0 for large IT flood GeoJSON
      - `bb5e4e8d` docs(gis): NEXT.md updated — Night 5 flood build
      - `d7602bc7` fix(gis): GFWED NetCDF variable name + gitignore + briefs README fix
      - `de977b4b` perf(gis): preload hints + preconnect for map libs
      - (+ shutdown repair commit once landed)
      Outbox msg queued. [2026-06-19 totebox@claude-code]
- [ ] **push-to-prod.sh gis** — after Stage 6; deploys preload hints to live site.
      [2026-06-17 totebox@claude-code]
- [ ] **check --strict gate** — F2/F3 dead links at project-editorial must resolve first.
      [2026-06-17 command@claude-code]

## Completed (Sessions 84+)

- [x] **Performance — preload hints + preconnect** — `<link rel="preconnect">` for
      openfreemap.org + `<link rel="preload">` for maplibre-gl.js/pmtiles.js/CSS added to both
      deployment www/index.html and archive source; ships with next push-to-prod.sh gis.
      [2026-06-19 totebox@claude-code]
- [x] **Post-overnight build verification** — PKS T1=692 ✓, park_ride=22,514 ✓,
      layer10 ✓, layer11 (120 MB) ✓, layer12-EU (151 KB) ✓, flood_hazard=855 ✓.
      [2026-06-19 totebox@claude-code]
- [x] **GFWED variable name fix** — NetCDF variable is `GPM.LATE.v5_FWI` not `FWI`;
      fixed in build-aec-flood.sh. [2026-06-19 totebox@claude-code]
- [x] **Log file cleanup** — `*.log` added to .gitignore (root + app-orchestration-gis).
      [2026-06-19 totebox@claude-code]
- [x] **Briefs README contamination** — restored to correct GIS briefs.
      [2026-06-19 totebox@claude-code]
- [x] **build-aec-flood.sh OGR_GEOJSON_MAX_OBJ_SIZE fix** (b881c640).
- [x] **build-aec-flood.sh numpy 2.x / USGS_TIF fix** (b1f2514d).
- [x] **AEC flood build Night 5** — layer11 ✓, layer12-EU ✓.
- [x] **overnight-aec-builds.sh path fix** / **build-aec-seismic.sh EU join fix**.
- [x] **Trademark Phase 3 — TOPIC/GUIDE content wikis** — MCorp™ + Capability Geometry™ applied across all three sub-clones (documentation, projects, corporate); 3 commits (3e3579b, ac6379f, f1451e9); copyright lines corrected to Woodfine Capital Projects Inc. [2026-06-19 totebox@claude-code]
- [x] **M9 EN/ES parity sweep — media-knowledge-projects** — all 53 ES articles at 84%+; 5 commit passes (7fa466b) [2026-06-19 totebox@claude-code]
- [x] **M7 snapshot dating** — corrected 7,594 → 6,493 in index.md; commit 4649f95 [2026-06-19 totebox@claude-code]
