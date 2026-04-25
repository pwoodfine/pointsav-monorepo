# NEXT.md — service-fs

> Last updated: 2026-04-25
> Read at session start. Update before session end so the next
> session knows where to pick up.

---

## Right now

- Surface seL4-unikernel-vs-MCP scaffold drift to Master Claude via
  cluster outbox (`~/Foundry/clones/project-data/.claude/outbox.md`,
  subject `ring1-scaffold-runtime-model-drift`). Activation has
  landed; the rewrite waits on Master ratification.

## Queue

- Once Master ratifies the rewrite plan: replace the no_std bare-
  metal scaffold with a hosted Ring 1 MCP-server skeleton — Tokio
  async runtime, per-tenant moduleId isolation, append-only API
  surface.
- Add the crate as a workspace member in the root `Cargo.toml` (it
  is not currently declared a member; see Layer 1 audit finding in
  `.claude/rules/cleanup-log.md` 2026-04-18 entry).
- Storage layout for the ledger — likely hash-addressed segment
  files in immutable directories. Decision deferred until the MCP
  API surface is fixed (the wire protocol drives the storage shape,
  not the other way around).
- Append-only invariant tests at the API surface — no path mutates
  a previously-persisted entry.
- ADR-07 audit hook: every Ring 2 caller's read is logged with
  moduleId + timestamp + opaque-cursor for downstream auditing.
- MCP server interface per Anthropic / Cloudflare 2026 reference —
  resources for "ledger reads", tools for "append".

## Blocked

- All scaffold-replacement work — Blocked on: Master Claude
  ratification of the rewrite plan in response to outbox message
  `ring1-scaffold-runtime-model-drift`.

## Deferred

- seL4 bare-metal file-system work — Deferred: belongs in a future
  seL4-related project alongside `vendor-sel4-kernel` /
  `moonshot-sel4-vmm`, not in this Ring 1 service. The current
  `src/main.rs` is the surviving artefact of that earlier framing
  and should be relocated when that project opens.

## Recently done

- 2026-04-25: project activated per `~/Foundry/CLAUDE.md` §9 — this
  CLAUDE.md, this NEXT.md, and the registry row created in one
  commit; runtime-model drift surfaced in CLAUDE.md "Current state"
  rather than silently propagated.
