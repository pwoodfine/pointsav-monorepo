---
schema: foundry-doc-v1
document_version: 0.1.0
component: app-mediakit-knowledge
status: thinking — five candidate inventions; doctrine touch pending Master via cluster outbox
last_updated: 2026-04-26
session: 2
authoring_context: synthesised from five parallel research-agent reports commissioned 2026-04-26 in response to operator's "we need inventions, AI+ something more than the sum of its parts" framing
---

# Inventions — leapfrog 2030 substrate

Five candidate inventions emerging from session-2 research. Each is
positioned against published prior art (with sources) so the
novelty kernel is honest. The reader should be able to verdict
"genuinely new", "novel composition of known parts", or "table
stakes elsewhere" for each, and find their own way to the citations.

The five do not stand alone — §6 covers the composition that gives
them more than the sum of their parts. §7 lists what has been
deliberately rejected so the substrate does not drift into
patterns that undermine its core properties.

The five names, for fast reference:

| # | Invention | Verdict |
|---|---|---|
| A | Substrate-enforced AI grounding | Structurally novel as composed system |
| B | Content-addressed federated AI adapters | Moderate; novel in composition + jurisdictional layer |
| C | Two-clock continuous disclosure with cryptographic anchors | Composition novel; primitives commodified |
| D | Disclosure-Diff as Signed Artefact + Subscriber Proof-of-Receipt | Novel as substrate-deliverable patterns |
| E | Constrained-Constitutional Authoring (CCA) | The killer — through-line that ties A+B+C+D into AI authoring |

The doctrine touch for these (extension of `disclosure-substrate.md`,
or new claims #31+) is Master scope; recommended in the session-2
outbox to Master 2026-04-26.

---

## A — Substrate-enforced AI grounding

### Claim

The substrate **refuses to render** AI output that is not (i)
forward-looking-information labelled per the active jurisdiction's
disclosure rules, (ii) citation-resolved against the substrate's
own `citations.yaml` registry, and (iii) constitutionally bound to
per-tenant / per-jurisdiction rule packs. The model can produce
whatever it likes; the publishing pipeline is the gate. AI
hallucinations cannot reach the disclosure surface because the
disclosure surface refuses to publish unsourced claims.

### Novelty kernel

Existing systems do grounding at the **model layer**. Anthropic's
Constitutional AI bakes principles into weights via RLAIF;
Constitutional Classifiers (arxiv 2501.18837) sit on the input/
output exchange and refuse violations; Deliberative Alignment
(arxiv 2412.16339) teaches reasoning over a safety spec. All
operate on prose; none binds individual claims to citation-graph
identifiers. Vectara's HHEM scores faithfulness; Perplexity attaches
URL citations (with a 37% citation-hallucination rate per Columbia
Journalism Review 2025). C2PA Content Credentials v2.3 binds
provenance metadata to media assets but the text vocabulary is
immature and operates at asset granularity. W3C Verifiable
Credentials 2.0 is the cryptographic primitive but no production
schema exists for "this paragraph contains five claims; each claim
is bound by VC signature to a specific registered citation."

The substrate-layer enforcement pattern — *deterministic gate +
citation graph + jurisdictional constitution + AI publication path*
— is **not currently shipped by anyone as a publishing primitive**.
Closest "table stakes" comparator is enterprise RAG with HHEM-style
scoring (Vectara), which is detection, not refusal-to-render.

### Why substrate-layer beats model-layer

- **Deterministic**: same input → same gate decision; no
  probabilistic refusal
- **Auditable**: gate logic is code (Rego / policy), not weights;
  a court / regulator can read the policy
- **Model-agnostic**: swap OLMo for Claude with no policy change
- **Regulatory-defensible**: until SEC / BCSC / FCA write
  AI-disclosure rules, an issuer demonstrating a deterministic
  substrate that *cannot* emit unlabeled FLI has a stronger
  posture than one relying on probabilistic model behaviour plus
  human review
- **Failure-bounded**: a model bug cannot leak past a deterministic
  gate

The substrate is to the model what TLS is to HTTP — independent of
what the application does, structurally enforced.

### Engineering seams in this crate

- Frontmatter schema (`forward_looking`, `disclosure_class`,
  `cites`) — Phase 1, exists
- Citation registry resolution against `~/Foundry/citations.yaml`
  — Phase 4, planned
- Per-tenant / per-jurisdiction constitution loaded from
  `<content_dir>/.wiki/constitution.md` — Phase 5+ (requires auth
  for tenant identity)
- Rego or equivalent policy engine for the gate logic — Phase 8
  (lands in `project-disclosure` cluster)
- Publishing-pipeline enforcement (CCA — Invention E) — Phase 9
  (project-disclosure cluster)

### Adjacent inventions strengthening this pattern

These came out of Agent 2's prior-art audit and are folded into
the broader catalogue:

- **Claim-granular C2PA extension** — define a C2PA assertion
  schema for "text claim → citation registry ID" with VC 2.0
  signing. Extends C2PA where the text gap currently exists.
- **Citation graph as queryable registry service** — raise
  `citations.yaml` from a YAML file to a registry service with a
  public-resolver endpoint, so external auditors can verify any
  rendered claim.
- **Jurisdictional rule packs as code** — compile NI 51-102, OSC
  51-721, SEC FLI safe-harbor language, FCA Listing Rules, ESMA
  Transparency Directive, ASX 3.1, EDINET / DART rules into
  policy packs (Rego or successor). Render the constitution
  machine-checkable per-tenant.
- **Public reproducibility ledger** — every rendered artefact
  emits a deterministic transcript (input, model output, citations
  resolved, gate decisions). Append-only, signed, externally
  verifiable. Closes the loop on "the substrate is the audit
  trail."
- **Negative-disclosure proofs** — when the substrate refuses a
  render, log *why* (which claim failed, which citation was
  missing). The refusal log is itself disclosure-grade evidence
  of the controls operating.

### Sources (Agent 2)

- [Constitutional AI: Harmlessness from AI Feedback (arXiv:2212.08073)](https://arxiv.org/abs/2212.08073)
- [Constitutional Classifiers (arXiv:2501.18837)](https://arxiv.org/abs/2501.18837)
- [Deliberative Alignment (arXiv:2412.16339)](https://arxiv.org/abs/2412.16339)
- [Vectara — Hallucination Evaluation Model](https://docs.vectara.com/docs/hallucination-and-evaluation/hallucination-evaluation)
- [AttributionBench (Ohio State NLP)](https://osu-nlp-group.github.io/AttributionBench/)
- [C2PA Specification 2.3](https://spec.c2pa.org/specifications/specifications/2.3/specs/C2PA_Specification.html)
- [W3C Verifiable Credentials 2.0 Recommendation (May 2025)](https://www.w3.org/TR/vc-data-model-2.0/)
- [Open Policy Agent](https://www.openpolicyagent.org/)
- [Compliance-to-Code (arXiv:2505.19804)](https://arxiv.org/abs/2505.19804)
- [SEC IAC AI disclosure recommendations Dec 2025](https://www.dandodiary.com/2025/12/articles/securities-laws/sec-investor-advisory-committee-recommends-ai-related-disclosure-guidelines/)
- [OSC Staff Notice 51-721](https://www.osc.ca/en/securities-law/instruments-rules-policies/5/51-721/osc-staff-notice-51-721-forward-looking-information-disclosure)

---

## B — Content-addressed federated AI adapters

### Claim

Cluster adapters (LoRA-style fine-tunes that specialise a base
model for a cluster's writing style + content domain) are
**first-class substrate citizens** — blake3-addressed, stored
alongside content in the same Git remote, distributed via the
same federation primitive as TOPICs. Customer A's adapter can be
subscribed to by Customer B with consent. Adapters compose at
request time per Doctrine claim #22 (Adapter Composition Algebra)
on the axes `base + constitutional + cluster + tenant + role`.
The substrate operator owns the adapters — not an API vendor.

### Novelty kernel

The components individually are not new. Hugging Face Hub migrated
to Xet (content-defined chunking, hash-addressed) in 2026 — content
addressing is now table stakes there. S-LoRA / Lorax / dLoRA / Ray
ship multi-LoRA serving in production. PEFT TIES / DARE handle
adapter merging at 2-3 axes. CivitAI is an adapter marketplace.
Petals does federated layer-streaming inference. Bittensor is a
token-incentivised AI services marketplace. FedEx-LoRA / FedSA-LoRA
do federated LoRA training.

What is unpublished is the integration:

1. **Adapters as Git-federated, content-addressed substrate
   citizens distributed by the same primitive as content**. HF Hub
   is centralised; CivitAI is centralised; Bittensor is a token
   marketplace; Petals is layer-streaming. "Same `git remote`
   ships your TOPICs and your adapters" is a clean composition
   primitive nobody has packaged.
2. **5-axis compositional routing** with the constitutional layer
   carrying jurisdictional rules baked in as an always-applied
   adapter. CivitAI doesn't have a constitutional layer; OpenAI's
   adapters are tenant-only with no shared substrate; Bittensor
   doesn't compose at all. This is the strongest single new
   primitive in the bundle.
3. **Cross-tenant adapter subscription with consent as a
   substrate artefact** — Customer B subscribes to Customer A's
   adapter via a consent record that lives in the same Git
   history. Multi-tenant SaaS has nothing comparable; data-room
   patterns get close but operate on documents, not weights.

Verdict: **moderate originality**. No individual claim is
patent-grade novel; the composite has no published precedent.
PointSav is *first to package* this combinatoric for a
regulated-SMB context. Most defensible if framed as an integration
invention situated in a continuous-disclosure substrate — not as
primary art on adapters or content addressing themselves.

### The strongest single sub-invention

**The constitutional-layer adapter as an always-composed
BCSC-jurisdiction enforcer.** Among all the components, this is
the one with no analogue anywhere in the prior art. CivitAI has
no constitutional layer; Bittensor has no constitutional layer;
OpenAI's system prompt is not weight-resident and not auditable;
HF Hub adapters are unconstrained.

PointSav can ship a per-jurisdiction adapter (NI 51-102 Canada,
OSC SN 51-721 Ontario, future SEC equivalents, FCA, ESMA, ASX,
EDINET, DART) trained to enforce forward-looking-information
labelling, cautionary-language insertion, and material-change
discipline at generation time. Composed under every request.
Content-addressed and auditable. Federated alongside the
disclosure content it governs.

This is what *"BCSC-compliance is the artefact"* (per `bcsc-disclosure-posture.md`)
looks like at the model layer.

### Engineering seams

This invention does NOT live in `app-mediakit-knowledge` —
adapter mechanics live in `service-slm` (the Doorman) and the
adapter training stack lives in the future `project-disclosure`
cluster scope. The wiki engine is a *consumer* of composed
adapters via the MCP server (Phase 4). What this crate must
not close off:

- Per-request context propagation through the MCP server
  carrying enough metadata for the Doorman to compose the right
  adapter axes (cluster + tenant + role)
- The frontmatter `disclosure_class` and `constitution` fields
  feed the constitutional-layer adapter selection

### Risks (real, not hypothetical, per Agent 3)

- **LoRA-Leak (arxiv 2507.18302)** — membership inference on
  LoRA-tuned LLMs at AUC 0.775 even under conservative settings.
  A cluster adapter trained on draft material-change disclosures
  could leak which drafts existed before publication. Mitigations
  (dropout, layer exclusion, selective data obfuscation per
  USENIX Sec '25) must be operator-default, not opt-in.
- **Colluding LoRA (arxiv 2603.12681)** — composite attacks where
  individually-benign adapters compose into jailbreak. The
  5-axis composition surface multiplies this risk. The
  constitutional-layer adapter must enforce alignment *after*
  composition, not before.
- **Consent / privacy** for federated subscription needs explicit
  revocation semantics. Once an adapter is fetched it can be
  retained — content addressing makes provenance auditable but
  does not make weights un-extractable.
- **US export controls** — EAR ECCN 4E091 controls weights of
  closed-weight AI models trained on >10^26 ops. OLMo at ~10^24
  ops is below the threshold so the base is unaffected, but
  adapters trained on top remain controlled if the resulting
  model effectively crosses the threshold or if the adapter
  encodes regulated dual-use capability. Cross-border cluster-adapter
  subscription needs an export-control review per jurisdiction
  adapter — fits naturally as a constitutional-layer rule.

### Sources (Agent 3)

- [LMSYS S-LoRA blog](https://www.lmsys.org/blog/2023-11-15-slora/), [arXiv 2311.03285](https://arxiv.org/pdf/2311.03285)
- [predibase/lorax](https://github.com/predibase/lorax)
- [HuggingFace TGI Multi-LoRA](https://huggingface.co/blog/multi-lora-serving)
- [HF Hub Storage Backends (Xet)](https://huggingface.co/docs/hub/storage-backends)
- [Petals.dev](https://petals.dev/) / [arXiv 2209.01188](https://arxiv.org/abs/2209.01188)
- [Bittensor.ai](https://bittensor.ai/)
- [CivitAI LoRA tag](https://civitai.com/tag/lora)
- [PEFT merging methods](https://huggingface.co/blog/peft_merging)
- [FedEx-LoRA (ACL '25)](https://github.com/RaghavSinghal10/fedex-lora)
- [LoRA-Leak (arXiv:2507.18302)](https://arxiv.org/abs/2507.18302)
- [Selective Data Obfuscation USENIX Sec '25](https://www.usenix.org/system/files/usenixsecurity25-zhang-kaiyuan.pdf)
- [Colluding LoRA (arXiv:2603.12681)](https://arxiv.org/html/2603.12681)
- [BIS AI Diffusion Framework Federal Register Jan 2025](https://www.federalregister.gov/documents/2025/01/15/2025-00636/framework-for-artificial-intelligence-diffusion)

---

## C — Two-clock continuous disclosure with cryptographic anchors

### Claim

Every TOPIC natively carries `published_at` (Git commit timestamp
+ OpenTimestamps Bitcoin anchor + optional eIDAS-qualified RFC
3161 TSA token) AND `valid_at` (frontmatter date the information
applies to). The substrate answers `state(topic, t) → (content,
proof)` returning the content as it was at time `t` plus
cryptographic proof of both clocks, deployable by an SMB issuer
without a regulator partnership and without trusting a single
archive vendor.

### Novelty kernel

Each ingredient is commodity:

- **OpenTimestamps** — continuously operated since 2016, Bitcoin-
  anchored Merkle aggregation through free public calendar servers,
  GitHub Actions auto-stamping every commit
- **RFC 3161 TSA** — DigiCert / Sectigo / Entrust / GlobalTrust
  / Evrotrust ETSI EN 319 422 services; eIDAS 2 Qualified
  Electronic Ledgers land EU-wide Dec 2026
- **Sigstore Rekor v2** — GA in 2025; PyPI / Maven Central /
  NVIDIA NGC issue Sigstore attestations in production
- **Bitemporal databases** — distinguish *valid time* from
  *transaction time*; finance-industry workhorse for two decades
- **XBRL period contexts** — distinguish *instant* from *duration*;
  FASB 2024 Subsequent Events TIG explicitly tags events that
  occurred after balance-sheet date but before issuance
- **OFAC sanctions** — publishes effective and end dates separately
  from publication time
- **C2PA Content Credentials** — declares AI provenance for media

Verdict: **composition novel; primitives commodified**. The novelty
kernel is not OTS, not RFC 3161, not bitemporal modelling, not XBRL
periods, not C2PA. It is the **substrate-level composition**:
every TOPIC carries both clocks anchored cryptographically, the
substrate answers temporal queries with proof, deployable by an
SMB issuer as part of its reporting infrastructure. The
**integration** does not exist; the closest analogue (HTX's 35-
month Merkle Proof-of-Reserves run for a single exchange) shows
the operational shape but neither generalises nor handles
two-clock semantics.

### What this fills

- **The Wayback gap** — Wayback Machine custody (limited
  evidentiary status post-Weinhoffer v. Davie Shoring 5th Cir.
  2022) → OTS-anchored custody you control
- **The SolarWinds gap** — prospective, continuous proof of
  website state, not retroactive archival forensics. SEC v.
  SolarWinds turned on what an issuer's *website* Security
  Statement said on what day, and counsel had to spend declarations
  to prove it; an OTS proof would settle that in a hash compare.
- **The XBRL-narrative gap** — structured "as of" semantics
  extended from numeric XBRL tags to disclosure prose
- **The RAG-grounding gap** — AI claim → citation hash → timestamp
  anchor, end-to-end (composes with Invention A and CCA)

### Engineering seams

- Frontmatter `published_at` + `valid_at` fields — Phase 8 schema
  extension
- OpenTimestamps integration via post-commit hook running
  `ots stamp` on every TOPIC commit — Phase 8
- RFC 3161 TSA configurable URL — Phase 8
- `state(topic, t)` query endpoint via REST + MCP — Phase 8
  (composed of Phase 4 git history + Phase 7 content-addressed
  read + Phase 8 timestamp proof verification)

### Sources (Agent 4)

- [OpenTimestamps GitHub](https://github.com/opentimestamps)
- [RFC 3161 TSP](https://www.ietf.org/rfc/rfc3161.txt)
- [Sectigo eIDAS TSPPS](https://www.sectigo.com/uploads/legal/Sectigo-eIDAS-TSPPS-v1.0.6.pdf)
- [Sigstore Rekor v2 GA](https://blog.sigstore.dev/rekor-v2-ga/)
- [IETF Merkle Tree Certificates draft](https://www.ietf.org/archive/id/draft-davidben-tls-merkle-tree-certs-09.html)
- [Bitemporal modeling — Wikipedia](https://en.wikipedia.org/wiki/Bitemporal_modeling)
- [FASB Subsequent Events XBRL TIG](https://xbrl.fasb.org/impguidance/SE2_TIG/subsequentevents_2.pdf)
- [SEC v. SolarWinds Complaint](https://www.sec.gov/files/litigation/complaints/2023/comp-pr2023-227.pdf)
- [Norton Rose Fulbright — Wayback Machine in court proceedings](https://www.nortonrosefulbright.com/en/knowledge/publications/57e50249/using-screenshots-from-the-wayback-machine-in-court-proceedings)
- [Estonia KSI blockchain (Guardtime)](https://e-estonia.com/solutions/cyber-security/ksi-blockchain/)
- [HTX 35-month Merkle Tree Proof of Reserves](https://www.ainvest.com/news/crypto-exchange-transparency-trustworthiness-2025-htx-35-month-merkle-tree-reserve-proof-benchmark-industry-security-asset-safety-2509/)
- [C2PA Content Credentials whitepaper Sep 2025](https://c2pa.org/wp-content/uploads/sites/33/2025/10/content_credentials_wp_0925.pdf)

---

## D — Disclosure-Diff as Signed Artefact + Subscriber Proof-of-Receipt

### Claim

Two new substrate-deliverable evidence objects layered on top of
the two-clock substrate (Invention C):

**(D1) Disclosure-Diff as Signed Artefact.** The substrate emits
`diff(snapshot_a, snapshot_b)` signed under the issuer key with
both endpoint OTS proofs embedded. Result: *the change itself* is
a first-class, citable, machine-verifiable artefact. Investors
and regulators stop arguing about whether the issuer's website
"changed materially" between dates — the diff *is* the disclosure.
This subsumes IAS 10 subsequent-events practice in machine-native
form and gives plaintiffs' counsel and BCSC enforcement staff a
primitive that doesn't exist today.

**(D2) Subscriber Proof-of-Receipt with Merkle Inclusion.** Every
subscriber to issuer X's substrate (analyst, regulator, retail
investor) gets a signed receipt embedding (i) the TOPIC content
hash, (ii) the daily Merkle root the issuer publishes to a public
chain, and (iii) the subscriber's identity. The investor can prove
*they received version V at time T*; the issuer can prove *they
published version V at time T*; the regulator can verify both
against the public Merkle root without trusting either party.

### Novelty

D1 has weak precedent in the e-discovery world (hashed PDF diffs
have been Bates-numbered and signed in litigation contexts since
the early 2010s) but no precedent as a *substrate-deliverable
disclosure object* — i.e., produced automatically as a first-class
substrate output rather than reconstructed adversarially after
litigation begins.

D2 has direct precedent in Certificate Transparency's
monitor/auditor split (RFC 9162 / 6962) and Sigstore Rekor's
inclusion-proof model. The novelty is the *application to capital-
markets continuous disclosure* — no corporate disclosure system
ships proof-of-receipt today. The closest analogue is regulator-
mandated email logging at investment banks, which is an
adversarial archival pattern, not a substrate primitive.

### Engineering seams

- Phase 8 — diff endpoint already implied by `git diff` access
  at Phase 4; signing + OTS embedding lands in Phase 8
- Subscriber registry — Phase 5 (auth) provides identity; webhook
  delivery (Phase 5) carries the receipt; Phase 8 adds the Merkle
  inclusion proof
- Daily Merkle root publication — runs as a cron job alongside the
  binary; publishes to OpenTimestamps + optional public chain via
  configurable adapter

### Sources (Agent 4 + adjacent)

- [SEC v. SolarWinds Complaint](https://www.sec.gov/files/litigation/complaints/2023/comp-pr2023-227.pdf) — demonstrates the gap D1 fills
- [USI Affinity: From Bates Stamping to Hashing](https://insurancefocus.usiaffinity.com/2018/09/from-bates-stamping-to-hashing-the-immeasurable-value-of-indexing-discovery.html)
- [RFC 9162 Certificate Transparency v2.0](https://www.rfc-editor.org/rfc/rfc9162.html)
- [Sigstore Rekor inclusion-proof model](https://docs.sigstore.dev/logging/overview/)
- [HTX 35-month Merkle Proof-of-Reserves benchmark](https://www.ainvest.com/news/crypto-exchange-transparency-trustworthiness-2025-htx-35-month-merkle-tree-reserve-proof-benchmark-industry-security-asset-safety-2509/)

---

## E — Constrained-Constitutional Authoring (CCA) — the killer

### Claim

The substrate's TOPIC schema (frontmatter shape, citation-ID
syntax, FLI label vocabulary, BCSC structural-positioning rules)
is compiled into a context-free grammar that the Doorman injects
as a logit constraint at AI decode time. The AI **cannot** emit
a TOPIC that fails the schema — not as a post-hoc lint, but as a
generation-time invariant. Every emitted TOPIC carries a
machine-checkable proof-of-grounding chain (citation IDs resolved
against `citations.yaml`, source content hashes pinned, adversary-AI
verdict signed as a W3C VC) committed inside the same Git commit
as the TOPIC. The substrate refuses to render a TOPIC whose proof
chain doesn't verify.

CCA is the synthesis of Inventions A + B + C + D pushed inside
the substrate boundary instead of treated as an external editor:

- A (substrate-enforced grounding) provides the refusal mechanism
- B (federated adapters) supplies the schema and the AI-tier
  composition (constitutional-layer adapter contributes the
  jurisdictional rules)
- C (two-clock cryptographic) supplies the timestamp +
  non-repudiation surface for the proof chain
- D (signed-diff + proof-of-receipt) makes every CCA-authored
  artefact a first-class disclosure object with cryptographic
  receipts

### Why constrained decoding makes this technically viable in 2026

[llguidance](https://github.com/guidance-ai/llguidance), XGrammar,
and native structured output achieve near-zero-overhead constrained
decoding in 2026 ([DEV: LLM Structured Output in 2026](https://dev.to/pockit_tools/llm-structured-output-in-2026-stop-parsing-json-with-regex-and-do-it-right-34pk),
[constrained-decoding tutorial](https://mbrenndoerfer.com/writing/constrained-decoding-structured-llm-output)).
The token-sampler-level constraint is no longer experimental.

The CFG that constrains TOPIC authoring is generated from:

- The frontmatter JSON Schema (already exists for §6)
- The citation-ID lexicon (from `citations.yaml`)
- The FLI label vocabulary (from
  `conventions/bcsc-disclosure-posture.md`)
- The "Do Not Use" terms list (from
  `POINTSAV-Project-Instructions.md` §5)
- The structural-positioning rules (CLAUDE.md §6)

The result: AI authoring becomes a productive contributor whose
output is structurally compliant with disclosure rules, where
human review focuses on substantive correctness rather than
formal compliance.

### Why this is "AI+ more than the sum of its parts"

In the conventional frame, AI productivity and compliance posture
trade off: more AI authoring → more risk of hallucination, more
need for human review, less compliance defensibility. CCA inverts
the trade-off: more AI authoring increases productivity *and*
strengthens compliance posture, because every AI output is
structurally bound to disclosure rules and cryptographically
witnessed.

The defining contribution: **the substrate is the compliance
witness; the AI is the productive author; the human is the editor
of last resort**. That reversal is the through-line.

### Why CCA is structurally beyond what hyperscalers / IR vendors can ship

- A hyperscaler-managed AI cannot give an SMB a per-tenant
  constitution that the customer owns and modifies — the
  constitution is hyperscaler IP and changes under the hyperscaler's
  release cadence
- An IR vendor (Q4, Confluence, Notion) cannot bake a regulator-
  jurisdiction CFG into their proprietary CMS without becoming a
  regulated party — and the substrate-portable, customer-owned,
  hash-addressed CFG is structurally incompatible with multi-tenant
  SaaS economics
- Foundry's customer-first ordering — operator owns substrate,
  schema, model, anchor, constitution — is the precondition CCA
  requires

### Engineering seams in this crate

- Frontmatter schema (Phase 1, exists) — source for the CFG
- Citation registry resolution (Phase 4, planned) — wires up
  `citations.yaml` as a query surface for the proof-of-grounding
  pipeline
- C2PA-style claim binding (Phase 7 seam, Phase 9 active)
- W3C VC signing (Phase 9 only, new dep)
- Doorman handshake (Phase 4 MCP server) — provides the surface
  the constrained-decoding pipeline calls back through

CCA depends on `project-disclosure` cluster scope to land
properly. ARCHITECTURE.md captures the engine-side seams so Phase
7 doesn't accidentally close them off.

### Recommended doctrine touch

Either (a) extend `conventions/disclosure-substrate.md` with a §8
"Constrained-Constitutional Authoring" section, OR (b) new DOCTRINE
claim #31 standing on its own. Recommendation: (b), because CCA
generalises beyond the disclosure substrate (any cluster could
adopt the pattern for its own quality discipline — engineering
docs, project narratives, code reviews). Surfaced to Master via
session-2 cluster outbox 2026-04-26 for the call.

### Sources (Agent 5 + adjacent)

- [llguidance — Super-fast Structured Outputs](https://github.com/guidance-ai/llguidance)
- [LLM Structured Output in 2026 (DEV)](https://dev.to/pockit_tools/llm-structured-output-in-2026-stop-parsing-json-with-regex-and-do-it-right-34pk)
- [Constrained Decoding tutorial](https://mbrenndoerfer.com/writing/constrained-decoding-structured-llm-output)
- [Anthropic Claude Constitution updated Jan 2026](https://www.anthropic.com/news/claudes-constitution)
- [Mitigating Hallucination in LLMs survey (arXiv:2510.24476)](https://arxiv.org/html/2510.24476v1)
- [Eliminating the Rego tax — Red Hat Mar 2026](https://next.redhat.com/2026/03/20/eliminating-the-rego-tax-how-ai-orchestrators-automate-kubernetes-compliance/)
- [SEC FDTA machine-readable direction](https://www.cov.com/en/news-and-insights/insights/2024/02/how-will-the-sec-expand-the-use-of-machine-readable-data)
- [OSC machine-readable regulatory framework consultation](https://www.osc.ca/en/news-events/news/osc-seeks-feedback-efforts-create-machine-readable-regulatory-framework)
- [ISDA/IIF/GFMA on Basel machine-readable Pillar 3](https://www.isda.org/2026/03/03/isda-iif-gfma-respond-to-basel-committee-on-machine-readable-pillar-3-disclosure)

---

## 6. Composition — how the five compound

A through B alone is constitutional AI at the substrate layer with
portable adapters. C alone is provable historical state. D alone is
new evidence objects.

The compounding emerges when all five compose:

```
                       ┌──────────────────────────────────┐
                       │  Constitutional layer adapter    │
                       │  (BCSC NI 51-102 / OSC 51-721)   │
                       │  — Invention B sub-element       │
                       └────────────────┬─────────────────┘
                                        │
                  Doorman composes per-request:
                  base + constitutional + cluster + tenant + role
                                        │
                                        ▼
            ┌───────────────────────────────────────────┐
            │  Doorman injects substrate CFG as logit   │
            │  constraint at decode time — Invention E  │
            └─────────────────────┬─────────────────────┘
                                  │
            AI emits TOPIC + proof-of-grounding chain
                                  │
                                  ▼
            ┌───────────────────────────────────────────┐
            │  Substrate-enforced grounding gate        │
            │  refuses render if proof doesn't verify   │
            │  — Invention A                            │
            └─────────────────────┬─────────────────────┘
                                  │
            Commit lands; OpenTimestamps anchor;
            published_at + valid_at recorded
            — Invention C
                                  │
                                  ▼
            ┌───────────────────────────────────────────┐
            │  Disclosure-Diff signed artefact          │
            │  + Subscriber Proof-of-Receipt issued     │
            │  — Invention D                            │
            └───────────────────────────────────────────┘
```

The substrate is, end-to-end, the compliance witness for every
AI-assisted disclosure event. The AI is the productive author.
The human is the editor of last resort. Each commit is a
cryptographically-verifiable disclosure event. Federation lets
this scale to a network of issuers without a central party.

This compounds with the existing substrate properties:

- **Per-tenant audit ledger** captures every constraint-violation
  rejection (negative-disclosure proof — Invention A adjacent)
- **Trajectory substrate** captures every accepted draft as
  training corpus (cluster adapter improves)
- **Federation** lets one Customer's CFG improvements (a new
  FLI-violation pattern they discovered) propagate as a
  content-addressed adapter to peers (Invention B)
- **Continued-pretraining** picks up the per-tenant constitution
  as fine-tuning corpus (substrate gets better at writing for
  *this* customer)

CCA is the through-line that ties all five compounding-substrate
properties into the AI-authoring loop.

---

## 7. Anti-patterns — what these inventions explicitly REJECT

The killer-question framing rejects:

- **Anything requiring a central registry for content addressing**
  — keep it BLAKE3 + Git + Iroh-style DHT discovery, never a SaaS
  index
- **Anything chain-locked** — chain-agnostic (OpenTimestamps to
  Bitcoin, RFC 3161 TSA, Rekor) is the right composition
- **Anything that requires AI to function** — Ring 3 is structurally
  optional per Doctrine. The candidate inventions are valuable
  when AI is present; they degrade gracefully to "no AI authoring"
  when not. Inventions A, C, D are AI-independent; B and E require
  AI to deliver value
- **Anything violating single-binary / no-Docker / systemd** —
  rules out heavy CRDT runtimes, blockchain nodes, distributed
  databases at the engine level
- **Anything that locks the substrate's compliance logic into a
  single jurisdiction** — every constitutional adapter is
  content-addressed and pluggable
- **Anything that introduces a SaaS dependency at the
  disclosure-record layer** — Q4-style "we host your IR" is
  exactly what the substrate inverts
- **CRDT-shaped disclosure records** — disclosure cannot say
  "issuer asserted X at time T" with concurrent edits and no
  authority decision. CRDTs may land for *draft co-authoring
  inside a cluster*; never for the disclosure-tier surface

---

## 8. References

The session-2 research synthesis citations are inline above.
Cross-references to the wider workspace:

- [`../ARCHITECTURE.md`](../ARCHITECTURE.md) — engineering design;
  Phase plan; API surface set; references this doc as
  companion-doc
- [`~/Foundry/DOCTRINE.md`](../../../../../DOCTRINE.md) — claim #29
  Substrate Substitution; claim #30 Project Triad Discipline; CCA
  proposed as claim #31 in session-2 outbox to Master
- [`~/Foundry/conventions/disclosure-substrate.md`](../../../../../conventions/disclosure-substrate.md) —
  landing point for inventions A, C, D, E (and B via constitutional
  layer); §5 currently retains `mediawiki-action-api-shim`
  pending Master review of session-2 outbox
- [`~/Foundry/conventions/compounding-substrate.md`](../../../../../conventions/compounding-substrate.md) —
  Three-Ring + Doorman composition; CCA is the AI-authoring
  application of this pattern
- [`~/Foundry/conventions/citation-substrate.md`](../../../../../conventions/citation-substrate.md) —
  citation discipline; Invention A's gating mechanism reads from
  this registry
- [`~/Foundry/conventions/trajectory-substrate.md`](../../../../../conventions/trajectory-substrate.md) —
  every commit feeds the cluster adapter; Inventions B and E
  build on this pipeline
- [`~/Foundry/conventions/bcsc-disclosure-posture.md`](../../../../../conventions/bcsc-disclosure-posture.md) —
  source for the FLI-labelling and structural-positioning rules
  that the CCA constraint enforces

---

*This doc is a thinking artefact, not yet a doctrine clause or a
shipped feature. Master decides on the doctrine touch; the
engineering seams in `app-mediakit-knowledge` evolve to leave room
for these inventions to land in `project-disclosure` cluster scope
without requiring engine rework.*
