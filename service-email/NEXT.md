# NEXT.md — service-email

> Last updated: 2026-04-25
> Read at session start. Update before session end so the next
> session knows where to pick up.

---

## Right now

- Rebase `src/auth.rs` + `src/graph_client.rs` onto the EWS-based
  MSFT auth pattern proven in the sibling
  `service-email-egress-ews/` project (operator decision
  2026-04-25). Specifically: replace the inline OAuth
  `client_credentials` flow with `AZURE_ACCESS_TOKEN` consumed
  from env (per `service-email-egress-ews/template.env` and the
  pattern in `service-email-egress-ews/egress-ingress/src/main.rs`
  + `service-email-egress-ews/egress-roster/src/main.rs`); replace
  Graph REST calls with EWS SOAP requests using
  `service-email-egress-ews/egress-roster/ews_payload.xml` as the
  envelope reference.

## Queue

- Decide consumption mode for the EWS code: workspace path-dep on
  the relevant `service-email-egress-ews/` sub-crate, or lift the
  pattern into `service-email/src/`. Path-dep is preferred (single
  source of truth) if the sub-crate's Cargo manifest can be made
  workspace-clean; pattern-lift is the fallback.
- Inventory the four `service-email-egress-ews/` sub-crates
  (`egress-ingress`, `egress-roster`, `egress-archive-ews`,
  `egress-prune`, `egress-balancer`, `egress-ledger`) and identify
  the minimum surface `service-email` needs to import. The
  separate registry note (2026-04-23) flags
  Cargo.toml-name-vs-directory-name mismatches in this area —
  beware those when wiring the dependency.
- Inventory the four pre-framework sub-directories in this crate
  (`ingress-harvester/`, `master-harvester-rs/`,
  `sovereign-splinter/`, `scripts/`) and the
  `TEMPLATE_INDEX_MSFT_ENTRA_ID.md` index template. Decide
  per-item: keep, rename, retire, or relocate.
- Replace `maildir::MaildirVault` writes with `service-fs` MCP
  append calls. The `MaildirVault` is a transition-period sink;
  the long-term shape persists message bodies through the WORM
  ledger.
- MCP server interface — expose ingest as a tool, surface read
  views as resources. One moduleId per process.
- Add `service-email` as a workspace member in the monorepo root
  `Cargo.toml` once the EWS rebase compiles cleanly (Layer 1
  audit finding in `.claude/rules/cleanup-log.md` 2026-04-18).
- Fixture-based test for SOAP payload serialisation — known-good
  XML round-trips through the EWS request constructor. Avoids
  needing a live mailbox for unit-level coverage.

## Blocked

- End-to-end ingestion testing — Blocked on: needs a real
  Exchange mailbox + valid `AZURE_ACCESS_TOKEN`. No automated
  harness today; the fixture-based payload test above is the
  closer alternative.
- Workspace `Cargo.toml` membership — Blocked on: monorepo
  workspace under-declaration is a separate cleanup
  (`.claude/rules/cleanup-log.md` 2026-04-18). The EWS rebase can
  land standalone first; member declaration follows.

## Deferred

- IMAP path through `service-email-egress-imap/` — Deferred:
  operator's 2026-04-25 instruction is EWS specifically. IMAP
  remains as a sibling adapter, not consumed from this crate
  unless a customer use case surfaces it.
- Outbound message sending — Deferred: this crate is the
  Communications Ledger (ingest path). Outbound message synthesis,
  if it ever lands, is a separate `service-email-template`
  concern downstream of the ledger.

## Recently done

- 2026-04-25: project activated per `~/Foundry/CLAUDE.md` §9 —
  this CLAUDE.md, this NEXT.md, and the registry row created in
  one commit; existing `src/auth.rs` (Graph OAuth) flagged as
  drift in CLAUDE.md "Current state" with the EWS rebase queued
  as Right-now per operator decision.
