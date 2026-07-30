# CLAUDE.md — app-workplace-memo

> **State:** Active  —  **Last updated:** 2026-05-07
> **Registry row:** `.agent/rules/project-registry.md`

---

## What this project is

`app-workplace-memo` (Workplace✦Memo) is a sovereign, offline-first desktop
document editor. Word/Pages muscle memory on the outside; self-contained
`.html` output on the inside. The output file embeds all fonts as base64 and
opens in any browser in perpetuity. It prints to PDF via the OS print
dialogue. Zero network calls, zero accounts, zero kill switch.

Stack: Tauri (Rust backend + OS WebView frontend) + vanilla JS. No bundler,
no React, no npm runtime dependencies. EUPL v1.2.

Dev platform: macOS 10.13 High Sierra (Tauri v1). Production target: Linux
(Tauri v2 — WebKitGTK).

## Current state

Scaffold complete (~47 files). Document editor with formatting toolbar,
bilingual READMEs, full ARCHITECTURE.md and DEVELOPMENT.md. The walking
skeleton has not been verified end-to-end on Linux. `CHANGELOG.md` is
entirely under `[Unreleased]` — v0.1.0 is not yet tagged.

## Build

```
npm install         # one-time
npm run tauri dev   # or: make dev
npm run tauri build # or: make build
```

Tauri v1 on macOS 10.13; Tauri v2 on Linux — see `DEVELOPMENT.md` for
platform-specific prerequisites.

## File layout

```
app-workplace-memo/
├── CLAUDE.md          this file
├── NEXT.md            open items
├── ARCHITECTURE.md    ADRs: Tauri, EUPL, CSP, font strategy
├── DEVELOPMENT.md     platform setup, prerequisites
├── CHANGELOG.md       unreleased; v0.1.0 pending
├── package.json       Tauri + npm scripts
├── Makefile           convenience aliases
├── src/               JS frontend (editor logic, toolbar, IPC)
├── src-tauri/         Rust backend (IPC commands, file I/O)
├── fonts/             bundled fonts (embedded at build time)
├── docs/              print pipeline, schema notes
└── scripts/           build helpers
```

## Hard constraints

- **No network calls.** `connect-src 'none'` is load-bearing.
- **No runtime npm dependencies.** Dev tooling only; runtime must be
  vendored and EUPL/Apache/MIT compatible.
- **No `unsafe-eval` in CSP.** The JS engine is written to avoid `eval()`.
- **HTML output is the canonical format.** Not `.docx`, not `.pdf` as
  source; those are derived outputs.
