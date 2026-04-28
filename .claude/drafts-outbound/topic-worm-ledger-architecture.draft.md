---
schema: foundry-draft-v1
state: draft-pending-language-pass
originating_cluster: project-data
target_repo: content-wiki-documentation
target_path: ./
target_filename: topic-worm-ledger-architecture.md
audience: vendor-public
bcsc_class: current-fact
language_protocol: PROSE-TOPIC
authored: 2026-04-27T17:30:00Z
authored_by: task-project-data (session e509c13609b4b632)
authored_with: opus-4-7
references:
  - ~/Foundry/conventions/worm-ledger-design.md
  - ~/Foundry/conventions/three-ring-architecture.md
  - ~/Foundry/conventions/zero-container-runtime.md
  - ~/Foundry/clones/project-data/service-fs/ARCHITECTURE.md
  - ~/Foundry/clones/project-data/service-fs/SECURITY.md
  - ~/Foundry/clones/project-data/service-fs/RESEARCH.md
  - ~/Foundry/DOCTRINE.md  # §II.7 Invention #7; §IX SOC 2 + external WORM standards
  - ~/Foundry/MEMO-2026-03-30-Development-Overview-V8.md  # §6.3 + §7
  - https://c2sp.org/tlog-tiles
  - https://c2sp.org/signed-note
  - https://datatracker.ietf.org/doc/html/rfc9162  # CT v2 (RFC 9162)
  - https://blog.sigstore.dev/rekor-v2-ga/  # Rekor v2 GA
  - https://github.com/sigstore/rekor-tiles
  - https://www.sec.gov/files/rules/final/2022/34-96034.pdf  # SEC 17a-4 2022 amendment
  - https://eur-lex.europa.eu/eli/reg_impl/2025/1946/oj  # eIDAS qualified preservation
  - https://www.etsi.org/deliver/etsi_ts/119500_119599/119511/  # ETSI TS 119 511
  - https://www.etsi.org/deliver/etsi_en/319400_319499/319401/  # ETSI EN 319 401 v3.2.1
notes_for_editor: |
  This is the high-value substantive cluster contribution from project-data.
  Material is already ratified at workspace tier (worm-ledger-design.md
  is the formal convention; ARCHITECTURE.md + SECURITY.md were accepted by
  Master 2026-04-26T10:35Z). Status mapping for refinement:

    - Sections 1-9 are CURRENT-FACT — implemented in this cluster's code
      (commits ee209e3, 1e86047, 10a7dd0, b4ae62d-area, 6262d10, fc03e57).
    - Section 10 (External standards alignment) cites the standards but
      does NOT claim certification. Per BCSC §6 rule 5: "what we DO, not
      what we say." Phrase as structural alignment with named regulatory
      schemes, not as a compliance claim. Pare to that posture.
    - Section 11 (Long-term trajectory) is FORWARD-LOOKING — wrap with
      planned/intended/may language + cautionary banner per BCSC §6 rule 1.
      Material assumptions: seL4 unikernel migration is a v1.0.0+ trajectory
      item per MEMO §7; moonshot-database is named in the Active Sovereign
      Replacements table; both are conditional on Sovereign Substrate
      Phase 2 work landing.

  Project-data's structural position: this is the cluster's most cohesive
  external-facing TOPIC. The Ring 1 boundary-ingest TOPIC (#10 in our
  task queue) overlaps with sections 1-2 here; recommend folding any
  Ring 1 standalone TOPIC into this one rather than splitting (the WORM
  ledger IS the substrate the four Ring 1 services share, so the
  architectural story is more cohesive together).

  Bilingual pair: project-language generates the .es.md per DOCTRINE §XII
  strategic-adaptation pattern (Spanish overview, not 1:1 translation).
  Suggested Spanish title: "El Substrato WORM de Foundry — Arquitectura
  del Ledger Inmutable de Cuatro Capas". Full Spanish overview should
  emphasise (1) las cuatro capas, (2) los dos sobres de arranque
  Linux/seL4, (3) la trazabilidad pública vía Sigstore Rekor.

  Citations to resolve to citation-substrate IDs:
    - C2SP tlog-tiles → c2sp-tlog-tiles
    - C2SP signed-note → c2sp-signed-note
    - RFC 9162 → rfc-9162
    - SEC 17a-4(f) → sec-17a-4-f
    - eIDAS 2025/1946 → eidas-qualified-preservation
    - ETSI TS 119 511 → etsi-ts-119-511
    - ETSI EN 319 401 → etsi-en-319-401
    - CEN TS 18170:2025 → cen-ts-18170-2025
    - Sigstore Rekor v2 → sigstore-rekor-v2
    - Trillian-Tessera → trillian-tessera
    - DOCTRINE Invention #7 → doctrine-invention-7
  All present in ~/Foundry/citations.yaml per citation-substrate convention.

  Length / depth: leave the technical depth in. Production audience can
  drill into RESEARCH.md for full alternatives + sources. This TOPIC
  should orient a technically literate reader to the architecture in one
  read; assume they will follow citations to the specs themselves.
---

# Foundry's WORM Ledger Substrate — Four-Layer Architecture, Two Boot Envelopes

Foundry's `service-fs` is the per-tenant Write-Once-Read-Many (WORM) immutable ledger that all customer-facing data lands in first. Every other Ring 1 boundary-ingest service in Foundry — the identity ledger (`service-people`), the communications ledger (`service-email`), the document ingest service (`service-input`) — writes through it. Every Ring 2 knowledge-extraction service reads from it via cursor-paged MCP queries. There is exactly one `service-fs` process per tenant `moduleId`; cross-tenant access is structurally impossible.

This document describes the four-layer architecture that makes that work, the two operational envelopes (Linux/BSD daemon today, seL4 Microkit unikernel long-term), the substrate-level alignment with named external WORM standards, and how Doctrine Invention #7 (monthly Sigstore Rekor anchoring) closes the public-verifiability loop.

## 1. Position in the system

`service-fs` sits at the boundary between the per-tenant data plane and the multi-tenant knowledge plane. The Three-Ring Architecture convention (`~/Foundry/conventions/three-ring-architecture.md`) names this position explicitly: Ring 1 services are MCP-server processes per tenant; the WORM ledger is the durable backbone they share within a tenant.

```
                    Ring 2 (multi-tenant via moduleId)
                    service-extraction (reader)
                              ▲
                              │ MCP read (cursor-paged)
                              │
              ┌───────────────┴───────────────┐
              │     service-fs                 │
              │     WORM Immutable Ledger      │
              │     per-tenant moduleId        │
              └───────────────▲───────────────┘
                              │ MCP append
              ┌───────────────┼───────────────┐
              │               │               │
       service-people  service-input    service-email
       (Ring 1)        (Ring 1)         (Ring 1)
```

Per-tenant boundary today: one daemon process per `moduleId` (separate process address spaces); filesystem permissions restricting per-tenant `FS_LEDGER_ROOT` access; request-time `X-Foundry-Module-ID` header check rejects mismatched callers with 403. Per-tenant boundary long-term: seL4 microkernel-level capability enforcement, formally verified (per MEMO §7 trajectory and the moonshot kernel substrate work).

## 2. The four-layer stack

The architecture is intentionally layered so each layer is independently swappable. The middle layer (L2) is the durable Rust trait contract that survives changes above and below it. The substrate-level four-layer pattern is governed by the workspace convention `~/Foundry/conventions/worm-ledger-design.md` (ratified workspace v0.1.7 / Doctrine v0.0.3, commit `6c0b79a`); this document describes how `service-fs` specifically applies it.

### L1 — Tile storage primitive (envelope-specific)

The on-disk format is **C2SP tlog-tiles** (https://c2sp.org/tlog-tiles) — the same format used by RFC 9162 v2 (Certificate Transparency v2), Trillian-Tessera, and Sigstore Rekor v2. Tiles are static text files containing 256 entries (or 256 hashes at intermediate Merkle levels), base64-encoded for plain-text inspection per Doctrine Pillar 1 (DARP — direct, auditable, reproducible, plain-text).

Concrete on-disk layout per tenant:

```
$FS_LEDGER_ROOT/<moduleId>/
├── checkpoint            — latest signed tree head (signed-note format)
├── tile/0/x000.b64       — leaf tile 0, entries 0–255
├── tile/0/x001.b64       — leaf tile 1, entries 256–511
├── tile/1/x000.b64       — height-1 tile, hashes covering 256 leaves each
├── tile/2/x000.b64       — height-2 tile, hashes covering 65,536 leaves each
└── audit-log/
    ├── checkpoint        — separate sub-ledger for ADR-07 read events
    └── tile/0/x000.b64
```

Today's L1 implementation is `PosixTileLedger` — POSIX files under `FS_LEDGER_ROOT`, atomic write-then-rename per tile, fsync after each tile and each checkpoint, finalised tiles marked read-only (mode 0o444). Long-term replacement: `MoonshotDatabaseLedger` over capability-mediated IPC to the `moonshot-database` substrate per MEMO §7. The bytes are identical — only the I/O mechanism differs.

### L2 — WORM Ledger API (Rust trait, target-independent)

The `LedgerBackend` Rust trait is the durable API contract. Today's surface: `open` / `append` / `read_since` / `root` / `checkpoint` / `verify_inclusion` / `verify_consistency`. The append-only invariant lives at the API surface — no public method removes or modifies an entry. Audit-log sub-ledger for ADR-07 read tracking lives behind the same trait (one trait, two instances per tenant).

Two implementations exist today: `InMemoryLedger` (test fixture) and `PosixTileLedger` (production). Both pass an identical 18-test trait-surface suite. Future implementations slot in behind the same trait without touching wire-layer or boot code.

### L3 — Wire protocol (per-tenant, MCP-aware)

Today: axum 0.7 HTTP routes — `GET /healthz`, `GET /readyz`, `GET /v1/contract`, `POST /v1/append`, `GET /v1/entries?since=N`, `GET /v1/checkpoint`, `POST /mcp` for the MCP-server interface. The `X-Foundry-Module-ID` header enforcement runs before any business handler; mismatch returns 403 with the expected vs supplied moduleId in the body. Same wire shape across both boot envelopes.

Long-term: a fuller MCP-server interface — MCP resources for `/v1/checkpoint` and per-tile reads, MCP tools for `/v1/append`. Both wire surfaces (HTTP and MCP) coexist; clients use whichever fits their integration pattern.

### L4 — Anchoring (workspace-tier, monthly)

Doctrine Invention #7 — Sigstore Rekor v2 anchoring of per-tenant checkpoints. A monthly cron bundles each tenant's latest signed checkpoint, submits to `https://log2025-1.rekor.sigstore.dev/api/v2/log/entries` (Rekor v2 GA per https://blog.sigstore.dev/rekor-v2-ga/ — year-sharded; URL rotates annually), and writes the returned tlog entry back into the originating tenant's ledger as an `anchor-rekor-<unix-ts>` payload. The anchor flow is independent of request-time work — the daemon is stateless about anchoring; it just produces signed checkpoints that the workspace anchoring run consumes.

The implementation (`service-fs/anchor-emitter/`) is a standalone Rust binary (own `[workspace]` to avoid openssl-sys conflicts in the parent monorepo). It uses `reqwest` blocking + rustls-tls (no tokio in this binary), generates an ephemeral Ed25519 keypair per anchor (the value being anchored is the Rekor timestamp + inclusion proof, not key identity), and exits with structured codes (0 success / 1 config / 2 fetch / 3 Rekor / 4 append). Master operates the systemd unit (`local-fs-anchor.{service,timer}` with `OnCalendar=*-*-01 02:30:00`, `Persistent=true`, `RandomizedDelaySec=900`); Customer Toteboxes get the same unit pinned at their per-tenant `FS_LEDGER_ROOT`.

## 3. The two boot envelopes

`service-fs` is intended to ship in two envelopes that share the same wire protocol and same storage format.

### Envelope A — Linux/BSD daemon under systemd (today)

Tokio async runtime, axum 0.7 HTTP server, std Rust. POSIX storage in `FS_LEDGER_ROOT/<moduleId>/`. Per-tenant boundary via separate process address spaces + filesystem permissions + the wire-layer header check. Deploys as a systemd unit (`infrastructure/local-fs/local-fs.service`). Runs anywhere with a Linux or BSD kernel — Foundry workspace VM, customer on-prem, GCE / EC2, eventually inside a Linux/BSD guest VM hosted by seL4 on hardware where seL4 cannot boot natively (the "Linux/BSD wrapper" case).

### Envelope B — seL4 Microkit Protection Domain unikernel (long-term)

`sel4-microkit` Rust runtime crate (per Microkit 1.3.0 — the tool itself was rewritten from Python to Rust). Storage: capability-mediated IPC to `moonshot-database` (per MEMO §7 Active Sovereign Replacements: PSDB capability-aware persistence). Capability tokens granted per tenant; cross-tenant access requires capability transfer (impossible without explicit grant — formal verification, not header check). System Description File declares the Protection Domain's capability allocations and IPC channels per tenant.

The Linux/BSD wrapper case is real production infrastructure pattern, not a degraded fallback: `libsel4vm` + `libsel4vmmplatsupport` + CAmkES VMM hosts a Linux/BSD guest where Envelope A code runs unchanged. The seL4 hypervisor provides verified isolation around the guest VM.

### Why this matters

Wire protocol identical between envelopes. Storage format identical. Only the runtime envelope and storage I/O mechanism differ. Customer migration from Envelope A to Envelope B is a runtime swap, not a data-format migration — every tile authored under POSIX storage is bit-identical to one authored under `moonshot-database` capability-mediated IPC.

## 4. Checkpoint format — C2SP signed-note

Adopt the C2SP signed-note format (https://c2sp.org/signed-note) verbatim — the same format used by Sigstore Rekor v2. A checkpoint is a small text artefact:

```
service-fs.foundry.example
17                                                <-- tree size
HQC1ZP2bbV3Hr1cI4aXxFQ8vQwG4sQYwR0uW4cEAhvA=     <-- root hash (base64 SHA-256)

— foundry-tenant-foundry signed-note-key-id Wm8s...   <-- signature
```

Signed by the per-tenant key (today: workspace administrator key per `~/Foundry/CLAUDE.md` §3; long-term: per-Totebox identity key bound to customer hardware). Witnesses (Foundry workspace co-signing every Customer Totebox by default per ratified decision D5; customer-chosen third party additionally) co-sign by appending additional signature lines.

The signed-note format is plaintext, line-oriented, recoverable by `cat`. Signature verification requires only Ed25519 (or future post-quantum equivalent per §6) — no proprietary toolchain.

## 5. Append flow

1. Client `POST /v1/append` with payload + `X-Foundry-Module-ID` header (and optionally `X-Foundry-Request-ID`).
2. Server validates moduleId matches `FS_MODULE_ID` env (rejects with 403 on mismatch).
3. Server generates monotonic sequence number (cursor), appends to in-memory pending buffer.
4. Sequencer batches pending entries (every N ms or M entries — tunable per tenant load profile).
5. On batch finalisation: write leaf tile bytes (atomic write-then-rename, then `fsync`); if a tile boundary is crossed, compute and write any newly-completed intermediate tiles; sign + write new checkpoint atomically.
6. Return cursor + checkpoint signature to client.

The append-only invariant lives at three places:

- **Rust API surface** — no public `LedgerBackend` method removes or modifies an entry.
- **Filesystem level** — finalised tiles marked read-only (`0o444` mode); future hardening via `chattr +i` when systemd unit lands and operator can grant `CAP_LINUX_IMMUTABLE`.
- **Cryptographic level** — Merkle hash chain detects any retroactive modification; consistency proofs against a Rekor-anchored checkpoint fail publicly if an operator alters history.

## 6. Read flow (Ring 2 callers)

1. Client `GET /v1/checkpoint` — fetch latest signed tree head.
2. Client `GET /v1/tile/0/xNNN.b64` — fetch specific leaf tile(s) for the cursor range of interest.
3. Client computes inclusion proof locally from intermediate tiles. No server-side database lookup; tiles are CDN-cacheable; verification is independent of the daemon.
4. Server logs the read attempt to `audit-log/` sub-ledger.

The current skeleton implements `GET /v1/entries?since=N` as a convenience wrapper that returns the entries directly without the client computing tile-level proofs — this is a transitional shape suitable for the in-memory placeholder backend; the post-ratification implementation adds tile-level endpoints alongside.

## 7. ADR-07 audit-log sub-ledger

Every read produces one append to `audit-log/` (its own sub-tile-tree, separate from the data ledger). Each audit entry is JSON: moduleId, request-id, since-cursor, entries-returned, timestamp.

The sub-ledger is itself WORM — auditors can verify retroactively that the audit log has not been tampered with. The sub-ledger checkpoint is anchored alongside the data ledger checkpoint in the monthly Rekor bundle. This satisfies SOC 2 PI4 (Processing Integrity — Outputs) and the BCSC continuous-disclosure rule that material reads are externally provable.

## 8. Cryptographic agility

Hash function and signature scheme are algorithm-agile to support future migration without re-formatting tiles:

- Today: SHA-256 hashes, Ed25519 signatures.
- Hash migration path: SHA-3 family or BLAKE3 — checkpoint format includes an algorithm identifier; new tiles use the new hash; checkpoints record both algorithms during the transition period.
- Signature migration path: post-quantum (NIST PQC standardisation candidates Dilithium / SPHINCS+) — same signed-note format with algorithm-tagged signatures.

This addresses the eIDAS qualified preservation requirement that proof-of-existence survive "future technological changes" (Article 4(3) of Commission Implementing Regulation (EU) 2025/1946) and Doctrine Pillar 2 (100-year readability of every Foundry artefact).

## 9. Bootstrapping a new tenant

Per-tenant `FS_LEDGER_ROOT/<moduleId>/` is created on first `open()`:

1. Verify directory is empty or contains a valid prior state.
2. Initialise empty leaf tile, checkpoint at tree size 0, signed by the tenant key.
3. On reload (subsequent `open()` calls): read the latest checkpoint, validate the signature, walk tiles back to recompute the root, refuse to start if the recomputed root doesn't match the signed root (tamper detection at startup — SOC 2 PI1 + CC7).

The chain-tampered detection is a hard failure mode — the daemon refuses to start, surfaces the discrepancy to the operator, and waits for manual intervention. This is the right failure mode: silently re-initialising a tampered ledger would defeat the entire WORM property.

## 10. External WORM standards alignment

`service-fs` is structurally aligned with two named external WORM standards alongside the Foundry-internal SOC 2 / DARP posture documented in `~/Foundry/DOCTRINE.md` §IX:

- **SEC Rule 17a-4(f)** (US broker-dealer electronic recordkeeping; 17 CFR §240.17a-4(f); 2022 amendment effective 2023-05-03 added an Audit-Trail alternative to WORM). Foundry targets the WORM path, not the Audit-Trail loophole. Compliance is structural: the storage substrate denies modification through cryptographic hash-chain immutability + filesystem-level write-once enforcement, not through policy or process.
- **eIDAS qualified preservation service** (Commission Implementing Regulation (EU) 2025/1946 in force 2026-01-06; ETSI TS 119 511; ETSI EN 319 401 v3.2.1; CEN TS 18170:2025). Foundry's plain-text C2SP tlog-tiles format + algorithm-agility design addresses the Article 4(3) requirement that proof-of-existence survive "irrespective of future technological changes."

Neither standard requires formal certification today. The design is alignment-ready; a future audit or qualified-service-provider designation is a v1.0.0+ trajectory item conditional on customer demand and Foundry corporate posture (presently a Vendor providing infrastructure to Customer Toteboxes per `~/Foundry/conventions/customer-first-ordering.md`).

The structural-vs-policy distinction is doctrinally important. A WORM property maintained by policy ("we promise not to modify") is rebuttable; a WORM property maintained by structure ("the bytes cannot be modified without breaking the published checkpoint signature") is verifiable by any third party. Foundry chose structural compliance because it composes — every customer who runs the same substrate inherits the same property without trusting Foundry's promises.

## 11. Long-term trajectory

The seL4 unikernel envelope (Envelope B) and the `moonshot-database` capability-aware persistence layer are planned v1.0.0+ trajectory items per MEMO §7 Active Sovereign Replacements. The migration path is intentional: Envelope A's POSIX backend remains as the indefinite fallback even after Envelope B ships, because the same C2SP tile bytes serialize identically across both backends. Customers running Envelope A get the same WORM property and the same Rekor anchoring story; they get formally-verified isolation by upgrading to Envelope B when their hardware supports seL4 native boot.

Material assumptions for the seL4 transition: the rust-sel4 + sel4-microkit ecosystem continues maturing (Microkit 1.3.0 was rewritten in Rust during 2025-2026, signalling first-class Rust support); the Sovereign Substrate Phase 2 work for moonshot-database lands per the Active Sovereign Replacements roadmap; customer demand for formally-verified isolation justifies the development cost. None of these are guarantees.

## 12. References

This TOPIC composes material from:

- `~/Foundry/conventions/worm-ledger-design.md` — the substrate-level convention (workspace v0.1.7 / Doctrine v0.0.3)
- `~/Foundry/conventions/three-ring-architecture.md` — Ring 1 contract
- `~/Foundry/conventions/zero-container-runtime.md` — deployment shape
- `~/Foundry/clones/project-data/service-fs/RESEARCH.md` — full synthesis with alternatives + ten ratification decisions
- `~/Foundry/clones/project-data/service-fs/ARCHITECTURE.md` — durable architecture overview
- `~/Foundry/clones/project-data/service-fs/SECURITY.md` — compliance posture mapping
- `~/Foundry/MEMO-2026-03-30-Development-Overview-V8.md` §6.3 (service-fs role) + §7 (moonshot trajectory)
- `~/Foundry/DOCTRINE.md` §II.7 (Invention #7 Integrity Anchor); §IX (SOC 2 + external WORM standards)

External standards and reference implementations:

- C2SP tlog-tiles: https://c2sp.org/tlog-tiles
- C2SP signed-note: https://c2sp.org/signed-note
- RFC 9162 (CT v2): https://datatracker.ietf.org/doc/html/rfc9162
- Sigstore Rekor v2 GA: https://blog.sigstore.dev/rekor-v2-ga/
- rekor-tiles client implementation: https://github.com/sigstore/rekor-tiles
- Trillian-Tessera: https://github.com/transparency-dev/tessera
- SEC Rule 17a-4 (2022 amendment): https://www.sec.gov/files/rules/final/2022/34-96034.pdf
- eIDAS Implementing Regulation (EU) 2025/1946: https://eur-lex.europa.eu/eli/reg_impl/2025/1946/oj
- ETSI TS 119 511: https://www.etsi.org/deliver/etsi_ts/119500_119599/119511/
- ETSI EN 319 401 v3.2.1: https://www.etsi.org/deliver/etsi_en/319400_319499/319401/
