# NEXT.md — app-workplace-pdf

> Last updated: 2026-05-27
> Read at session start. Update before session end.

---

## Current state

Foundation scaffold: README.md, CLAUDE.md. No Tauri crate yet.
Wave 2 — active development begins after Wave 1 trio ships.

## Wave 2 — when activating

- [ ] Add src-tauri/ skeleton (Cargo.toml, build.rs, src/main.rs, tauri.conf.json)
- [ ] Add `minimumSystemVersion: "10.13"` to tauri.conf.json
- [ ] Add pdfium-render dependency to Cargo.toml
- [ ] Download PDFium binary (Apache 2.0) from pdfium-binaries releases
- [ ] Implement: open PDF via file picker, render pages, navigate
- [ ] Implement: print via OS print dialogue
- [ ] Smoke test: open a multi-page PDF; all pages render on macOS 10.13
- [ ] Add to project-software binary-targets.yaml
