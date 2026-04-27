# NEXT.md — service-people

> Last updated: 2026-04-27
> Read at session start. Update before session end so the next
> session knows where to pick up.

---

## Right now

- MCP server interface — resources for identity reads, tools for
  identity append/update. Per-tenant moduleId isolation. Schema
  (schema/identity-record.schema.json) is stable as of 2026-04-27.

## Queue
- Append integration with `service-fs` — identity record writes
  flow through the WORM ledger; this crate never persists
  directly.
- Deterministic entity-resolution rules — canonical-key matching
  only (ADR-07; no AI). Surfaces ambiguity to the operator (per
  ADR-10 / F12), does not silently merge.
- Add `service-people` as a workspace member in the monorepo root
  `Cargo.toml` once a real schema and `lib.rs` exist (Layer 1
  audit finding in `.claude/rules/cleanup-log.md` 2026-04-18).

## Queue

- **`sovereign-acs-engine/` rename.** Cargo.toml `name` field is
  `sovereign-acs-engine` — "sovereign" prefix is a Do-Not-Use term
  per workspace conventions (cleanup-log.md history). Rename to
  `people-acs-engine`. The directory name itself is non-canonical
  too; target layout is to fold the email-anchoring logic into
  `service-people`'s main library once the schema lands, making the
  standalone binary unnecessary. Track rename alongside schema work.
- **`spatial-ledger/` retirement.** The batch ledger-writer will be
  superseded once the MCP + service-fs WORM append pipeline is live.
  Retire when MCP integration is working end-to-end.
- **`spatial-crm/` retirement.** Cross-ring coupling — writes
  directly to `service-slm/transient-queues`, which is Ring 2
  territory. Functionality superseded by service-extraction (Ring 2)
  once it is wired to consume Ring 1 parsed documents. Retire when
  service-extraction covers the regex extraction use-case.
- **`service-people.py` + `ledger_personnel.json` retirement.**
  Pre-framework Python script and its placeholder seed data. The
  Python script's dual-timezone campaign-contact logic is a different
  domain from the identity ledger (communication scheduling, not
  canonical identity). Both retire once the Rust MCP service has a
  real schema and real data.
- MCP server interface — resources for identity reads, tools for
  identity append/update. Per-tenant moduleId isolation.
- Append integration with `service-fs` — identity record writes
  flow through the WORM ledger; this crate never persists directly.
- Deterministic entity-resolution rules — canonical-key matching
  only (ADR-07; no AI). Surfaces ambiguity to the operator (per
  ADR-10 / F12), does not silently merge.
- Add `service-people` as a workspace member in the monorepo root
  `Cargo.toml` once a real schema and `lib.rs` exist (Layer 1
  audit finding in `.claude/rules/cleanup-log.md` 2026-04-18).

## Blocked

- Schema definition — Blocked on: sub-directory inventory (now
  complete per Recently done 2026-04-26). Inventory revealed a
  working UUIDv5 anchoring approach in `sovereign-acs-engine/`
  that informs the schema. Unblocked.

## Deferred

- Cross-tenant identity sharing — Deferred: out of scope for Ring
  1 by `~/Foundry/conventions/three-ring-architecture.md`. If it
  ever lands, it lives in Ring 2 / Ring 3.
- Embedding-based fuzzy identity matching — Deferred (and
  doctrinally constrained): ADR-07 keeps Ring 1 zero-AI.
  Fuzzy matching, if needed, runs in Ring 2 with a deterministic
  read-only contract.

## Recently done

- 2026-04-26: **pre-framework subdirectory inventory complete.**
  Five subdirectories + two root artefacts assessed; decisions:
  | Item | Decision |
  |---|---|
  | `sovereign-acs-engine/` | **Keep** — deterministic email-anchoring via UUIDv5; well-structured Anchor/Claim JSONL schema; informs Identity Ledger design. **Rename** Cargo `name` from `sovereign-acs-engine` → `people-acs-engine` (Do-Not-Use "sovereign" prefix, per cleanup conventions); eventually fold into service-people library. |
  | `spatial-ledger/` | **Keep** — batch ledger-writer that generates `substrate/ledger_personnel.jsonl` from `discovery-queue/`. Precursor to WORM append pipeline. Retire once MCP + service-fs integration is live. |
  | `spatial-crm/` | **Retire-pending** — cross-ring coupling (writes to `service-slm/transient-queues` directly, violating Ring 1 boundary). Regex extraction functionality superseded by service-extraction (Ring 2). Retire when service-extraction is wired. |
  | `substrate/` | **Runtime data container** — `ledger_personnel.jsonl` (9 real identity records from OpenStack ML) untracked from git and gitignored this session. Physical directory remains for the running service. |
  | `tools/` | **Relocated** — `extract-people-ledger.sh` moved to `scripts/` per repo-layout.md; `tools/` directory removed. Done this session. |
  | `service-people.py` | **Retire-pending** — pre-framework dual-timezone campaign-contact script; different domain from identity ledger. Retire alongside schema work. |
  | `ledger_personnel.json` | **Retire-pending** — placeholder seed contacts (WMC-001/002/003). Retire once real schema and data land. |

- 2026-04-25: project activated per `~/Foundry/CLAUDE.md` §9 —
  this CLAUDE.md, this NEXT.md, and the registry row created in
  one commit; existing sub-directories left in place for
  inventory in the next session.
