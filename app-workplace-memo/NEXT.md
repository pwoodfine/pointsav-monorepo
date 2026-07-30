# NEXT.md — app-workplace-memo

> Last updated: 2026-05-07
> Read at session start. Update before session end.

---

## Right now

- **Walking skeleton on Linux (Tauri v2).** The scaffold has never been
  built end-to-end on the production target. First milestone: `npm run
  tauri build` succeeds on Ubuntu 22.04 or Debian 12; the binary opens,
  creates a document, saves it, and re-opens it correctly.

- **WebKitGTK CSS `@page` verification.** Print output (`@media print`
  layout) must be verified on WebKitGTK — behaviour differs from WKWebView
  (macOS) and WebView2 (Windows).

## Pending

- `README.md` bilingual link audit — confirm `README.es.md` link is
  present and working.
- CHANGELOG: land v0.1.0 entry once walking skeleton is verified.
- Registry row promotion: Scaffold-coded → Active is already recorded;
  verify it reflects reality once first build is confirmed.

## Blocked

- Nothing — scaffold work can proceed independently of other clusters.

## Done

- Scaffold created (47 files): JS frontend, Rust IPC, HTML/CSS, docs.
- CLAUDE.md added 2026-05-07 (Active-state conformance).
