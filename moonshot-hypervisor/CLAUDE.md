# CLAUDE.md — moonshot-hypervisor

> **State:** Scaffold-coded (spec only — no implementation)
> **Last updated:** 2026-06-29
> **Registry row:** `pointsav-monorepo/.agent/rules/project-registry.md`
> **Three-path role:** Option A — Pure seL4 PDs, no VMs

---

## What this project is

**Option A of the three-path seL4 architecture.** Pure seL4 protection domains for
the entire PPN node stack. No VMs. No VMM. No Linux guest. Smallest possible TCB.

This is a moonshot (planned/intended) — not active development. Implementation begins
after Option C (moonshot-sel4-vmm) ships and the seL4 PD model is proven stable
for ≥6 months. See `BRIEF-ppn-infrastructure-reference.md` §19 for full spec.

**Gate condition:** `moonshot-sel4-vmm` ships and three-path PD model proven stable.

---

## Architecture

Every component is a seL4 protection domain (PD). PDs communicate via seL4 IPC
channels. No VM, no QEMU, no Linux kernel in the TCB.

```
seL4 microkernel
├── PD: wireguard      — WireGuard mesh (IPC-based, no Linux socket layer)
├── PD: fleet-tracker  — service-vm-fleet resource tracking
├── PD: spawn-manager  — service-vm-host QEMU spawner (or removed; unikernel model)
├── PD: doorman        — inference request routing
└── PD: ppn-control    — os-network-admin control plane
```

Capability topology determines all inter-PD communication. No channel = no
information flow. This is the formally proved invariant — see seL4 documentation.

---

## What must be ported to seL4 PDs

| Component | Current (Option B Linux) | Target (Option A PD) | Blocker |
|---|---|---|---|
| WireGuard | Linux kernel module + wg CLI | seL4 PD; crypto via seL4 IPC; no net_device API | Requires WireGuard userspace reimplementation |
| service-vm-fleet | Rust/Axum HTTP service | Rust seL4 PD (no tokio, no std net) | Needs sel4cp async runtime |
| service-vm-host (QEMU spawner) | QEMU process + KVM fd | PD or removed; unikernel model | Major scope |
| Doorman | Tokio HTTP/WebSocket | seL4 IPC-based request routing | Needs async PD runtime |
| os-network-admin | Tokio UDP + WireGuard CLI | seL4 PD owning WireGuard PD channel | Needs WireGuard PD first |

---

## Formal verification target

- **Primary:** AArch64 EL2 with confidentiality proof (in progress as of 2026-06-29).
  Integrity proof available April 2025 (UK NCSC). Once confidentiality proof is
  published, Option A on AArch64 EL2 achieves all CIA properties at the kernel layer.
- **Alternate:** RISC-V64 (HiFive Unleashed). Deepest proofs today (binary-level +
  integrity + confidentiality). Constrained hardware — no consumer device deployment path.
- **x86-64:** Functional correctness only. Option A does not improve x86-64 security
  posture vs Option B. Use AArch64 for any formal security claim.

---

## Hard constraints

- No VM. No VMM. No Linux guest in Option A.
- Do not introduce Linux runtime dependencies into PD code.
- seL4 PD code: `#![no_std]`, `#![no_main]`. Use `sel4cp` / `sel4runtime` crates.
- seL4 IPC only for inter-PD communication. No shared memory pools without explicit
  seL4 memory window grants.
- Capability topology: only explicit seL4 capability grants create information channels.
  This invariant must be preserved in all architectural decisions.

---

## Dependencies

- `moonshot-kernel/` — seL4 kernel configuration reference (seL4 15.0.0, verified configs)
- `moonshot-toolkit/` — build orchestrator (task #14 must ship first)
- `moonshot-sel4-vmm/` — Option C is the intermediate step; Option A gates on it
- `vendor-sel4-kernel/` — 1,074 vendored seL4 source files (seL4 15.0.0)
