# NEXT.md — app-workplace-presentation

> Last updated: 2026-05-27
> Read at session start. Update before session end.

---

## Foundation (complete 2026-05-27)

- [x] src-tauri/ Tauri v1.7 skeleton (Cargo.toml, build.rs, src/main.rs, tauri.conf.json)
- [x] src/index.html placeholder
- [x] CLAUDE.md added; minimumSystemVersion 10.13 confirmed

## Wave 1 — active

- [ ] Copy icons from `app-workplace-memo/src-tauri/icons/`
- [ ] Add package.json for Tauri CLI dev workflow
- [ ] Run `npm run build` on macOS 10.13; verify binary opens
- [ ] Design slide canvas: HTML/CSS-based, single-file output goal
- [ ] Implement slide create/reorder/delete
- [ ] Text block with bold/italic/size controls
- [ ] Image insert from local file via Tauri dialog API
- [ ] Export to self-contained HTML
- [ ] Open/save presentation files
- [ ] Smoke test on WireGuard PPN: no network calls leak
- [ ] Add to project-software binary-targets.yaml once first build passes
