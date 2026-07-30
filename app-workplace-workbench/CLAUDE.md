# CLAUDE.md — app-workplace-workbench

> **State:** Active — Wave 1 | **Last updated:** 2026-05-27
> **Registry row:** `.agent/rules/project-registry.md`

---

## What this project is

`app-workplace-workbench` is a Tauri v1.7 WebView shell for the privategit
development workbench. It loads `app-privategit-workbench` (the HTTP web IDE,
managed by project-development) at a configurable localhost port in a native
macOS window.

No logic from the workbench is forked into this crate. The HTTP server runs
independently; this app provides a desktop window to access it.

Platform: macOS 10.13 High Sierra (Tauri v1). Apache-2.0 licence.

## Architecture

- `src-tauri/src/main.rs`: reads configured port from app data dir; exposes
  `get_workbench_url` and `set_workbench_port` IPC commands
- `src/index.html`: invokes `get_workbench_url` via `__TAURI__`; navigates
  to the returned URL on load
- `src-tauri/tauri.conf.json`: CSP allows `http://127.0.0.1:*`; window starts
  at `index.html` which then redirects

## Before first build on macOS

1. Copy icons: `cp -r ../app-workplace-memo/src-tauri/icons src-tauri/`
2. `npm install`
3. `npm run build` (or `npm run dev` for development)

## Wave 1 scope

- Port configurability: first-run UX to set the workbench port
- Window title update to show the connected URL
- Graceful error page when workbench server is not running

## Hard rules

- No code import from `app-privategit-workbench`; it remains an independent process
- `minimumSystemVersion: "10.13"` must stay in tauri.conf.json
- CSP narrows to a specific port when the workbench port is fixed in production
