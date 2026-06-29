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
