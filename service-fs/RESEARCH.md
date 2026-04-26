# RESEARCH.md — service-fs storage architecture

> **State:** Active research, not a ratified design  —  **Last updated:** 2026-04-26
> **Scope:** Per-project research note for `service-fs`. Synthesises
> Foundry-side material (DOCTRINE.md, MEMO §6.3 + §7,
> conventions/three-ring-architecture.md,
> conventions/zero-container-runtime.md) with industry standards
> (SEC 17a-4, eIDAS qualified preservation, SOC 2 TSC) and the
> 2025/2026 transparency-log architecture (Trillian-Tessera,
> Sigstore Rekor v2, RFC 9162 v2 tile-based CT).
> **Output:** A proposed layered design for service-fs that satisfies
> the WORM legal-compliance language in MEMO §6.3, the SOC 2 +
> DARP posture in DOCTRINE §IX, and the long-term seL4-unikernel
> Totebox-Archive trajectory in MEMO §7. Decisions for Master
> ratification are in §11.
> **Per CLAUDE.md §6 BCSC posture:** every claim about future capability
> uses planned/intended language; every cited Foundry source is a
> ratified or signed artefact; every external source is cited at the
> end of this document.

---

## 1. Why this document exists

The first NEXT.md item for `service-fs` after the Tokio MCP-server
skeleton landed (commit `af73232`, 2026-04-26) is to swap the
in-memory `WormLedger` storage for a real on-disk format. The
operator flagged 2026-04-26 that this decision is structural
because:

- Totebox Archives are intended to eventually run on a seL4
  microkernel with each `service-*` (including `service-fs`) as a
  unikernel.
- A Linux/BSD wrapper is intended for hosts where seL4 cannot boot
  natively.
- The MEMO calls service-fs WORM-compliant for legal reasons; the
  operator queried whether SOC 3 / DARP framing still makes sense
  in 2026.
- The operator asked for a leapfrog-2030 design, cross-checked
  against industry practice.

This research document supports a Master-ratification decision on
the storage format. No code changes in this commit; `RESEARCH.md`
is the deliverable per the framework convention used by other
research-phase projects in the registry (`app-console-bim`,
`app-orchestration-bim`, `app-workplace-bim`, `service-bim` all
carry `RESEARCH.md` as their initial substantive content).

---

## 2. Problem statement

`service-fs` is the per-tenant Ring 1 WORM Immutable Ledger that
every other Ring 1 producer (`service-people`, `service-email`,
`service-input`) writes through and that every Ring 2 consumer
(`service-extraction`) reads from. Per MEMO §6.3 line 194, it is
*"strictly programmed as 'Read/Append-Only.' It physically lacks
the ability to delete records, ensuring absolute Write-Once,
Read-Many (WORM) legal compliance."*

The current `WormLedger` is a `Vec<Entry>` behind a `Mutex` — a
placeholder that enforces append-only at the API surface but does
not survive a daemon restart. The storage swap needs to:

1. **Survive restart** (durability — BCSC continuous-disclosure
   rule 4 implicitly requires it; SOC 2 Processing Integrity
   explicitly requires it).
2. **Be cryptographically tamper-evident** (so a third party can
   prove the log has not been retroactively modified — Foundry's
   Invention #7 Integrity Anchor expects this).
3. **Run in two envelopes** without code or wire-protocol
   divergence:
   - **Envelope A — Hosted:** Linux/BSD daemon under systemd
     (today; per `conventions/zero-container-runtime.md`).
   - **Envelope B — Native:** seL4 Microkit Protection Domain
     unikernel on a Totebox Archive (long-term per MEMO §7).
4. **Be plain-text inspectable** (Pillar 1; DARP — searchable
   without proprietary software per DOCTRINE §IX).
5. **Be per-tenant by infrastructure**, not just by header check
   (Doctrine §IV.b strict isolation).
6. **Compose with monthly Sigstore Rekor anchoring** per
   DOCTRINE Invention #7 so Foundry's existing audit-anchoring
   discipline extends to the Customer's Totebox archive without
   bolt-on machinery.

---

## 3. What Foundry already says (chapter and verse)

| Source | Position | Implication for service-fs storage |
|---|---|---|
| `MEMO §6.3` line 194 | service-fs "physically lacks the ability to delete records" — WORM legal compliance | Append-only invariant must be enforced below the API surface, not just inside the Rust type system |
| `MEMO §7` Active Sovereign Replacements table | `vendor-sel4-kernel` → `moonshot-kernel` (no_std Rust); Sled DB → `moonshot-database` (PSDB: Capability-aware); Tantivy → `moonshot-index` (Deterministic Indexing); Microkit (Python/CMake) → `moonshot-toolkit` (Rust-Only) | Long-term storage backend is `moonshot-database`'s capability-aware persistence, not POSIX files |
| `MEMO §6.2` line 62 | Layer 0 system-substrate is "Mathematically verified seL4/C core. Root of Trust" | Today's seL4 dependency is real, not aspirational; Microkit Rust 1.3.0 is the current static-system framework |
| `conventions/three-ring-architecture.md` §"MCP boundary at Ring 1" | "Ring 1 services are MCP-server processes; each service exposes a stable wire protocol, not a Rust API" | The wire protocol is the cross-envelope contract; storage backend can vary, the wire cannot |
| `conventions/zero-container-runtime.md` | "Every Foundry deployment runs as a Linux binary under systemd on a plain VM or bare-metal host" | Envelope A (today) is non-negotiable; the design must not require a container runtime even in the Linux/BSD wrapper case |
| `conventions/customer-first-ordering.md` | Service composition order matches the customer's install order | service-fs is installed before service-input/people/email; its on-disk format is observed first by the customer's IT |
| `DOCTRINE §IX` SOC 2 / SOC 3 / DARP Posture | Workspace SOC 2 audit-ready + DARP-aligned; Processing Integrity criterion explicitly cites "Integrity Anchors monthly to Sigstore Rekor (Invention #7)" | service-fs storage format must be a natural input to the monthly Rekor anchoring run |
| `DOCTRINE §II.7` Invention #7 Integrity Anchor | "Monthly + per-MINOR-bump: workspace state hash + manifest hashes + discipline ledger hash → bundled, signed, posted to Sigstore Rekor public transparency log. Externally verifiable; anyone can prove this state existed at this time under this identity" | Sigstore Rekor is the anchoring substrate; the service-fs storage format should produce hashes that fit into the same bundle |
| `DOCTRINE §IX` DARP definition | "Data Archive Retrieval Protocol... satisfied by Pillar 1 (plain text only): all operational state is UTF-8 text; all artifacts are version-controlled or rsync-snapshotted; no proprietary formats; readable with `cat` and `grep`" | Storage format must be a plain-text or trivially-decodable structure; no opaque binary database files |
| `DOCTRINE Pillar 2` (claim #2) | "100-year readability" | The format must outlast specific software; aim for formats that a human in 2126 can read with documented primitives (SHA-256, hex, JSON) |

DARP is a Foundry-internal acronym, not a regulatory standard
(verified from `DOCTRINE.md` line 462). The compliance-flavour
part of WORM that points outward at industry comes from the SEC
and from EU eIDAS. Both are surveyed in §4.

---

## 4. External standards review

### 4.1 SEC Rule 17a-4(f) — the canonical US WORM standard

The 2022 SEC amendment to Rule 17a-4 (effective 2023-01-03,
compliance 2023-05-03) modernised the original WORM-only mandate.
Broker-dealers can now elect EITHER:

- **WORM:** "non-rewriteable, non-erasable" electronic records.
  Optical-disc media must serialise + time-stamp every unit. The
  spirit is that the storage substrate itself denies modification.
- **Audit-Trail alternative:** records may be modified or deleted,
  but every change must be logged in a "complete and time-stamped
  audit trail" preserving the original record, all changes, and
  the identity of the change-maker.

For service-fs, the WORM path is structurally simpler and matches
MEMO §6.3 ("physically lacks the ability to delete"). The
Audit-Trail alternative exists as a regulatory loophole for
vendors whose storage cannot guarantee true immutability (e.g.,
SaaS providers running on mutable cloud blob storage). Foundry
is in a different posture: the storage format itself can be
non-rewriteable through cryptographic chaining + immutable
filesystem semantics.

### 4.2 eIDAS Qualified Preservation Service (EU 2025/1946 + ETSI
TS 119 511 + CEN TS 18170:2025)

EU Commission Implementing Regulation 2025/1946 (in force
2026-01-06) plus ETSI TS 119 511 v1.2.1 (2025-10) plus
CEN TS 18170:2025 establish the **qualified electronic
preservation service** framework: the EU equivalent of WORM with
stronger requirements around long-term integrity, authenticity,
proof of existence, and accessibility "irrespective of future
technological changes." This is the closest external standard to
DOCTRINE's "100-year readability" Pillar 2.

Practical implications for service-fs:
- Cryptographic verifiability of log integrity must survive
  hash-function deprecation (today: SHA-256; planned migration
  path: SHA-3 family or BLAKE3).
- Proof-of-existence must be witnessed by at least one party
  outside the issuer. Sigstore Rekor (DOCTRINE Invention #7)
  satisfies this via public transparency-log inclusion.

### 4.3 SOC 2 Trust Services Criteria (CC1–CC9 + PI1–PI5)

The criteria most touched by service-fs storage:

- **CC6 (Logical and Physical Access):** the storage substrate
  must enforce per-tenant isolation. Today via process-level +
  filesystem-permission separation (Envelope A); long-term via
  seL4 capability-based access (Envelope B, structurally
  stronger).
- **CC7 (System Operations):** monitoring + change detection.
  The append-only Merkle tree is itself a change-detection
  primitive — any retroactive modification produces a verifiable
  inclusion-proof failure.
- **PI1 (Processing Integrity — Inputs):** every append is
  signed by a moduleId-bound capability or header. Cross-tenant
  attempts are rejected (today: 403 in `service-fs/src/http.rs`
  `enforce_module_id`; long-term: seL4 capability transfer
  refusal at the IPC layer).
- **PI4 (Processing Integrity — Outputs):** every read is logged
  to the audit hook (per ADR-07 in CLAUDE.md §6). Audit log is
  itself an append-only sub-ledger.

Auditor evidence pattern recommended in 2025 SOC 2 guidance
(see Sources): "Store logs in immutable storage to prevent
tampering (such as S3 with object lock)." Foundry's equivalent
is the local tile-based log + monthly Rekor anchoring (no S3,
no managed runtime, per `zero-container-runtime.md`).

---

## 5. Modern architectural patterns (2025/2026)

### 5.1 Tile-based static transparency log (Trillian-Tessera,
Sigstore Rekor v2, RFC 9162 v2)

The dominant 2026 pattern for verifiable append-only logs.
Replaces the older "dynamic API + database" pattern that
Trillian v1 and Rekor v1 used.

**Core idea:** the log is a Merkle tree. Tree state is divided
into **tiles** of fixed width (typically 256 elements). Each tile
is a static file containing either intermediate hashes at a
specific tree height OR leaf entries. Tiles are written once and
never modified — their hash is stable, so they cache trivially.

**Operational properties:**
- No dynamic API server required to serve reads. An HTTP file
  server (or a static blob store) is sufficient.
- Tiles are CDN-cacheable; clients can mirror the entire log.
- Inclusion proofs and consistency proofs are computed entirely
  client-side from tiles — no server-side database lookup.
- Append still requires sequencing logic (the writer side), but
  this is small and stateless once a tile is finalised.
- Witnesses (independent third parties) can co-sign log
  checkpoints, providing non-repudiable proof that the log was a
  specific shape at a specific time.

**Why this fits service-fs:**

| service-fs requirement | How tile-based log satisfies |
|---|---|
| Survive restart (durability) | Tiles are plain files on disk; daemon restart re-opens them |
| Cryptographic tamper-evidence | Merkle tree structure; consistency proofs detect retroactive changes |
| Two envelopes (hosted + seL4) | Envelope A: POSIX tiles served by axum's static-file handler. Envelope B: same tile format written through seL4 capability IPC to `moonshot-database`; tiles are still files (in the seL4 sense, capability-addressed objects), just accessed differently |
| Plain-text inspectable (DARP) | Tile format is simple — typically newline-delimited base64 of hashes or entries; readable with `cat`/`xxd`/`jq` and standard tooling |
| Per-tenant by infrastructure | One tile-tree per moduleId; cross-tenant access requires cross-process IPC or cross-capability transfer |
| Composes with Rekor anchoring | Foundry already anchors monthly to Sigstore Rekor v2, which IS itself a tile-based log; the same checkpoint primitive used internally is the one anchored externally |
| 100-year readability (Pillar 2) | Tile format is documented in RFC 9162; a 2126 reader needs only SHA-256 + hex + JSON to decode |

**Concrete shape:**

```
$FS_LEDGER_ROOT/<moduleId>/
├── checkpoint                  — latest signed tree head: tree size, root hash, signature, witness signatures
├── tile/0/x000.b64             — leaf tile 0, entries 0–255 (base64 lines)
├── tile/0/x001.b64             — leaf tile 1, entries 256–511
├── tile/1/x000.b64             — height-1 tile, hashes covering 256 leaves each
├── tile/2/x000.b64             — height-2 tile
└── audit-log/x000.jsonl        — sub-ledger: read events per ADR-07 audit hook
```

Append flow:
1. Client `POST /v1/append` with payload + moduleId header
2. Server validates moduleId, generates monotonic sequence number,
   appends to in-memory pending buffer
3. Sequencer batches pending entries every N ms or M entries
4. On batch finalisation: write leaf tile bytes, compute updated
   Merkle root, write any newly-completed intermediate tiles,
   sign + write new checkpoint
5. Return cursor (sequence number) + checkpoint signature to
   client

Read flow (Ring 2 callers):
1. Client `GET /v1/checkpoint` — fetch latest signed tree head
2. Client `GET /v1/tile/0/xNNN.b64` — fetch specific leaf tile
3. Client computes inclusion proof locally from intermediate tiles
4. ADR-07 audit hook logs the read attempt to `audit-log/`

### 5.2 ImmuDB (Append-only Hash Tree) as a comparison point

ImmuDB uses an "Append-only Hash Tree" (AHT) with similar
properties: versioned Merkle tree, transaction log with chained
linear hash. It is open-source (Apache 2.0), Go, and battle-tested
in regulatory-compliance contexts.

**Considered as service-fs backend; rejected because:**
- ImmuDB is a server, not a library — adopting it means adding a
  daemon + IPC layer below service-fs, doubling the
  process-supervision surface.
- ImmuDB's storage format is not plain-text; binary segment files
  require ImmuDB itself to read them. Violates DARP / Pillar 1.
- Capability-aware seL4 native target is not on ImmuDB's roadmap;
  the future Envelope B path requires re-implementing the access
  layer regardless.

Useful as a **reference design** for the AHT structure (we adopt
the chained-linear-hash idea; we reject the daemonised
implementation).

### 5.3 Sigstore Rekor v2 as the external anchoring substrate

Already chosen by DOCTRINE Invention #7. Rekor v2 GA (2025) is
itself a tile-based log via Trillian-Tessera. The implication:
service-fs's checkpoint format should be a Rekor-friendly entry
shape so that Foundry's monthly anchoring can post a service-fs
checkpoint to Rekor without re-bundling.

Specifically: Foundry's monthly anchor bundle (per Invention #7)
should include, for each Customer Totebox tenant:
- service-fs checkpoint (tree size + root hash + signature)
- service-fs audit-log checkpoint (sub-ledger root hash)
- Bundle signed by the per-tenant identity, posted to Rekor

This gives the Customer's Totebox archive externally-verifiable
tamper-evidence WITHOUT exposing tenant data to Rekor (only
hashes and signatures leave the Totebox).

---

## 6. The seL4 unikernel transition story

### 6.1 Today: hosted Linux daemon (Envelope A)

`service-fs` is a Tokio + axum binary on Linux/BSD under systemd.
Storage is POSIX files under `FS_LEDGER_ROOT`. Per-tenant
isolation is by separate process per moduleId + filesystem
permissions. Reference shape: `slm-doorman-server` in the
project-slm cluster.

### 6.2 Long-term: seL4 Microkit Protection Domain (Envelope B)

`service-fs` is a Microkit Protection Domain (PD) — a
unikernel-style Rust binary linked against `sel4-microkit` (the
official Rust runtime crate per seL4 Microkit 1.3.0). The PD is
declared in a System Description File (SDF) along with its
capability allocations and IPC channels. Storage is mediated by
`moonshot-database` (PSDB: Capability-aware) — an
seL4-native database that holds the per-tenant tile tree in
capability-addressed objects.

Per Microkit 1.3.0, the build tool is now Rust (rewritten from
Python/CMake) — matching `moonshot-toolkit`'s declared target.

### 6.3 The Linux/BSD wrapper case

For hosts where seL4 cannot boot natively (legacy hardware, tier
restrictions, customer-mandated host OS), the seL4 hypervisor
mode hosts a Linux or BSD guest that in turn runs the
service-* binaries in their Envelope A shape. This is the
`libsel4vm` + `libsel4vmmplatsupport` + CAmkES VMM pattern, well-
documented in seL4 docs and seL4 Summit 2025 abstracts.

### 6.4 The dual-target Rust binary pattern

Same Rust source compiles to two targets:

- **Target A — `cargo build --bin service-fs`:** std Rust, Tokio,
  axum HTTP server, POSIX file I/O. Today.
- **Target B — `cargo build --bin service-fs --target
  aarch64-unknown-none-sel4 --features microkit`:** no_std Rust,
  sel4-microkit runtime, no Tokio (Microkit IPC instead), no
  axum (a thin sel4-microkit-aware HTTP shim or pure-MCP
  IPC handler). Long-term.

The wire protocol (JSON-over-HTTP today; MCP-over-IPC long-term)
is identical. The storage abstraction (`WormLedger` API:
`open` / `append` / `read_since` / `checkpoint` / `verify_inclusion`
/ `verify_consistency`) is identical. Only the runtime envelope
differs.

This is the pattern documented in `SLM-STACK.md` for `mistral.rs`
("Cross-compilation: `cargo build --target aarch64-unknown-linux-gnu`
works. Deploy to ARM Toteboxes, x86, whatever.") — generalised
to also cover the no_std/seL4 target.

### 6.5 Why this matters for the storage decision

The storage format must be **target-independent**. Tile-based
static logs are exactly that: tiles are bytes; what writes them
(POSIX file I/O on Target A; capability-mediated `moonshot-database`
write on Target B) is implementation detail. The format outlives
both implementations.

---

## 7. Proposed design for service-fs

A four-layer stack, each layer independently swappable:

```
┌──────────────────────────────────────────────────────────────┐
│ L4 — Anchoring (workspace-tier, monthly)                      │
│   Rekor v2 anchoring of per-tenant checkpoints per            │
│   DOCTRINE Invention #7. Workspace cron → bundle              │
│   checkpoints → sigstore-rs sign → Rekor POST.                │
│   Same code path serves Vendor (Foundry workspace) +          │
│   Customer (Totebox). Master-operated for SMB Customers.      │
└──────────────────────────────────────────────────────────────┘
                              ▲
┌──────────────────────────────────────────────────────────────┐
│ L3 — Wire protocol (per-tenant, MCP-aware)                    │
│   Today: axum routes with X-Foundry-Module-ID header          │
│   enforcement. Long-term: MCP-server interface layered on     │
│   top — MCP resources for /v1/checkpoint + /v1/tile/N/M;      │
│   MCP tools for /v1/append.                                   │
│   Same wire shape on Target A (hosted) + Target B (seL4).     │
└──────────────────────────────────────────────────────────────┘
                              ▲
┌──────────────────────────────────────────────────────────────┐
│ L2 — WORM Ledger API (Rust trait, target-independent)         │
│   open / append / read_since / checkpoint / verify_inclusion  │
│   / verify_consistency. Append-only invariant enforced at     │
│   the API surface. Audit-log sub-ledger for ADR-07.           │
│   This is the long-term-stable contract. Already present      │
│   today in src/ledger.rs as WormLedger; needs the            │
│   verify_* methods + checkpoint type.                         │
└──────────────────────────────────────────────────────────────┘
                              ▲
┌──────────────────────────────────────────────────────────────┐
│ L1 — Tile storage primitive (envelope-specific)               │
│   Target A (Linux/BSD): POSIX files under FS_LEDGER_ROOT,     │
│   atomic write-then-rename per tile, fsync after each tile    │
│   + after each checkpoint write.                              │
│   Target B (seL4 unikernel): same tile bytes, but written     │
│   through capability-mediated IPC to moonshot-database        │
│   (PSDB) as capability-addressed objects.                     │
└──────────────────────────────────────────────────────────────┘
```

### 7.1 Tile format (concrete)

Adopt the **C2SP tlog-tiles** specification (the ratified
format used by RFC 9162 v2 / Rekor v2 / Trillian-Tessera).
Tile width is 256 entries; height is determined by tree
position. Tile files are text — newline-delimited base64 of
either entry blobs (leaf level) or 32-byte SHA-256 hashes
(intermediate levels).

Adopting an existing spec rather than inventing a Foundry-
specific format gets us:
- Pre-existing client libraries (sigstore-rs, transparency-dev/tessera)
- Interoperability with Sigstore Rekor v2 (same checkpoint shape)
- Verification by any third-party tooling
- 100-year readability via a published RFC

### 7.2 Checkpoint format

Adopt the **C2SP signed-note** format (also used by Rekor v2).
A checkpoint is a small text artefact:

```
service-fs.foundry.example
17    <-- tree size
HQC1ZP2bbV3Hr1cI4aXxFQ8vQwG4sQYwR0uW4cEAhvA=    <-- root hash (base64 SHA-256)

— foundry-tenant-foundry signed-note-key-id-here Wm8s...   <-- signature
```

Signed by the per-tenant key (today: workspace ps-administrator
key per CLAUDE.md §3; long-term: per-Totebox identity key).
Witnesses (Foundry workspace, customer-chosen third party) may
co-sign by appending additional `— <name> <key-id> <sig>` lines.

### 7.3 ADR-07 audit log as a sub-ledger

Every read gets one append to `audit-log/` (its own sub-tile-
tree). Each audit entry is JSON: moduleId, request-id, since-
cursor, entries-returned, timestamp. The sub-ledger is itself
WORM — auditors can verify it has not been tampered with
post-hoc.

### 7.4 Bootstrapping a new tenant

Per-tenant `FS_LEDGER_ROOT/<moduleId>/` is created on first
`open()`. Initial state: empty leaf tile, checkpoint at tree
size 0, signed by the tenant key. Reload-on-restart reads the
latest checkpoint, validates the signature, walks tiles back to
recompute the root, refuses to start if the recomputed root
doesn't match the signed root (tamper detection at startup).

### 7.5 Cryptographic agility

All hashes are SHA-256 today (matches Rekor v2 + Trillian-
Tessera). The checkpoint format includes an algorithm identifier
so a future migration to BLAKE3 or SHA-3 can be carried out
non-disruptively (new tiles use the new hash; checkpoints
record both algorithms during the transition).

---

## 8. Compliance mapping

| Requirement | How the proposed design satisfies |
|---|---|
| MEMO §6.3 WORM "physically lacks the ability to delete" | Tile files are atomic-rename written; finalised tiles immediately marked filesystem-immutable (`chattr +i` on Linux ext4/xfs, equivalent flags on BSD UFS/ZFS); on seL4 native, capability-addressed objects in moonshot-database have no "delete" capability granted to service-fs. |
| SEC 17a-4(f) WORM (US, broker-dealer-equivalent) | Append-only invariant + cryptographic chaining + filesystem immutability. The WORM path is satisfied; the Audit-Trail alternative loophole is not needed. |
| eIDAS qualified preservation (EU, 2026/01) | Long-term integrity via Merkle chain + monthly Rekor witness. Algorithm-agility for hash function deprecation. Plain-text tile format for accessibility "irrespective of future technological changes." |
| SOC 2 CC6 (Logical Access) | Per-tenant moduleId enforcement at L3 today; structural per-tenant capability isolation at L1 long-term. |
| SOC 2 PI1/PI4 (Processing Integrity) | Append signature; read-audit sub-ledger; checkpoint-on-restart tamper detection. |
| SOC 2 CC7 (System Operations) | Inclusion + consistency proofs are change-detection primitives; any retroactive modification produces a verifiable failure. |
| DOCTRINE §IX DARP | Tile format is plain text + base64 hashes; readable with `cat`, `xxd`, `jq`. RFC 9162 + C2SP tlog-tiles published. No proprietary format. |
| DOCTRINE Pillar 1 (plain text only) | Tiles are text. Checkpoints are text. Audit log is JSONL. |
| DOCTRINE Pillar 2 (100-year readability) | Format documented in published RFCs. SHA-256 + base64 + JSON are 1990s-vintage primitives that any future archivist can decode. |
| DOCTRINE Invention #7 (Integrity Anchor) | service-fs checkpoint shape is a direct input to the monthly Rekor anchor bundle. Rekor v2 IS itself a tile-based log; the substrate matches end-to-end. |
| ADR-07 (zero AI in Ring 1) | Pure deterministic processing. No model inference at any layer. Audit-log sub-ledger has zero AI involvement. |
| Doctrine §IV.b (strict per-tenant isolation) | Today: separate process per moduleId + filesystem permissions. Long-term: seL4 capability-mediated access — structurally cannot route across tenants. |

---

## 9. What's "novel" relative to existing systems (synthesis claims)

Most of the building blocks are off-the-shelf 2025/2026 industry
practice: tile-based logs are standard; per-tenant separation is
basic; seL4 unikernels are an active research area. The
synthesis points worth flagging as Foundry-specific:

1. **Per-tenant moduleId enforcement at the WORM layer** (rather
   than at a higher application layer). Most regulatory WORM
   systems are single-tenant or multi-tenant-by-policy; service-fs
   is multi-tenant-by-infrastructure. The seL4 capability model
   makes this structurally hard to violate.
2. **Dual-target Rust binary across Linux daemon and seL4
   unikernel** with the same wire protocol and same tile format.
   This is the cross-cutting pattern that makes the substrate
   portable from a customer's Hetzner VM to a future ToteboxOS
   appliance without rewrite.
3. **Customer's Totebox archive becomes externally verifiable via
   the same Rekor anchoring substrate the Vendor uses.** This
   extends DOCTRINE Invention #7 from a Vendor audit-posture
   feature to a Customer evidentiary feature, at zero marginal
   complexity (the bundle format is the same).
4. **Plain-text tile format throughout** (DARP / Pillar 1) — most
   verifiable-log implementations use opaque binary stores under
   an API surface; using the C2SP plain-text tile spec means a
   2126 forensic analyst can decode service-fs storage with
   nothing but `cat`, `xxd`, `base64`, and a SHA-256 implementation.

These are Foundry-specific *applications* of standard primitives,
not novel cryptography. The leapfrog comes from the integration,
not from the building blocks.

---

## 10. Alternatives considered and rejected

| Alternative | Rejected because |
|---|---|
| **Roll our own format** (newline-delimited JSON entries with hash chain) | Loses interop with Rekor v2 / Trillian-Tessera tooling; loses the existing client-library ecosystem; loses the published-RFC durability story; reinvents the wheel. |
| **Adopt ImmuDB as a library or daemon dependency** | Not plain-text (DARP failure); daemonised model adds process-supervision surface; no seL4 native target on roadmap. |
| **Use Sled embedded DB** (today's `vendor-sel4-kernel` substrate uses it per MEMO §7) | Opaque binary format (DARP failure); will be replaced by `moonshot-database` per the moonshot table — no point investing in a backend that's already targeted for replacement. |
| **Hold storage decision until `moonshot-database` is ready** | Blocks all of Ring 1 ingest progress on a structural-placeholder project (moonshot-database is currently a 4-file scaffold with no code). The L1/L2 split lets us ship POSIX storage today and swap to moonshot-database when it lands, without changing the API contract. |
| **Just use git as the storage substrate** (every append is a commit) | Considered seriously — git is plain-text, hash-chained, and mature. Rejected because: (a) git's hash function is SHA-1 by default (deprecated for security), SHA-256 git is still rare; (b) per-append commits are heavy (one fork-exec per write); (c) git's append-only-ness depends on operator discipline (force-push exists), not on the format itself. The tile-based log gets the good parts (hash chain, plain text, mature tooling) without the bad parts. |
| **Audit-Trail alternative per SEC 2022 amendment** (allow modification + log changes) | Loses the cleaner WORM narrative; introduces deletion/modification code paths that would need careful auditing; the storage format already supports true WORM, so the loophole isn't needed. |
| **Use S3 with object lock** (the SOC 2 example pattern from web research) | Violates `zero-container-runtime.md` (we don't run on managed cloud services); violates portability (Foundry must run on Hetzner / on-prem / Totebox appliance, not just AWS); adds vendor lock-in (Doctrine Property 1: Substrate Sovereignty). |

---

## 11. Open decisions for Master ratification

These are the calls that should land in a workspace-tier
convention (proposed location:
`~/Foundry/conventions/worm-ledger-design.md`) rather than in
service-fs's per-project files, because the same decisions apply
to any future Ring 1 producer that needs WORM persistence (e.g.,
`service-extraction`'s materialised graphs, or future audit
sub-ledgers in other services).

**D1 — Tile spec adoption.** Adopt **C2SP tlog-tiles** verbatim
as the on-disk format? Recommended yes.

**D2 — Checkpoint spec adoption.** Adopt **C2SP signed-note**
format for checkpoints? Recommended yes.

**D3 — Hash function.** SHA-256 today with algorithm-agility for
future BLAKE3 or SHA-3 migration? Recommended yes.

**D4 — Filesystem immutability on Linux.** Use `chattr +i` on
finalised tiles to make them filesystem-immutable, or rely on
write-then-rename + permissions only? `chattr +i` is stronger
but requires elevated capability (CAP_LINUX_IMMUTABLE) which
the daemon shouldn't normally hold. Recommendation: write-then-
rename + 0o444 read-only mode + filesystem-level enforcement
(ext4/xfs `journal_data` mode) for v0.1.x; revisit `chattr +i`
when systemd unit lands and operator can grant the capability.

**D5 — Witness coordination.** Foundry workspace as a witness
for every Customer Totebox? Or per-Totebox witness chosen by
the Customer? Recommendation: Foundry workspace witnesses by
default (zero customer setup); Customer can add additional
witnesses in their Totebox config (federation property).

**D6 — Anchoring cadence.** Monthly per Invention #7 — leave
unchanged? Yes, unless a SOC 2 auditor pushes for daily.

**D7 — moonshot-database integration timing.** When
moonshot-database is ready (currently a 4-file placeholder), how
does service-fs migrate? Recommendation: dual-target the storage
backend behind the L2 trait; flip to moonshot-database when it
ships, retain POSIX backend as Envelope A fallback.

**D8 — Audit-log sub-ledger granularity.** One audit entry per
read-call, or per-entry-returned? Per-call is cheaper; per-entry
is finer-grained. Recommendation: per-call with `entries_returned`
field captures the volume without per-entry write cost.

**D9 — Customer-tier anchoring.** Does Customer Totebox tile
storage anchor to Rekor (Customer's posture) or to Foundry's
internal anchor (Vendor's posture forwarding)? Recommendation:
both — Customer-tier posts checkpoints to Rekor with their own
key (sovereignty); Foundry workspace bundles same checkpoints
into the Vendor-side monthly anchor for redundant verifiability.

**D10 — Re-add to workspace `[members]` after openssl-sys
cleanup.** Independent of this design; tracked in NEXT.md.

---

## 12. Implementation roadmap (for the next Task Claude session in
this cluster, after Master ratifies the design)

The work breaks into approximately five Task-tier commits on
`cluster/project-data` once the design is ratified:

1. **L2 trait extraction:** factor `WormLedger` into a
   `LedgerBackend` trait (open/append/read_since/checkpoint/
   verify_*) with the current in-memory backend as one
   implementation. Tests still pass against the trait.
2. **L1 POSIX tile backend:** implement a `PosixTileLedger`
   backend that writes C2SP tlog-tiles to `FS_LEDGER_ROOT`.
   New tests: durability (write, simulate restart, read back);
   inclusion proof verification; consistency proof verification.
3. **Checkpoint signing wiring:** integrate a signed-note signing
   key (per-tenant; bootstrap from `FS_SIGNING_KEY` env var
   pointing to a key file). Add `/v1/checkpoint` endpoint.
4. **Audit-log sub-ledger:** introduce a separate
   `WormLedger` instance for the audit log, written to
   `FS_LEDGER_ROOT/<moduleId>/audit-log/`. Wire the existing
   `tracing::info!` call at the read site to also write an
   audit entry.
5. **MCP-server interface layer:** wrap the existing axum routes
   in MCP resource/tool semantics per the Anthropic/Cloudflare
   2026 spec.

Long-term work (post-v0.1.0; outside this Task cluster's scope):

6. **Target B build:** add the `microkit` Cargo feature, swap
   Tokio/axum for sel4-microkit IPC, swap POSIX storage for
   moonshot-database access. Cross-compile via
   `moonshot-toolkit`.
7. **Workspace-tier monthly Rekor anchoring:** Master adds
   `bin/anchor-tile-checkpoints.sh` to bundle service-fs
   checkpoints into the existing monthly anchor run.

---

## 13. Sources

### Foundry-side (in this workspace)

- `~/Foundry/DOCTRINE.md` — §II claim 2 (100-year readability),
  §II.7 Invention #7 (Integrity Anchor), §IV.b (strict per-
  tenant isolation), §IX (SOC 2 / SOC 3 / DARP Posture)
- `~/Foundry/MEMO-2026-03-30-Development-Overview-V8.md` — §6.3
  line 194 (service-fs WORM compliance language), §7 Moonshots
  table (vendor-sel4-kernel → moonshot-kernel; Sled →
  moonshot-database)
- `~/Foundry/CLAUDE.md` — §3 (identity/SSH signing), §6
  (language standard, ADR hard rules)
- `~/Foundry/conventions/three-ring-architecture.md` — Ring 1
  contract, MCP boundary, moduleId discipline
- `~/Foundry/conventions/zero-container-runtime.md` — deployment
  shape, ELF binary under systemd, no managed runtime
- `~/Foundry/conventions/customer-first-ordering.md` — install-
  order alignment
- `~/Foundry/conventions/bcsc-disclosure-posture.md` —
  forward-looking-information rule
- `~/Foundry/conventions/compounding-substrate.md` — service-fs
  position in the compounding loop
- `~/Foundry/SLM-STACK.md` — cross-compilation pattern (mistral.rs
  precedent for dual-target Rust binary)

### External standards

- [SEA Rule 17a-4 and Related Interpretations | FINRA.org](https://www.finra.org/rules-guidance/guidance/interpretations-financial-operational-rules/sea-rule-17a-4-and-related-interpretations)
- [SEC.gov | Amendments to Electronic Recordkeeping Requirements for Broker-Dealers](https://www.sec.gov/investment/amendments-electronic-recordkeeping-requirements-broker-dealers)
- [Crawling into modernity: SEC amends WORM recordkeeping requirements for broker-dealers and SBSDs | Davis Polk](https://www.davispolk.com/insights/client-update/crawling-modernity-sec-amends-worm-recordkeeping-requirements-broker-dealers)
- [ETSI EN 319 401 V3.2.1 (2026-01)](https://www.etsi.org/deliver/etsi_en/319400_319499/319401/03.02.01_60/en_319401v030201p.pdf)
- [Commission Implementing Regulation (EU) 2025/1946 — qualified preservation services](https://www.eurlexa.com/act/en/32025R1946/present/text)
- [SOC 2 Trust Services Criteria: The Complete CC1-CC9 Reference Guide](https://truvocyber.com/blog/soc-2-trust-services-criteria-guide)

### Modern verifiable-log architecture

- [RFC 9162: Certificate Transparency Version 2.0](https://www.rfc-editor.org/rfc/rfc9162.html)
- [Tile-Based Transparency Logs | Trillian](https://transparency.dev/articles/tile-based-logs/)
- [Rekor v2 GA — Sigstore Blog](https://blog.sigstore.dev/rekor-v2-ga/)
- [Rekor — Sigstore](https://docs.sigstore.dev/logging/overview/)
- [GitHub — sigstore/rekor: Software Supply Chain Transparency Log](https://github.com/sigstore/rekor)
- [GitHub — transparency-dev/tessera: Go library for building tile-based transparency logs](https://github.com/transparency-dev/tessera)
- [Announcing the Alpha release of Trillian Tessera](https://blog.transparency.dev/announcing-the-alpha-release-of-trillian-tessera)
- [What 2025 Holds for Certificate Transparency and the Transparency.dev Ecosystem](https://blog.transparency.dev/what-2025-holds-for-certificate-transparency-and-the-transparencydev-ecosystem)
- [End of Life Plan for RFC 6962 Certificate Transparency Logs — Let's Encrypt](https://letsencrypt.org/2025/08/14/rfc-6962-logs-eol)
- [Deep dive into the internals of an immutable database, immudb](https://arriqaaq.medium.com/deep-dive-into-the-internals-of-an-immutable-database-immudb-3bdf9a0c2faa)
- [immudb — immutable database based on zero trust, SQL and Key-Value](https://immudb.io/)

### seL4 unikernel + virtualization

- [The seL4 Microkernel | seL4](https://sel4.systems/)
- [seL4 Summit 2025 Abstracts](https://sel4.systems/Summit/2025/abstracts2025.html)
- [GitHub — seL4/microkit: Microkit framework for the seL4 microkernel](https://github.com/seL4/microkit)
- [GitHub — seL4/rust-sel4: Rust support for seL4 userspace](https://github.com/seL4/rust-sel4)
- [GitHub — seL4/rust-microkit-http-server-demo](https://github.com/seL4/rust-microkit-http-server-demo)
- [Microkit Release 1.3.0 — Microkit tool rewritten in Rust](https://docs.sel4.systems/releases/microkit/1.3.0.html)
- [VMM library (libsel4vmmplatsupport) | seL4 docs](https://docs.sel4.systems/projects/virtualization/docs/libsel4vmm.html)
- [CAmkES VMM | seL4 docs](https://docs.sel4.systems/projects/camkes-vm/)
- [Genode on seL4 | Genode](https://genode.org/documentation/articles/sel4_part_2)
- [seL4 Microkernel for virtualization use-cases: Potential directions towards a standard VMM](https://www.mdpi.com/2079-9292/11/24/4201)
- [CHERI-seL4 and CHERI-Microkit Released | CHERI Alliance](https://cheri-alliance.org/cheri-sel4-and-cheri-microkit-released/)

---

*This document is a Task-tier research synthesis. Workspace-tier
formalisation is proposed in the cluster outbox to Master Claude
under subject `worm-ledger-design-convention-proposal`.*
