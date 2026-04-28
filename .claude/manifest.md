---
schema: foundry-cluster-manifest-v1
cluster_name: project-data
cluster_branch: cluster/project-data
created: 2026-04-25
backfilled: 2026-04-26 (manifest schema), 2026-04-26 (triad per Doctrine v0.0.4), 2026-04-28 (triad → tetrad per Doctrine v0.0.10 / claim #37)
state: active

# Project Tetrad Discipline per Doctrine claim #37 + Doctrine v0.0.10.
# Upgraded from triad on 2026-04-28 by adding the wiki leg below.
# See ~/Foundry/conventions/project-tetrad-discipline.md.
tetrad:
  vendor:
    - repo: pointsav-monorepo
      path: ./
      upstream: vendor/pointsav-monorepo
      focus: service-fs/, service-people/, service-email/, service-input/ (Ring 1 services)
  customer:
    - fleet_deployment_repo: customer/woodfine-fleet-deployment
      catalog_subfolder: cluster-totebox-corporate/
      tenant: woodfine
      purpose: Ring-1-services-deployment-on-Customer-Totebox
      status: leg-pending — Task to draft GUIDEs for service-fs operator runbook (provision, daily-ops, recovery, decommission); first GUIDE-* lands when service-fs storage swap is testable
    - fleet_deployment_repo: vendor/pointsav-fleet-deployment
      catalog_subfolder: vault-privategit-source/
      tenant: pointsav
      purpose: Ring-1-services-deployment-on-the-workspace-VM (PointSav-as-tenant)
      status: leg-pending — same GUIDEs; pointsav-tenant variant; first GUIDE staged 2026-04-28 in drafts-outbound/ (guide-fs-anchor-emitter.draft.md)
  deployment:
    - path: /srv/foundry  # vault-privategit-source-1 (workspace VM as PointSav tenant Ring 1 instance)
      tenant: pointsav
      shape: long-running-service
      shared_with: [project-slm]
      runtime_artifacts:
        - (planned) /usr/local/bin/local-fs + /etc/systemd/system/local-fs.service
        - /var/lib/local-fs/ledger/ (per service-fs/ARCHITECTURE.md)
        - /usr/local/bin/fs-anchor-emitter + local-fs-anchor.{service,timer} (live since workspace v0.1.27; armed for 2026-05-01 monthly fire pending PD.1 v0.0.2 body-shape redeploy)
      status: leg-pending — Master to draft systemd unit at infrastructure/local-fs/ when Task confirms K4-equivalent ready; fs-anchor-emitter half is active
    - path: ~/Foundry/deployments/cluster-totebox-corporate-N/ (planned; -N when Customer Totebox provisioned)
      tenant: woodfine
      shape: corporate-archive
      shared_with: [project-orgcharts]
      status: leg-pending — provision when Woodfine Totebox is stood up; v0.5.0+ trajectory
  wiki:
    - repo: vendor/content-wiki-documentation
      drafts_via: clones/project-data/.claude/drafts-outbound/
      gateway: project-language Task
      planned_topics:
        # Substantive draft already staged 2026-04-28 (per Tetrad
        # backfill discipline; over-delivers vs. skeleton requirement)
        - topic-worm-ledger-architecture.md  # active — bulk draft staged 2026-04-28
        # Skeletons + future bulk drafts
        - topic-ring1-boundary-ingest.md  # planned — Phase 1A milestone TOPIC; overlaps with worm-ledger; may merge or split
        - topic-doctrine-invention-7-rekor-anchoring.md  # planned — operator-readable explanation of monthly Sigstore Rekor anchoring; year-shard rotation; TUF discovery
        - topic-identity-ledger-schema.md  # planned — UUIDv5(NAMESPACE_DNS, email) deterministic identity, Person record, Anchor/Claim pattern from people-acs-engine
        - topic-adr-07-zero-ai-in-ring-1.md  # planned — why Ring 1 deterministic-only, how it composes with Ring 3 Doorman boundary
      status: active — first bulk draft (topic-worm-ledger-architecture) staged 2026-04-28; awaiting project-language sweep + refinement

clones:
  - repo: pointsav-monorepo
    role: primary
    path: ./
    upstream: vendor/pointsav-monorepo
trajectory_capture: pending

adapter_routing:
  trains:
    - cluster-project-data       # own cluster adapter (Ring 1 services + WORM ledger)
    - engineering-pointsav       # Vendor engineering corpus
    # NOTE: future tenant-woodfine added when Customer Totebox runtime kicks in
    # (per Doctrine §IV.b strict tenant isolation; tenant adapter trains
    # inside the Customer Totebox, not in workspace)
  consumes:
    - constitutional-doctrine    # always
    - engineering-pointsav       # always — Vendor knowledge
    - cluster-project-data       # own cluster context
    - role-task                  # current role

# Reverse-Funnel Editorial Pattern triggers (Doctrine claim #35;
# conventions/cluster-wiki-draft-pipeline.md). When this cluster
# hits one of these milestones, stage a bulk draft at
# .claude/drafts-outbound/ for project-language to refine.
wiki_draft_triggers:
  - phase_milestone:
      description: substantive code milestone like "all four Ring 1 services have MCP servers + canonical schemas + at least one end-to-end test"
      typical_protocol: PROSE-TOPIC
      target_repo: content-wiki-documentation
  - architectural_decision_ratified:
      description: workspace-tier convention or Doctrine claim that originated as a project-data RESEARCH.md and was ratified by Master (e.g., worm-ledger-design.md, Doctrine Invention #7)
      typical_protocol: PROSE-TOPIC
      target_repo: content-wiki-documentation
  - service_activated:
      description: new Ring 1 service moves from Reserved-folder / Scaffold-coded to Active per project-registry; per-project README needs first authoring or substantive refresh
      typical_protocol: PROSE-README
      target_repo: pointsav-monorepo  # in-place README under the project dir
  - deployment_artifact_shipped:
      description: Master ships an IaC artifact for one of this cluster's services (binary installed, systemd unit + timer ARMED, etc.); operator runbook GUIDE warranted
      typical_protocol: PROSE-GUIDE
      target_repo: woodfine-fleet-deployment  # catalog GUIDE inside the deployment subfolder
  - schema_published:
      description: canonical schema (Identity Ledger, WORM checkpoint, Hashedrekord wrap) reaches stable shape and warrants public-facing TOPIC describing the contract
      typical_protocol: PROSE-TOPIC
      target_repo: content-wiki-documentation
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
