# NEXT.md — service-people

> Last updated: 2026-04-25
> Read at session start. Update before session end so the next
> session knows where to pick up.

---

## Right now

- Inventory the existing sub-directories (`sovereign-acs-engine/`,
  `spatial-crm/`, `spatial-ledger/`, `substrate/`, `tools/`) and
  the `service-people.py` + `ledger_personnel.json` artefacts.
  Decide per-item: keep, rename, retire, or relocate. The decisions
  inform the schema work that follows.

## Queue

- Define the Identity Ledger schema — canonical key per identity,
  multi-endpoint communication addresses, role/relationship
  attributes. Publish in a `schema/` subdirectory; downstream Ring
  2 consumers depend on it.
- MCP server interface — resources for identity reads, tools for
  identity append/update. Per-tenant moduleId isolation.
- Append integration with `service-fs` — identity record writes
  flow through the WORM ledger; this crate never persists
  directly.
- Deterministic entity-resolution rules — canonical-key matching
  only (ADR-07; no AI). Surfaces ambiguity to the operator (per
  ADR-10 / F12), does not silently merge.
- Add `service-people` as a workspace member in the monorepo root
  `Cargo.toml` once a real schema and `lib.rs` exist (Layer 1
  audit finding in `.claude/rules/cleanup-log.md` 2026-04-18).

## Blocked

- Schema definition — Blocked on: sub-directory inventory above. We
  may inherit a schema; we may not. Decide before re-defining.

## Deferred

- Cross-tenant identity sharing — Deferred: out of scope for Ring
  1 by `~/Foundry/conventions/three-ring-architecture.md`. If it
  ever lands, it lives in Ring 2 / Ring 3.
- Embedding-based fuzzy identity matching — Deferred (and
  doctrinally constrained): ADR-07 keeps Ring 1 zero-AI.
  Fuzzy matching, if needed, runs in Ring 2 with a deterministic
  read-only contract.

## Recently done

- 2026-04-25: project activated per `~/Foundry/CLAUDE.md` §9 —
  this CLAUDE.md, this NEXT.md, and the registry row created in
  one commit; existing sub-directories left in place for
  inventory in the next session.
