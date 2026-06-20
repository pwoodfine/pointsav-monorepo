---
schema: foundry-journal-v1
artifact_type: JOURNAL
state: draft
version: "0.3"
title: "Capability-Secured Session Orchestration: A Runtime Architecture for Multi-Tenant AI Workload Isolation"
target_journal: "MLSys (ACM Conference on Machine Learning and Systems)"
target_publisher: "ACM"
impact_factor: ""
acceptance_rate: "22% (2025)"
alternate_venue: "OSDI (USENIX, ~20-25% AR); EuroSys (ACM SIGOPS, ~18% AR)"
authors:
  - name: "Mathew Woodfine"
    affiliation: "Woodfine Management Corp., New York, NY, USA"
    email: corporate.secretary@woodfinegroup.com
    orcid: ""
    credit_roles:
      - Conceptualization
      - Methodology
      - Software
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
  - "D.4.1 Process Management"
  - "D.4.6 Security and Protection"
  - "I.2.11 Distributed Artificial Intelligence"
keywords:
  - session orchestration
  - capability-based security
  - AI workload isolation
  - multi-tenant runtime
  - reproducible execution
  - transparency logs
  - tiered inference
bcsc_class: no-disclosure-implication
ai_tool_used: "claude-sonnet-4-6 (Anthropic)"
corresponding_author: corporate.secretary@woodfinegroup.com
word_count_body: 2614
word_count_target: 9500
submission_status: not-submitted
cites: []
forbidden_terms_cleared: true
section_status:
  abstract: complete
  s1_introduction: complete
  s2_literature_review: complete
  s3_methodology: complete
  s4_implementation: stub
  s5_evaluation: stub
  s6_discussion: stub
  s7_limitations: complete
  s8_conclusion: stub
  s9_formal_hypotheses: complete
  s10_falsification: complete
refs_status:
  count: 0
  quality: absent
  blockers:
    - "References section entirely unpopulated — [To be populated] placeholder only"
    - "§4 Implementation and §5 Evaluation pending benchmark harness completion"
preprint_posted: true
preprint_posted_date: 2026-06-11
doi: ""
license: "CC BY 4.0"
cite_as: "Woodfine, Mathew, Woodfine, Peter M., & Woodfine, Jennifer M. (2026). Capability-Secured Session Orchestration: A Runtime Architecture for Multi-Tenant AI Workload Isolation. Working Paper v0.3, 12 June 2026. Woodfine Management Corp., New York, NY."
revision_history:
  - version: "0.1"
    date: "2026-05-28"
    changes: "Frontmatter and skeleton only; HOLD pending J2 submission; preprint notice added"
  - version: "0.2"
    date: "2026-06-11"
    changes: "HOLD lifted; Abstract, Introduction, Literature Review, Methodology, Limitations, Formal Hypotheses, and Falsification Programme written; remaining sections stubbed for completion after implementation evidence is gathered"
  - version: "0.3"
    date: "2026-06-12"
    changes: "Language pass complete; two internal system-name references removed from stub annotations; forbidden_terms_cleared: true"
notes_for_editor: |
  Working draft — core argument sections written; evaluation results and conclusion
  pending implementation evidence (latency benchmarks and concurrent-session isolation tests).
  Target venue rationale: the contribution is a systems architecture result (session isolation
  primitives) with ML-specific motivation (inference workload isolation). MLSys covers this
  intersection. OSDI is the backup venue if the systems angle dominates the submission.
  ORCID IDs required before submission; word count target 9,500 words; current body ~2,600.
---

> **Working Paper · Version 0.3 · 2026-06-12 · CC BY 4.0**
> This manuscript is a working draft. It has not been peer reviewed. Findings are preliminary and subject to revision without notice. Correspondence: corporate.secretary@woodfinegroup.com.
>
> *Cite as:* Woodfine, Mathew, Woodfine, Peter M., & Woodfine, Jennifer M. (2026). Capability-Secured Session Orchestration: A Runtime Architecture for Multi-Tenant AI Workload Isolation. Working Paper v0.3, 12 June 2026. Woodfine Management Corp., New York, NY.

> **Forward-Looking Statements**
> Certain statements in this paper describe intended research directions, planned system capabilities, and anticipated outcomes. These statements reflect the authors' current expectations and are based on reasonable assumptions and work in progress as of the date above. Actual results, measurements, and findings may differ materially. Readers should not place undue reliance on such statements; they are subject to revision as research progresses and new data become available.

# Capability-Secured Session Orchestration: A Runtime Architecture for Multi-Tenant AI Workload Isolation

**Mathew Woodfine, Peter M. Woodfine, and Jennifer M. Woodfine**
Woodfine Management Corp., New York, NY, USA
*Corresponding author:* corporate.secretary@woodfinegroup.com

---

## Abstract

Contemporary AI inference frameworks — including vLLM, Triton Inference Server, and TensorRT-LLM — are optimised for throughput maximisation across shared GPU resources. They do not, however, publish per-session capability state to a customer-controlled audit log, nor do they enforce isolation boundaries between concurrent agent sessions that operate over distinct data domains. The consequence is that multi-tenant deployments of AI inference workloads rely on operator trust and process-level separation rather than verifiable, transferable capability proofs. This paper introduces an architecture for capability-secured session orchestration in which each AI agent session is bound at startup to a dedicated archive directory whose scope is enforced by a version-controlled capability manifest, a PID-anchored session lock, and a structured mailbox protocol for inter-session communication. Sessions route inference requests through a tiered gateway that records each invocation against a transparency log inherited from the substrate described in companion work on verified capability ledgers. We formalise the isolation properties of this architecture, state the primary and null hypotheses, and describe a falsification programme consisting of concurrent-session contamination tests and overhead measurement benchmarks. Measured overhead for the session-binding and lock-acquisition protocol is intended to remain below five milliseconds at session startup, with per-invocation capability-check overhead expected to remain below one millisecond. Full empirical results are pending implementation of the benchmark harness described in Section 10.

---

## 1. Introduction

Deploying AI inference workloads in multi-tenant environments presents an isolation challenge that current runtime frameworks address incompletely. A research organisation, a professional services firm, or an engineering team running concurrent AI agent sessions over distinct data domains — personnel records, geospatial datasets, legal filings, build artefacts — faces a common problem: how to guarantee that an agent session operating over domain A cannot observe, modify, or contaminate the uncommitted state of an agent session operating over domain B, even when both sessions share the same physical host, the same model weights, and the same inference endpoint.

Existing approaches to this problem fall into two categories. Process-level isolation places each session in a separate operating system process with distinct address spaces and file-descriptor namespaces. Container-level isolation extends this to network namespaces and mount points. Both approaches impose non-trivial resource overhead and do not, by themselves, produce an auditable record of which session invoked which capability at what time. A production deployment of concurrent AI sessions requires not just isolation but accountability: the ability to demonstrate, after the fact, that session A did not access the data belonging to session B.

The architecture described in this paper addresses this gap through three principal contributions. *First*, we define the archive-scoped session model, in which the scope of an AI agent session is determined by a version-controlled directory manifest read at startup, rather than by runtime access-control lists applied per-request. *Second*, we introduce the session lock protocol, a lightweight fencing mechanism that uses a PID-anchored lock file containing the system boot identifier to provide crash-consistent session ownership with automatic stale-lock detection across host reboots. *Third*, we describe the tiered inference gateway, in which inference requests are routed through a three-tier hierarchy — embedded local model, hub-mediated remote model, and external API — with each tier transition recorded to a transparency log that is verifiable against the capability ledger substrate described in companion work [CITATION NEEDED — J2].

Together these three mechanisms produce a session-orchestration runtime in which isolation is enforced structurally, not by policy, and is auditable via a transparency log that can be transferred to a new operator without requiring trust in the original host.

---

## 2. Literature Review

Research on capability-based security systems dates to the foundational work of Dennis and Van Horn [EXTERNAL CITATION], who proposed that access rights should be unforgeable tokens rather than identity-based lists. The seL4 microkernel [EXTERNAL CITATION] realises this principle in a formally verified implementation, providing strong end-to-end guarantees about capability delegation and revocation. The companion paper in this research programme [CITATION NEEDED — J2] extends the seL4 capability model to construct a customer-rooted ledger substrate using RFC 9162 Merkle transparency logs and multi-signature checkpoints, establishing the formal foundation on which the session-orchestration architecture described here is built.

Work on multi-tenant AI inference has concentrated primarily on GPU memory management and request scheduling. Orca [EXTERNAL CITATION] and vLLM [EXTERNAL CITATION] introduce iteration-level scheduling and paged attention respectively, achieving near-maximal GPU utilisation under concurrent request loads. Neither system publishes an audit record of per-session capability use. NVIDIA's Triton Inference Server offers model versioning and isolation between named model repositories, but isolation is at the model level, not at the individual session-data-domain level.

Research on session management in distributed systems addresses related concerns at the network layer. RFC 8446 (TLS 1.3) provides cryptographic session establishment with forward secrecy. The Certificate Transparency framework [EXTERNAL CITATION] provides the append-only log structure that, in this work, is repurposed for capability-invocation auditing rather than certificate issuance. The specific innovation is the combination of version-controlled manifest-based scoping with PID-anchored lock fencing and transparency-log-backed invocation recording, which together provide isolation guarantees that are structural rather than credential-based.

Prior work on sandbox environments for AI agents includes work on tool-use sandboxing [EXTERNAL CITATION] and network-isolated agent execution [EXTERNAL CITATION]. These approaches focus on preventing harmful tool calls, not on providing auditable isolation between concurrently running agents over distinct data domains. Our work is complementary: the archive-scoped session model enforces data-domain separation, and the transparency log provides the audit trail that tool-use sandboxes do not.

---

## 3. Methodology

### 3.1 The Archive-Scoped Session Model

The fundamental unit of isolation in this architecture is the *archive*: a version-controlled directory that contains the code, data, capability manifests, and session-state files belonging to a single data domain. An AI agent session is bound to exactly one archive at startup. The session reads a machine-readable manifest from the archive root, which declares the archive's identifier, its data-domain membership, and its relationships to other archives in the deployment. The session lock file, written immediately after manifest ingestion, records the session's operating system process identifier and the system boot identifier obtained from the kernel's random boot key. This combination allows a subsequent session starting in the same archive to distinguish between three lock states: a lock whose PID is alive (concurrent session — halt and warn), a lock whose PID is absent but whose boot identifier matches the current boot (crashed session — treat as stale after operator confirmation), and a lock whose boot identifier does not match the current boot (pre-reboot session — remove automatically and proceed).

No session may write to the state files of a different archive. The prohibition is enforced architecturally: the session holds no file-descriptor reference and no path variable that points outside its own archive boundary except for the explicitly sanctioned communication channel described in Section 3.2.

### 3.2 The Mailbox Protocol

Inter-session communication is mediated by a structured mailbox protocol rather than by shared memory, shared file descriptors, or direct API calls between sessions. Each archive contains an inbox file and an outbox file at a canonical path within the archive's agent-state directory. Messages are prepended to the outbox by the originating session and retrieved from the inbox by the receiving session; the physical delivery step is performed by a designated coordination session that sweeps archive outboxes and deposits messages into the correct inboxes. This design means that two concurrent feature-tier sessions never communicate directly: all inter-domain information passes through the coordination tier, creating a message-passing model with a single auditable chokepoint.

Message schema is enforced by a machine-readable frontmatter block that includes originating session role, destination role, subject, creation timestamp, priority, and lifecycle status. A message that cannot be routed to its named destination — because the destination archive does not exist, or because the destination role is incorrect for the current deployment — is rejected at write time by a routing validation step, preventing cross-domain contamination through misdirected messages.

### 3.3 The Tiered Inference Gateway

Inference requests generated by a session are routed through a three-tier gateway. Tier A serves requests from an embedded model running on the local host; it requires no network egress and imposes no data-transfer boundary-crossing. Tier B routes requests through a hub-mediated connection to a larger remote model; the request and response cross the deployment's private network boundary but remain within the operator's administrative domain. Tier C routes to external API endpoints operated by third parties and is subject to the strictest data-minimisation constraints: structured entity data must not be transmitted to Tier C endpoints, a requirement derived from the session's capability manifest rather than from per-request policy.

Each tier transition is recorded to the transparency log associated with the session's archive. The log entry includes: the tier selected, the request timestamp, a content-addressed identifier of the prompt template, and the capability scope declared in the session manifest. The log is append-only and, in deployments linked to the verified capability ledger substrate [CITATION NEEDED — J2], is anchored to the ledger's Merkle tree. This enables post-hoc verification that a session operating over sensitive data used only Tier A inference, without requiring the verifier to inspect the session's runtime state.

### 3.4 Session Startup and Shutdown Sequences

The startup sequence is deterministic and sequentially ordered. A session reads its manifest, writes its lock, reads the inference gateway's health state, reads its inbox, reads any active workspace-wide hazard notices, reads domain-specific capability rules, and reads a rolling session-context digest before requesting operator work. The shutdown sequence is equally deterministic: the session writes or updates any in-progress work artefacts, writes new preferences or decisions to its memory store, updates its cross-session carry-forward checklist, stages any produced artefacts to the appropriate outbox, commits any uncommitted work in its version-controlled files, verifies the resulting working tree is clean, and removes its session lock. Reproducibility of the startup and shutdown sequences is a design requirement: a session that deviates from the canonical sequence cannot safely make claims about the isolation of its operations.

---

## 4. Implementation

*[Stub — pending completion; section will be filled when implementation evidence is gathered from the reference deployment described in §3.1–§3.4.]*

The implementation consists of four components: the manifest validator, the session lock writer, the mailbox router, and the inference gateway. The manifest validator is a shell script that confirms the archive identifier and tetrad declarations are present before allowing the session to proceed. The session lock writer is an atomic file-creation operation. The mailbox router is a coordination-tier binary that sweeps archive outboxes on a configurable interval. The inference gateway is a Rust binary that accepts HTTP requests from co-located session processes, routes them to the appropriate tier, and appends the invocation record to the archive's transparency log.

### 4.1 Architecture Context Update — 2026-06-19

The following entries record implementation decisions confirmed during the reference deployment development cycle. All forward-looking statements use intended/planned/may/target language per BCSC disclosure posture.

**Three-binary architecture formally confirmed.** The runtime comprises three distinct binaries with non-overlapping roles: `os-console` (host TUI — the keyboard-native SSH terminal interface to the Totebox Archive), `os-totebox` (WORM vault — sovereign data persistence with capability-enforced write-once semantics), and `os-orchestration` (federation hub — stateless aggregation layer mediating inter-domain communication). This separation enforces the data-domain isolation described in §3.1 at the binary boundary rather than at the process boundary alone.

**seL4 Microkit Protection Domain design locked for os-totebox.** The intended os-totebox runtime targets a seven-PD seL4 Microkit stack. The planned priority assignment runs from watchdog-pd at priority 250 (highest; health monitoring) down through coordination-pd, capability-pd, and write-pd, to service-extraction at priority 110. The design is intended to ensure that a compromised lower-priority service cannot preempt or interfere with the integrity-critical PDs above it. Full PD specification is maintained in BRIEF-os-totebox-build-out.md.

**Capability Geometry™ defined.** The isolation guarantee described informally in §3.1 and §7 is formalised as Capability Geometry: the seL4 capability DAG structure ensures that a compromised service-slm instance provably cannot reach service-fs, because no capability path exists between those two PDs in the declared DAG. This property is verifiable from the static PD configuration without runtime inspection, satisfying the structural-not-policy isolation requirement stated in §1.

**First Ring 2 service implemented.** `service-people` (GET /v1/people, port :9091) was committed at `997b8d22`, constituting the first concrete implementation of the Ring 2 service layer described in §3.1. This provides early benchmark baseline data for the startup and per-inference overhead measurements planned for §5.

**service-extraction added to workspace.** The service-extraction crate has been added to the pointsav-monorepo Cargo workspace, enabling cross-crate dependency resolution between the extraction pipeline and the capability-checking infrastructure. This is a prerequisite for the §5 evaluation harness.

**Planning documents created.** BRIEF-os-totebox-build-out.md is the intended canonical planning document for the WORM vault build-out, covering the seven-PD seL4 stack and Phase H1 targets. BRIEF-os-orchestration-build-out.md is the intended canonical planning document for the federation layer, covering the capability-broker PD design and Yo-Yo Tier B integration.

**Phase H1 target.** The near-term implementation target is a moonshot-sel4-vmm PD runtime of approximately 300 lines of code, with a moonshot-toolkit TOML specification at `examples/os-totebox.toml`. These targets may be revised as implementation proceeds.

---

## 5. Evaluation

*[Stub — pending benchmark harness completion.]*

The evaluation will measure three quantities: session startup overhead introduced by the manifest-read and lock-write steps; per-inference overhead introduced by the tier-selection and log-append steps; and cross-session contamination under concurrent write workloads. Target figures: startup overhead below five milliseconds; per-inference overhead below one millisecond; zero contamination events across ten thousand concurrent-write trials.

---

## 6. Discussion

*[Stub — pending evaluation results.]*

---

## 7. Limitations

The archive-scoped session model enforces isolation at the level of version-controlled directory boundaries. It does not enforce isolation at the level of the underlying filesystem: two sessions with access to the same mount point can, in principle, communicate through filesystem side-channels outside the designated mailbox paths. The current design trusts the operating system's process isolation to prevent such side-channels; a deployment on hardware that provides stronger isolation guarantees — for example, a seL4-based hypervisor in which each session archive is mounted inside a separate protection domain — would eliminate this residual trust. Such a deployment is described as a planned direction in companion work [CITATION NEEDED — J2] and is an intended extension of the architecture described here.

The transparency log is append-only but not yet anchored to an external witness in deployed configurations. The planned integration with the capability ledger substrate will provide this anchoring. Until that integration is complete, the log's tamper-evidence properties depend on the integrity of the host filesystem and are not independently verifiable.

The mailbox routing step introduces a communication latency that is bounded by the coordination session's sweep interval. In the current implementation this interval is at the operator's discretion, which means real-time inter-session communication is not supported. Workloads that require synchronous cross-domain data exchange would need to be redesigned to fit an asynchronous message-passing model before deploying in this framework.

---

## 8. Conclusion

*[Stub — pending evaluation results.]*

---

## 9. Formal Hypotheses

**H₁** (*Primary — isolation*): Two AI agent sessions bound to distinct archives by the startup lock protocol and mailbox routing constraints exhibit zero cross-session state contamination during concurrent operation on a shared host. Specifically: neither session can observe the other's uncommitted version-controlled state, and no message sent by one session appears in the other session's inbox without an explicit relay step performed by the coordination tier.

**H₀** (*Null*): The archive-binding and lock protocol produce no measurable improvement in session isolation compared to an unpartitioned baseline in which all sessions share a single working directory and communicate through shared file-descriptor references.

**H₂** (*Overhead*): The session-binding protocol adds less than five milliseconds to session startup time and less than one millisecond per inference invocation to request latency, as measured against a baseline session that performs no manifest read, no lock write, and no log append.

---

## 10. Falsification Programme

*Test F1 — Cross-archive uncommitted state visibility.* A test harness spawns two concurrent sessions in distinct archives on the same host. Session A writes a file to a path within its archive but does not commit it. Session B is instructed to read from a path that, under the archive-scoped model, is outside its declared boundary. If Session B successfully reads Session A's uncommitted file, H₁ is falsified. The test is repeated one thousand times with randomised file content to rule out cache coincidence.

*Test F2 — Mailbox routing contamination.* A test harness sends a message with a `to:` field naming Archive B but places the message physically in Archive A's outbox without using the coordination-tier relay. The coordination-tier router is then run. If the message appears in Archive B's inbox without a corresponding relay log entry, H₁ is falsified.

*Test F3 — Lock protocol fencing under concurrent startup.* A test harness starts two sessions in the same archive simultaneously. If both sessions proceed past the lock-write step without one halting, H₁ is falsified, because the single-session-per-archive invariant has been violated.

*Test F4 — Startup latency under lock protocol.* A test harness measures the wall-clock duration of the startup sequence for one hundred sequential session launches with the lock protocol active, and one hundred without. If the mean overhead exceeds five milliseconds, H₂ is falsified.

*Test F5 — Per-inference log-append latency.* A test harness issues one thousand inference requests through the tiered gateway with log-append active and one thousand without. If the mean overhead exceeds one millisecond, H₂ is falsified.

---

## AI Use Disclosure

This manuscript was prepared with the assistance of a large language model (claude-sonnet-4-6, Anthropic). The AI system was used for drafting and structural organisation of sections. All scientific claims, hypotheses, architectural decisions, experimental design choices, and interpretations of results are the work of the named human authors. The authors accept full responsibility for the accuracy and integrity of the content.

---

## CRediT Contributor Roles

**Mathew Woodfine**: Conceptualization, Methodology, Software, Writing — Original Draft, Writing — Review & Editing. **Peter M. Woodfine**: Conceptualization, Validation, Writing — Review & Editing. **Jennifer M. Woodfine**: Formal Analysis, Writing — Review & Editing.

---

## Conflict of Interest Declaration

The authors declare no conflict of interest.

---

## Funding Statement

No external funding received.

---

## Data Availability Statement

The session-orchestration runtime implementation described in this paper is intended for release as open-source software under the Apache 2.0 licence. Benchmark datasets and test harness scripts will be published alongside the implementation at the time of journal submission. Pre-submission code is available to reviewers on request.

---

## References

*[To be populated. Key anticipated citations: Dennis & Van Horn (1966) capability systems; Klein et al. (2009) seL4 formal verification; Yu et al. (2022) Orca iteration-level scheduling; Kwon et al. (2023) vLLM paged attention; Laurie et al. (2021) RFC 9162 Certificate Transparency; companion paper J2 — "Composing Capability Geometry from Verified Primitives" (Woodfine et al., 2026) — cite when published.]*

---

*Version 0.2 draft — 2026-06-11*
*Sections 1–3, 7, 9–10 written. Sections 4, 5, 6, 8 stubbed pending implementation evidence.*
*HOLD lifted 2026-06-11: implementation material for §4 is being gathered alongside the reference deployment described in §3.1–§3.4.*
*Language pass complete 2026-06-12: forbidden_terms_cleared: true.*
