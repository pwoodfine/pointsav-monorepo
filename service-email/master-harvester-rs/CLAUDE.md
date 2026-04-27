---
state: archived
last_updated: 2026-04-27
retired_reason: Superseded by service-fs WORM append pipeline + service-extraction Ring 2
registry_pointer: pointsav-monorepo/.claude/rules/project-registry.md
version: 0.0.1
---

# CLAUDE.md — master-harvester-rs

> **State: Archived — Last updated: 2026-04-27 — Retired: Superseded by service-fs WORM append pipeline + service-extraction Ring 2 processing**

This project is retired and no longer developed. The master-harvester pattern (batch processing of harvested records) is being replaced by the Ring 1 / Ring 2 split: service-fs handles deterministic WORM appends (Ring 1), and service-extraction (Ring 2) runs the extraction / entity-resolution logic on read-side queries against the ledger.

Existing code remains in place for reference; no new work should be undertaken in this directory.

---

## Archive record

Archived during sixth session (2026-04-27) per Master's 2026-04-26T02:25 pickup recommendation:
"(3) ingress-harvester/ + master-harvester-rs/ formal retirement (CLAUDE.md archive headers + registry rows)".

The directory contained a Rust crate (`Cargo.toml` + `src/`) with no external callers (verified via grep in `scripts/` 2026-04-27). Both directories marked for retirement when the MCP + service-fs append pipeline shipped to live (service-fs activated 2026-04-25, per `.claude/rules/project-registry.md` service-fs row).
