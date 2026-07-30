# NEXT.md — app-workplace-proforma

> Last updated: 2026-05-27
> Read at session start. Update before session end.

---

## Current state

Active scaffold (~45 files). Tauri v1.7 + Rust + vanilla JS frontend.
`src-tauri/tauri.conf.json` has `minimumSystemVersion: "10.13"` ✓.
EUPL v1.2. CLAUDE.md present.

## Wave 2 scope (foundation laid)

This app is Wave 2 — foundation committed 2026-05-27 to allow testing notes
and session context to accumulate. Active development starts after Wave 1
trio (workbench, memo, presentation) reaches exit criteria.

## Pending

- [ ] Verify `minimumSystemVersion: "10.13"` is present in tauri.conf.json
- [ ] Smoke test: build on macOS 10.13; binary opens and creates a spreadsheet
- [ ] Confirm EUPL-1.2 is consistent across all source files
- [ ] Add to project-software binary-targets.yaml when Wave 2 begins
- [ ] Wire endpoint configuration: connect proofreader (9097) and Doorman (9092)
