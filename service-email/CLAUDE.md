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

**Auth surface in src/ is drift — operator-confirmed rebase
target.** The existing `src/main.rs` + `src/auth.rs` +
`src/graph_client.rs` implement an in-process OAuth
`client_credentials` flow against `login.microsoftonline.com`
with scope `https://graph.microsoft.com/.default`, and call
Microsoft Graph REST endpoints. **Operator decision 2026-04-25:**
service-email should reuse the EWS-based MSFT authentication
pattern that is already working in the sibling
`service-email-egress-ews/` project, not the in-src Graph OAuth
flow.

What "EWS-based MSFT authentication" means in this repo, based on
the inventory of `service-email-egress-ews/`:

- Access token is pre-acquired (out-of-process) and passed in via
  `AZURE_ACCESS_TOKEN` env var (alongside `EXCHANGE_TARGET_USER`
  and per-tenant config in `template.env`).
- The Rust process consumes the token; it does not run the OAuth
  handshake inline. This decouples auth refresh from the daemon
  loop and matches the pattern used in
  `service-email-egress-ews/egress-ingress/src/main.rs` and
  `service-email-egress-ews/egress-roster/src/main.rs`.
- For mailbox protocol surface, `service-email-egress-ews/egress-
  roster/ews_payload.xml` is the EWS SOAP envelope reference
  (with `ExchangeImpersonation` of the target user).

The Tokio async runtime model already in `service-email/src/main.rs`
is fine — it matches the ratified Ring 1 hosted-process intent
(`~/Foundry/conventions/zero-container-runtime.md`). Only the
auth/protocol surface changes.

Other pre-framework artefacts in this directory (uninventoried):

- `ingress-harvester/` — sub-directory; uninventoried
- `master-harvester-rs/` — sub-directory; uninventoried
- `sovereign-splinter/` — sub-directory; uninventoried
- `scripts/` — sub-directory; uninventoried (likely shell helpers)
- `TEMPLATE_INDEX_MSFT_ENTRA_ID.md` — index template; review-and-decide

Inventory + decisions (keep / rename / retire / relocate) for those
items run alongside the auth rebase.

## Build and test

`cargo check` inside this directory builds against the existing
Cargo.toml (tokio, reqwest, serde, uuid, chrono). It will continue
to compile during the rebase if changes are sequenced (touch
`auth.rs` and `graph_client.rs` together; keep `main.rs`
compatible until both halves of the new wire path are in place).

End-to-end testing of the rebased path needs a real Exchange
mailbox + valid `AZURE_ACCESS_TOKEN`; no automated harness for
that exists today. Add a fixture-based test for the SOAP payload
serialisation so unit-level coverage doesn't depend on a live
mailbox.

## File layout

```
service-email/
├── Cargo.toml
├── README.md, README.es.md
├── TEMPLATE_INDEX_MSFT_ENTRA_ID.md   — index template; review-and-decide
├── CLAUDE.md, NEXT.md
├── src/
│   ├── main.rs                       — Tokio daemon loop (runtime model OK)
│   ├── auth.rs                       — Graph OAuth (REPLACE per EWS rebase)
│   ├── graph_client.rs               — Graph REST client (REPLACE per EWS rebase)
│   └── maildir.rs                    — Maildir vault writer (review for service-fs handoff)
├── ingress-harvester/                — pre-framework; uninventoried
├── master-harvester-rs/              — pre-framework; uninventoried
├── sovereign-splinter/               — pre-framework; uninventoried
└── scripts/                          — pre-framework; uninventoried
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
- **Do not delete the existing Graph code paths until the EWS
  rebase compiles and is reviewed.** The current path is what's
  working today; the rebase is sequenced, not a hard cutover.

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
