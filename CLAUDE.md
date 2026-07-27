@~/Foundry/AGENT.md

# pointsav-monorepo — Sub-clone Guide

> This CLAUDE.md covers the `pointsav-monorepo/` sub-clone common to all
> Totebox Archives that use the monorepo as their vendor leg.
> **Archive-level guidance (mission, tetrad, MCP startup) lives in `../CLAUDE.md`.**
> **Workspace AGENT.md takes precedence on conflict.**

---

## Sub-clone role

`pointsav-monorepo` is the vendor-leg sub-clone for any Totebox Archive whose
work lives in this monorepo. Commit here via `commit-as-next.sh`; promote to
canonical (`vendor/pointsav-monorepo`) via Command Session `promote.sh`.

The monorepo is shared — multiple Totebox Archives may have clones of it.
Never force-push; never reset --hard without operator approval and all sessions
confirming inactive.

## Fast gates

```
cargo check -p <crate>
cargo test -p <crate>
cargo clippy -p <crate> -- -D warnings
cargo fmt -p <crate> --check
```

Substitute the crate for the active project. For project-knowledge: `app-mediakit-knowledge`.

## Key layout

```
pointsav-monorepo/
├── Cargo.toml              workspace manifest (all member crates)
├── app-mediakit-knowledge/ wiki engine (project-knowledge focus)
├── app-console-*/          TUI cartridges (project-console focus)
├── service-*/              backend services
└── scripts/                xtask, dtcg-to-css.py, stage6-gate.sh
```

## Commit rules

- `git add <specific files>` — never `git add .`
- `~/Foundry/bin/commit-as-next.sh "<type>(<scope>): <message>"` from sub-clone CWD
- Run `cargo test -p <crate>` before every commit
- If unpromoted commits exist, write `"Stage 6 pending — <archive> — <crate>"` to outbox

## Conflicts

Surface via archive outbox (`../.agent/outbox.md`) — not here.
Do not write to another archive's state files.
