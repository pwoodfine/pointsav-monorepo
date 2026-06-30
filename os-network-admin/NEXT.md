# NEXT.md — os-network-admin

> Last updated: 2026-06-29
> State: Active — Phase S3 done; daemon mode + Phase S4 next

---

## Right now

- Phase S3 complete (commit `13ef4654`): fleet watch + auto WireGuard peer-table + WORM ledger; 8/8 tests.
- Activated 2026-06-29 per project framework §9. CLAUDE.md written.
- Next priority: daemon build mode for iMac Linux Mint test.

## Queue

- `[ ]` Add daemon build mode feature flag:
  - `Cargo.toml`: `[features] daemon = []`
  - `src/main.rs`: `#[cfg(feature = "daemon")]` blocks for WireGuard CLI path (no seL4 deps)
  - Confirm `cargo build --release --features daemon` produces a standalone Linux binary
- `[ ]` Package daemon as AppImage (Linux):
  - Download `appimagetool` from AppImageKit
  - Wrap binary + WireGuard config helper script in AppDir
  - Produces `os-network-admin-<ver>-x86_64.AppImage`
- `[ ]` Test daemon on iMac Linux Mint (Intel x86-64, 2010-2012):
  - Install: `sudo apt install wireguard` + configure `wg0`
  - Run daemon: `./os-network-admin-<ver>-x86_64.AppImage`
  - Confirm peer joins fleet (service-vm-fleet at foundry-workspace)
  - Three-node mesh verified: Laptop A + foundry-workspace + iMac
- `[ ]` Sign daemon AppImage with `identity/id_pointsav-administrator` Ed25519 key
- `[ ]` Upload to software.pointsav.com at $1 USDC (after three-node mesh test passes)
- `[x]` Add daemon build mode feature flag (done 2026-06-29):
  - `Cargo.toml`: `[features] daemon = []` + `[workspace]` self-contained
  - `src/fleet_watch.rs`: Phase S3 fleet watch loop — 30s poll, `wg set` via subprocess, WORM event append; 3/3 tests pass
  - `src/main.rs`: `#[cfg(feature = "daemon")]` guards; Phase S1 UDP path preserved under `#[cfg(not(feature = "daemon"))]`
  - `cargo build --release --features daemon` → ELF x86-64, 526 KB
  - `scripts/package-appimage.sh`: AppImage scaffold (requires appimagetool on PATH)
  - TODO: wire HTTP fleet polling (FLEET_URL) + HTTP WORM append (SERVICE_FS_URL) when fleet endpoint is live
- `[x]` Package daemon as AppImage (Linux) — done 2026-06-29:
  - appimagetool installed from AppImageKit release 13 (`obsolete-appimagetool-x86_64.AppImage`, APPIMAGE_EXTRACT_AND_RUN=1)
  - `APPIMAGE_EXTRACT_AND_RUN=1 CARGO_TARGET_DIR=/srv/foundry/cargo-target/mathew ./scripts/package-appimage.sh 0.1.0-beta.1`
  - Output: `os-network-admin-0.1.0-beta.1-x86_64.AppImage` (414 KB, gitignored)
  - Binary deps: libc + libgcc_s only — works on any Linux Mint 21.x without bundling
- `[ ]` Test daemon on iMac Linux Mint (Intel x86-64, 2010-2012) — **UNBLOCKED 2026-06-30**:
  - **Live download URL (confirmed 200):** `https://software.pointsav.com/releases/os-network-admin/latest/x86_64`
  - iMac already on WireGuard mesh at `10.8.0.7` — no WireGuard setup needed
  - Dogfood install on iMac:
    ```bash
    curl -fsSL https://software.pointsav.com/releases/os-network-admin/latest/x86_64 -o os-network-admin
    chmod +x os-network-admin
    mkdir -p ~/.local/share/ppn
    echo '{"public_key":"32i7rpMPWmABjs83ojsiW7spn/v+CPlWv2vBCIRJdDc=","wg_ip":"10.8.0.9"}' > ~/.local/share/ppn/nodes.jsonl
    sudo APPIMAGE_EXTRACT_AND_RUN=1 WG_IFACE=wg0 NODES_JSONL_PATH=/home/$USER/.local/share/ppn/nodes.jsonl ./os-network-admin
    ```
  - Note: URL convention from project-software differs from spec — `/releases/.../latest/` not `/download/.../beta/`
  - Confirm peer joins fleet (service-vm-fleet at foundry-workspace): `sudo wg show wg0`
  - Three-node mesh verified (D7): Laptop A + foundry-workspace + iMac
- `[x]` Sign daemon binary with `identity/id_pointsav-administrator` Ed25519 key
  - Signed 2026-06-29: SHA256 `a3becd581fb841fc9bc8893b6a6a5fbecc87dd1ca37a787f3430d511263a14ec`
  - Sig at `/srv/foundry/cargo-target/mathew/release/os-network-admin.sig` (294 B)
- `[x]` Send daemon binary to project-software for BETA listing on software.pointsav.com:
  - Gate: binary builds clean (DONE — 526 KB ELF x86-64)
  - Send outbox to project-software: binary path + sig path + version `0.1.0-beta.1`
  - Instruct project-software: BETA label, payment disconnected, CLI curl download URL
  - Include system requirements: CAP_NET_ADMIN, WireGuard kernel module, x86-64 Linux
  - project-software also needs to build full product catalog page structure (all projects)
  - Do NOT wait for D7 mesh test — BETA upload is for proof-of-existence
  - D7 mesh test gates the PAID listing ($1 USDC) only — separate operator approval

## Phase S4 — Genesis Protocol

- `[ ]` Wire `system-network-interface::conduct_pairing_ceremony()` to UDP server (:9206)
- `[ ]` CPace-based pairing ceremony: new node sends join request; os-network-admin
  operator approves via TUI (ratatui); pairing writes to `~/.local/share/ppn/nodes.jsonl`
- `[ ]` Test Genesis Protocol end-to-end on Laptop A bare-metal boot
- **Gate:** os-infrastructure must boot bare-metal first (Phase S4 requires a live genesis node)

## Test milestones

- `[ ]` **iMac Linux Mint (daemon)** — primary near-term test:
  - VT-x may not be available on oldest iMac hardware (Core 2 Duo Westmere)
  - Daemon mode requires no VT-x; pure x86-64 binary
  - WireGuard install: `sudo apt install wireguard` on Mint 21.x
  - Confirm: `wg show wg0` shows foundry-workspace as a peer
  - Confirm: `service-vm-fleet` at foundry-workspace lists iMac as a node

## Deferred

- AArch64 OS mode — after x86-64 daemon test passes.
- Windows daemon (`.exe`) — post three-node mesh test. Needs wintun driver for WireGuard.
- macOS daemon (`.pkg`) — post Windows. Needs Network Extension entitlement.
- Phase S5+ (per-tenant subnets, VXLAN-over-WG) — gated on Phase S4 + os-network-admin stability.

## Recently done

- 2026-06-29: project activation — CLAUDE.md + NEXT.md written; state: Scaffold-coded → Active.
- 2026-06-14, `13ef4654`: Phase S3 — fleet watch loop; auto WireGuard peer-table + WORM ledger; 8/8 tests.
- 2026-06-14, `3bafaec5`: Phase S2 — UDP :9206 listener; PING→PONG; PPN_PEERS env var.
