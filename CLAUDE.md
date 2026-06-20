@~/Foundry/AGENT.md

# project-design — Archive Guide

> **State:** active | **Last updated:** 2026-06-20
> **Cluster manifest:** `.agent/manifest.md`
> **Workspace AGENT.md takes precedence on conflict.**

---

## Cluster mission

Design system cluster for PointSav. Two main components:
1. **`app-privategit-design`** — Rust/axum SSR browser serving design.pointsav.com (port 9094). Schema-aware rendering for COMPONENT, TOKEN, RESEARCH, MARKETING, and BUNDLE artifact types. Phases A–D live (routes split, inotify watcher, SSE sidebar, AI bridge).
2. **`pointsav-design-system`** (sub-clone) — DTCG 2025.10 token repository. `dtcg-bundle.json` is the canonical token file; `components/`, `research/`, `assets/` hold DESIGN-* artifacts.

Also manages design asset pipelines: `woodfine-media-assets`, `pointsav-media-assets` (staging-tier commit + promote).

**Live service:** `app-privategit-design` v0.2.0 at `local-design.service` port 9094. Binary: `/usr/local/bin/app-privategit-design` (sha256 `1883110e`, deployed 2026-06-20).
**v0.3.0 in progress:** marketing.rs + bundle.rs renderers + composite token groups. Plan: `/home/jennifer/.claude/plans/no-make-a-plan-abundant-forest.md`.

## Tetrad

See `.agent/manifest.md` `tetrad:` block for the canonical declaration
across vendor / customer / deployment / wiki legs.

## At session start

Per `~/Foundry/AGENT.md` § Session roles:

1. Confirm role: `~/Foundry/bin/foundry-role.sh` (Totebox Session expected)
2. Write session lock: `.agent/engines/<engine-id>/session.lock`
3. Read `.agent/manifest.md` — cluster mission + tetrad
4. Call `get_session_brief(role="totebox", archive="project-design")` — replaces inbox, NOTAM, session-context reads
5. Read `~/Foundry/NOTAM.md` — workspace warnings
6. Read `.agent/rules/*.md` if present

## Build / test / lint

```bash
# from /srv/foundry/clones/project-design/
cargo check -p app-privategit-design
cargo test -p app-privategit-design
cargo clippy -p app-privategit-design -- -D warnings
cargo fmt -p app-privategit-design
cargo build --release -p app-privategit-design
```

Binary output: `$CARGO_TARGET_DIR/release/app-privategit-design` (CARGO_TARGET_DIR=/srv/foundry/cargo-target/jennifer).

## Hard rules (workspace-level, do not duplicate; reference only)

- `~/Foundry/AGENT.md` § Hard rules — identity store immutable, never
  chmod; preview before writing; edit in place (no _V2 files);
  one session per repo; Bloomberg standard; BCSC posture; SYS-ADR-07/10/19.
- `~/Foundry/CLAUDE.md` § Size discipline — per-archive CLAUDE.md ≤ 150 lines.

## Commit + promote

Commits to pointsav-design-system sub-clone use: `~/Foundry/bin/commit-as-next.sh "<msg>"` from within `pointsav-design-system/`.
Commits to archive root (CLAUDE.md, BRIEFs, etc.) use: `~/Foundry/bin/commit-as-next.sh "<msg>"` from `clones/project-design/`.
Stage 6 promotion via `~/Foundry/bin/promote.sh` from Command Session.
**Stage 6 pending after Phase 2:** new design-system intake commits need promote.

## MCP tools — `foundry` server (use at startup)

`get_session_brief(role="totebox", archive="project-design")` replaces manually reading
inbox.md, outbox.md, NOTAM.md, session-context.md. Call it first.

| Tool | When to use |
|---|---|
| `get_session_brief` | **First call at startup** — inbox, outbox, NOTAM, session-context |
| `send_mailbox_message` | Send any mailbox message (M-2/M-10 audit compliant) |
| `query_datagraph` | Entity lookup before answering about people/projects |
| `ask_local` | OLMo 7B local inference — free, SYS-ADR-07-safe |

## Artifact types — project-design scope

DESIGN-COMPONENT = component guide (CSS+HTML recipe, not DTCG tokens).
DESIGN-TOKEN-CHANGE = delta to dtcg-bundle.json; requires master_cosign if touching legal/semantic groups.
DESIGN-RESEARCH = design system research; routes to pointsav-design-system/research/.
DESIGN-BUNDLE = TOKEN + STYLESHEET + TEMPLATE bundle (ratified 2026-06-20); namespace: component.document.legal.*.
ASSET = raw asset file (SVG, PNG, YAML palette).
