# NEXT.md — app-workplace-gis

> Last updated: 2026-05-27
> Read at session start. Update before session end.

---

## Current state

Foundation scaffold: README.md, CLAUDE.md. No Tauri crate yet.
Wave 2 — active development begins after Wave 1 trio ships.

## Wave 2 — when activating

- [ ] Add src-tauri/ skeleton with WebView approach (like app-workplace-workbench)
- [ ] Add `minimumSystemVersion: "10.13"` to tauri.conf.json
- [ ] Bundle MapLibre GL JS locally (BSD-3-Clause, clean for Apache-2.0 host)
- [ ] Implement tile viewer pointing at configured endpoint
- [ ] Implement configurable endpoint (default: PPN GIS address)
- [ ] Smoke test: clusters render on macOS 10.13 over WireGuard PPN
- [ ] Add to project-software binary-targets.yaml
