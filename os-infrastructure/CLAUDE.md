# CLAUDE.md — os-infrastructure

> **State:** Active
> **Last updated:** 2026-06-29
> **Version:** 0.0.1 (pre-release; not yet on software.pointsav.com)
> **Registry row:** `pointsav-monorepo/.agent/rules/project-registry.md`
> **Three-path role:** Option B — seL4 EL2/VT-x + CAmkES VMM + Linux guest (current path)

---

## What this project is

The sovereign OS for PPN infrastructure nodes. Boots on bare metal, cloud VMs, and
leased servers. Provides the seL4 hypervisor layer, WireGuard mesh join, and the
environment that hosts all PPN services inside a Linux (Debian 12) guest VM.

**Price:** $19 USDC on software.pointsav.com.
**Distribution:** `.iso` (bare metal, GRUB2 multiboot), `.qcow2` (cloud VM import).
**Source:** Available free on GitHub. Payment is for the pre-built Ed25519-signed binary.

---

## Architecture (Option B — current)

```
GRUB2 bootloader
└── seL4 microkernel (EL2 on AArch64 / VT-x on x86-64)
    └── CAmkES VMM protection domain
        └── Linux (Debian 12) guest VM
            ├── WireGuard (wg0 interface, mesh join)
            ├── service-vm-fleet   :9203
            ├── service-vm-host    :9220
            └── os-network-admin (optional co-boot)
```

**Capability topology:** seL4 capability graph determines all component isolation.
Formal integrity proof on AArch64 EL2 (April 2025, UK NCSC). x86-64 = functional
correctness only (runtime/dev target). See `moonshot-kernel/CLAUDE.md`.

---

## Distribution artifacts

| Artifact | Format | Size (est.) | Build path |
|---|---|---|---|
| `os-infrastructure-<ver>-x86_64.iso` | ISO 9660 + GRUB2 | ~800 MB | moonshot-toolkit → Microkit 2.2.0 → seL4 pc99 + Debian 12 QCOW2 |
| `os-infrastructure-<ver>-aarch64.iso` | ISO 9660 + GRUB2 | ~800 MB | moonshot-toolkit → Microkit 2.2.0 → seL4 AArch64 + Debian 12 QCOW2 |
| `os-infrastructure-<ver>-x86_64.qcow2` | QCOW2 | ~2 GB | Same as ISO, different bootloader target |

All artifacts: Ed25519-signed with `identity/id_pointsav-administrator`.

---

## Build path

```
moonshot-toolkit build os-infrastructure/system-spec.toml
  → Microkit 2.2.0 SDK (x86_64_generic_vtx or aarch64 target)
  → seL4 15.0.0 (vendor-sel4-kernel/ vendored source)
  → CAmkES VMM component
  → Linux guest QCOW2 (Debian 12 genericcloud — from infrastructure/virt/work/)
  → AssembleImage: seL4 boot image + CAmkES VMM + Linux QCOW2 payload
  → GRUB2 ISO wrap (x86) or AArch64 bootable image
  → Ed25519 sign with identity/id_pointsav-administrator
```

**Legacy:** `forge_iso.sh` and `build_iso/` are the original shell-based ISO build tools.
Kept as migration reference until `moonshot-toolkit build` produces an equivalent bootable
image end-to-end (task #14). Do not delete until then.

---

## Test milestones (gate to software.pointsav.com listing)

### Three-node mesh test (D7) — GATE

1. **Laptop A (bare metal):** write `.iso` to USB, boot under VT-x. seL4 starts,
   CAmkES VMM brings up Linux guest, WireGuard joins mesh. Peer registers in
   `service-vm-fleet`. Confirm WG IP visible in fleet.

2. **foundry-workspace (QEMU/TCG):** import `.qcow2` via `qemu-system-x86_64`.
   No KVM on GCP VM (TCG fallback). seL4 boots, Linux guest up, WireGuard joins.
   Peer registers in fleet.

3. **iMac Linux Mint (daemon mode via os-network-admin):** not a bare-metal os-infrastructure
   boot. Uses os-network-admin daemon to join mesh. Confirms heterogeneous fleet.

All three nodes visible in fleet → listing on software.pointsav.com unlocked.

---

## NEXT.md — open items

See `os-infrastructure/NEXT.md`.

---

## Existing scaffold

`forge_iso.sh`, `build_iso/`, `src/` — original ISO build scaffold. Keep until
`moonshot-toolkit build` replaces end-to-end. Do not delete or git-rm.

---

## Hard constraints

- **Option B only** until moonshot-sel4-vmm gate conditions are met.
- **GRUB2 Multiboot/Multiboot2** for x86-64 boot. No UEFI officially (Neutrality Atoll
  pushing this; do not implement UEFI until explicitly approved).
- **SYS-ADR-10 (F12 mandatory):** No file enters the ISO without an explicit operator
  commit action. The build pipeline does not self-publish.
- **SYS-ADR-19:** No automated publishing to software.pointsav.com. Manual upload step.
- **Capability topology claims:** Only use "topology determines security" language for
  AArch64 EL2 builds. x86-64 builds have no formal verification claim.
- **Ed25519 signing:** Required for all distribution artifacts before upload.
  Key: `identity/id_pointsav-administrator` (never automate signing).
