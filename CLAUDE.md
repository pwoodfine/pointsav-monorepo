@~/Foundry/AGENT.md

# project-proforma — Archive Guide

> **State:** active | **Last updated:** 2026-06-19
> **Cluster manifest:** `.agent/manifest.md`
> **Workspace AGENT.md takes precedence on conflict.**

---

## Cluster mission

SLM inference infrastructure — Doorman (Tier A/B/C routing + circuit breaker),
OLMo 7B Tier A (local CPU inference via `local-slm.service`),
OLMo 32B Tier B (yoyo-batch L4 GPU; us-central1-b target),
DataGraph entity enrichment (LadybugDB via `service-content`),
and LoRA training pipeline.

**Active cartridges:** F2 People, F4 Content, F9 SLM, F11 System, F12 Input (Anchor), F3 Email, F6 Bookkeeper.
**Phase 10 complete 2026-06-16:** F2 PeopleCartridge scaffold, Rgb color helpers, session persistence, chassis reconnect watchdog.
**Phase H1 complete 2026-06-19:** moonshot-sel4-vmm `#![no_std]` PD runtime + console_hello.c seL4 rootserver; QEMU gate passed ("Hello from os-console seL4 PD"). Use `-m 1G` (DTB reports 1 GiB).
**Next:** Phase 11 — F7 BIM cartridge (`app-console-bim`); blocked on project-bim Phase 1 service. Phase H2 — VirtIO serial PD + ratatui on seL4.

## Tetrad

See `.agent/manifest.md` `tetrad:` block for the canonical declaration
across vendor / customer / deployment / wiki legs.

## At session start

Per `~/Foundry/AGENT.md` § Session roles:

1. Confirm role: `~/Foundry/bin/foundry-role.sh` (Totebox Session expected)
2. Write session lock: `.agent/engines/<engine-id>/session.lock`
3. Read `.agent/manifest.md` — cluster mission + tetrad
4. Call `get_session_brief(role="totebox", archive="project-proforma")` — replaces inbox, NOTAM, session-context reads
5. Read `~/Foundry/NOTAM.md` — workspace warnings
6. Read `.agent/rules/*.md` if present (may be absent for newer archives)

## Hard rules (workspace-level, do not duplicate; reference only)

- `~/Foundry/AGENT.md` § Hard rules — identity store immutable, never
  chmod; preview before writing; edit in place (no _V2 files);
  one session per repo; Bloomberg standard; BCSC posture; SYS-ADR-07/10/19.
- `~/Foundry/CLAUDE.md` § Size discipline — per-archive CLAUDE.md ≤ 150 lines.

## Commit + promote

Commits via `~/Foundry/bin/commit-as-next.sh "<message>"` (from archive root or `service-slm/`).
Stage 6 promotion via `~/Foundry/bin/promote.sh` from Command Session.
**Stage 6 pending:** commits through `fc4d0978` (Phase 10) need promote; git divergence on main is a Command-side blocker.

## Conflicts

If a workspace rule conflicts with anything stated here, **stop and surface
the conflict via outbox to command session** — do not silently override.

## MCP tools — `foundry` server (use at startup)

`get_session_brief(role="totebox", archive="project-proforma")` replaces manually reading
inbox.md, outbox.md, NOTAM.md, session-context.md. Call it first.
`send_mailbox_message()` replaces hand-editing YAML frontmatter.

| Tool | When to use |
|---|---|
| `get_session_brief` | **First call at startup** — inbox, outbox, NOTAM, session-context |
| `send_mailbox_message` | Send any mailbox message (M-2/M-10 audit compliant) |
| `query_mailbox` | Sweep archives — scope="all" in one call |
| `get_doorman_status` | Tier A/B/C + circuit state |
| `get_service_status` | Apprenticeship queue + audit-ledger counts |
| `query_datagraph` | Entity lookup before answering about people/projects |
| `ask_local` | OLMo 7B local inference — free, SYS-ADR-07-safe; graph context auto-injected |
| `cast_apprenticeship_verdict` | Sign + submit verdict on a shadow-captured attempt |
| `mutate_datagraph` | Create/update graph entities (requires explicit operator intent) |
| `submit_extraction` | Queue prose for entity extraction pipeline |

## Artifact types — bright-line rules

TOPIC = explains WHAT/WHY; public wiki; bilingual EN+ES; survives decommission; reader has no login.
GUIDE = instructs HOW-NOW; woodfine-fleet-deployment/<name>/; English-only; dies with deployment.
SOFT  = Ed25519 license key + marketplace listing + price → software.pointsav.com.
CODE  = runs our systems; no customer license; internal deploy only (published OSS with no key = CODE).
Split rule: declaratives → TOPIC, imperatives → GUIDE; same slug, different prefix, no shared sentences.
Cash register test: licensable + marketplace-listed → SOFT; everything else → CODE.
Storefront (app-privategit-marketplace) is CODE; the merchandise it sells is SOFT.
