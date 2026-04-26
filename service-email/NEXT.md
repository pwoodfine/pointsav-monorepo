# NEXT.md — service-email

> Last updated: 2026-04-25
> Read at session start. Update before session end so the next
> session knows where to pick up.

---

## Right now

- **`sovereign-splinter/` rename.** Cargo.toml `name` field is
  `sovereign-splinter` — "sovereign" prefix is Do-Not-Use per
  workspace conventions. Rename to `email-splitter` (or fold into
  `service-email/src/` as the `.eml` parsing module). The
  `scripts/spool-daemon.sh` binary path reference updates with it.

## Queue

- **`ingress-harvester/` + `master-harvester-rs/` retirement.**
  Both use the deprecated in-process OAuth pattern. EWS rebase is
  now live in `service-email/src/`; retire these once the new path
  is confirmed end-to-end. The micro-batching concept from
  `master-harvester-rs/` (BATCH_SIZE=3) is worth preserving in the
  daemon loop (currently polls at 60s with one message per tick).
- Decide consumption mode for the EWS code: workspace path-dep on
  the relevant `service-email-egress-ews/` sub-crate, or lift the
  pattern into `service-email/src/`. The lift approach has been
  taken in this rebase (pattern copied from egress-roster/ews_payload.xml
  reference); path-dep can revisit if the egress crates' Cargo
  name-vs-directory mismatches are resolved.
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

- 2026-04-26: **EWS auth rebase complete.**
  `src/auth.rs` — replaced inline OAuth2 `client_credentials` flow
  with `EwsCredentials::from_env()` reading `AZURE_ACCESS_TOKEN` +
  `EXCHANGE_TARGET_USER` + optional `EWS_ENDPOINT` from env.
  `src/graph_client.rs` → renamed to `src/ews_client.rs` via
  `git mv`; fully rewritten as `EwsClient` implementing three EWS
  SOAP operations (FindItem, GetItem with IncludeMimeContent, UpdateItem
  IsRead=true). `src/main.rs` — daemon loop rewritten using
  `EwsCredentials` + `EwsClient`; reads `TOTEBOX_ARCHIVE_PATH`;
  includes 50ms anti-throttle pause between per-message EWS calls.
  `Cargo.toml` — removed `serde`/`serde_json`; changed reqwest to
  `rustls-tls` (avoids openssl-sys blocker); added `base64 = "0.22"`;
  added `[workspace]` table (standalone crate isolation). Six unit
  tests cover XML parsing helpers and base64 round-trip; all pass
  clean.

- 2026-04-26: **pre-framework subdirectory inventory complete.**
  Four subdirectories + one root template assessed; decisions:
  | Item | Decision |
  |---|---|
  | `ingress-harvester/` | **Retire-pending** — Rust async; EWS SOAP harvester but with inline OAuth `client_credentials` (deprecated pattern); hardcoded folder IDs. Retire once EWS rebase lands. |
  | `master-harvester-rs/` | **Retire-pending** — Rust async; Graph API (deprecated); dynamic folder discovery + micro-batching (BATCH_SIZE=3) concepts worth porting to rebased daemon. |
  | `sovereign-splinter/` | **Keep core; rename** — Rust binary; mailparse-based `.eml` parser; routing logic (maildir → service-people/discovery-queue + service-slm/transient-queues + assets/inert-media) superseded by MCP append calls. "sovereign" prefix is Do-Not-Use → queue rename to `email-splitter`. |
  | `scripts/` | **Correctly placed** — `spool-daemon.sh` already in `scripts/` per repo-layout.md. Calls sovereign-splitter binary; update path reference when renamed. |
  | `TEMPLATE_INDEX_MSFT_ENTRA_ID.md` | **Relocated** — Moved from repo root to `docs/` (repo-layout.md compliance). Done this session. |

- 2026-04-25: project activated per `~/Foundry/CLAUDE.md` §9 —
  this CLAUDE.md, this NEXT.md, and the registry row created in
  one commit; existing `src/auth.rs` (Graph OAuth) flagged as
  drift in CLAUDE.md "Current state" with the EWS rebase queued
  as Right-now per operator decision.
