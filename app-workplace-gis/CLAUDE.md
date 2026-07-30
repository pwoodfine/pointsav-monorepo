# CLAUDE.md — app-workplace-gis

> **State:** Scaffold-coded — Wave 2 | **Last updated:** 2026-05-27
> **Registry row:** `.agent/rules/project-registry.md`

---

## What this project is

`app-workplace-gis` is a sovereign desktop GIS viewer. Tauri WebView shell
loading a MapLibre GL-based tile viewer that connects to gis.woodfinegroup.com
(or a local tile server) over WireGuard PPN.

Platform: macOS 10.13 High Sierra (Tauri v1). Apache-2.0 licence.

## Architecture

WebView approach (same as app-workplace-workbench): the tile viewer runs as
a local web page loaded by Tauri. The GIS tile data is served from the
configured endpoint. MapLibre GL JS runs inside the WebView.

## Wave 2 scope

- View cluster map (T1/T2/T3 layers)
- Configurable endpoint (default: gis.woodfinegroup.com or PPN address)
- Navigate, zoom, click clusters for details
- Export current view

## Hard rules

- `minimumSystemVersion: "10.13"` in tauri.conf.json
- CSP allows tile server endpoint and MapLibre CDN (or bundle MapLibre locally)
- Apache-2.0; MapLibre GL JS is BSD-3-Clause — clean to bundle
