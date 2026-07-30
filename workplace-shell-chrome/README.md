# workplace-shell-chrome

Shared backend substrate for the `app-workplace-*` Tauri app family.

## What this is (and isn't)

This is a **real, linkable Rust lib crate** — the "ideal outcome" per the
2026-07-14 overnight-run task, not the JS/CSS-first fallback pattern the
staged `DESIGN-RESEARCH-workplace-shell-chrome.draft.md` and
`BRIEF-workplace-institutional-quality-roadmap.md` (Track B / R4) considered
as a lower-risk alternative. That fallback was designed around the fact that
none of the 6 `app-workplace-*` Tauri crates currently compile end-to-end on
this host (`soup2-sys`/`libsoup-2.4` blocker — see the BRIEF's Phase 1 Work
Log). This crate takes a narrower path that sidesteps that constraint: it
depends on **nothing Tauri-related** — only `serde` + `serde_json` + `std` —
so it compiles and its tests run cleanly on this host today, independent of
whether the consumer apps' full binaries do. See `src/lib.rs`'s
"Verification status" doc comment for the full, honest account of what is
and is not verified as a result.

## What it does

Generalizes the `get_X` / `set_X` / `has_X_config` JSON config-store triad
that `app-workplace-workbench` and `app-workplace-gis` each independently
reinvented (found 2026-07-13; see the design-research draft, finding 1):

| Function | Replaces |
|---|---|
| `load_config::<T>(dir, filename)` | workbench's `load_port`, gis's `load_config` |
| `save_config(dir, filename, &cfg)` | the body of `set_workbench_port`/`set_tile_endpoint` |
| `has_config(dir, filename)` | the body of `has_workbench_config`/`has_gis_config` |
| `ensure_app_data_dir(dir)` | the body of every app's `setup()` closure |

Each app keeps its own thin `#[tauri::command]` wrapper (Tauri's
`generate_handler!` macro needs the command function visible at its call
site, so the wrapper can't move into this crate) and its own config struct
shape (`WorkbenchConfig { port: u16 }`, `GisConfig { endpoint: String }`) —
only the duplicated I/O + serde + fallback logic moved here.

## Consumers

- `app-workplace-workbench/src-tauri` (`Cargo.toml` path dependency)
- `app-workplace-gis/src-tauri` (`Cargo.toml` path dependency)

Both are standalone `[workspace]` crates (excluded from the root monorepo
`Cargo.toml` workspace, per this repo's convention for apps not yet promoted
to workspace membership) — this crate is declared the same way, so it forms
its own single-member workspace and is pulled in cleanly as a path
dependency from either consumer without workspace-nesting conflicts.

## Verifying this crate standalone

```sh
cd workplace-shell-chrome
cargo check   # no Tauri deps — should pass regardless of the app-workplace-* soup2-sys blocker
cargo test    # exercises the real read/write/fallback/existence code path
```

## Not yet done (tracked in `BRIEF-workplace-institutional-quality-roadmap.md`)

- Retrofit into the remaining 4 `app-workplace-*` crates (`memo`, `proforma`,
  `presentation`, `pdf`) once this two-app proof case is reviewed.
- Promotion of the shared *frontend* patterns (command palette, keybinding
  table, status/error toast — Track B's B1/B2) — this crate is backend-only;
  the frontend substrate is a separate, JS/CSS-based effort per the roadmap.
- Full end-to-end `cargo check` verification of the two consumer apps, which
  requires Track A (`libsoup2.4-dev` install, ask-first) to land first.
