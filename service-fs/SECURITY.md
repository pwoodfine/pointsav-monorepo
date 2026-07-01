# SECURITY.md — service-fs compliance posture

> **Status:** Ratified at workspace tier 2026-04-26 — design
> convention authored at `~/Foundry/conventions/worm-ledger-design.md`
> (workspace v0.1.7 / Doctrine v0.0.3, commit `6c0b79a`); external
> WORM standards landed in DOCTRINE §IX (workspace v0.1.6 / Doctrine
> v0.0.3, commit `ecee9fb`); this posture statement reviewed and
> accepted with no contradictions per Master's reply 2026-04-26T10:35Z.
> **Last updated:** 2026-04-26
> **Scope:** WORM compliance posture for `service-fs` — what
> external standards it targets, how the design satisfies each, and
> what is NOT promised today.
> **Per CLAUDE.md §6 BCSC posture:** every claim about future
> certification or capability uses planned/intended language with a
> stated reasonable basis.

---

## 1. The compliance claim, in one sentence

`service-fs` is the per-tenant Ring 1 WORM (Write-Once-Read-Many)
Immutable Ledger for the Totebox Archive. It is designed to
satisfy the WORM requirements of two external standards plus the
SOC 2 Trust Services Criteria most relevant to immutable storage,
without depending on any proprietary cloud service or managed
runtime.

Per MEMO §6.3 line 194: *"strictly programmed as 'Read/Append-
Only.' It physically lacks the ability to delete records, ensuring
absolute Write-Once, Read-Many (WORM) legal compliance."*

---

## 2. External standards targeted

### 2.1 SEC Rule 17a-4(f) — US broker-dealer electronic recordkeeping

The canonical US WORM standard. Originally mandated WORM-only
storage; the **2022 SEC amendment** (effective 2023-01-03,
compliance 2023-05-03) modernised the rule to allow either:

- **WORM path** — non-rewriteable, non-erasable electronic records.
  Storage substrate itself denies modification.
- **Audit-Trail alternative** — records may be modified or deleted
  but every change is logged in a complete, time-stamped audit
  trail preserving the original record, all changes, and the
  identity of the change-maker.

**`service-fs` targets the WORM path,** not the Audit-Trail
alternative. The Audit-Trail loophole exists for vendors whose
storage cannot guarantee true immutability (typically SaaS
providers running on mutable cloud blob storage). `service-fs`'s
storage substrate is structurally non-rewriteable through
cryptographic hash-chain immutability + filesystem-level
write-once enforcement.

References:
- [SEA Rule 17a-4 and Related Interpretations | FINRA.org](https://www.finra.org/rules-guidance/guidance/interpretations-financial-operational-rules/sea-rule-17a-4-and-related-interpretations)
- [SEC.gov | Amendments to Electronic Recordkeeping Requirements for Broker-Dealers](https://www.sec.gov/investment/amendments-electronic-recordkeeping-requirements-broker-dealers)

### 2.2 eIDAS Qualified Preservation Service — EU long-term electronic preservation

The EU equivalent of WORM, with stronger long-term-preservation
requirements. Three layered specs apply:

- **EU Commission Implementing Regulation 2025/1946** — in force
  2026-01-06; lays down rules for qualified preservation services
  for qualified electronic signatures and seals.
- **ETSI TS 119 511 v1.2.1 (2025-10)** — policy and security
  requirements for trust service providers providing long-term
  preservation of digital signatures or general data using digital
  signature techniques.
- **CEN TS 18170:2025** — functional requirements for designing
  and managing compliant, reliable, and interoperable archiving
  services.
- **ETSI EN 319 401 v3.2.1 (2026-01)** — general policy
  requirements for trust service providers (the underlying
  framework).

**Why eIDAS matters for `service-fs`:** the EU framework requires
preservation evidence to demonstrate "long-term integrity,
authenticity, proof of existence and accessibility... irrespective
of future technological changes." This is the closest external
standard to DOCTRINE Pillar 2 (100-year readability). Foundry's
plain-text tile format + algorithm-agility design (SHA-256 today,
BLAKE3/SHA-3 tomorrow) addresses this directly.

References:
- [ETSI EN 319 401 V3.2.1 (2026-01)](https://www.etsi.org/deliver/etsi_en/319400_319499/319401/03.02.01_60/en_319401v030201p.pdf)
- [Commission Implementing Regulation (EU) 2025/1946](https://www.eurlexa.com/act/en/32025R1946/present/text)

### 2.3 SOC 2 Trust Services Criteria (CC1–CC9 + PI1–PI5)

Foundry's broader audit posture per DOCTRINE §IX. The criteria
most touched by `service-fs` storage:

| Criterion | What it requires | How `service-fs` satisfies it |
|---|---|---|
| **CC6** Logical and Physical Access | Per-tenant access control | `X-Foundry-Module-ID` header check today; seL4 capability-based access long-term |
| **CC7** System Operations | Change detection + monitoring | Merkle inclusion + consistency proofs detect retroactive modification |
| **PI1** Processing Integrity — Inputs | Append authorisation | Per-tenant moduleId enforcement at the WORM layer (rejection at ingest) |
| **PI4** Processing Integrity — Outputs | Read traceability | ADR-07 audit-log sub-ledger captures every read with moduleId + request-id + cursor + timestamp |

DOCTRINE §IX states the workspace is **SOC 2 audit-ready** and
**DARP-aligned** from v0.0.1. Formal SOC 3 attestation requires
a SOC 2 Type 2 audit first (~$91k for orgs <50 employees per the
DOCTRINE estimate) and is pursued when org size justifies the
spend. `service-fs`'s posture supports a future audit; no
external attestation today.

---

## 3. Foundry-internal standards `service-fs` satisfies

| Standard | Source | How `service-fs` satisfies it |
|---|---|---|
| **WORM legal compliance** | MEMO §6.3 line 194 | Append-only invariant enforced at the Rust API surface; on-disk filesystem-immutable tiles; cryptographic hash chain detects retroactive modification |
| **DARP — Data Archive Retrieval Protocol** | DOCTRINE §IX line 462 | Plain-text tile format (C2SP tlog-tiles per RFC 9162 v2); base64-encoded SHA-256 hashes; readable with `cat`, `xxd`, `jq` and standard tooling |
| **ADR-07 zero AI in Ring 1** | CLAUDE.md §6 | No LLM inference, no embedding-based filtering, no AI-assisted normalisation anywhere in this crate |
| **Pillar 1 plain text only** | DOCTRINE §I | Tiles are text. Checkpoints are text. Audit log is JSONL |
| **Pillar 2 100-year readability** | DOCTRINE §II claim 2 | Format documented in published RFCs; SHA-256 + base64 + JSON are 1990s-vintage primitives that any future archivist can decode |
| **Doctrine §IV.b strict per-tenant isolation** | DOCTRINE §IV.b | Today: separate process per moduleId + filesystem permissions. Long-term: seL4 capability-mediated access — structurally cannot route across tenants |
| **Invention #7 Integrity Anchor** | DOCTRINE §II.7 | Tile-based log checkpoints feed directly into the monthly Sigstore Rekor v2 anchor bundle (Rekor v2 IS itself a tile-based log; same format end-to-end) |
| **BCSC continuous-disclosure posture** | `conventions/bcsc-disclosure-posture.md` | Append signatures + monthly external Rekor anchoring + read-audit sub-ledger together provide continuous proof-of-state suitable for NI 51-102 review |

---

## 4. What is NOT promised today

Stated explicitly so this posture statement does not overclaim:

- **No formal SOC 2 Type 2 attestation** — design is audit-ready;
  formal audit is a v1.0.0 trajectory item per DOCTRINE §VIII
  versioning table (Q4 2027 target).
- **No formal eIDAS qualified preservation service designation** —
  qualified status requires a Trust Service Provider audit + EU
  member-state authority approval. Design is aligned; designation
  is a future commercial decision.
- **No SEC 17a-4(f) attestation as a Designated Third Party (D3P)** —
  the 2022 amendment removed the D3P designation requirement for
  most cases; an internal Compliance Officer Letter is still
  required. Foundry-affiliated entities are not currently
  reporting issuers; the discipline supports a future filing
  without retrofit per DOCTRINE §IX BCSC posture.
- **No quantum-resistant signatures yet** — checkpoint signing
  uses Ed25519 today (pre-quantum). Algorithm-agility in the
  checkpoint format supports a future post-quantum migration
  (Dilithium or SPHINCS+ candidate per NIST PQC standardisation)
  without re-formatting tiles.
- **No third-party witness today** — Foundry workspace anchors to
  Sigstore Rekor monthly per Invention #7. Adding additional
  independent witnesses (e.g., Customer-chosen) is a
  worm-ledger-design D5 decision pending Master ratification.

---

## 5. Per-tenant boundary mechanism

Per Doctrine §IV.b and the cluster manifest, `service-fs` is
**per-tenant by infrastructure**, not by policy:

- **Today (Envelope A — Linux/BSD daemon):** one `service-fs`
  process per `moduleId`. Each process holds its own
  `FS_LEDGER_ROOT` directory with filesystem permissions
  restricting cross-tenant access. Request-time
  `X-Foundry-Module-ID` header check rejects mismatches with 403
  (see `src/http.rs::enforce_module_id`).
- **Long-term (Envelope B — seL4 Microkit unikernel):** each
  per-tenant `service-fs` Protection Domain holds a capability
  for its own per-tenant storage objects in `moonshot-database`.
  Cross-tenant access requires capability transfer, which is
  structurally impossible without explicit grant — enforced by
  the seL4 microkernel itself, formally verified.

The seL4 capability model upgrades the per-tenant boundary from
"enforced by header check + filesystem permissions" to
"mathematically impossible without a granted capability" — a
substantial security improvement on the long-term Totebox path.

---

## 6. Design surface that supports the compliance claim

Brief overview; full architecture in `ARCHITECTURE.md`; full
synthesis with alternatives in `RESEARCH.md`.

The compliance claim depends on four design properties:

1. **Append-only invariant at the Rust API surface** — no public
   method on `WormLedger` mutates or deletes a previously-
   persisted entry. Today enforced by absence of such methods +
   3 unit tests (`src/ledger.rs::tests`); long-term enforced by
   filesystem-level immutability of finalised tiles.
2. **Cryptographic chaining via Merkle tree** — every entry is
   hashed and chained into a Merkle tree per the C2SP tlog-tiles
   spec. Inclusion proofs prove an entry is present; consistency
   proofs prove the log has not been retroactively modified.
3. **External witnessing via Sigstore Rekor v2** — monthly anchor
   bundles include `service-fs` per-tenant checkpoints, posted to
   a public transparency log. Anyone can verify post-hoc that a
   given log state existed at a given time under a given identity.
4. **Plain-text format for accessibility** — tile files are
   readable with standard Unix tooling. Checkpoint files are
   signed-note format (text). Audit-log sub-ledger is JSONL.

Any one of these four properties failing reduces the compliance
claim. All four together compose the WORM posture.

---

## 7. Threat model — what this posture defends against

| Threat | Defence |
|---|---|
| Operator deletes records to hide history | Filesystem-immutable finalised tiles + cryptographic hash chain. Even if an operator has root, the public Rekor anchor preserves the tampered-with state's hash; consistency proof against the new state fails publicly |
| Storage corruption (disk bit-flip, etc.) | Per-entry hash check on read; checkpoint signature on the tree root detects any silent corruption |
| Cross-tenant data leak | Per-tenant moduleId enforcement (today: header check; long-term: capability isolation) |
| Hash function deprecation | Algorithm-agility in checkpoint format; new tiles use new hash; checkpoints record both algorithms during transition |
| Quantum break of signature scheme | Algorithm-agility in signature format; pre-quantum Ed25519 today; post-quantum migration via algorithm-tagged signed-notes (NIST PQC candidate) |
| Forge-and-replace attack on the entire log | Sigstore Rekor anchor preserves prior log shape; consistency proof against the forged state fails on any subsequent verification |

What this posture does NOT defend against (out of scope):

- **Pre-write tampering** — if a producer (e.g., service-email)
  writes garbage, the WORM ledger will permanently store that
  garbage. Producer-side data validation is the producer's
  concern, not service-fs's.
- **Side-channel attacks on the underlying hardware** — covered
  at a lower layer (system-substrate + seL4 mathematical
  verification of the kernel; out of scope for service-fs itself).
- **Key compromise of the per-tenant signing key** — covered by
  identity-store discipline (CLAUDE.md §3) + per-tenant key
  rotation procedures (workspace-tier; not in scope here).

---

## 8. References

### Foundry-side

- `~/Foundry/DOCTRINE.md` §IX (SOC 2 / SOC 3 / DARP Posture);
  §II.7 (Invention #7 Integrity Anchor); §II claim 2 (100-year
  readability); §IV.b (strict per-tenant isolation).
- `~/Foundry/MEMO-2026-03-30-Development-Overview-V8.md` §6.3
  (service-fs WORM compliance language line 194); §7 (moonshot
  trajectory: vendor-sel4-kernel → moonshot-kernel; Sled →
  moonshot-database).
- `~/Foundry/CLAUDE.md` §3 (identity / SSH signing); §6 (ADR
  hard rules including ADR-07 zero AI).
- `~/Foundry/conventions/bcsc-disclosure-posture.md` —
  continuous-disclosure operational form.
- `~/Foundry/conventions/three-ring-architecture.md` — Ring 1
  contract.
- `~/Foundry/conventions/zero-container-runtime.md` — deployment
  shape.
- `service-fs/RESEARCH.md` — full synthesis with alternatives,
  ten ratification decisions, and complete sources list.
- `service-fs/ARCHITECTURE.md` — proposed four-layer stack
  overview.

### External standards (full URLs)

US:
- [SEA Rule 17a-4 and Related Interpretations | FINRA.org](https://www.finra.org/rules-guidance/guidance/interpretations-financial-operational-rules/sea-rule-17a-4-and-related-interpretations)
- [SEC.gov | Amendments to Electronic Recordkeeping Requirements for Broker-Dealers](https://www.sec.gov/investment/amendments-electronic-recordkeeping-requirements-broker-dealers)
- [Crawling into modernity: SEC amends WORM recordkeeping requirements | Davis Polk](https://www.davispolk.com/insights/client-update/crawling-modernity-sec-amends-worm-recordkeeping-requirements-broker-dealers)

EU:
- [Commission Implementing Regulation (EU) 2025/1946](https://www.eurlexa.com/act/en/32025R1946/present/text)
- [ETSI EN 319 401 V3.2.1 (2026-01)](https://www.etsi.org/deliver/etsi_en/319400_319499/319401/03.02.01_60/en_319401v030201p.pdf)

SOC 2:
- [SOC 2 Trust Services Criteria: The Complete CC1-CC9 Reference Guide](https://truvocyber.com/blog/soc-2-trust-services-criteria-guide)
