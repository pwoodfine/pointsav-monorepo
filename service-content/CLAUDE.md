# CLAUDE.md — service-content

> **State:** Active  —  **Last updated:** 2026-06-22
> **Registry row:** `pointsav-monorepo/.agent/rules/project-registry.md`

---

## What this project is

service-content is the DataGraph service: it watches CORPUS files from
service-extraction, extracts entities into LadybugDB (Tier A hardware) or
SQLite (Micro/e2-micro), and exposes them to the Doorman over HTTP.

Ring 3a read-side sibling to service-slm. The Doorman calls
`GET /v1/graph/context` before every inference to inject entity context
into system prompts. Write path (graph mutations) proxied through the
Doorman's `POST /v1/graph/mutate` endpoint.

## Current state

**Scaffold-coded** — binary deployed and running as `local-content.service`
on the workspace VM. HTTP 200 from `/healthz` with **7,445 entities** in
LadybugDB graph (2026-06-01).

| Feature | State |
|---|---|
| LadybugDB graph store (Tier A) | Live — 2GB buffer pool; 4G MemoryMax |
| SQLite graph store (Micro) | Code-complete; test-only |
| Taxonomy upsert (guides, archetypes, domain, topics) | Live on startup |
| Corpus drain loop | Live |
| Corpus watcher loop | Live |
| `/v1/graph/context` | Live — Doorman queries before inference |
| `/v1/graph/mutate` | Live — proxied through Doorman |
| ER alias table + in-batch entity resolution | Code-complete, 55/55 tests — deploys via Stage 6 |
| RelatedTo write path (`upsert_edges`) | Code-complete — deploy pending |
| `query_context` alias-aware canonical resolution | Code-complete — deploy pending |
| created_at first-write-wins (D9) | Code-complete — deploy pending |
| Extraction schema `additionalProperties:false` (D8) | Code-complete — deploy pending |
| Sprint 5: persistent `processed_ledgers` | **DEPLOYED** (commit `5ad06ec9`, 2026-06-01) — JSONL sidecar at `$SERVICE_CONTENT_GRAPH_DIR/processed_ledgers.jsonl` |
| Tier B (Yo-Yo) extraction | Deferred — circuit breaker open until Yo-Yo online |
| `/v1/draft/generate` | 503 pre-D4 (Doorman unconfigured for Tier C auth) |

## Build and test

```
cargo test -p service-content       # 55 tests
cargo clippy -p service-content --all-targets -- -D warnings
```

Full build (requires lbug cmake deps on system):

```
SERVICE_CONTENT_GRAPH_BACKEND=sqlite \
SERVICE_CONTENT_CORPUS_DIR=/tmp/corpus \
SERVICE_CONTENT_CRM_DIR=/tmp/crm \
    cargo run -p service-content
```

## Key env vars

| Var | Default | Purpose |
|---|---|---|
| `SERVICE_CONTENT_GRAPH_BACKEND` | `lbug` | `lbug` or `sqlite` |
| `SERVICE_CONTENT_LBUG_DB_PATH` | `/var/lib/local-content/graph/ladybug.db` | LadybugDB path |
| `SERVICE_CONTENT_LBUG_BUFFER_POOL_MB` | `64` | Buffer pool size in MB |
| `SERVICE_CONTENT_CORPUS_DIR` | (required) | Directory of CORPUS_*.json files |
| `SERVICE_CONTENT_CRM_DIR` | (required) | CRM taxonomy directory |
| `SERVICE_CONTENT_DOORMAN_ENDPOINT` | `http://127.0.0.1:9080` | Doorman for Tier B inference |

## Hard constraints

- **Do not bypass the Doorman.** All LLM inference goes through `service-slm`.
  SYS-ADR-07: structured data (entity extraction results) must never cross
  the external AI boundary — only prose payloads do.
- **Do not write to the graph from service-content directly** (Sprint 3
  pending): graph mutations should route through Doorman `/v1/graph/mutate`.

## Pending (open AUTO-TODO items)

- Sprint 3 (PUSH inversion): delete PULL path, queue graph mutations in Doorman
- Sprint 4: move `/v1/draft/generate` to slm-doorman-server
- `is_already_processed` integration test on LbugGraphStore (SQLite-only now)
- memory.conf + crash-loop-guard.conf tracked in infrastructure/ (Command scope)

## Dependencies

- **service-slm** — Doorman at `SERVICE_CONTENT_DOORMAN_ENDPOINT` for Tier B
- **lbug v0.16** — embedded graph DB; cmake required for from-source build
- **foundry-nodeclass** — detects Micro/Hardware/Accelerated node class
