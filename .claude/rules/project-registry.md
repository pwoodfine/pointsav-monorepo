# Project Registry — pointsav-monorepo

Living inventory of every top-level project directory with its current
state. Read at session start. Update when activating, retiring, or
reclassifying a project. Registry drift (a directory not in the
table, or a table row without a matching directory) is visible and
must be closed.

State vocabulary — see `~/Foundry/CLAUDE.md` §8 for definitions.

Last updated: 2026-04-25.

---

## App — Console surface (`app-console-*`)

| Project | State | Type | Notes |
|---|---|---|---|
| app-console-bim | Reserved-folder | app-console | 1 file (RESEARCH.md); research phase |
| app-console-bookkeeper | Active | app-console | Activated 2026-04-22 via framework §8 (pilot); HTML-plugin pattern (view + cartridge); registry row was originally mis-classified; `README.*` and data-binding pending |
| app-console-content | Scaffold-coded | app-console | 8 files; in workspace members |
| app-console-email | Scaffold-coded | app-console | 4 files |
| app-console-help | Reserved-folder | app-console | READMEs only |
| app-console-input | Scaffold-coded | app-console | 6 files |
| app-console-keys | Reserved-folder | app-console | READMEs only |
| app-console-mesh | Reserved-folder | app-console | Placeholder |
| app-console-minutebook | Reserved-folder | app-console | READMEs only |
| app-console-people | Scaffold-coded | app-console | 5 files |
| app-console-vault | Reserved-folder | app-console | Placeholder |

## App — MediaKit surface (`app-mediakit-*`)

| Project | State | Type | Notes |
|---|---|---|---|
| app-mediakit-distributions | Scaffold-coded | app-mediakit | 4 files |
| app-mediakit-knowledge | Scaffold-coded | app-mediakit | 4 files |
| app-mediakit-marketing | Scaffold-coded | app-mediakit | 4 files |
| app-mediakit-telemetry | Scaffold-coded | app-mediakit | 14 files; MaxMind `.mmdb` pending move to build-time fetch |

## App — Network surface (`app-network-*`)

| Project | State | Type | Notes |
|---|---|---|---|
| app-network-cluster | Reserved-folder | app-network | Placeholder |
| app-network-gateway | Reserved-folder | app-network | Placeholder |
| app-network-help | Reserved-folder | app-network | Placeholder |
| app-network-infrastructure | Reserved-folder | app-network | Placeholder |
| app-network-keys | Reserved-folder | app-network | Placeholder |
| app-network-media | Reserved-folder | app-network | Placeholder |
| app-network-radar | Reserved-folder | app-network | Placeholder |
| app-network-vault | Reserved-folder | app-network | Placeholder |

## App — Orchestration surface (`app-orchestration-*`)

| Project | State | Type | Notes |
|---|---|---|---|
| app-orchestration-bim | Reserved-folder | app-orchestration | 1 file (RESEARCH.md); triggered taxonomy expansion to seventh in-force domain on 2026-04-22 |

## App — PrivateGit surface (`app-privategit-*`)

| Project | State | Type | Notes |
|---|---|---|---|
| app-privategit-design-system | Scaffold-coded | app-privategit | 4 files |
| app-privategit-source-control | Scaffold-coded | app-privategit | 4 files |

## App — Totebox surface (`app-totebox-*`)

| Project | State | Type | Notes |
|---|---|---|---|
| app-totebox-corporate | Scaffold-coded | app-totebox | 4 files |
| app-totebox-real-property | Scaffold-coded | app-totebox | 4 files; canonical is `cluster-totebox-property` per rename table |

## App — Workplace surface (`app-workplace-*`)

| Project | State | Type | Notes |
|---|---|---|---|
| app-workplace-bim | Reserved-folder | app-workplace | 1 file (RESEARCH.md); research phase |
| app-workplace-memo | Scaffold-coded | app-workplace | 47 files; running on Linux Mint per sibling's doc; CLAUDE.md + NEXT.md pending for Active |
| app-workplace-presentation | Active | app-workplace | 52 files; CLAUDE.md present; Phase 5 |
| app-workplace-proforma | Active | app-workplace | 45 files; CLAUDE.md present but marked "local-only"; conformance pending |

## OS (`os-*`)

| Project | State | Type | Notes |
|---|---|---|---|
| os-console | Scaffold-coded | os | 9 files |
| os-infrastructure | Scaffold-coded | os | 20 files; ISO artefact in directory — tracking status TBD |
| os-interface | Scaffold-coded | os | 4 files; legacy name — canonical is `os-orchestration` (rename in flight) |
| os-mediakit | Scaffold-coded | os | 4 files |
| os-network-admin | Scaffold-coded | os | 12 files; ISO artefact — tracking status TBD |
| os-privategit | Scaffold-coded | os | 4 files |
| os-totebox | Scaffold-coded | os | 6 files; IMG artefact — tracking status TBD |
| os-workplace | Scaffold-coded | os | 4 files |

## System (`system-*`)

| Project | State | Type | Notes |
|---|---|---|---|
| system-audit | Reserved-folder | system | 2 files |
| system-core | Scaffold-coded | system | 5 files |
| system-gateway-mba | Scaffold-coded | system | 8 files; in workspace members |
| system-interface | Scaffold-coded | system | 4 files |
| system-network-interface | Scaffold-coded | system | 6 files |
| system-resolution | Reserved-folder | system | 2 files |
| system-security | Scaffold-coded | system | 22 files; in workspace members |
| system-slm | Scaffold-coded | system | 4 files |
| system-substrate | Scaffold-coded | system | 4 files |
| system-substrate-broadcom | Scaffold-coded | system | 4 files; hardware bridge |
| system-substrate-freebsd | Scaffold-coded | system | 4 files; hardware bridge |
| system-substrate-wifi | Scaffold-coded | system | 4 files; hardware bridge |
| system-udp | Reserved-folder | system | 3 files |
| system-verification | Reserved-folder | system | 2 files |

## Service (`service-*`)

| Project | State | Type | Notes |
|---|---|---|---|
| service-bim | Reserved-folder | service | 1 file (RESEARCH.md); research phase |
| service-content | Scaffold-coded | service | 37 files; in workspace members |
| service-egress | Scaffold-coded | service | 4 files |
| service-email | Active | service | 18+ files; in workspace members; activated 2026-04-25 (project-data Task Claude); existing `src/auth.rs` + `src/graph_client.rs` use Microsoft Graph REST + inline OAuth `client_credentials` — flagged as drift in CLAUDE.md "Current state"; operator decision 2026-04-25 (real user-turn) directs rebase onto EWS-based MSFT auth pattern proven in sibling `service-email-egress-ews/` (env-var `AZURE_ACCESS_TOKEN` + SOAP `ews_payload.xml`); Tokio runtime model preserved; four pre-framework sub-directories (`ingress-harvester/`, `master-harvester-rs/`, `sovereign-splinter/`, `scripts/`) left in place pending inventory |
| service-email-egress-ews | Scaffold-coded | service | EWS protocol adapter; doubly-nested wrapper flattened 2026-04-23 (prior "consolidation" plan reversed — kept separate from `-imap` because they are two protocol-specific implementations, not duplicates); 6 sub-crates including EWS-only `egress-prune` and `egress-balancer`; Cargo.toml name mismatches (13 total across both) remain as separate audit finding |
| service-email-egress-imap | Scaffold-coded | service | IMAP protocol adapter; doubly-nested wrapper flattened 2026-04-23; 4 sub-crates; parallel structure to `-ews` but without prune/balancer |
| service-email-template | Scaffold-coded | service | 5 files |
| service-extraction | Active | service | 21 files; CLAUDE.md present but stale (see NEXT.md Item 9) |
| service-fs | Active | service | activated 2026-04-25 (project-data Task Claude, `ee209e3`); seL4-unikernel scaffold drift surfaced 2026-04-26 (cluster outbox `ring1-scaffold-runtime-model-drift`) and ratified by Master same date — Decision 1 (rewrite as Tokio MCP-server skeleton), Decision 2 (relocate scaffold to `vendor-sel4-fs/`), Decision 3 (hold workspace membership until rewrite compiles); relocation landed this commit; new Cargo.toml + src/ skeleton lands in the next commit; transitional state — directory currently has only CLAUDE.md + NEXT.md |
| service-http | Scaffold-coded | service | 9 files |
| service-input | Active | service | 4 files (README.md + README.es.md + CLAUDE.md + NEXT.md); created and activated 2026-04-25 (project-data Task Claude) in two consecutive commits per CLAUDE.md §9; Ring 1 generic document ingest — pluggable parser dispatcher (oxidize-pdf, docx-rust, calamine, pulldown-cmark per SLM-STACK §3.4); writes through `service-fs` (WORM), read by `service-extraction` (Ring 2) over MCP; Cargo crate scaffold pending |
| service-message-courier | Reserved-folder | service | 1 file |
| service-people | Active | service | 17+ files; in workspace members; activated 2026-04-25 (project-data Task Claude); existing scaffold is near-empty Rust stub (`system_status()` placeholder) plus pre-framework Python (`service-people.py`), seed JSON (`ledger_personnel.json`), and 5 uninventoried sub-directories (`sovereign-acs-engine/`, `spatial-crm/`, `spatial-ledger/`, `substrate/`, `tools/`); inventory of those sub-directories is the first NEXT.md item before schema work |
| service-pty-bridge | Scaffold-coded | service | Renamed 2026-04-23 from `pointsav-pty-bridge` (brand-prefix violation resolved); 1 source file (`src/main.rs`); not a workspace member |
| service-search | Reserved-folder | service | 1 file |
| service-slm | Scaffold-coded | service | Contains `router/` (Rust runtime, renamed 2026-04-23 from `cognitive-forge/`) and `router-trainer/` (Python distillation workflow, moved in 2026-04-23 from former top-level `tool-cognitive-forge/`); both names replace the retired "cognitive-forge" term per Do-Not-Use list |
| service-totebox-egress | Scaffold-coded | service | 18 files |
| service-vpn | Scaffold-coded | service | 11 files |

## Moonshot (`moonshot-*`)

| Project | State | Type | Notes |
|---|---|---|---|
| moonshot-database | Scaffold-coded | moonshot | 4 files |
| moonshot-gpu | Scaffold-coded | moonshot | 4 files |
| moonshot-hypervisor | Scaffold-coded | moonshot | 4 files |
| moonshot-index | Scaffold-coded | moonshot | 4 files |
| moonshot-kernel | Scaffold-coded | moonshot | 4 files |
| moonshot-network | Scaffold-coded | moonshot | 4 files |
| moonshot-protocol | Scaffold-coded | moonshot | 4 files |
| moonshot-sel4-vmm | Scaffold-coded | moonshot | 4 files |
| moonshot-toolkit | Scaffold-coded | moonshot | 5 files; Rust-only build orchestrator per repo CLAUDE.md |

## Tool (`tool-*`)

| Project | State | Type | Notes |
|---|---|---|---|
| tool-acs-miner | Scaffold-coded | tool | 3 files; in workspace members |
| tool-archive-rescue | Reserved-folder | tool | 3 files |
| tool-edgar-extractor | Reserved-folder | tool | 2 files |
| tool-egress-pull | Scaffold-coded | tool | 4 files |
| tool-template-rescue | Reserved-folder | tool | 3 files |

## Vendor (`vendor-*`)

| Project | State | Type | Notes |
|---|---|---|---|
| vendor-azure-auth | Reserved-folder | vendor | 1 file |
| vendor-gpu-drivers | Reserved-folder | vendor | 1 file |
| vendor-linux-systemd | Reserved-folder | vendor | 1 file |
| vendor-microsoft-graph | Reserved-folder | vendor | 1 file |
| vendor-phi3-mini | Reserved-folder | vendor | 2 files |
| vendor-sel4-fs | Reserved-folder | vendor | 6 files (Cargo.toml + Cargo.lock + .cargo/config.toml + src/main.rs + README.md + README.es.md); created 2026-04-26 (project-data Task Claude) per CLAUDE.md §9 as relocation target for the bare-metal seL4 scaffold previously at `service-fs/` (cluster outbox `ring1-scaffold-runtime-model-drift`, ratified by Master Decision 2 same date); joins the seL4 lineage alongside `vendor-sel4-kernel` and `moonshot-sel4-vmm`; activation deferred until seL4-track work resumes; not in workspace members |
| vendor-sel4-kernel | Scaffold-coded | vendor | 1074 files; vendored external seL4 kernel source |
| vendor-slm-engine | Reserved-folder | vendor | 3 files |
| vendor-virtio | Reserved-folder | vendor | 1 file |
| vendor-wireguard | Reserved-folder | vendor | 1 file |

## Other / special

| Project | State | Type | Notes |
|---|---|---|---|
| discovery-queue | Not-a-project | runtime data | 22 `TX-*_identity.json` files; gitignore + move to `service-fs/data/` |
| target | Not-a-project | build output | Rust cargo output; in .gitignore |
| xtask | Scaffold-coded | xtask | 2 files; in workspace members; Rust xtask convention |

---

## Summary (2026-04-25)

- **Active:** 8 (`app-console-bookkeeper`, `app-workplace-presentation`, `app-workplace-proforma`, `service-email`, `service-extraction`, `service-fs`, `service-input`, `service-people`)
- **Scaffold-coded:** 50
- **Reserved-folder:** 37
- **Defect:** 0
- **Not-a-project:** 2 (`discovery-queue`, `target`)
- **Dormant:** 0
- **Archived:** 0

**Total rows:** 99.
