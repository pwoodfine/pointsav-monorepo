# CLAUDE.md — os-network-admin

> **State:** Active
> **Last updated:** 2026-06-29
> **Version:** Phase S3 complete; S4 + daemon mode pending
> **Registry row:** `pointsav-monorepo/.agent/rules/project-registry.md`

---

## What this project is

The PPN mesh control plane. Manages WireGuard peer tables, node join approval
(CPace pairing ceremony), and mesh frame routing. Dual-mode:

1. **Bootable seL4 OS** for dedicated hardware (`.iso`/`.qcow2`)
2. **Daemon** for existing Linux (AppImage/.deb) — primary test-milestone path

**Price:** $1 USDC on software.pointsav.com.
**Distribution:** `.iso` (bare metal), `.qcow2` (cloud), AppImage/.deb (Linux daemon).

---

## Current state — Phase S3 complete (commit `13ef4654`)

`fleet_watch.rs` module: polls fleet every 30s, reads `~/.local/share/ppn/nodes.jsonl`
for approved pubkeys, on new node issues `wg set <WG_IFACE> peer <pubkey>
allowed-ips <wg_ip>/32`, writes WORM event to service-fs `/v1/append`.

**Environment vars:** `WG_IFACE`, `FLEET_URL`, `SERVICE_FS_URL`, `NODES_JSONL_PATH`
**Runtime requirement:** `CAP_NET_ADMIN` or root for `wg set`
**Tests:** 8/8 pass (pure `compute_pending_peers()` unit tests)

Phase roadmap:
- S1 (done): 16-byte mesh frames; pairing server polled
- S2 (done, `3bafaec5`): UDP :9206 listener; PING→PONG; `PPN_PEERS` env var
- S3 (done, `13ef4654`): fleet watch + WireGuard peer-table auto-program + WORM ledger
- **S4 (next):** Wire `system-network-interface::conduct_pairing_ceremony()` to UDP server; Genesis Protocol

---

## Dual-mode architecture

### Mode 1 — Bootable seL4 OS (Option B)

Same seL4 → CAmkES VMM → Linux guest architecture as `os-infrastructure`. The
os-network-admin binary runs inside the Linux guest as a systemd unit, serving as the
dedicated mesh control plane node.

Used for: dedicated hardware PPN nodes where the only job is running the mesh.

### Mode 2 — Linux daemon (primary near-term path)

```
cargo build --release --features daemon
```

Produces a standalone Linux binary. No seL4, no VMM, no full OS boot. WireGuard is
managed via system `wg` command or `wireguard-rs`. Runs on any Linux system.

Used for: joining the mesh from an existing Linux install without re-imaging.
Target test: iMac 2010-2012 Intel x86-64, Linux Mint.

**AppImage packaging:** daemon binary wrapped in AppImage for two-click Linux install.
`.deb` packaging for Debian/Ubuntu/Mint `apt install` path.

---

## Target hardware — iMac 2010-2012 (Intel x86-64)

Typical spec: Intel Core i3/i5 (Sandy Bridge or Westmere, 2010-2012), 4-8 GB RAM.
VT-x support: yes (Sandy Bridge+ i5/i7); Westmere Core 2 Duo: likely no VT-x.

For daemon mode: x86-64 Linux binary, no VT-x required. `cargo build --release` from
Linux Mint shell, or download AppImage from software.pointsav.com.

WireGuard installation: `sudo apt install wireguard` (Linux Mint 21.x has wireguard in
apt). Configure wg0, start daemon. Peers register in fleet automatically (Phase S3).

---

## Software.pointsav.com listing

Gate: three-node mesh test (D7). See `BRIEF-ppn-infrastructure-reference.md` §21.

1. Laptop A: os-infrastructure ISO → seL4 + Linux guest + WireGuard
2. foundry-workspace: os-infrastructure QCOW2 under QEMU
3. **iMac: os-network-admin daemon** → installs on Linux Mint, joins mesh

All three nodes in `service-vm-fleet` → upload + list at $1 USDC.

---

## Hard constraints

- **SYS-ADR-10 (F12 mandatory):** No automated pairing ceremony execution without
  explicit operator confirmation.
- **SYS-ADR-19:** No automated publishing to software.pointsav.com or WORM ledger.
- **Capability topology claims:** x86-64 daemon mode has no seL4 formal verification.
  Do not use "topology determines security" language for daemon-mode deployments.
  Reserve that claim for AArch64 seL4 OS mode (future).
- **CAP_NET_ADMIN:** The daemon requires elevated capability (or root) for `wg set`.
  Document this prominently in GUIDE. Do not silently drop capability.
- **Ed25519 signing:** Required for all distribution artifacts before upload.

---

## NEXT.md — open items

See `os-network-admin/NEXT.md`.
