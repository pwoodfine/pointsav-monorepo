# app-mediakit-foodservice

The food-service platform engine for `foodservice.woodfinegroup.com`.

[ 🇪🇸 Leer este documento en Español ](./README.es.md)

> **State:** Active (P1 scaffold, 2026-06-24).
> Registry row: `pointsav-monorepo/.agent/rules/project-registry.md`.
> Archive: `clones/project-foodservice`.

## What it is

A single Rust binary (axum 0.8) that renders pages **server-side** from typed
section-manifests. Follows the `app-mediakit-marketing` pattern: shared chrome
chassis from `app-mediakit-shell`, agent-first MCP authoring, F12 human
approval gate (SYS-ADR-10, SYS-ADR-19).

- **Content model:** `content/<slug>/page.yaml` — ordered typed sections
- **Chrome:** `app-mediakit-shell` (header, footer, all CSS)
- **Port:** `127.0.0.1:9103` (default)
- **Env prefix:** `SERVICE_FOODSERVICE_*`

## Run

```
cargo run -p app-mediakit-foodservice -- serve \
  --content-dir app-mediakit-foodservice/content \
  --state-dir /tmp/foodservice-state \
  --module-id woodfine \
  --bind 127.0.0.1:9103 \
  --enable-mcp
```

## HTTP surface

| Route | Purpose |
|---|---|
| `GET /` | Home page |
| `GET /page/{slug}` | Named page |
| `GET /es` | Home page (Spanish) |
| `GET /es/page/{slug}` | Named page (Spanish) |
| `GET /healthz` | Health check |
| `POST /api/mcp` | MCP JSON-RPC 2.0 (when `--enable-mcp`) |
| `GET /api/pending` | List proposals awaiting approval |
| `POST /api/pending/{id}/approve` | Approve (F12) |

## Build and test

```
cargo check -p app-mediakit-foodservice
cargo test -p app-mediakit-foodservice
cargo clippy -p app-mediakit-foodservice -- -D warnings
```

## Status

P1 scaffold: crate structure, stub content pages (home, contact, disclaimer).
Live site `foodservice.woodfinegroup.com` was unreachable at scaffold time
(2026-06-24) — content was not migrated. Full content authoring is a later
phase; see the archive BRIEF.
