# ARCHITECTURE.md — service-fs

> **Status:** Ratified at workspace tier 2026-04-26 — design
> convention authored at `~/Foundry/conventions/worm-ledger-design.md`
> (workspace v0.1.7 / Doctrine v0.0.3, commit `6c0b79a`); this
> per-project architecture overview reviewed and accepted with no
> contradictions per Master's reply 2026-04-26T10:35Z.
> **Last updated:** 2026-04-26
> **Scope:** Durable architecture overview for `service-fs` —
> the four-layer stack and the two boot envelopes. The substrate-
> level four-layer pattern is now governed by
> `~/Foundry/conventions/worm-ledger-design.md`; this per-project
> file documents how `service-fs` specifically applies it.
> **What this is not:** the full synthesis with alternatives and
> sources is `RESEARCH.md`; the compliance posture is `SECURITY.md`;
> the operational state and constraints are `CLAUDE.md`. This file
> is the architecture summary alone — read this first to orient,
> then drill into the others as needed.

---

## 1. Position in the system

`service-fs` is the **per-tenant Ring 1 WORM Immutable Ledger**
that all other Ring 1 producers (`service-people`,
`service-email`, `service-input`) write through. Ring 2 consumers
(`service-extraction`) read from it as MCP clients; they never
touch the originating producer service.

```
                    Ring 2 (project-slm cluster)
                    service-extraction (reader)
                              ▲
                              │ MCP read (cursor-paged)
                              │
              ┌───────────────┴───────────────┐
              │     service-fs (this crate)    │
              │     WORM Immutable Ledger      │
              │     per-tenant moduleId        │
              └───────────────▲───────────────┘
                              │ MCP append
              ┌───────────────┼───────────────┐
              │               │               │
       service-people  service-input    service-email
       (Ring 1)        (Ring 1)         (Ring 1)
```

**Hard rule:** there is exactly one `service-fs` process per
tenant `moduleId`. Cross-tenant access is rejected at the wire
layer today (header check) and structurally impossible long-term
(seL4 capability isolation).

---

## 2. The four-layer stack

The architecture is intentionally layered so each layer is
independently swappable. The middle layer (L2) is the durable
contract that survives changes above and below it.

```
┌─────────────────────────────────────────────────────────────┐
│ L4 — Anchoring (workspace-tier, monthly)                     │
│   Sigstore Rekor v2 anchoring of per-tenant checkpoints per  │
│   DOCTRINE Invention #7. Workspace cron → bundle             │
│   checkpoints → sigstore-rs sign → Rekor POST.               │
│   Master-operated. Same code path serves Vendor (Foundry     │
│   workspace) + Customer (Totebox).                           │
└─────────────────────────────────────────────────────────────┘
                              ▲
┌─────────────────────────────────────────────────────────────┐
│ L3 — Wire protocol (per-tenant, MCP-aware)                   │
│   Today: axum HTTP routes with X-Foundry-Module-ID header    │
│   enforcement. Long-term: MCP-server interface layered on    │
│   top — MCP resources for /v1/checkpoint + /v1/tile/N/M;     │
│   MCP tools for /v1/append.                                  │
│   Same wire shape across both boot envelopes.                │
└─────────────────────────────────────────────────────────────┘
                              ▲
┌─────────────────────────────────────────────────────────────┐
│ L2 — WORM Ledger API (Rust trait, target-independent)        │
│   open / append / read_since / checkpoint /                  │
│   verify_inclusion / verify_consistency.                     │
│   Append-only invariant enforced at the API surface.         │
│   Audit-log sub-ledger for ADR-07 read tracking.             │
│   THIS IS THE DURABLE CONTRACT. It survives changes to L1    │
│   (storage backend) and L3 (wire protocol) above and below.  │
└─────────────────────────────────────────────────────────────┘
                              ▲
┌─────────────────────────────────────────────────────────────┐
│ L1 — Tile storage primitive (envelope-specific)              │
│   Envelope A (Linux/BSD daemon today):                       │
│     POSIX files under FS_LEDGER_ROOT, atomic write-then-     │
│     rename per tile, fsync after each tile + each checkpoint.│
│   Envelope B (seL4 Microkit unikernel long-term):            │
│     Same tile bytes, written through capability-mediated     │
│     IPC to moonshot-database (PSDB) as capability-addressed  │
│     objects.                                                 │
│   Tile format: C2SP tlog-tiles (RFC 9162 v2 / Trillian-      │
│   Tessera / Sigstore Rekor v2 — same format end-to-end).     │
└─────────────────────────────────────────────────────────────┘
```

### 2.1 Why four layers and not three

L4 is separated from L3 because anchoring is **workspace-tier
periodic work** (monthly cron) — not request-time work in the
service-fs process. Putting it in its own layer makes the
service-fs daemon itself stateless about anchoring; it just
produces signed checkpoints that the workspace anchoring run
consumes.

L1 is separated from L2 because the storage backend is
**envelope-specific** while the API contract is target-
independent. The L2 trait is the reason the same Rust code can
swap from POSIX storage today to capability-mediated
moonshot-database long-term without changes above L2.

---

## 3. The two boot envelopes

`service-fs` is intended to ship in two envelopes that share the
same wire protocol and same storage format:

### 3.1 Envelope A — Linux/BSD daemon under systemd (today)

- **Runtime:** Tokio async runtime, axum 0.7 HTTP server.
- **Storage:** POSIX files in `FS_LEDGER_ROOT/<moduleId>/`.
- **Per-tenant boundary:** one daemon process per moduleId
  (separate process address spaces); filesystem permissions
  restricting per-tenant `FS_LEDGER_ROOT` access; request-time
  `X-Foundry-Module-ID` header check (`src/http.rs::enforce_module_id`).
- **Build:** `cargo build --bin service-fs` — std Rust.
- **Deployment:** systemd unit at
  `infrastructure/local-fs/local-fs.service` (Master-owned;
  pending workspace v0.1.x increment per Master's 2026-04-26
  inbox message).
- **Reference shape:** `slm-doorman-server` in the project-slm
  cluster (`78031c4`).
- **Where it runs:** any Linux or BSD host — Foundry workspace
  VM, Hetzner box, customer on-prem, GCE / EC2, eventually inside
  a Linux/BSD guest VM hosted by seL4 (the "Linux/BSD wrapper"
  case for hardware where seL4 can't boot natively).

### 3.2 Envelope B — seL4 Microkit Protection Domain unikernel (long-term)

- **Runtime:** `sel4-microkit` Rust runtime crate (per Microkit
  1.3.0; tool itself rewritten from Python to Rust).
- **Storage:** `moonshot-database` (PSDB: Capability-aware) per
  MEMO §7 Active Sovereign Replacements table. Capability tokens
  granted per tenant; cross-tenant access requires capability
  transfer (impossible without explicit grant).
- **Per-tenant boundary:** structural — seL4 microkernel-level
  capability enforcement, formally verified.
- **Build:** `cargo build --bin service-fs --target ...-sel4
  --features microkit` — no_std Rust.
- **System Description File (SDF):** declares the Protection
  Domain's capability allocations and IPC channels per tenant.
- **Where it runs:** future ToteboxOS appliance (per MEMO §6 and
  the Compounding Substrate doctrine); bare-metal seL4 boot.

### 3.3 The Linux/BSD wrapper case

For hosts where seL4 cannot boot natively (legacy hardware,
customer infrastructure constraints), seL4's hypervisor mode
hosts a Linux/BSD guest VM that in turn runs `service-fs` in
Envelope A. This is the `libsel4vm` + `libsel4vmmplatsupport`
+ CAmkES VMM pattern (well-documented in seL4 Summit 2025
abstracts). Envelope A code runs unchanged inside the guest;
the seL4 hypervisor provides the verified isolation around the
guest VM.

### 3.4 Why this matters

The wire protocol (HTTP/MCP) is identical between envelopes.
The storage format (C2SP tlog-tiles) is identical between
envelopes. Only the runtime envelope and the storage I/O
mechanism differ. This is the same dual-target pattern
SLM-STACK.md already uses for `mistral.rs` ("Cross-compilation:
`cargo build --target aarch64-unknown-linux-gnu` works. Deploy
to ARM Toteboxes, x86, whatever."), generalised to also cover
the no_std/seL4 target.

---

## 4. Tile format adoption — C2SP tlog-tiles

Adopt the C2SP tlog-tiles spec verbatim — the format used by
RFC 9162 v2 (Certificate Transparency v2), Trillian-Tessera,
and Sigstore Rekor v2.

**Tile structure:**
- Tiles are static files. Each tile contains 256 entries.
- Leaf-level tiles (height 0) hold the actual entry blobs (or
  hashes of them).
- Intermediate-level tiles (heights 1, 2, ...) hold 32-byte
  SHA-256 hashes covering 256 entries each at the level below.
- Tile filenames encode the height and the index:
  `tile/<height>/x<index>.b64` — base64-encoded text content.

**Why text and not binary:** DARP / Pillar 1 requires
plain-text inspection. Tile content is base64-encoded SHA-256
hashes (32 bytes → 44 chars + newline) — readable with `cat`,
`xxd`, `base64 -d`. Standard Unix tooling, no proprietary
parser.

**Why an existing spec and not our own:** interop with Sigstore
Rekor v2 (same checkpoint shape — feeds Invention #7 anchoring
directly); existing client libraries (sigstore-rs,
transparency-dev/tessera); 100-year readability via published
RFC; verification by any third-party tooling.

**Concrete on-disk layout per tenant:**

```
$FS_LEDGER_ROOT/<moduleId>/
├── checkpoint            — latest signed tree head (signed-note format):
│                           tree size + root hash + algorithm tag +
│                           per-tenant signature + optional witness
│                           co-signatures
├── tile/0/x000.b64       — leaf tile 0, entries 0–255 (newline-delimited
│                           base64-encoded payload bodies)
├── tile/0/x001.b64       — leaf tile 1, entries 256–511
├── tile/1/x000.b64       — height-1 tile, hashes covering 256 leaves each
├── tile/2/x000.b64       — height-2 tile, hashes covering 65,536 leaves each
└── audit-log/
    ├── checkpoint        — separate sub-ledger for ADR-07 read events
    └── tile/0/x000.b64
```

---

## 5. Checkpoint format — C2SP signed-note

Adopt the C2SP signed-note format (also used by Rekor v2).
A checkpoint is a small text artefact:

```
service-fs.foundry.example
17                                                <-- tree size
HQC1ZP2bbV3Hr1cI4aXxFQ8vQwG4sQYwR0uW4cEAhvA=     <-- root hash (base64 SHA-256)

— foundry-tenant-foundry signed-note-key-id Wm8s...   <-- signature
```

Signed by the per-tenant key (today: workspace ps-administrator
key per CLAUDE.md §3; long-term: per-Totebox identity key).
Witnesses (Foundry workspace, customer-chosen third party)
co-sign by appending additional `— <name> <key-id> <sig>` lines.

---

## 6. Append flow

1. Client `POST /v1/append` with payload + `X-Foundry-Module-ID`
   header (and optionally `X-Foundry-Request-ID`).
2. Server validates moduleId matches `FS_MODULE_ID` env (rejects
   with 403 on mismatch — `enforce_module_id`).
3. Server generates monotonic sequence number (cursor), appends
   to in-memory pending buffer.
4. Sequencer batches pending entries (every N ms or M entries).
5. On batch finalisation: write leaf tile bytes (atomic
   write-then-rename, then `fsync`); if a tile boundary is
   crossed, compute and write any newly-completed intermediate
   tiles; sign + write new checkpoint atomically.
6. Return cursor + checkpoint signature to client.

The append-only invariant lives at three places:

- **Rust API surface** — no public `WormLedger` method removes
  or modifies an entry (today's `src/ledger.rs`).
- **Filesystem level** — finalised tiles marked read-only
  (`0o444` mode); future hardening via `chattr +i` when systemd
  unit lands and operator can grant `CAP_LINUX_IMMUTABLE`.
- **Cryptographic level** — Merkle hash chain detects any
  retroactive modification; consistency proofs against a
  Rekor-anchored checkpoint fail publicly if an operator alters
  history.

---

## 7. Read flow (Ring 2 callers)

1. Client `GET /v1/checkpoint` — fetch latest signed tree head.
2. Client `GET /v1/tile/0/xNNN.b64` — fetch specific leaf tile(s)
   for the cursor range of interest.
3. Client computes inclusion proof locally from intermediate
   tiles (no server-side database lookup; tiles are CDN-cacheable).
4. Server logs the read attempt to `audit-log/` sub-ledger:
   one JSON record per read with moduleId, request-id,
   since-cursor, entries-returned, timestamp.

The current skeleton implements `GET /v1/entries?since=N` as a
convenience wrapper that returns the entries directly without
the client computing tile-level proofs — this is a transitional
shape suitable for the in-memory placeholder backend; the
post-ratification implementation adds tile-level endpoints
alongside.

---

## 8. ADR-07 audit-log sub-ledger

Every read produces one append to `audit-log/` (its own
sub-tile-tree, separate from the data ledger). Each audit entry
is JSON: moduleId, request-id, since-cursor, entries-returned,
timestamp.

The sub-ledger is itself WORM — auditors can verify retroactively
that the audit log has not been tampered with. The sub-ledger
checkpoint is anchored alongside the data ledger checkpoint in
the monthly Rekor bundle.

This satisfies SOC 2 PI4 (Processing Integrity — Outputs) and
the BCSC continuous-disclosure rule that material reads are
externally provable.

---

## 9. Bootstrapping a new tenant

Per-tenant `FS_LEDGER_ROOT/<moduleId>/` is created on first
`open()`:

1. Verify directory is empty or contains a valid prior state.
2. Initialise empty leaf tile, checkpoint at tree size 0, signed
   by the tenant key.
3. On reload (subsequent `open()` calls): read the latest
   checkpoint, validate the signature, walk tiles back to
   recompute the root, refuse to start if the recomputed root
   doesn't match the signed root (tamper detection at startup —
   SOC 2 PI1 + CC7).

---

## 10. Cryptographic agility

Hash function and signature scheme are **algorithm-agile** to
support future migration without re-formatting tiles:

- **Today:** SHA-256 hashes, Ed25519 signatures.
- **Hash migration path:** SHA-3 family or BLAKE3 — checkpoint
  format includes an algorithm identifier; new tiles use the
  new hash; checkpoints record both algorithms during the
  transition.
- **Signature migration path:** post-quantum (NIST PQC
  standardisation candidate — Dilithium / SPHINCS+) — same
  signed-note format with algorithm-tagged signatures.

This addresses the eIDAS qualified preservation requirement that
proof-of-existence survive "future technological changes" and
DOCTRINE Pillar 2 (100-year readability).

---

## 11. Mapping to Rust modules

```
service-fs/src/
├── main.rs       — Tokio entrypoint (Envelope A) or microkit entry
│                   (Envelope B); env-driven configuration; spins L3
├── http.rs       — L3 wire protocol (axum routes today; MCP layered
│                   later); per-tenant moduleId enforcement; ApiError
├── ledger.rs     — L2 WORM Ledger API (open / append / read_since /
│                   checkpoint / verify_*); append-only invariant;
│                   3 unit tests today
└── (future)
    ├── tile.rs           — L1 storage backend trait + POSIX impl
    ├── checkpoint.rs     — signed-note serialisation + signing
    ├── audit_log.rs      — sub-ledger for ADR-07 read tracking
    └── mcp.rs            — MCP-server protocol layer
```

Today's `src/ledger.rs` is the L2 placeholder (in-memory
`Vec<Entry>`). The module split lands as part of the
post-ratification implementation roadmap — see `RESEARCH.md`
§12.

---

## 12. Read also

- `service-fs/RESEARCH.md` — full synthesis, alternatives
  considered, ten ratification decisions, complete sources list.
- `service-fs/SECURITY.md` — compliance posture (SEC 17a-4(f),
  eIDAS, SOC 2 TSC mapping).
- `service-fs/CLAUDE.md` — operational state, hard constraints,
  inherited rules.
- `service-fs/NEXT.md` — work queue.
- `~/Foundry/conventions/three-ring-architecture.md` — Ring 1
  contract.
- `~/Foundry/conventions/zero-container-runtime.md` — deployment
  shape.
- `~/Foundry/MEMO-2026-03-30-Development-Overview-V8.md` §6.3
  + §7 — service-fs role and moonshot trajectory.
- `~/Foundry/DOCTRINE.md` §IX (SOC 2 / DARP posture); §II.7
  (Invention #7 Integrity Anchor).
