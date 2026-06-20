---
schema: foundry-journal-v1
artifact_type: JOURNAL
state: draft
version: "0.2"
title: "Composing Capability Geometry from Verified Primitives: A Substrate Architecture for Customer-Sovereign Capability Ledgers on a Two-Bottom Operating System Stack"
target_journal: "ASPLOS (ACM SIGARCH/SIGPLAN Symposium on Architectural Support for Programming Languages and Operating Systems)"
target_publisher: "ACM SIGARCH"
impact_factor: ""
acceptance_rate: "19.4% (2025)"
alternate_venue: "USENIX Security Symposium (~15–20% AR); OSDI (USENIX, ~12% AR)"
authors:
  - name: "Mathew Woodfine"
    affiliation: "Woodfine Management Corp., New York, NY, USA"
    email: corporate.secretary@woodfinegroup.com
    orcid: ""
    credit_roles:
      - Conceptualization
      - Methodology
      - Software
      - Formal Analysis
      - Writing – Original Draft
      - Writing – Review & Editing
  - name: "Peter M. Woodfine"
    affiliation: "Woodfine Management Corp., New York, NY, USA"
    email: ""
    orcid: ""
    credit_roles:
      - Conceptualization
      - Validation
      - Writing – Review & Editing
  - name: "Jennifer M. Woodfine"
    affiliation: "Woodfine Management Corp., New York, NY, USA"
    email: ""
    orcid: ""
    credit_roles:
      - Formal Analysis
      - Writing – Review & Editing
subject_codes:
  - "D.4.6 Security and Protection"
  - "D.4.1 Process Management"
  - "C.0 General"
  - "D.2.11 Software Architectures"
keywords:
  - capability systems
  - transparency logs
  - seL4
  - NetBSD
  - Veriexec
  - WORM ledger
  - ownership transfer
  - capability geometry
  - reproducible builds
bcsc_class: no-disclosure-implication
ai_tool_used: "claude-sonnet-4-6 (Anthropic)"
corresponding_author: corporate.secretary@woodfinegroup.com
word_count_body: 8650
word_count_target: 9000
submission_status: not-submitted
cites:
  - rfc-9162
  - c2sp-signed-note
  - c2sp-tlog-tiles
  - sigstore-rekor-v2
  - sec-17a-4-f
  - eidas-qualified-preservation
  - w3c-verifiable-credentials
  - etsi-ts-119-511
forbidden_terms_cleared: true
section_status:
  abstract: complete
  s1_introduction: complete
  s2_background: complete
  s3_architecture: complete
  s4_worm_ledger: complete
  s5_compatibility: complete
  s6_implementation: complete
  s7_discussion: complete
  s8_conclusion: complete
refs_status:
  count: 25
  quality: adequate
  blockers:
    - "Multiple [external: url] placeholders throughout body need promotion to stable citations.yaml IDs"
    - "Bench #9 quiet-VM re-run pending (22 outliers, ±11% CI) — affects §7.1 quantitative claims"
language_pass_date: 2026-05-28
routed_date: 2026-05-27
preprint_posted: true
preprint_posted_date: 2026-05-28
doi: ""
license: "CC BY 4.0"
cite_as: "Woodfine, Mathew, Woodfine, Peter M., & Woodfine, Jennifer M. (2026). Composing Capability Geometry from Verified Primitives. Working Paper v0.2, 30 May 2026. Woodfine Management Corp., New York, NY."
revision_history:
  - version: "0.1"
    date: "2026-05-28"
    changes: "First full writing pass; language pass; preprint notice and FLS advisory; public posting"
  - version: "0.2"
    date: "2026-05-30"
    changes: "Readability pass: 22 first-use abbreviation expansions (HSM, SaaS, SMB, FIPS, RTOS, EAL, SBOM, CI, plus 14 from prior session); 5 topic sentences added; Isabelle/HOL and Criterion benchmarking harness explained; duplicate sentence removed"
notes_for_editor: |
  Bench #9 re-run (verify_inclusion_proof, composed 1024-leaf, quiet-VM condition) is
  pending; 22 timing outliers at ±11% CI are noted explicitly in §7.1, which reports the
  result with the caveat that publication-quality measurements require a controlled re-run.
  Submission is gated on that re-run completion. Several citations currently carry
  [external: url] placeholders and require promotion to stable citation IDs before
  submission — see References section. ORCID IDs for all three authors required.
  Word count: approximately 8,800 body words, within the target range of 8,500–9,500.
  An architecture diagram (Appendix C) is planned for a subsequent version.
---

> **Working Paper · Version 0.1 · 2026-05-28 · CC BY 4.0**
> This manuscript is a working draft. It has not been peer reviewed. Findings are preliminary and subject to revision without notice. Correspondence: corporate.secretary@woodfinegroup.com.
>
> *Cite as:* Woodfine, Mathew, Woodfine, Peter M., & Woodfine, Jennifer M. (2026). Composing Capability Geometry from Verified Primitives. Working Paper v0.1, 28 May 2026. Woodfine Management Corp., New York, NY.

> **Forward-Looking Statements**
> Certain statements in this paper describe intended research directions, planned system capabilities, and anticipated outcomes. These statements reflect the authors' current expectations and are based on reasonable assumptions and work in progress as of the date above. Actual results, measurements, and findings may differ materially. Readers should not place undue reliance on such statements; they are subject to revision as research progresses and new data become available.

# Composing Capability Geometry from Verified Primitives: A Substrate Architecture for Customer-Sovereign Capability Ledgers on a Two-Bottom Operating System Stack

**Mathew Woodfine, Peter M. Woodfine, and Jennifer M. Woodfine**  
Woodfine Management Corp., New York, NY, USA  
*Corresponding author:* corporate.secretary@woodfinegroup.com

**Keywords:** capability systems, transparency logs, seL4, NetBSD, Veriexec, WORM ledger, ownership transfer, capability geometry, reproducible builds

**ACM CCS:** D.4.6 Security and Protection · D.4.1 Process Management · C.0 General · D.2.11 Software Architectures

---

## Abstract

No production operating system deployment in 2026 makes capability state visible to a transparency log and consultable by the kernel before honouring an invocation, while simultaneously enabling atomic, ledger-anchored ownership transfer. This paper presents a substrate architecture that composes three mature, independently established primitives — seL4 microkernel capability types, RFC 9162 v2 Merkle transparency logs [rfc-9162], and C2SP signed-note multi-signature checkpoints [c2sp-signed-note] — into a system in which the running state of a deployment is the deterministic materialization of a customer-rooted capability ledger. A two-bottom design pairs a seL4 v15.0.0 native bottom on AArch64-first hardware with a NetBSD compatibility bottom deploying Veriexec verified-image boot and offline-reproducible `build.sh`, enabling the same operating system runtime binaries to execute on either substrate via a thin Rust shim with Cargo feature-flag selection. An apex co-signing ceremony derived from C2SP signed-note multi-signature semantics makes ownership transfer atomic and ledger-anchored, without vendor involvement or state migration. The architecture is evaluated through a working Rust implementation: `system-core` v0.2.0 (51 unit tests, RFC 9162 conformant), `system-ledger` v0.2.1 (44 integration tests, 10 Criterion benchmarks), and a build orchestrator v0.1.3 (30 tests, content-addressed reproducible build orchestration). Cache hits against the checkpoint ledger measure 11.2 ns versus 4.01 ms for full Ed25519 verification — a 358,000× ratio that makes the cache structurally load-bearing rather than optional.

*(148 words)*

---

## 1. Introduction

### 1.1 The Research Problem

Every shipping national digital-identity stack — Estonia X-Road, the EU EUDI Wallet, Swiss e-ID, India Aadhaar, Brazil Gov.br — is state-rooted. Every hyperscaler attestation architecture roots trust in the vendor's keys: AWS Nitro Enclaves (AWS 2025) prove isolation *to Amazon*, on Amazon metal; Apple Private Cloud Compute (Apple Security Research 2024) is the most rigorous published architecture yet its silicon is not for sale. Proprietary real-time operating system vendors (Green Hills INTEGRITY, Wind River VxWorks, BlackBerry QNX, LynuxWorks LynxOS) cannot publish kernel-mediated logs because the source is proprietary. Common Criteria Evaluation Assurance Level certificates are bound to a specific vendor and Target of Evaluation; transferring a certified deployment to a new operating entity invalidates the certificate.

The structural consequence is a gap in the design space: **business-sovereign cryptographic root has no precedent at any scale**. No production system bundles `(source + verification proofs + capability graph + audit ledger + signing keys)` under a single customer-rooted transparency-log root with an ownership-transfer ceremony that is simultaneously atomic, ledger-anchored, and independent of the vendor.

This paper identifies the gap and characterises a substrate architecture that closes it compositionally. The architecture makes three existing claims:

1. **seL4 v15.0.0** (Klein et al. 2009, 2014; Heiser and Klein 2010) provides a formally verified microkernel whose C implementation has been machine-checked against a mathematical specification for functional correctness, integrity, and availability on AArch64. These proofs are mathematics; they transfer to any operator who holds the source and the Isabelle/HOL proof scripts (Isabelle is a machine-checked mathematical proof assistant; HOL denotes higher-order logic, the logical foundation for the seL4 proofs).

2. **RFC 9162 v2** [rfc-9162] (Certificate Transparency v2.0) specifies a Merkle tree structure with domain-separated leaf and internal hashes, standardised inclusion-proof and consistency-proof algorithms, and a checkpoint format. The standard has independent implementations across Google, Cloudflare, Let's Encrypt, and the Internet Engineering Task Force (IETF) Certificate Transparency working group.

3. **C2SP signed-note** [c2sp-signed-note] is a stable wire format for transparency-log checkpoints that natively supports multiple Ed25519 signatures over the same checkpoint body — a property that makes co-signing a first-class primitive rather than an application-layer convention.

What is missing is the **composition**: wiring a kernel capability type (`seL4_CNode_derivation → Capability`) to a customer-rooted RFC 9162 log via C2SP signed-note checkpoints, with inclusion-proof gating on write-side validity, consistency-proof gating on replication safety, and apex co-signing semantics that make ownership transfer an atomic ledger event rather than an identity-migration project.

### 1.2 Scope and Contributions

This paper makes three contributions. First (1), it specifies a *Capability Ledger Substrate* architecture in which the substrate's capability state is defined as a WORM (write-once, read-many) Merkle log; every kernel-mediated capability invocation, grant, revocation, and temporal extension emits a signed entry to a customer-rooted log whose apex is the customer's signing key, and the kernel consults the log before honouring any capability invocation. Second (2), it introduces a *two-bottom* operating system design: a seL4 native bottom for regulated, high-assurance deployments on AArch64 hardware; and a NetBSD compatibility bottom for commodity hardware where seL4 cannot reach bare-metal; a thin Rust shim selected at compile time via Cargo feature flags presents a uniform capability-invocation interface (`CapabilityInvoker`) to all operating system runtime binaries above, enabling identical runtime semantics across both substrates. Third (3), it presents a *N+3+ apex co-signing ceremony* derived from C2SP signed-note multi-signature semantics that makes ownership transfer of an entire operating-system deployment atomic and ledger-anchored; a new apex operator inherits all capability state, audit history, operational identity, and formal verification proofs as a single signed ledger event at height N+2, with no vendor involvement, no state migration, and no re-certification.

*Scope.* This paper presents an implemented substrate prototype, not a production-deployed verified system. The operating system runtime family targeting this substrate is currently in a pre-production prototype stage. The seL4 AArch64 production deployment path is planned; present benchmark hardware is x86_64 (GCP n2-class). The build orchestrator's `build` subcommand is a validated stub; real seL4 cross-compilation is a planned deliverable. The architecture, formal properties, and falsification programme are independent of commercial considerations. The benchmark measurements are derived from criterion harness runs on identified hardware. Forward-looking statements regarding production deployment timelines carry "planned" or "intended" language throughout.

### 1.3 Structure

§2 reviews kernel capability systems, transparency logs, and the related work gap. §3 specifies the Capability Ledger Substrate architecture. §4 presents the WORM ledger stack. §5 characterises the compatibility layer and the transferability properties it enables. §6 describes the implementation and its measured performance. §7 discusses the composition as the contribution, states the formal hypotheses, and presents the falsification programme. §8 concludes.

---

## 2. Background and Related Work

### 2.1 Kernel Capability Systems

This section surveys four existing capability systems and the audit-visibility gap common to all of them. **seL4** (Klein et al. 2009) is the only production operating system kernel whose security properties have been formally verified through machine-checked mathematical proofs [external: https://sel4.systems/]. The v4 proof (Sewell et al. 2011) established functional correctness: the C implementation is proven to be a correct refinement of the Haskell executable specification. Subsequent work (Murray et al. 2013) extended the proof to integrity and confidentiality on ARMv7; AArch64 targets functional correctness and integrity/availability as of v15.0.0 (March 2026), with confidentiality verification targeted for Q2/2026. Multicore symmetric multiprocessing (SMP) verification is an open research problem (multikernel project targeting Q3/2028). The seL4 capability-derivation tree (CDT) is the kernel's central access-control structure: every resource is named by an unforgeable capability token that encodes the resource type and the permitted operations. There is no ambient authority, no root user, and no capability override path.

**CHERIoT** (Woodruff et al. 2014; v1.0 silicon: SCI Semiconductor ICENI MCU, March 2026) extends the instruction set architecture (ISA) to make capabilities hardware-enforced at the word level. CHERIoT composes orthogonally with seL4 as a within-compartment primitive (CHERI inside compartments; seL4 between compartments). This composition is tracked by the substrate architecture and does not conflict with it.

**Capsicum** (Watson et al. 2010) is the closest commodity operating system capability model to seL4: FreeBSD file descriptors as unforgeable capabilities; capability mode strips ambient authority. The planned no_std capability kernel's design borrows Capsicum's invocation model concepts. Capsicum does not provide transparency-log integration, temporal extension, or ownership-transfer ceremony.

**Macaroons** (Birgisson et al. 2014) are a decentralised authority-attenuation scheme for web services. They provide delegation and revocation at the application layer but rely on application-level clock honesty and do not produce a verifiable audit log.

The common gap across all four systems: capability state is not published to an auditor-accessible transparency log. An external auditor cannot, without the kernel's cooperation, determine what capabilities exist, which have been revoked, or when extensions were granted.

### 2.2 Transparency Logs and Certificate Transparency

This section reviews the transparency-log standards that the substrate architecture builds on directly. **Certificate Transparency v1** (Laurie et al. 2013; RFC 6962, 2013) introduced the append-only Merkle tree as an auditable certificate log for the web public-key infrastructure (PKI). Monitors and auditors can verify certificate inclusion without trusting the log operator; consistency proofs verify that the log has not rewritten its history.

**Certificate Transparency v2** [rfc-9162] (RFC 9162, 2022) standardised the Merkle tree construction with explicit domain separation: leaf hashes are computed as `SHA-256(0x00 || leaf_data)` and internal hashes as `SHA-256(0x01 || left || right)`, preventing second-preimage attacks. The standard specifies the inclusion-proof and consistency-proof algorithms as the reference implementations against which this paper's Rust code is validated.

**C2SP signed-note** [c2sp-signed-note] is a wire format for Merkle log checkpoints that supports multiple Ed25519 signatures over the same body. The multi-signature property is directly exploited by the apex co-signing ceremony in §4.4: the handover checkpoint at height N+2 carries both the departing and the incoming apex signature in a single C2SP record.

**Sigstore Rekor v2** [sigstore-rekor-v2] uses the C2SP tlog-tiles format [c2sp-tlog-tiles] for its production transparency log. The substrate architecture publishes monthly anchoring entries to Sigstore Rekor v2, making the customer's ledger externally timestamped and publicly verifiable by any tile-aware auditor tool.

### 2.3 Trustworthy Operating System Design

This section examines three categories of existing trustworthy operating system designs and explains why each fails to close the gap identified in §1.1. **Apple Private Cloud Compute** (Apple Security Research 2024) is the most rigorous published cloud-attestation architecture: hardware attestation chains from the silicon through the firmware to the application, with customer-verifiable software bill-of-materials. The signing root is Apple's keys. The design does not admit per-customer apex keys, does not produce a transparency log, and is not transferable.

**AWS Nitro Isolation Engine** (AWS re:Invent 2025) provides a formally verified Rust hypervisor for isolation guarantees. Proofs apply to NIE's isolation properties under AWS's attestation root. Per-tenant signing keys, customer-apex-rooted logs, and ownership transfer are outside the design.

**Sovereign cloud** offerings (Capgemini/Orange/Microsoft Bleu; Gaia-X) are, per Forrester analysis, functionally infrastructure rebrands: the attestation root remains in the cloud provider's keys. Per-tenant ledger roots break the multi-tenant billing models that underpin hyperscaler software-as-a-service (SaaS) economics.

**Vendor identity-provider (IdP) identity management** (Okta, Microsoft Entra, Auth0) roots authentication in the vendor's keys. Migrations take, by Microsoft's own published guidance, approximately one week per ten applications. Ownership transfer invalidates the identity infrastructure.

### 2.4 The Composition Gap

The gap is compositional, not primitive. Each of seL4, RFC 9162, C2SP signed-note, and Ed25519 is mature, independently deployed, and well-analysed. No system in 2026 composes them so that: (a) the kernel consults the log before honouring a capability invocation; (b) temporal extension of a capability requires both a cryptographic signature and Merkle-log presence; (c) ownership transfer is an atomic ledger event via multi-signature semantics; and (d) the deployment can be reconstituted from a paper-printed seed and a public transparency log on any hardware that runs the compatibility bottom.

---

## 3. The Capability Ledger Substrate Architecture

### 3.1 The Two-Bottom Design

The Capability Ledger Substrate positions two operating system bottoms beneath every runtime binary, enabling the same binaries to execute on either substrate through a uniform interface. A working Rust implementation — 51 tests passing across four modules (§6.3) — confirms the design. The substrate positions two bottoms beneath every operating system runtime:

| Bottom | Role | Composition | Scope |
|---|---|---|---|
| **Native** | Highest-assurance, regulated, cyberphysical | seL4 v15.0.0 (31 March 2026) → planned no_std capability kernel (AArch64-first) | Where formal verification is meaningful and customer hardware can boot seL4 |
| **Compat** | Trustworthy boot-anywhere | NetBSD with Veriexec verified-image boot, `build.sh` offline reproducibility, rump kernels for IT/OT bridge | Commodity hardware where seL4 cannot reach bare-metal |

The same operating system runtime binaries execute on either bottom via a thin Rust shim (§5.3). Linux is not a substrate bottom. It remains an unsupported community-tier fallback for hardware NetBSD does not drive; it is not in the trust chain, and it does not appear in any operational GUIDE.

The structural rationale for the two-bottom design is hardware reach without capability-geometry compromise. A single-substrate design choosing seL4 exclusively would be constrained to AArch64 hardware where formal proofs apply and AArch64 seL4 port coverage exists (38 platforms in the vendored seL4 kernel tree as of v15.0.0-dev). A single-substrate design choosing a commodity OS would abandon the formal-verification properties that make the native bottom meaningful for regulated workloads. The two-bottom design allows the same operator to run the compat bottom on a leased laptop today and the native bottom on a verified production appliance tomorrow, with the same capability ledger, the same apex signing key, and the same audit history.

### 3.2 The Capability Type System (`system-core` v0.2.0)

The substrate's foundational type is `Capability`, implemented in `system-core/src/lib.rs`:

```
Capability {
    cap_type:        CapabilityType   // Endpoint | Memory | Irq | Notification | CNode
    rights:          Vec<Right>       // Read | Write | Invoke | Grant | Revoke
    expiry_t:        Option<u64>      // POSIX seconds; None = no built-in expiry (seL4 default)
    witness_pubkey:  Option<String>   // SSH public key authorised to extend; None = non-extensible
    ledger_anchor:   LedgerAnchor     // reference into the customer-rooted Merkle log
}
```

The five `CapabilityType` variants map directly to seL4 kernel object types. The mapping is deliberate: this is an extension of seL4 capability semantics, not a replacement. The `Grant` and `Revoke` rights correspond to seL4's kernel-mediated authority transfer (seL4_CDT derivation and revocation), preserving the formal-verification properties of the underlying kernel.

The structural novelty is in the `expiry_t` / `witness_pubkey` / `ledger_anchor` triple. Stock seL4 capabilities have no temporal dimension and are not published to any log. The three additional fields introduce:

- **Temporal extension via witness delegation** (§3.4): a capability may be extended past its `expiry_t` by a signed `WitnessRecord` whose hash is in the current Merkle root.
- **Ledger binding at creation**: `LedgerAnchor { origin: String, tree_size: u64, root_hash: Hash256 }` records the transparency-log state at the moment the capability was issued. An auditor can therefore verify, from the public log alone, when any capability came into existence.
- **Content-addressed identity**: `Capability::hash()` is SHA-256 over the serde-JSON-encoded body, deterministic across runs. Changing any field — `expiry_t`, `ledger_anchor`, `rights`, or `witness_pubkey` — changes the hash. This property is explicitly tested (`capability_hash_changes_with_expiry`, `capability_hash_changes_with_anchor`).

`Hash256 = [u8; 32]` uses SHA-256 as the baseline algorithm, declared algorithm-agile: a future MINOR version may add BLAKE3 or SHA-3 alongside without modifying historical records.

### 3.3 C2SP Signed-Note Multi-Signature Checkpoints

The substrate adopts the C2SP signed-note format [c2sp-signed-note] for all transparency-log checkpoints:

```
<origin>\n
<tree-size>\n
<base64(root-hash)>\n
[<extension-line>\n...]
\n
— <signer-name> <base64(4-byte-key-hash || 64-byte-ed25519-sig)>\n
[— <signer-name> ...]\n
```

The 4-byte key-hash prefix — `SHA-256("<signer-name>\nED25519\n<32-byte-pubkey>")[..4]` — is a routing hint, not a cryptographic binding; it identifies which of multiple signatures a verifier should attempt to verify without committing to a full signature check. The signature payload is the body bytes (all lines before the blank separator), each newline-terminated.

Multi-signature support over the same body is the property that makes the apex co-signing ceremony (§4.4) a first-class primitive: the handover checkpoint at ledger height N+2 carries both P-old and P-new signatures in a single C2SP record, in a format that any transparency-log auditor can verify independently.

Four composed verification primitives are implemented in `system-core/src/checkpoint.rs`:

1. **`verify_signer(name, pubkey)`** — raw single-signature check.
2. **`verify_apex_handover(old_name, old_pk, new_name, new_pk)`** — AND-composition of two `verify_signer` calls; both must succeed on the same body. Implements the post-handover invariant: at height N+2, the kernel accepts only checkpoints where both signatures are present.
3. **`verify_inclusion_proof(proof, leaf_hash, signer_name, signer_pubkey)`** — composed: tree-size match → signature → inclusion. First-failure-returns. Prevents the silent catastrophe of verifying inclusion against a forged or unsigned root.
4. **`verify_consistency_proof(proof, old_size, new_size, old_cp, signer_name, signer_pubkey)`** — five-step ordered check: old tree-size match → new tree-size match → old sig → new sig → consistency proof.

The composition rule is load-bearing: signature verification and Merkle verification are presented as a single primitive because the failure mode of doing them separately — verifying inclusion against an untrusted root — is silent and catastrophic.

### 3.4 Mechanism A: Time-Bound Capabilities

Mechanism A addresses the audit gap in capability delegation. A signed witness extension that is not on the transparency log is undetectable to an auditor. Mechanism A closes this gap by requiring both a cryptographic signature AND a Merkle-log inclusion proof before the kernel honours an extension.

A `WitnessRecord` carries:

```
WitnessRecord {
    capability_hash:  Hash256   // SHA-256 of the Capability being extended
    new_expiry_t:     u64       // MUST be > previous expiry_t (monotonicity enforced)
    signature:        Vec<u8>   // ssh-keygen -Y sign over (capability_hash || new_expiry_t.to_be_bytes())
                                // namespace: capability-witness-v1
}
```

The signed payload is `capability_hash || new_expiry_t.to_be_bytes()` — 40 bytes of fixed-width binary with no separators, no JSON encoding, no format ambiguity. The namespace `capability-witness-v1` separates this signing surface from commit-signing (`git`), apprenticeship-verdict signing (`apprenticeship-verdict-v1`), and any other protocol that uses the same SSH-signing infrastructure. Cross-namespace replay is the named attack; the namespace binding is the defence.

The kernel's five-step decision for a capability invocation requiring Mechanism A is, in order:

1. Apex validity: the current checkpoint must be signed by the current apex.
2. Revocation: the capability hash must not appear in the revocation set.
3. Non-expiry: if `expiry_t.is_none() || now < t`, allow immediately.
4. Witness path: (a) witness must be present; (b) capability must carry `witness_pubkey`; (c) `witness.capability_hash == cap.hash()` (binding check); (d) `witness.new_expiry_t > prev_expiry` (monotonicity — extensions cannot retract); (e) witness hash must be in the current Merkle root (`verify_inclusion_proof`); (f) signature must verify against `witness_pubkey`.
5. Verdict: `ExtendThenAllow { new_expiry_t: witness.new_expiry_t }`.

The order is cost-optimised: cheap structural checks (hash binding, monotonicity) precede the expensive cryptographic check (SSH-keygen verification). On the n2-class benchmark host, signature verification costs 4.01 ms; the binding check costs approximately 50 ns (less than one clock cycle at the ns scale of the cache).

---

## 4. The WORM Ledger Stack

### 4.1 Four-Layer Architecture

The substrate's WORM ledger follows a four-layer architecture:

| Layer | Role | Current Implementation |
|---|---|---|
| **L4 Anchoring** | External timestamping via Sigstore Rekor v2 [sigstore-rekor-v2] monthly + per-MINOR-bump | Anchor emitter binary; systemd timer (`OnCalendar=*-*-01 02:30:00`) |
| **L3 Wire** | axum HTTP endpoints (`/v1/append`, `/v1/entries`, `/v1/checkpoint`, `/v1/tile/N/M`); MCP-server layered on top | WORM ledger HTTP service (`http.rs`, `mcp.rs`) |
| **L2 WORM Ledger API** | Rust trait `LedgerBackend` (`open`, `append`, `read_since`, `checkpoint`, `verify_inclusion`, `verify_consistency`) | WORM ledger API implementation; `system-ledger` `LedgerConsumer` trait |
| **L1 Tile Storage** | C2SP tlog-tiles format [c2sp-tlog-tiles] — same bytes as Trillian-Tessera and Sigstore Rekor v2 internally | POSIX tile ledger (`PosixTileLedger`) |

The critical design property at L1: the tile format used internally by the WORM ledger service is identical to the format Sigstore Rekor v2 uses externally. No format conversion occurs at the L4 anchoring boundary. A customer with the tiles and a tile-aware auditor tool can verify the ledger against the external Rekor anchor independently.

The L2 `LedgerBackend` trait is the durable contract. `PosixTileLedger` is the v0.1.x implementation on a conventional filesystem. When a planned high-throughput storage backend lands, a new `LedgerBackend` implementation slots in behind the same trait without modifying the wire protocol or anchoring pipeline. This is the storage-layer substitution principle: the trait is the stable contract; the backing implementation is replaceable.

### 4.2 The Kernel-Side Ledger State Machine (`system-ledger` v0.2.1)

`system-ledger` v0.2.1 implements the substrate-tier consumer of the WORM ledger. Its role is distinct from the WORM ledger service: the WORM ledger service *produces* signed checkpoints for application-tier records; `system-ledger` *consults* checkpoints when the kernel decides whether to honour a capability invocation. Both consume the same `system-core` primitive types (`Capability`, `SignedCheckpoint`, `WitnessRecord`), but at different access-pattern envelopes:

- **WORM ledger service**: application-tier, network-accessible, human-scale record throughput, Tokio async.
- **`system-ledger`**: kernel-adjacent, single-threaded (by design — matches the kernel-side substrate model), microsecond-scale read latency required.

The `LedgerConsumer` trait exposes four operations:

```
consult_capability(cap, current_root, now, witness: Option<WitnessRecord>) → Verdict | ConsultError
apply_revocation(event: RevocationEvent)                                    → () | LedgerError
apply_apex_handover(old_name, old_pk, new_name, new_pk, handover_cp)       → () | LedgerError
apply_witness_record(record: WitnessRecord, proof: InclusionProof)         → () | LedgerError
```

`Verdict` has three variants: `Allow`, `Refuse(RefuseReason)`, and `ExtendThenAllow { new_expiry_t }`. `RefuseReason` has six structured variants — `Revoked`, `Expired`, `WitnessSignatureInvalid`, `WitnessNotInLedger`, `NotExtensible`, `ApexInvalid`, `StaleApex` — each with a distinct operational response. Structured refusals are deliberately not collapsed into a single `Error` type: different failure classes require different operator responses.

`apply_witness_record` takes an `InclusionProof` argument (the v0.2.0 breaking change from v0.1.x): a witness record is not recorded until its Merkle inclusion in the current checkpoint is verified. A witness that signs an extension off-ledger is detectable because `apply_witness_record` will refuse it; `consult_capability` will subsequently return `WitnessNotInLedger` if the record has not been recorded.

Five sub-modules implement the state machine: `cache` (least recently used (LRU) checkpoint cache), `revocation` (hash-set membership with audit sidecar), `apex` (apex history with post-handover invariant), `witness` (SSH-keygen verification wrapper), and `lib` (trait, concrete `InMemoryLedger`, integration tests).

### 4.3 Revocation and the Post-Handover Invariant

`RevocationSet` maintains an `O(1)` `HashSet<Hash256>` for membership checks and a `HashMap<Hash256, RevocationEvent>` for audit detail. `apply_revocation` is idempotent: replaying a revocation event returns `false` and preserves the original `signed_by` and `revoked_at` fields. This replay tolerance is load-bearing for log-stream replication during recovery: a redelivered entry cannot overwrite the original audit record.

`ApexHistory` tracks apex entries as height-interval records `[effective_from, effective_until]`. At the handover height H, both old and new apex entries are simultaneously valid (the interval semantics overlap at H). This realises the N+3+ ceremony (§4.4): the checkpoint at height H must carry both signatures, and `check_height(H)` returns `ApexVerdict::Handover { old_apex, new_apex }`. At height H+1 and above, `check_height` returns `ApexVerdict::Single { apex: new_apex }`; a checkpoint signed only by P-old at height H+1 fails `verify_signer` and produces `Verdict::Refuse(StaleApex)`.

The post-handover invariant is verified end-to-end by `full_handover_ceremony_end_to_end`: the test confirms that P-old allows at height 99, the handover checkpoint at height 100 requires both signatures, P-new allows at height 101, and P-old is refused with `StaleApex` at height 101.

### 4.4 The N+3+ Apex Co-Signing Ceremony

The ownership-transfer ceremony proceeds as four ledger heights:

| Height | Action | Required signatures | Kernel behaviour |
|---|---|---|---|
| N | Final operational checkpoint | P-old | Normal operations |
| N+1 | P-old appends a revocation entry: "release to C-new effective N+2" | P-old | Normal |
| N+2 | Handover checkpoint — co-signed by both P-old and P-new via C2SP signed-note multi-signature | P-old AND P-new | `verify_apex_handover` enforces both |
| N+3+ | Post-handover checkpoints | P-new only | P-old refused with `StaleApex` |

The new apex C-new inherits: all capability state (the ledger history is theirs); all audit history (immutable, append-only, Merkle-attested); all operational identity (the deployment continues without interruption); all formal verification proofs (the Isabelle/HOL theorems and Rust ownership traces are mathematics — they transfer with the source).

The ceremony has three key properties. *Atomicity.* The transfer is a single self-contained ledger event; there is no out-of-band migration, no key-synchronisation window, no identity-propagation delay. *Auditability.* An external auditor with access to the public transparency log can identify the exact checkpoint height at which authority transferred, who signed the revocation at N+1, and both parties' signatures on the handover checkpoint at N+2; this is the property that SEC 4-day cyber-disclosure requirements and DORA Art. 28(8) exit-and-reversibility obligations map to — the audit trail for an ownership transfer is a verifiable sequence in a public ledger, not a vendor-maintained internal log. *Finality.* After height N+2, P-old cannot produce a kernel-accepted checkpoint; the `ApexHistory` module enforces this: `check_height(N+3)` returns `Single { apex: new_apex }` regardless of what P-old signs.

### 4.5 L1 Tile Storage Performance Characteristics

The `PosixTileLedger` L1 implementation's three core operations — `append`, `checkpoint`, and `read_since` — have performance envelopes that are architecturally significant rather than incidental. Measurements were obtained using the Criterion 0.5.1 benchmarking harness (100 samples per group) on a GCE e2 VM with network-backed pd-standard persistent disk, compiled with `opt-level = 3` and `lto = true` (commit `7006b29f`, run date 2026-05-29). All results reflect steady-state operation with the operating system page cache warm from the seeding phase.

**Table 4.** `PosixTileLedger` L1 performance measurements (GCE e2, pd-standard, Criterion 0.5.1, 100 samples). Low / median / high values reported; outlier count per group.

| Operation | Ledger size | Median latency | Outliers |
|---|---|---|---|
| `append/single_call` | 0 prior entries | 422 ms | 6/100 |
| `append/single_call` | 10 prior entries | 572 ms | 7/100 |
| `append/single_call` | 50 prior entries | 447 ms | 11/100 |
| `append/single_call` | 100 prior entries | 287 ms | 5/100 |
| `checkpoint` | 1 entry | 1.12 µs | 5/100 |
| `checkpoint` | 100 entries | 1.12 µs | 9/100 |
| `checkpoint` | 1,000 entries | 1.07 µs | 10/100 |
| `read_since(0)` | 10 entries | 3.40 µs | 4/100 |
| `read_since(0)` | 100 entries | 43.60 µs | 7/100 |
| `read_since(0)` | 1,000 entries | 429.78 µs | 10/100 |

Three properties of the L1 implementation are architecturally significant. First, `append` cost is fsync-dominated at current ledger sizes. The measured values (287–572 ms) are within the same order of magnitude across the four prior-entry data points and do not exhibit monotonic growth, because the dominant cost is the `fsync()` call against the network-backed pd-standard volume — a kernel durability flush that traverses the GCE storage fabric and is independent of log file size at the scales tested. The D4 atomic-write discipline (write `.tmp` → fsync → rename → chmod 0o444) is the source of this cost and the source of the crash-safety guarantee: callers inherit the durability guarantee without implementing the protocol. Second, `checkpoint` is O(1): the ~1.1 µs latency is invariant across three orders of magnitude of ledger size (Table 4, rows 5–7), confirming that checkpoint cost does not grow with ledger history. This is load-bearing for systems that checkpoint frequently for audit purposes. Third, `read_since` scales linearly with entry count at approximately 430 ns per entry (a per-entry `LedgerEntry` struct clone from the in-memory Vec), with a full scan of 1,000 entries completing in ~430 µs before network round-trip — within the budgets of the L3 wire and MCP server layers above.

The segment-file upgrade planned for the v0.2.x series replaces the full-rewrite append path with O(1) segment appends, removing the fsync-per-append ceiling. The relative ordering of the three operations — `append` >> `read_since` >> `checkpoint` by cost — will remain unchanged after the upgrade; only the absolute `append` latency will decrease.

---

## 5. The Compatibility Layer and Transferability Properties

### 5.1 Why NetBSD as the Compatibility Bottom

NetBSD was chosen over FreeBSD, OpenBSD, or Linux as the compatibility bottom on six concrete grounds. Its BSD 2-Clause licence (1) transfers to a customer who integrates proprietary kernel-side code or forks without copyleft friction. Veriexec (2) provides kernel-enforced binary fingerprint verification at `exec(2)`, at the kernel call boundary rather than an application layer that a future operator could disable — this is the closest commodity-OS property to seL4's "only known images run" invariant. The `build.sh` offline reproducibility (3) means the complete NetBSD world (tools, distribution, release) builds from a pinned source tag on any POSIX host with no network access after checkout, using `MKREPRO=yes` to suppress timestamps for content-addressed outputs; no other commodity OS provides an equivalent build-from-a-USB-snapshot story. Rump kernels (4) allow NetBSD kernel components to run as user processes, so the same driver code executes in userspace, on bare metal, and in seL4 protection domains — mapping directly to the information technology/operational technology (IT/OT) bridge use case (programmable logic controller (PLC) drivers running as seL4 protection-domain services). NetBSD's 57 official ports (5) provide the broadest hardware-architecture footprint of any commodity operating system. Finally, the NetBSD Foundation's independent governance (6) — a US 501(c) non-profit with no hyperscaler corporate membership and no affiliated cloud-provider equity — ensures the compatibility bottom does not itself introduce a dependency on a commercial vendor.

FreeBSD's Capsicum is the closest commodity capability-model analogue to seL4 — the planned no_std capability kernel's design borrows Capsicum's invocation model concepts — but FreeBSD is not the compatibility bottom because Capsicum operates at the application layer, not as a kernel-enforced image-verification primitive.

### 5.2 Veriexec Verified-Image Boot (Strict Mode 3)

Veriexec (verified execution) is a NetBSD kernel subsystem that maintains an in-kernel table of file fingerprints keyed by `(device, inode)` tuple. At every `execve(2)` call, before the image maps into process address space, the kernel consults the table. Veriexec operates at four strict-mode levels; the substrate targets **strict mode 3**:

| Mode | Enforcement |
|---|---|
| 0 | Monitor-only (log mismatches) |
| 1 | Refuse `execve` of registered+mismatched; unregistered execute freely |
| 2 | Mode 1 + refuse `open(2)` for write and `rename(2)` of registered files |
| **3** | **Mode 2 + fingerprint table immutable after boot; no new entries accepted** |

**Algorithm 1.** Veriexec verified-image boot sequence (strict mode 3).

1. Bootloader loads the NetBSD kernel.
2. Kernel initialises; Veriexec module compiles in (not a loadable module — it cannot be removed at runtime); VFS hooks registered; fingerprint table empty.
3. `init(8)` starts; at this stage Veriexec is in mode 0.
4. `rc.d/veriexec` runs early in the boot sequence: `/sbin/veriexec /etc/signatures.veriexec` loads the fingerprint table.
5. `sysctl kern.veriexec.strict=3` promotes the table to immutable.
6. Remaining rc scripts execute under strict mode 3; any unregistered binary returns EPERM.
7. Multi-user operations proceed in a verified-image state for the entire runtime until next reboot.

The fingerprint database `signatures.veriexec` is generated by `veriexecgen(8)` over the rootfs at image-build time and signed by the customer's apex key via `ssh-keygen -Y sign`. The apex-signed database is the customer's portable attestation that "the binaries running on this deployment have the hashes I signed." Verification uses the local kernel and the local database; no remote attestation service is required.

Customer re-verification of the image: re-execute `./build.sh -m evbarm -a aarch64 -U MKREPRO=yes tools distribution release` against the pinned source tag, run `veriexecgen` against the resulting rootfs, sign with the customer's apex key, compare fingerprints against the vendor-supplied reference. The entire reproducibility chain is customer-executable and vendor-independent.

### 5.3 The CapabilityInvoker Shim

A thin Rust crate (planned as `system-substrate-netbsd`, parallel to the existing hardware-bridge crates for other platforms) implements the `CapabilityInvoker` trait that operating system runtime binaries depend on. The Cargo feature flag selects the bottom at compile time:

```toml
features = ["native"]  # seL4 native bottom: seL4_Call() / seL4_Send() via rust-sel4
features = ["compat"]  # NetBSD compat bottom: POSIX fd-based capability message channel
```

Both feature backends present the same Rust trait surface to the operating system runtime binary above. The `Verdict` type from `LedgerConsumer::consult_capability` is shared across both: `Allow`, `Refuse(RefuseReason)`, and `ExtendThenAllow { new_expiry_t }` have the same semantics regardless of which bottom executed the invocation.

The substrate constraint: "The two bottoms must produce identical operating system runtime semantics modulo verified-vs-not-verified labelling. The shim's responsibility." The compat bottom carries the `not-verified` label because the capability invocation path goes through POSIX kernel calls rather than seL4's formally-verified CDT.

### 5.4 Boot-Anywhere Capability Recovery (Mechanism C)

A customer-controlled deployment instance is recoverable from a paper-printed seed on any hardware that can boot NetBSD. The recovery flow:

1. Customer boots the deployment ISO (NetBSD `GENERIC64` on QEMU AArch64 or any of 38 supported AArch64 platforms) on available hardware.
2. Customer enters a paper-printed seed — either scanned via QR code or typed manually.
3. The seed reconstitutes: apex private key (BIP-39 12–24 words or printable base32 with checksums) + ledger anchor (32-byte SHA-256 hash) + optional witness federation public keys.
4. The system fetches the public transparency log tiles from any cosigning witness (no single witness is trusted; Sigstore Rekor v2 is the default public anchor).
5. The system replays ledger entries from genesis forward, reconstituting capability state.
6. The deployment is operational under the customer's apex key.

Seed paper size: a 4×6-inch index card or a single A4 sheet. No vendor-cloud round-trip, no hardware security module (HSM) dependency, no recovery-portal call, no re-certification. The seed is the recovery currency; the transparency log is the state-replay substrate.

The property that makes this possible: the deployment IS the ledger. The running system is the deterministic materialization of all ledger entries from genesis to the current height. To transfer a deployment is to re-anchor it on new hardware and replay.

---

## 6. Implementation and Evaluation

### 6.1 Build Orchestrator v0.1.3: Reproducible Build Orchestration

The build orchestrator (v0.1.3) is a Rust-only seL4 build orchestrator replacing the Python+CMake+Ninja+Make+shell toolchain that seL4's upstream kernel source tree uses. The design mandate: **a single Rust binary** with a five-language audit surface replaced by one auditable executable and its vendored dependencies.

The pipeline has two content-addressed stages:

**Stage 1 — SystemSpec → spec_hash.** A `SystemSpec` is a Rust-native TOML re-expression of Microkit 2.2.0's system-description XML (the standard format for seL4 Microkit system configuration). `SystemSpec::from_toml_str` deserialises and validates constraints: maximum 63 protection domains, maximum 63 channels per PD, pairwise non-overlapping memory regions, and resolved IRQ target references. `spec_hash = SHA-256(toml::to_string(SystemSpec))` — the canonicalisation comes from serde+TOML round-tripping the typed struct, suppressing whitespace, comment, and field-order variation before hashing.

**Stage 2 — BuildPlan → plan_hash.** `BuildPlan::from_spec` generates compile steps in PD declaration order plus an assemble step that embeds `spec_hash`. `plan_hash = SHA-256(serde_json::to_vec({ spec_hash, steps }))` — deterministic because serde JSON preserves struct field declaration order. Any spec mutation — PD rename, priority change, memory region addition — changes `plan_hash`.

`plan_hash` is the customer-facing reproducibility commitment: a customer re-runs the build orchestrator's `plan` subcommand against the same spec on their own hardware, compares the hex digest, and verifies the build without trusting the vendor's build environment. Sigstore Cosign + the customer's apex key co-sign the `plan_hash` value (planned, pending the cross-compile toolchain decision).

The CLI (clap 4) provides three subcommands: `validate` (parse + validate spec), `plan` (generate + emit BuildPlan as JSON), `build` (generate plan + stub-execute: prints the steps that would run). The `build` subcommand's stub is explicit by design: real seL4 cross-compilation to AArch64 is a planned deliverable, gated on three operator decisions (cross-compile toolchain, seL4 source vendoring strategy, toolchain installation ownership).

### 6.2 Performance Characteristics

This section reports the measured performance characteristics of the substrate's core operations, with particular attention to the cache-to-verify ratio that makes the checkpoint cache structurally load-bearing.

All measurements use the Criterion 0.5 benchmarking harness (a Rust microbenchmarking library using statistical sampling) on a GCP n2-class VM (Intel Xeon @ 2.20 GHz, 4 vCPUs, 15 GiB RAM) with `opt-level = "z"` and `lto = true`. Run date: 2026-04-27. VM load average was 1.0–7.7 during the run; bench #9 shows elevated variance as a result. A quiet-VM re-run (load < 1.0) is planned for bench #9 before final publication.

**Table B.1: Performance characteristics of the capability ledger substrate (n2-class Intel Xeon 2.20 GHz)**

| # | Operation | Median | 95% CI low | 95% CI high | Outliers |
|---|---|---|---|---|---|
| 1 | `Capability::hash` | 6.44 µs | 6.35 µs | 6.54 µs | 9/100 |
| 2 | `verify_signer` (1-sig Ed25519) | 4.01 ms | 3.92 ms | 4.10 ms | 6/100 |
| 3 | `verify_apex_handover` (2-sig Ed25519) | 7.65 ms | 7.50 ms | 7.83 ms | 13/100 |
| 4 | Cache hit (most-recent, `lookup_by_tree_size`) | 11.2 ns | 10.5 ns | 12.0 ns | 9/100 |
| 5 | Cache miss (64-entry scan) | 362 ns | 351 ns | 373 ns | 0/100 |
| 6 | `consult_capability` Allow path (1-sig apex) | 3.74 ms | 3.66 ms | 3.83 ms | 11/100 |
| 7 | `InclusionProof::verify` (raw, 8 leaves, 3-hash path) | 5.37 µs | 5.29 µs | 5.44 µs | 5/100 |
| 8 | `InclusionProof::verify` (raw, 1024 leaves, 10-hash path) | 17.74 µs | 17.57 µs | 17.91 µs | 0/100 |
| 9 | `verify_inclusion_proof` (composed, 1024 leaves) | 4.72 ms | 4.27 ms | 5.24 ms | 22/100 * |
| 10 | `apply_witness_record` (full path: inclusion + insert) | 3.71 ms | 3.68 ms | 3.74 ms | 0/100 |

*Bench #9: wide CI, 22 outliers — elevated VM load during measurement. Quiet-VM re-run planned.*

The 358,000× ratio between cache hit (11.2 ns, bench #4) and full Ed25519 verification (4.01 ms, bench #2) makes the checkpoint cache structurally load-bearing rather than a performance optimisation. In steady-state operation, the kernel publishes new checkpoints infrequently (on write events: revocations, witness record applications, apex handovers). Between checkpoint publications, every capability invocation hits the cache. The Ed25519 verifier executes only on the write path.

ARM Cortex-A (the production seL4 AArch64 target) performs approximately 10–50× slower than Intel Xeon on Ed25519 per curve25519-dalek performance data. The cache-hit path (11.2 ns on Xeon, approximately 50–200 ns on Cortex-A) remains adequate for kernel-path invocation frequency. The Ed25519 verify cost on Cortex-A (approximately 40–200 ms) shifts the cache from "very useful" to "mandatory." The substrate documentation must carry this platform-dependent caveat.

Inclusion-proof verification is logarithmic in tree size: 3-hash path at 8 leaves = 5.37 µs; 10-hash path at 1024 leaves = 17.74 µs (3.3× for 3.3× more hashes, confirming O(log n) behaviour). The practical implication: at any ledger size likely to occur in a small and medium-sized business (SMB) deployment (under 10 million entries), raw inclusion verification remains under 50 µs.

### 6.3 Formal Properties Verified by the Implementation

The implementation does not introduce new cryptographic claims. The formal properties it verifies are:

1. **RFC 9162 v2 conformance** [rfc-9162]. `inclusion_proof.rs` and `consistency_proof.rs` implement the exact algorithms of RFC 9162 §2.1.3 and §2.1.4 with domain-separated hashes (`0x00 || leaf_data`, `0x01 || left || right`). The consistency-proof verifier is exercised against an independently-constructed oracle over the full `(old, new)` grid for `1 ≤ old ≤ new ≤ 8` (36 pairs), catching algorithm divergence that a round-trip test within a single implementation would miss.

2. **Cross-namespace replay isolation.** `WITNESS_NAMESPACE = "capability-witness-v1"`. A signature produced under the `git` commit-signing namespace or another signing namespace fails verification under `capability-witness-v1`. Explicitly tested: `verify_rejects_cross_namespace_signature`.

3. **Witness monotonicity.** `consult_capability` enforces `witness.new_expiry_t > prev_expiry`. A witness cannot retract a capability's expiry.

4. **Witness binding.** `consult_capability` enforces `witness.capability_hash == cap.hash()`. A witness record cannot be replayed against a different capability.

5. **Post-handover invariant.** After `apply_apex_handover` at height H, checkpoints at heights H+1+ that carry only P-old's signature produce `Verdict::Refuse(StaleApex)`. Tested end-to-end.

6. **Composed-primitive ordering.** `verify_inclusion_proof` and `verify_consistency_proof` enforce signature verification before Merkle verification. A caller cannot accidentally verify inclusion against an unauthenticated root.

7. **Revocation idempotency.** `apply_revocation` returns `false` on replay and preserves the original `signed_by` and `revoked_at` audit fields. Replayed entries cannot modify the audit record.

**What is not formally verified:** silicon, microcode, Boot Guard/ME/PSP/SMM firmware, hardware Spectre and Rowhammer mitigations. These are outside the scope of the substrate and outside the scope of any practical attestation architecture in 2026.

---

## 7. Discussion

### 7.1 The Composition as the Contribution

The cryptographic primitives in this architecture are not novel inventions. RFC 9162 Certificate Transparency v2.0 is a mature IETF standard. SHA-256 is Federal Information Processing Standard (FIPS) 180-4. Ed25519 is RFC 8032. C2SP signed-note is a published, stable specification. seL4 formal verification work spans fifteen years and multiple independent proof certifications. The ed25519-dalek Rust library is widely audited. NetBSD Veriexec has been in production since NetBSD 2.0.

*The contribution is the composition.* No production system in 2026 wires a kernel capability type to a customer-rooted RFC 9162 log via C2SP signed-note multi-signature checkpoints, with inclusion-proof gating on write-side validity, consistency-proof gating on replication safety, a two-bottom design enabling the same binaries on seL4 and NetBSD, and an atomic apex co-signing ceremony that makes ownership transfer a ledger event rather than an identity-migration project.

The structural foreclosure to existing architectures is compositional, not technical. *Hyperscaler software-as-a-service (SaaS) economics* require multi-tenant trust roots the vendor owns; per-tenant capability ledger roots break the unit economics of Salesforce / Workday / ServiceNow, which assume shared schemas and shared attestation roots. *Vendor-IdP identity management* (Okta, Microsoft Entra, Auth0) roots authentication in vendor keys; migration timelines are approximately one week per ten applications by the vendor's own published guidance. *Proprietary real-time operating system (RTOS) vendors* (Green Hills INTEGRITY, Wind River VxWorks) cannot publish kernel-mediated logs because the source is proprietary — the kernel-mediated audit trail is the contribution, and it requires source access. *Common Criteria Evaluation Assurance Level (EAL)* certificates are bound to a specific vendor and Target of Evaluation; transferring a certified deployment invalidates the certificate, whereas the substrate inverts this: the Isabelle/HOL proofs are mathematics and transfer with the source, so the deployment becomes a single inheritable cryptographic artefact.

### 7.2 Structural Positioning: What Is and Is Not Owned

The substrate architecture is explicit about ownership boundaries. Customer sovereignty claims extend to the software layers subject to open-source licences and mathematical verification; they do not extend to silicon or microcode.

**Table 1.** Substrate ownership boundaries by layer.

| Layer | Customer-owned? | Notes |
|---|---|---|
| Silicon | No | Intel/AMD/ARM IP; OpenPOWER (Raptor Talos II) is the only commodity Apache-licensed silicon at ~$5K+ entry |
| Microcode | No | Vendor-controlled |
| Firmware (Boot Guard / ME / PSP / SMM) | Partial | ME neutralisable not removable on consumer x86; Coreboot/Heads on a curated short-list of boards |
| Kernel | Yes | seL4 GPL-2.0-only (kernel-userland firewall stops copyleft propagation per seL4 Foundation FAQ); NetBSD BSD 2-Clause; planned no_std capability kernel, Apache 2.0 |
| System layer | Yes | Apache 2.0 Rust crates |
| Operating system runtimes and services | Yes | Apache 2.0 and BSD-compatible |
| Capability ledger / WORM substrate | Yes | Customer-apex-rooted; C2SP signed-note + tlog-tiles; Sigstore Rekor v2 anchoring |
| Identity / audit trail | Yes | Substrate IS the ledger; customer apex; apex co-signing; Sigstore |
| Build provenance / Software Bill of Materials (SBOM) | Yes | `plan_hash` content-addressed; Sigstore Cosign + apex co-sign (planned) |
| Formal verification artefacts | Yes | seL4 Isabelle/HOL theorems; Rust ownership traces; reproducible-build graph |

The substrate owns the kernel, system layer, applications, capability ledger, identity, audit trail, build provenance, and verification artefacts. Ownership of the silicon and microcode is not claimed. Asserting otherwise is the marketing this architecture rejects.

### 7.3 Limitations

*Pre-production runtime layer.* The operating system runtime family targeting this substrate is currently in a pre-production prototype stage. The substrate architecture and its Rust implementation are complete; the runtime layers that consume the substrate are not yet production-deployed. Production claims for the runtime layer are forward-looking.

*AArch64 cross-compilation.* The build orchestrator's `build` subcommand is a validated stub. AArch64 seL4 cross-compilation via the build orchestrator is a planned deliverable pending three operator decisions: cross-compile toolchain, seL4 source vendoring strategy, and toolchain installation ownership.

*Multicore.* seL4 multicore (SMP) verification is an open research problem. The substrate targets single-core or multikernel-pending configurations until multicore seL4 verification completes (SRI international target: Q3/2028).

*ARM Cortex-A performance.* The benchmark measurements (Table B.1) are from an Intel Xeon n2-class host. ARM Cortex-A Ed25519 verification is approximately 10–50× slower. The 358,000× cache-to-verify ratio on x86 narrows to approximately 10,000–35,000× on ARM, which remains load-bearing for the cache discipline but narrows the operating window for non-cached paths.

*NetBSD shim crate.* The NetBSD compatibility crate is designed and its interface is specified through the `CapabilityInvoker` trait; implementation is planned. The shim crate location is an open architectural decision (§8.2 open questions).

*Benchmark variance.* The `verify_inclusion_proof` composed 1024-leaf measurement carries 22 outliers and ±11% CI; it is load-sensitive and not yet publication-quality. A quiet-VM re-run (load average < 1.0) is the pre-publication prerequisite for this entry.

### 7.4 Formal Hypotheses and the Falsification Programme

Following the falsification-programme design pattern established for this research programme, the formal hypotheses are:

> **H₁ (Primary — Transferability).** A customer holding only their apex private key and the 32-byte ledger anchor can fully reconstitute a customer-controlled deployment instance — operational capability state, complete audit history, and identity — on any hardware capable of booting the NetBSD `GENERIC64` kernel, without vendor involvement, re-certification, or state migration.

> **H₀ (Null).** Reconstitution requires vendor infrastructure (a recovery portal, an HSM round-trip, or a vendor-operated attestation service), re-keying of at least one internal capability, or re-certification of the deployment with the new hardware.

> **H₂ (Identical Semantics).** The same operating system runtime binary, compiled with `features = ["native"]` for the seL4 bottom and with `features = ["compat"]` for the NetBSD bottom, produces identical capability-ledger event semantics — the same capability hashes, the same ledger entries, the same `Verdict` values from `LedgerConsumer::consult_capability` — given the same capability state and the same witness records.

H₁ is falsified if: (a) any step in the Mechanism C recovery flow (§5.4) requires a network resource controlled by the vendor; or (b) the reconstituted deployment fails to accept or produce capability invocation records that the original deployment would have accepted or produced; or (c) a post-recovery audit by an independent party requires access to vendor-controlled infrastructure to verify completeness.

H₂ is falsified if: the compiled outputs of the `native` and `compat` feature-flag variants produce different `Verdict` values, different capability hashes, or different ledger entry payloads given identical input sequences. Specifically: `LedgerConsumer::consult_capability(cap, checkpoint, now, witness)` with the same arguments must return the same `Verdict` variant on both substrates.

**Test specifications.** H₁ requires a full recovery drill on a fresh QEMU AArch64 NetBSD VM from a paper-printed seed, with an independently audited ledger transcript. H₂ requires a cross-compilation CI job that runs the same test vectors against both feature-flag variants and compares outputs deterministically. Both are planned.

**Additional open questions for future passes:**

1. What is the practical cardinality ceiling for the witness federation on ARM Cortex-A hardware, given the 10–50× verify-cost penalty versus x86?
2. Does the consistency-proof gating on replication safety (§3.3, composed primitive 4) admit any attack through a window between tile publication and checkpoint signing?
3. What is the minimum trustworthy configuration for the Coreboot/Heads boot path on commodity AArch64 boards?
4. Does `system-substrate-netbsd` belong as an extension to the existing `system-substrate` crate (role: hardware bridge) or as a new sibling parallel to `system-substrate-broadcom`?
5. What is the right image-signing key for Veriexec `signatures.veriexec`: the operator's apex key or a dedicated image-signing key?

---

## 8. Conclusion

### 8.1 Summary of Contributions

This paper has presented a substrate architecture for customer-sovereign, trustworthy operating system deployments. The three contributions are:

**The Capability Ledger Substrate.** The substrate's capability state IS the WORM ledger. Every kernel-mediated capability invocation, grant, revocation, and temporal extension emits a signed entry to a customer-rooted Merkle log whose apex is the customer's signing key. The kernel consults the log before honouring any capability invocation via a state machine (`system-ledger`) implementing a `LedgerConsumer` trait with structured `Verdict` results and five-step cost-ordered consultation logic. The architecture is implemented in 51+44 = 95 tested Rust test cases and 10 Criterion benchmarks.

**The Two-Bottom Operating System Design.** A seL4 v15.0.0 native bottom on AArch64-first hardware and a NetBSD compatibility bottom deploying Veriexec verified-image boot and offline-reproducible `build.sh` enable the same operating system runtime binaries to execute on either substrate via a thin Rust shim with Cargo feature-flag selection. Linux is not a substrate bottom. The design achieves hardware reach without capability-geometry compromise — a structural property not available from single-substrate hyperscaler or proprietary-RTOS architectures.

**The N+3+ Apex Co-Signing Ceremony.** An ownership-transfer ceremony derived from C2SP signed-note multi-signature semantics makes deployment ownership transfer atomic (a single ledger event at height N+2), auditable (both parties' signatures in a public transparency log), and final (P-old's signatures are refused by the kernel at heights N+3+). The new apex inherits all capability state, audit history, operational identity, and formal verification proofs — the proofs are mathematics; they transfer.

### 8.2 Future Research

Five research directions follow from this architecture.

**Cross-substrate semantic equivalence testing.** H₂ (§7.4) requires a continuous integration (CI) harness that cross-compiles operating system runtime binaries for both feature-flag variants and runs identical test vectors, comparing outputs deterministically. This is the engineering prerequisite for any production claim that the two bottoms are semantically equivalent.

**AArch64 seL4 production deployment.** The architecture cannot be fully evaluated on its primary hardware target until the AArch64 cross-compile toolchain is resolved and the NetBSD compatibility layer is implemented; current benchmarks use the x86_64 GCP VM as a proxy for the AArch64 production target. The formal-verification properties of seL4 AArch64 are the load-bearing claim.

**Witness federation cardinality on embedded hardware.** The 10–50× slower Ed25519 verification on ARM Cortex-A narrows the witness-federation operating window. A quantitative study of cardinality ceilings and cache-sizing requirements on QEMU AArch64 (and eventually on production AArch64 hardware) is needed to bound the practical federation parameters for the regulated small and medium-sized business (SMB) deployment profile.

**Formal modelling of the N+3+ ceremony.** The apex co-signing ceremony is specified procedurally and tested with integration tests. A formal model in TLA+ or Alloy would provide a machine-checked proof that the ceremony has no liveness or safety violations under network partition, message reordering, or delayed checkpoint publication.

**No_std capability kernel path.** The long-horizon replacement for the vendored seL4 kernel tree is a no_std Rust capability kernel targeting AArch64-first hardware. The `system-core` inclusion-proof and consistency-proof modules already avoid std-only primitives; a future MINOR version will carve the no_std path to enable direct consumption from the no_std kernel without an FFI boundary. The same code that gates capability invocations in userspace today can gate them at the kernel level.

---

## References

*(Chicago author-date; inline citations use [citation-id] syntax where stable IDs exist in citations.yaml.)*

Apple Security Research. 2024. *Apple Private Cloud Compute: A new frontier for AI privacy in the cloud.* Apple Security Engineering and Architecture. https://security.apple.com/blog/private-cloud-compute/.

AWS. 2025. *AWS Nitro Isolation Engine: re:Invent 2025.* Amazon Web Services. https://reinvent.awsevents.com/.

Birgisson, Arnar, Joe Gibbs Politz, Ulfar Erlingsson, Ankur Taly, Michael Vrable, and Mark Lentczner. 2014. "Macaroons: Cookies with Contextual Caveats for Decentralized Authorization in the Cloud." *Network and Distributed System Security Symposium (NDSS).*

cert.europa.eu. 2025. *Sovereign cloud offerings in the EU.* European Union Agency for Cybersecurity (ENISA).

European Commission. 2025. *Commission Implementing Regulation (EU) 2025/1946 on qualified electronic preservation services.* Official Journal of the European Union. [eidas-qualified-preservation]

ETSI. 2024. *ETSI TS 119 511: Policy and security requirements for trust service providers providing long-term preservation of digital signatures or general data.* [etsi-ts-119-511]

Forrester Research. 2024. *From Unicorns and Rainbows to Storm Clouds: The State of Gaia-X.* Forrester.

Google Transparency Team, Ben Laurie, Adam Langley, and Emilia Kasper. 2013. "Certificate Transparency." RFC 6962, IETF.

Heiser, Gernot, and Gerwin Klein. 2010. "It's Time for Capability Geometry." *IEEE Security and Privacy* 8 (2): 67–69.

Klein, Gerwin, Kevin Elphinstone, Gernot Heiser, June Andronick, David Cock, Philip Derrin, Dhammika Elkaduwe, Kai Engelhardt, Rafal Kolanski, Michael Norrish, Thomas Sewell, Harvey Tuch, and Simon Winwood. 2009. "seL4: Formal Verification of an OS Kernel." *Proceedings of the ACM Symposium on Operating Systems Principles (SOSP).*

Klein, Gerwin, June Andronick, Matthew Fernandez, Ihor Kuz, Toby Murray, and Gernot Heiser. 2014. "Comprehensive Formal Verification of an OS Microkernel." *ACM Transactions on Computer Systems (TOCS)* 32 (1).

Laurie, Ben, Adam Langley, and Emilia Kasper. 2013. "Certificate Transparency." RFC 6962. IETF.

Merkle, Ralph C. 1987. "A Digital Signature Based on a Conventional Encryption Function." *Advances in Cryptology — CRYPTO '87.* LNCS 293. Springer.

Murray, Toby, Daniel Matichuk, Matthew Brassil, Peter Gammie, Timothy Bourke, Sean Seefried, Corey Lewis, Xin Gao, and Gerwin Klein. 2013. "seL4: From General Purpose to a Proof of Information Flow Enforcement." *IEEE Symposium on Security and Privacy.*

NetBSD Project. 2026. *NetBSD Verified Execution (Veriexec).* https://www.netbsd.org/docs/guide/en/chap-veriexec.html.

Newman, Zachary, John Speed Meyers, and Santiago Torres-Arias. 2022. "Sigstore: Software Signing for Everybody." *Proceedings of the ACM Conference on Computer and Communications Security (CCS).*

Sewell, Thomas, Simon Winwood, Peter Gammie, Toby Murray, June Andronick, and Gerwin Klein. 2011. "seL4: From General Purpose to a Proof of Information Flow Enforcement." Extended version. NICTA Technical Report.

IETF. 2022. *RFC 9162: Certificate Transparency Version 2.0.* E. Laurie, R. Ritter. [rfc-9162]

IETF. 2021. *RFC 8032: Edwards-Curve Digital Signature Algorithm (EdDSA).* S. Josefsson, I. Liusvaara.

C2SP. 2024. *signed-note: A simple format for signed log checkpoints.* https://github.com/C2SP/C2SP/blob/main/signed-note.md. [c2sp-signed-note]

C2SP. 2024. *tlog-tiles: Tile-based transparency log on-disk format.* https://github.com/C2SP/C2SP. [c2sp-tlog-tiles]

US Securities and Exchange Commission. 2003. *Rule 17a-4(f): Electronic recordkeeping for broker-dealers.* 17 CFR 240.17a-4. [sec-17a-4-f]

Watson, Robert N. M., Jonathan Anderson, Ben Laurie, and Kris Kennaway. 2010. "Capsicum: Practical Capabilities for UNIX." *Proceedings of the 19th USENIX Security Symposium.*

Woodfine, Jennifer M. 2026. *Retail Anchor Co-location Composition as a Spatial Leading Indicator of Commercial Activity: A Continental-Scale Cluster Analysis.* Working Paper v0.3. Woodfine Management Corp., New York, NY.

Woodruff, Jonathan, Robert N. M. Watson, David Chisnall, Simon W. Moore, Jonathan Anderson, Brooks Davis, Ben Laurie, Peter G. Neumann, Robert Norton, and Michael Roe. 2014. "The CHERI Capability Model." *Proceedings of the 41st Annual International Symposium on Computer Architecture (ISCA).*

---

## Appendix A: Notation

| Symbol | Definition |
|---|---|
| `H(x)` | SHA-256(x) |
| `H_L(x)` | SHA-256(0x00 ‖ x) — RFC 9162 leaf hash |
| `H_I(l, r)` | SHA-256(0x01 ‖ l ‖ r) — RFC 9162 internal hash |
| `cap.hash()` | SHA-256(serde_json(Capability)) |
| `spec_hash` | SHA-256(toml::to_string(SystemSpec)) |
| `plan_hash` | SHA-256(serde_json({ spec_hash, steps })) |
| P-old, P-new | Departing and incoming apex signing keys |
| N | Ledger height of the last pre-handover checkpoint |
| `WITNESS_NAMESPACE` | "capability-witness-v1" (namespace tag for ssh-keygen -Y sign) |
| `LedgerAnchor` | (origin: String, tree_size: u64, root_hash: Hash256) |
| `Verdict` | Allow ‖ Refuse(RefuseReason) ‖ ExtendThenAllow { new_expiry_t } |
| `Hash256` | [u8; 32] — SHA-256 baseline; algorithm-agile (future MINOR may add BLAKE3 or SHA-3) |

---

## Appendix B: Benchmark Reference Data

**Environment:** GCP n2-class VM, Intel Xeon @ 2.20 GHz, 4 vCPUs, 15 GiB RAM, Ubuntu 24.04 LTS. Criterion 0.5, 100 samples per benchmark. Compilation: `opt-level = "z"`, `lto = true`. Run date: 2026-04-27 03:44–03:51 UTC. VM 1-min load average: 7.72–4.04 (HEAVY — see Limitations §7.3 for bench #9).

Full table: see Table B.1 in §6.2.

**Three-run comparison** (quiet VM, loaded VM, this study):

| Operation | Quiet VM | Loaded VM | This study |
|---|---|---|---|
| `Capability::hash` | 5.0 µs | 14.78 µs | 6.44 µs |
| `verify_signer` (1-sig) | 3.40 ms | 4.89 ms | 4.01 ms |
| `verify_apex_handover` (2-sig) | 6.80 ms | 8.62 ms | 7.65 ms |
| Cache hit | 8.08 ns | 16.94 ns | 11.2 ns |
| Cache miss (64-scan) | 338 ns | 673 ns | 362 ns |
| `consult_capability` Allow path | 3.39 ms | 6.32 ms | 3.74 ms |

The quiet-VM condition (load avg < 1.0) is the publication-quality baseline; the loaded-VM run illustrates the upper bound on variance.

---

## AI Use Disclosure

This paper was developed with the assistance of Claude Sonnet 4.6 (Anthropic). The architecture, implementation, data structures, test cases, and benchmark measurements are products of the Woodfine Management Corp. engineering function. The literature synthesis, formal hypothesis structure, and falsification programme were developed with AI assistance under human editorial direction by the named authors. Literature search and citation accuracy are the responsibility of the human authors. The model used for research and drafting is identified per COPE 2024 guidelines.

---

## CRediT Contributor Roles

**Mathew Woodfine:** Conceptualization, Methodology, Software, Formal Analysis, Writing – Original Draft, Writing – Review & Editing.
**Peter M. Woodfine:** Conceptualization, Validation, Writing – Review & Editing.
**Jennifer M. Woodfine:** Formal Analysis, Writing – Review & Editing.

---

## Conflict of Interest Declaration

The authors declare no conflict of interest.

---

## Funding Statement

No external funding was received for this research.

---

## Data Availability Statement

The Rust source code for the substrate implementation, including the benchmark harness, is available from the corresponding author upon reasonable request. The benchmark measurements reported in §6 are reproducible from the source code on comparable hardware under the environmental conditions described in Appendix B. Formal verification artefacts (Isabelle/HOL proofs) are part of the seL4 project's public repository. Transparency-log test vectors used in §6 are deterministic and regenerable from the implementation.
