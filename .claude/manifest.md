---
schema: foundry-cluster-manifest-v1
cluster_name: project-data
cluster_branch: cluster/project-data
created: 2026-04-25
backfilled: 2026-04-26
state: active
clones:
  - repo: pointsav-monorepo
    role: primary
    path: ./
    upstream: vendor/pointsav-monorepo
deployment_instance: null
trajectory_capture: pending
---

# Cluster manifest — project-data

Single-clone cluster (N=1). Backfilled 2026-04-26 per Doctrine
v0.0.2 §IV.c.

## Scope

Ring 1 of the three-ring architecture (boundary ingest;
per-tenant; MCP-server processes; ADR-07: zero AI):

- `service-fs` — Immutable WORM ledger (Day-1 ingest target)
  - Status: **Active** since 2026-04-25 (`ee209e3`)
  - Drift surfaced 2026-04-26 (00:10 UTC outbox to Master): the
    existing `src/main.rs` is a `#![no_std]` bare-metal seL4
    unikernel scaffold contradicting the ratified Ring 1 shape
  - Master ratified rewrite direction 2026-04-26 (see inbox);
    rewrite + `vendor-sel4-fs/` relocation are next session's
    work
- `service-input` — Generic document / file ingestion
  - Status: **Active** since 2026-04-25 (`fa1f71e`, `1490e27`)
  - Created in this cluster (no prior directory or registry row)
  - Parser-dispatcher scaffold queued
- `service-people` — Identity Ledger
  - Status: **Active** since 2026-04-25 (`c45b308`)
  - Pre-framework subdirectories (`sovereign-acs-engine`,
    `spatial-crm`, `spatial-ledger`, `substrate`, `tools`) need
    inventory + keep/rename/retire/relocate decisions
- `service-email` — Communications Ledger (.eml parsing,
  EWS / IMAP egress, Microsoft Graph integration)
  - Status: **Active** since 2026-04-25 (`032afe8`)
  - Drift surfaced: `src/auth.rs` + `src/graph_client.rs` use
    in-process Graph OAuth `client_credentials`; operator
    decided 2026-04-25 to rebase onto the EWS-based MSFT auth
    pattern from sibling `service-email-egress-ews/`
  - Pre-framework subdirectories (`ingress-harvester`,
    `master-harvester-rs`, `sovereign-splinter`, `scripts`)
    need inventory

## Branch

`cluster/project-data` (created 2026-04-25 from local upstream
`main`).

## Remotes (within the clone)

- `origin` — canonical via admin SSH alias
- `origin-staging-j` — Jennifer's staging-tier mirror
- `origin-staging-p` — Peter's staging-tier mirror

Push policy: staging-tier only. Never push to `origin`. Per
Doctrine §V Action Matrix and v0.0.10 auto-mode safety brief.

## Trajectory capture

Pending. `bin/capture-edit.sh` and `bin/capture-trajectory.sh` will
be installed by Master in a v0.1.x increment per
`conventions/trajectory-substrate.md` L1–L2. Until then, commits
are not yet flowing to corpus; behavioural changes for Task: none.

## Cross-cluster coordination

- `project-slm` cluster runs Ring 2+3 (`service-slm`,
  `service-content`, `service-extraction`, `service-search`).
- `service-content` (project-slm cluster) eventually consumes
  `service-fs` schemas via MCP. Schema design coordination via
  outbox to Master, who relays.

## Mailbox

- Inbox: `~/Foundry/clones/project-data/.claude/inbox.md`
- Outbox: `~/Foundry/clones/project-data/.claude/outbox.md`
- Trajectory log: `~/Foundry/clones/project-data/.claude/trajectory-log.md`
  (created on first capture)

## State as of backfill (2026-04-26)

| Item | State |
|---|---|
| Cluster activation (4 projects) | Done 2026-04-25 |
| service-fs runtime-model drift surfaced | Yes — outbox 2026-04-26 |
| Master ratification of rewrite direction | Done 2026-04-26 (inbox) |
| service-fs hosted Tokio MCP-server rewrite | Pending — next session |
| `vendor-sel4-fs/` relocation | Pending — next session |
| Workspace `Cargo.toml` `[members]` update | Pending — next session |
| service-email EWS auth rebase | Pending — Right-now |
| Pre-framework subdir inventory (people + email) | Pending — Queue |

## Day-1 priority order (per
`conventions/customer-first-ordering.md`)

1. `service-fs` — WORM ledger; everything writes through it
2. `service-input` — parsers; ingest path needs to work
   end-to-end through service-fs
3. `service-people`, `service-email` — domains attach later

---

*Backfilled 2026-04-26 in workspace v0.1.0 / Doctrine v0.0.2.*
