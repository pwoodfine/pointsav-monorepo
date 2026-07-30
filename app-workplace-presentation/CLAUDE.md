# CLAUDE.md — app-workplace-presentation

> **State:** Active — Wave 1 | **Last updated:** 2026-05-27
> **Registry row:** `.agent/rules/project-registry.md`

---

## What this project is

`app-workplace-presentation` is a sovereign, offline-first desktop presentation
tool. PowerPoint/Keynote muscle memory on the outside; canonical output format
TBD (PPTX-compatible JSON or self-contained HTML) on the inside.

Platform: macOS 10.13 High Sierra (Tauri v1). EUPL-1.2 licence.

## Current state

Foundation scaffold: Tauri v1.7 `src-tauri/` skeleton added 2026-05-27.
Frontend (`src/`) is a placeholder page. No IPC commands implemented yet.
First milestone: editor UI that creates a slide, adds text, and exports.

## Build

1. Copy icons: `cp -r ../app-workplace-memo/src-tauri/icons src-tauri/`
2. `npm install` (if package.json present) or use `cargo tauri` directly
3. `cargo tauri build` from the `src-tauri/` directory

## Wave 1 scope

- Slide editor: create, reorder, delete slides
- Text blocks with basic formatting (bold, italic, size)
- Image insert from local file
- Export to self-contained HTML (opens in any browser, prints to PDF)
- Open/save presentation files

## Hard rules

- `minimumSystemVersion: "10.13"` must stay in tauri.conf.json
- `connect-src 'none'` — zero outbound network connections
- EUPL-1.2 licence: all contributions must be compatible
