# CLAUDE.md — service-email

> **State:** Active  —  **Last updated:** 2026-04-25
> **Version:** 0.0.1  (per `~/Foundry/CLAUDE.md` §7 and DOCTRINE.md §VIII)
> **Registry row:** `pointsav-monorepo/.claude/rules/project-registry.md`
>
> When state changes, update this header AND the registry row in the
> same commit. Drift between the two is a documentation defect.
>
> Per-commit: bump PATCH; tag `vservice-email-MAJOR.MINOR.PATCH`
> annotated and SSH-signed; commit message ends with
> `Version: M.m.P` trailer; `CHANGELOG.md` records one line per PATCH.

---

## What this project is

Ring 1 boundary-ingest service: the per-tenant Communications
Ledger. Pulls messages from upstream Microsoft Exchange mailboxes,
parses `.eml` payloads, attaches them to identities resolved
through `service-people`, and writes them through `service-fs`
into the per-tenant WORM Immutable Ledger. Read downstream by
`service-extraction` (Ring 2) via MCP wire protocol.

Sibling crates in this cluster carry the protocol-specific
adapters and ancillary functions:

- `service-email-egress-ews/` — EWS protocol adapter (working
  EWS auth + SOAP payload reference)
- `service-email-egress-imap/` — IMAP protocol adapter
- `service-email-template/` — message-template rendering

The two adapter crates are deliberately separate per the registry
note (2026-04-23 audit reversed an earlier consolidation plan —
they are protocol-specific implementations, not duplicates).

## Current state

**EWS auth rebase complete (2026-04-26).** The former inline OAuth
`client_credentials` + Graph REST path has been replaced:

- `src/auth.rs` — `EwsCredentials::from_env()` reads
  `AZURE_ACCESS_TOKEN`, `EXCHANGE_TARGET_USER`, optional
  `EWS_ENDPOINT`. Token is pre-acquired out-of-process; the daemon
  does not run the OAuth handshake.
- `src/ews_client.rs` (formerly `graph_client.rs`) — `EwsClient`
  implementing FindItem / GetItem (IncludeMimeContent) / UpdateItem
  via EWS SOAP over HTTPS with bearer auth. String-based XML parsing
  (no extra dep); base64 decode of MimeContent.
- `src/main.rs` — polling daemon loop using `EwsCredentials` +
  `EwsClient` + `MaildirVault`.
- `Cargo.toml` — reqwest with `rustls-tls` (no openssl-sys); base64
  dep added; serde/serde_json removed; `[workspace]` added for
  standalone crate isolation.
- Six unit tests pass clean (`cargo test --manifest-path
  service-email/Cargo.toml`).

Next: `sovereign-splinter/` rename (Do-Not-Use "sovereign" prefix) +
`ingress-harvester/` / `master-harvester-rs/` retirement.

Pre-framework sub-directory inventory (2026-04-26; decisions in NEXT.md):

- `ingress-harvester/` — Rust async; EWS SOAP email harvester using
  inline OAuth client_credentials from `auth-credentials.env` +
  hardcoded folder IDs; retire-pending (deprecated auth pattern)
- `master-harvester-rs/` — Rust async; Graph API email fetcher with
  dynamic folder discovery + BATCH_SIZE=3 micro-batching; retire-pending
  (Graph API approach deprecated by EWS rebase; folder-discovery +
  micro-batching patterns worth porting)
- `sovereign-splinter/` — Rust binary; mailparse-based `.eml` parser
  that routes to `service-people/discovery-queue` (identity signals) +
  `service-slm/transient-queues` (body text) + `assets/inert-media`
  (attachments); "sovereign" prefix is Do-Not-Use; core parsing logic
  kept — superseded routing will be replaced by MCP append calls
- `scripts/` — correctly placed per repo-layout.md; contains
  `spool-daemon.sh` (watches maildir/new/, calls sovereign-splinter)
- `docs/TEMPLATE_INDEX_MSFT_ENTRA_ID.md` — Entra ID auth index
  template; moved from root to docs/ (repo-layout.md compliance)

Inventory + decisions (keep / rename / retire / relocate) for those
items run alongside the auth rebase.

## Build and test

```
cargo check --manifest-path service-email/Cargo.toml
cargo test  --manifest-path service-email/Cargo.toml
```

Six unit tests in `ews_client.rs` cover XML parsing helpers and
base64 round-trip; all pass clean. End-to-end ingestion testing
requires a real Exchange mailbox + valid `AZURE_ACCESS_TOKEN`;
no automated harness for that exists today.

## File layout

```
service-email/
├── Cargo.toml
├── README.md, README.es.md
├── CLAUDE.md, NEXT.md
├── src/
│   ├── main.rs         — Tokio async daemon loop
│   ├── auth.rs         — EwsCredentials::from_env() (AZURE_ACCESS_TOKEN)
│   ├── ews_client.rs   — EwsClient: FindItem / GetItem / UpdateItem SOAP
│   └── maildir.rs      — MaildirVault writer (transition sink -> service-fs)
├── docs/
│   └── TEMPLATE_INDEX_MSFT_ENTRA_ID.md  — Entra ID auth index template
├── ingress-harvester/  — pre-framework; retire-pending (inline OAuth pattern)
├── master-harvester-rs/ — pre-framework; retire-pending (Graph API deprecated)
├── sovereign-splinter/ — pre-framework; keep parsing core; Do-Not-Use prefix
└── scripts/
    └── spool-daemon.sh — maildir watcher; calls sovereign-splinter
```

## Hard constraints — do not violate

- **ADR-07: zero AI in Ring 1.** No LLM-assisted parsing, no
  embedding-based message classification, no AI-driven sender
  resolution. Identity resolution is delegated to `service-people`
  (also Ring 1, also deterministic).
- **WORM via `service-fs`.** Once the rebase lands, persisted
  message bodies go through `service-fs`'s MCP append surface, not
  to local disk. The current `maildir::MaildirVault` writes to a
  configured `TOTEBOX_ARCHIVE_PATH` — that's a transition surface,
  not the long-term shape.
- **Per-tenant boundary.** One process per `moduleId`. No
  cross-tenant mailbox ingestion.
- **Auth runs out-of-process.** Per the operator-confirmed EWS
  pattern, `AZURE_ACCESS_TOKEN` is consumed from env; the daemon
  does not perform the OAuth handshake inline. Token refresh is
  upstream concern.
## Dependencies on other projects

- Reads from: upstream Microsoft Exchange (per-tenant mailboxes
  via EWS).
- Resolves identities through: `service-people` (Ring 1, this
  cluster) — sender/recipient → canonical identity.
- Writes to: `service-fs` (Ring 1, this cluster) — every parsed
  `.eml` lands in the WORM ledger.
- Read by: `service-extraction` (Ring 2, `project-slm` cluster) —
  reads ledger entries via MCP.
- Sibling reference: `service-email-egress-ews/` — the EWS auth +
  SOAP payload reference for the rebase. Consumption mode (cargo
  path-dep vs pattern lift) is a NEXT.md decision.

## What not to do

- Do not introduce a second OAuth client_credentials flow in this
  crate. The EWS rebase moves auth out-of-process; reintroducing
  inline auth is the very drift this rebase closes.
- Do not bypass `service-people` for sender/recipient resolution.
  Identity is a Ring 1 concern owned by `service-people`; this
  crate consumes its API, does not duplicate the schema.
- Do not couple `service-email`'s Cargo.toml to all four
  `service-email-egress-*` sub-crates wholesale. Pick the minimum
  surface needed for the EWS auth rebase; consume more only when
  a concrete need surfaces.
- Do not delete or rewrite the pre-framework sub-directories
  (`ingress-harvester/`, `master-harvester-rs/`,
  `sovereign-splinter/`) until they are inventoried. Some may
  carry the right thinking for the rebase.

---

## Inherited rules — do not duplicate, do not silently override

This project inherits rules from two parent scopes. Do NOT copy
their content into this file; reference them.

- **Repo-level:** `pointsav-monorepo/CLAUDE.md` (when added; the
  monorepo does not yet carry a repo-level `CLAUDE.md` — see
  `~/Foundry/NEXT.md` Stage 4) — prefix taxonomy, canonical names,
  ADR hard rules (SYS-ADR-07, -10, -19), Do-Not-Use vocabulary,
  bilingual README rule, BCSC / Sovereign Data Foundation
  disclosure.
- **Workspace-level:** `~/Foundry/CLAUDE.md` — identity store,
  commit flow (`bin/commit-as-next.sh`), promotion flow
  (`bin/promote.sh`), authoritative-document priority, rules of
  engagement.

If a rule at this level conflicts with an inherited rule, **stop
and surface the conflict** — do not silently override.
