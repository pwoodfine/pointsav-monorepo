# CLAUDE.md — app-workplace-pdf

> **State:** Scaffold-coded — Wave 2 | **Last updated:** 2026-05-27
> **Registry row:** `.agent/rules/project-registry.md`

---

## What this project is

`app-workplace-pdf` is a sovereign desktop PDF viewer and print tool.
Uses `pdfium-render` crate (Apache 2.0 — wraps Google PDFium via FFI).

Platform: macOS 10.13 High Sierra (Tauri v1). Apache-2.0 licence.

## Wave 2 scope

- Open PDF via file picker; navigate pages
- Zoom, pan, text selection
- Print via OS print dialogue
- Retrieve PDFs from Foundry services over WireGuard PPN

## Dependency note

`pdfium-render` requires the PDFium binary to be statically linked for
macOS distribution. The PDFium binary (Apache 2.0) must be downloaded
from pdfium-binaries releases and bundled in `src-tauri/`.

## Hard rules

- `minimumSystemVersion: "10.13"` in tauri.conf.json
- Apache-2.0 licence — no LGPL/GPL dependencies linked
- PDFium binary is Apache 2.0 — clean to bundle
