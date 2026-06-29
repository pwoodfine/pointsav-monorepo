@~/Foundry/AGENT.md

# project-gis — Archive Guide

> **State:** active | **Last updated:** 2026-06-20
> **Cluster manifest:** `.agent/manifest.md`
> **Workspace AGENT.md takes precedence on conflict.**

## Commit + promote

Commits via `~/Foundry/bin/commit-as-next.sh "<message>"` from archive root.
**Stage 6 self-service (this archive):** `~/Foundry/bin/self-service-promote.sh`
— pushes code commits to staging mirrors + appends to `promote-queue.jsonl`.
Command Session processes canonical merge. Do NOT run `promote.sh` directly.

## MCP tools — `foundry` server (use at startup)

`get_session_brief(role="totebox", archive="project-gis")` replaces manually reading
inbox.md, outbox.md, NOTAM.md, session-context.md. Call it first.

| Tool | When to use |
|---|---|
| `get_session_brief` | **First call at startup** — inbox, outbox, NOTAM, session-context |
| `send_mailbox_message` | Send any mailbox message (M-2/M-10 audit compliant) |
| `query_datagraph` | Entity lookup before answering about people/projects |
| `ask_local` | OLMo 7B local inference — free, SYS-ADR-07-safe |
