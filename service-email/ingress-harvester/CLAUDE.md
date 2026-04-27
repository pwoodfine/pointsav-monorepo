---
state: archived
last_updated: 2026-04-27
retired_reason: Superseded by service-fs WORM append pipeline
registry_pointer: pointsav-monorepo/.claude/rules/project-registry.md
version: 0.0.1
---

# CLAUDE.md — ingress-harvester

> **State: Archived — Last updated: 2026-04-27 — Retired: Superseded by service-fs WORM append pipeline + service-extraction Ring 2 processing**

This project is retired and no longer developed. The ingress-harvester pattern (watching an inbound folder for new payloads) is being replaced by Ring 1 MCP servers that accept inbound documents via the `/v1/append` endpoint in service-fs, providing deterministic, auditable writes to the WORM ledger instead of filesystem watching.

Existing code remains in place for reference; no new work should be undertaken in this directory.

---

## Archive record

Archived during sixth session (2026-04-27) per Master's 2026-04-26T02:25 pickup recommendation:
"(3) ingress-harvester/ + master-harvester-rs/ formal retirement (CLAUDE.md archive headers + registry rows)".

The directory contained a Rust crate (`Cargo.toml` + `src/`) with no external callers (verified via grep in `scripts/` 2026-04-27). Both directories marked for retirement when the MCP + service-fs append pipeline shipped to live (service-fs activated 2026-04-25, per `.claude/rules/project-registry.md` service-fs row).
