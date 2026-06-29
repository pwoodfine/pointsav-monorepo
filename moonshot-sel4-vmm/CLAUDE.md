# CLAUDE.md — moonshot-sel4-vmm

> **State:** Scaffold-coded (spec only — no implementation)
> **Last updated:** 2026-06-29
> **Registry row:** `pointsav-monorepo/.agent/rules/project-registry.md`
> **Three-path role:** Option C — seL4 PDs own WireGuard + PPN control plane; Linux VM for workloads

---

## What this project is

**Option C of the three-path seL4 architecture.** Hybrid: seL4 protection domains own
WireGuard and the PPN network control plane. A Linux guest VM hosts non-critical workloads
(Doorman, service-vm-fleet, etc.). The security boundary is the WireGuard PD — the Linux
VM cannot reach WireGuard state without an explicit seL4 capability channel.

This is a moonshot (planned/intended) — not active development. Implementation begins
after Option B (`os-infrastructure`) ships and the three-node mesh test passes, then
≥6 months of stability. See `BRIEF-ppn-infrastructure-reference.md` §19 for full spec.

**Gate condition:** Option B (`os-infrastructure`) ships and three-node mesh test passes.

---

## Architecture

```
seL4 microkernel (AArch64 EL2 or x86-64 VT-x)
├── PD: wireguard-control   — WireGuard peer table management (os-network-admin logic)
├── PD: ppn-gate            — seL4 capability channel enforcement (what can call what)
└── VMM: CAmkES VMM         — Linux guest VM
    └── Linux (Debian 12) guest
        ├── service-vm-fleet   :9203
        ├── service-vm-host    :9220
        ├── Doorman            :8011
        └── media-* services   (non-critical workloads)
```

The Linux VM has NO wg0 interface. It cannot modify WireGuard peer tables. Peer
management requests flow from Linux VM → seL4 IPC channel → wireguard-control PD.

---

## What changes vs Option B (current path)

| Component | Option B (Linux controls) | Option C (PD controls) |
|---|---|---|
| WireGuard peer table | Linux `wg set` command, any root process | seL4 PD; only wireguard-control PD has capability |
| os-network-admin | Linux userspace process | Ported to wireguard-control seL4 PD |
| Linux VM | Has wg0 interface + all WireGuard access | No wg0; WireGuard is outside the VM |
| Peer addition request | direct `wg set` from fleet | seL4 IPC to wireguard-control PD; PD validates + applies |
| Security claim | seL4 isolates guests from each other | seL4 isolates WireGuard from Linux; Linux escape = no mesh access |

---

## Formal verification coverage

Same as Option B: AArch64 EL2 gets integrity proof (April 2025); x86-64 gets functional
correctness only. But Option C extends the security surface: the wireguard-control PD
is a formally verified component (its capability access pattern is part of the seL4 proof).
Linux compromise does not grant WireGuard access — no seL4 channel exists.

---

## What must be ported

1. **WireGuard management** (`os-network-admin` IPC subset): the part that calls `wg set`
   and manages peer tables. Must become a seL4 PD without tokio, without std net.
   Uses sel4cp async runtime or event-loop pattern.

2. **seL4 IPC channel** from Linux VM to wireguard-control PD: Linux guest calls
   `virtio-ipc` endpoint (or 9P/shared memory with seL4 memory window grant) to request
   peer additions. PD validates the request and applies via internal WireGuard state.

---

## Hard constraints

- wireguard-control PD: `#![no_std]`, `#![no_main]`. No Linux runtime in the PD.
- Linux VM must NOT have `/dev/net/tun` access to create WireGuard interfaces. Enforced
  by VMM config (remove virtio-net TAP capability from Linux guest).
- Capability topology: wireguard-control PD has exactly one inbound channel (from Linux VM
  via VMM relay) and one outbound (to seL4 notification for mesh state changes). No other
  channels. This must be enforced in the CAmkES VMM configuration.

---

## Dependencies

- `moonshot-kernel/` — seL4 kernel configuration reference
- `moonshot-toolkit/` — build orchestrator (must ship first)
- `os-infrastructure/` — Option B; must ship first (gate condition)
- `vendor-sel4-kernel/` — 1,074 vendored seL4 source files
