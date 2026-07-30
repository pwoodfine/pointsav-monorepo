# NEXT.md — os-infrastructure

> Last updated: 2026-06-29
> State: Active (pre-release; three-node mesh test required before listing)

---

## Right now

- Activated 2026-06-29 per project framework §9. CLAUDE.md written.
- Build pipeline (moonshot-toolkit task #14) is the blocking prerequisite for all milestones below.
- Legacy `forge_iso.sh` / `build_iso/` scaffold in place as migration reference.

## Queue

- `[ ]` Write `system-spec.toml` for x86-64 boot target:
  - Protection domains: seL4 root task + CAmkES VMM PD
  - Linux guest: Debian 12 genericcloud QCOW2 (at `infrastructure/virt/work/debian-12*.qcow2`)
  - CAmkES VMM: wire Linux guest VCPU + memory regions
- `[ ]` Implement `build` subcommand in moonshot-toolkit (task #14) with Microkit 2.2.0 SDK
  targeting `x86_64_generic_vtx` — this is the primary blocker
- `[ ]` Wire WireGuard inside Linux guest: install `wireguard-tools`, bring up `wg0` at boot,
  load config from `/etc/wireguard/wg0.conf`
- `[ ]` Wire `service-vm-fleet` + `service-vm-host` inside Linux guest as systemd units
- `[ ]` Build bootable `.iso` from `moonshot-toolkit BuildPlan` for x86-64

## Test milestones

- `[ ]` **Laptop A — bare metal** (VT-x, Sandy Bridge i5-2400S):
  write ISO to USB, boot, confirm seL4 starts, Linux guest up, WireGuard peer registers
  in `service-vm-fleet`. Confirm from foundry-workspace: `curl http://<wg-ip>:9203/nodes`
- `[ ]` **foundry-workspace — QEMU/TCG**:
  import `.qcow2` via `qemu-system-x86_64 -nographic`. No KVM (GCP TCG). seL4 boots,
  Linux guest up, WireGuard joins. Peer registers in fleet.

## Gate

Three-node mesh test (above + iMac os-network-admin daemon) unlocks software.pointsav.com listing.

## Deferred

- AArch64 build target — after x86-64 milestone passes and three-node mesh test completes.
  AArch64 is the verified production target (integrity proof April 2025) but comes second.
- UEFI boot — pending Neutrality Atoll decision. Use GRUB2 Multiboot for now.
- `forge_iso.sh` removal — deferred until `moonshot-toolkit build` replaces it end-to-end.
- software.pointsav.com upload — blocked on three-node mesh test + Ed25519 signing ceremony.

## Recently done

- 2026-06-29: project activation — CLAUDE.md + NEXT.md written; state: Scaffold-coded → Active.
