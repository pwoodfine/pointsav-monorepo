# CLAUDE.md — moonshot-kernel

> **State:** Scaffold-coded (configuration reference)
> **Last updated:** 2026-06-29
> **Registry row:** `pointsav-monorepo/.agent/rules/project-registry.md`
> **Role:** seL4 kernel configuration reference — source of truth for all os-* builds

---

## What this project is

The seL4 kernel configuration reference for all PPN os-* builds. Not a Rust crate —
this directory documents the authoritative kernel configuration choices, verified
configurations, and constraint decisions so that all `os-*` builds reference a single
source of truth.

**Pinned version:** seL4 15.0.0 (released 2025-03-31, vendored in `vendor-sel4-kernel/`).
**Framework:** Microkit 2.2.0 (wraps seL4 15.0.0; recommended for new seL4 projects).
**Source:** `vendor-sel4-kernel/` — 1,074 files, vendored.

---

## Verified configurations as of 2026-06-29

### AArch64 EL2 (verified hypervisor mode)

- **Configuration file:** `settings/verified/AArch64_verified.cmake` (in vendor-sel4-kernel)
- **Proof status:**
  - Functional correctness: ✓ (established, C-level)
  - Integrity: ✓ (April 2025, UK NCSC funded)
  - Confidentiality: in progress
- **This is the only verified hypervisor-mode configuration as of mid-2026.**
- `KernelMaxNumNodes = "1"` — single-core verified only (SMP unverified on all arches).
- Used in: `moonshot-sel4-vmm` (Option C), `moonshot-hypervisor` (Option A).
- Production target: GCP C4A Arm instance (~$50–100/month); AArch64 bare-metal nodes.

### RISC-V64 (deepest formal proofs)

- **Configuration:** RISC-V64 verified config (binary-level proof + integrity + confidentiality)
- **Proof status:**
  - Functional correctness: ✓ (C-level + binary-level)
  - Integrity: ✓
  - Confidentiality: ✓
- Most complete proofs of any seL4 configuration.
- Constrained hardware: verified config targets HiFive Unleashed. Not a consumer deployment target.
- Not a priority path for PPN products; documented as reference.

### x86-64 (`X64_verified.cmake`)

- **Configuration file:** `settings/verified/X64_verified.cmake`
- **Proof status:**
  - Functional correctness: ✓ (C-level ONLY)
  - Binary-level proof: ✗
  - Integrity: ✗
  - Confidentiality: ✗
- **No formal security claims permissible for x86-64 seL4 deployments.**
- Used in: runtime/development targets only. Laptop A, foundry-workspace (QEMU/TCG), iMac.
- Do NOT use the "topology determines security" claim for x86-64 products.

---

## Microkit 2.2.0 targets

Microkit 2.2.0 (pinned version) provides these hardware targets:

| Target | File suffix | VT-x/AMD-V required? | Verified? |
|---|---|---|---|
| `aarch64` | `aarch64-` | No (bare metal EL2) | AArch64 EL2 integrity proof |
| `x86_64_generic` | `x86_64_generic-` | No (TCG/software) | Functional correctness only |
| `x86_64_generic_vtx` | `x86_64_generic_vtx-` | Yes (VT-x/AMD-V hardware) | Functional correctness only |
| `riscv64` | `riscv64-` | No | RISC-V64 deepest proofs |

`x86_64_generic` and `x86_64_generic_vtx` were added in Microkit 2.1.0 (November 26, 2025).

---

## seL4 capability topology — the security invariant

The seL4 capability model defines a directed graph (the capability graph, CSpace).
Every object access goes through a capability pointer. The formally proved invariant:
**only connectivity begets connectivity** — if A has no capability pointer to B (direct
or via intermediary), A cannot observe, modify, or call B.

This is the basis for PPN's security claim. In seL4 terms: "topology determines security"
— the capability graph topology is an upper bound on information flow.

Use "topology" not "geometry" in all documentation and product copy. Academic precedents:
Miller (2000) "Robust Composition: Towards a Unified Approach to Access Control and
Concurrency Control"; Drossopoulou (2016) "Capability Patterns"; Fuchsia (Google) uses
"component topology" for the same concept in production.

---

## Capability graph properties (for product claims)

These properties hold on AArch64 EL2 (integrity proof, April 2025):

1. **No ambient authority.** No capability = no access. No ambient root, no ambient kernel.
2. **Confinement.** A component cannot leak capabilities to components it cannot already reach.
3. **Integrity.** A compromised component in one partition cannot affect another partition
   unless seL4 has granted an explicit capability channel.

These properties do NOT hold on x86-64 (functional correctness only — not proved at
information-flow level).

---

## Hard constraints — kernel configuration

- `KernelMaxNumNodes = "1"` — verified configurations are single-core. SMP is unverified
  on all architectures. Do not enable SMP in production verified deployments.
- Do not mix verified and unverified configurations in a single product claim.
- seL4 15.0.0 is the pinned version. Do not update without updating all vendor-sel4-kernel
  files and verifying that the proof still applies to the new version.
- AArch64 EL2 is the only target for which formal security claims may be made in
  product copy and public documentation.

---

## Dependencies

- `vendor-sel4-kernel/` — vendored seL4 15.0.0 source (1,074 files)
- `moonshot-toolkit/` — build orchestrator that reads `moonshot-kernel/` configuration
