@~/Foundry/AGENT.md

# project-system — Archive Guide

> **State:** active | **Last updated:** 2026-06-20
> **Cluster manifest:** `.agent/manifest.md`
> **Workspace AGENT.md takes precedence on conflict.**

---

## Cluster mission

Core infrastructure cluster for PointSav's Rust monorepo (`pointsav-monorepo`).
Primary active crates:

- **`os-totebox`** — NetBSD unikernel VM image; Veriexec strict=1, SLIRP networking, SSH gate; v0.2 Phase 2 complete 2026-06-14
- **`service-vm-fleet`** — PPN fleet controller; axum :9203; heartbeat ingestion + advisory placement; 26 tests
- **`service-vm-host`** — PPN per-node heartbeat agent; /proc + /dev/kvm + QMP poll; 7 tests
- **`service-vm-tenant`** — PPN customer VM proxy; axum :9221; Bearer auth + quota + WORM audit; 11 tests
- **`system-vm-fleet-types`** — shared PPN VM wire types; `no_std`-compatible; 4 tests
- **`moonshot-toolkit`** v0.3.1 — seL4 build orchestrator (standalone `[workspace]`; invoke via `--manifest-path`)
- **`moonshot-sel4-vmm`** — `#![no_std]` seL4 AArch64 PD runtime (standalone `[workspace]`)

**Phase H1 complete 2026-06-19:** QEMU gate passed; `moonshot-toolkit/examples/os-console-hello.toml` spec exists.
**Next:** os-totebox Phase H1 spec — awaiting project-data PD target confirmation (msg sent 2026-06-20).

## Tetrad

See `.agent/manifest.md` `tetrad:` block for the canonical declaration.

## At session start

Per `~/Foundry/AGENT.md` § Session roles:

1. Confirm role: `~/Foundry/bin/foundry-role.sh` (Totebox Session expected)
2. Write session lock: `.agent/engines/<engine-id>/session.lock`
3. Read `.agent/manifest.md` — cluster mission + tetrad
4. Call `get_session_brief(role="totebox", archive="project-system")` — replaces inbox, NOTAM, session-context reads
5. Read `~/Foundry/NOTAM.md` — only if `notam_active: true` from step 4
6. Read `.agent/rules/*.md` if present

## Build / test / lint

```bash
# Monorepo workspace crates
cargo check -p <crate>
cargo test -p <crate>
cargo clippy -p <crate> -- -D warnings

# moonshot-toolkit (standalone workspace — use --manifest-path from archive root)
cargo run --manifest-path moonshot-toolkit/Cargo.toml -- build moonshot-toolkit/examples/os-console-hello.toml
cargo test --manifest-path moonshot-toolkit/Cargo.toml --all-targets   # 35 tests

# moonshot-sel4-vmm (standalone workspace)
cargo test --manifest-path moonshot-sel4-vmm/Cargo.toml
```

## Hard rules (workspace-level, do not duplicate; reference only)

- `~/Foundry/AGENT.md` § Hard rules — identity store immutable, never chmod; preview before writing;
  edit in place (no _V2 files); one session per repo; Bloomberg standard; BCSC posture; SYS-ADR-07/10/19.
- `~/Foundry/CLAUDE.md` § Size discipline — per-archive CLAUDE.md ≤ 150 lines.

## Commit + promote

Commits via `~/Foundry/bin/commit-as-next.sh "<message>"` from archive root.
Stage 6 promotion via `~/Foundry/bin/promote.sh` from Command Session.

## MCP tools — `foundry` server (use at startup)

`get_session_brief(role="totebox", archive="project-system")` replaces manually reading
inbox.md, outbox.md, NOTAM.md, session-context.md. Call it first.

| Tool | When to use |
|---|---|
| `get_session_brief` | **First call at startup** — inbox, outbox, NOTAM, session-context |
| `send_mailbox_message` | Send any mailbox message (M-2/M-10 audit compliant) |
| `query_datagraph` | Entity lookup before answering about people/projects |
| `ask_local` | OLMo 7B local inference — free, SYS-ADR-07-safe |
