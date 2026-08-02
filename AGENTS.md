# AGENTS.md

> Vendor-neutral pointer file for coding agents (per the AGENTS.md
> spec, donated to the Linux Foundation December 2025).
>
> This repo's substantive operational guide for AI agents — including
> coding conventions, identity rules, commit flow, and session
> protocols — lives in **`CLAUDE.md`** at this same directory.
>
> If you are a coding agent (Cursor, Copilot, Cody, Aider, Claude
> Code, or any other), read `CLAUDE.md` first.

## Quick reference

- **Operational guide**: `CLAUDE.md` (this directory)
- **Constitutional charter**: `~/Foundry/DOCTRINE.md`
- **Workspace navigation**: `~/Foundry/CLAUDE.md`
- **Open work queue**: `NEXT.md`
- **Version history**: `CHANGELOG.md`

## Why two files

Foundry maintains both `CLAUDE.md` and `AGENTS.md`:

- `CLAUDE.md` carries the substantive operational content. Long
  history; well-known to Anthropic's Claude Code agent; the canonical
  source.
- `AGENTS.md` is the vendor-neutral discovery convention — coding
  agents from non-Anthropic vendors look for this filename first.

Both files lead to the same instructions. Update `CLAUDE.md`; this
file points at it.

## License

This file inherits the repo's `LICENSE` file at the same directory.

---

*Per `~/Foundry/conventions/root-files-discipline.md` Tier 2.
Template at `~/Foundry/templates/AGENTS.md.tmpl`.*
