---
artifact: brief
name: BRIEF-tool-proforma-leapfrog-2030
status: active
created: 2026-05-23
updated: 2026-06-02
version: 0.15.9
owner: totebox@project-proforma
supersedes:
  - BRIEF-proforma-engine (.agent/briefs/ — archived below)
  - BRIEF-proforma-V2 (briefs/ — archived below)
research_sources:
  - leapfrog-2030-panel-2026-05-23 (7 opus agents: accounting, finance, M&A, banking,
    data science, software architecture, market research)
  - d2-formula-panel-2026-05-23 (3 opus agents: accounting, finance, data science — PCLP 1 model)
  - aa21-an28-cell-audit-2026-05-23 (Excel/data opus agent — AA21:AN28 MV block cell formulas)
  - wcp-panel-2026-05-23 (3 opus agents: accounting IS/LP cascade, finance 4-valuation methods,
    data science Rust struct design + LP lag computation)
  - ad2-panel-2026-05-23 (4 opus agents: accounting IFRS consolidation, finance dilution mechanics,
    data science Ad2 Rust struct, banking investor presentation + compliance review)
  - ad1-panel-2026-05-23 (4 opus agents: accounting IFRS 9/IFRS 2/deferred-tax, finance summary-block
    design + CAGR, data science Ad1Config/Ad1Data Rust structs, banking BCSC/NI-45-106 compliance)
  - bencal-panel-2026-05-23 (4 opus agents: accounting IFRS 10.27/IAS 12/investment-entity election,
    finance portfolio composition + valuation matrix Block A-F, data science bencal.rs rewrite spec,
    banking BCSC/Form-45-106F1 compliance + commission income review)
  - je-tb-panel-2026-05-23 (4 opus agents: accounting CoA + JE mapping per entity, finance
    stakeholder value + assurance workflow, data science Rust JE/TB structs + phase gate,
    banking regulatory analysis + competitive positioning vs ARGUS)
  - snapshot-panel-2026-05-23 (4 opus agents: accounting CSAE 3420 + 5-layer schema,
    finance stakeholder value + IP risk, data science Rust schema design + sizing,
    banking regulatory posture + WORM tiers + competitive moat)
  - ri-tier-panel-2026-05-24 (4 opus agents: accounting IAS 1/34/ASPE/CSRS 4250 + standards
    correction, finance MD&A blocks + SEDAR + prospectus-grade, data science ReportingTier
    enum + ASPE dispatch, banking NI 51-102/52-107 + cross-tier triggers)
  - d7-legacy-jv-panel-2026-05-24 (4 opus agents: accounting ASPE 3061/IAS 23 construction
    phase + JV partner capital, finance MOIC/IRR geometric-vs-single-shot + stakeholder value,
    data science JvConfig/JvData/D7vsD2Comparison Rust design, banking construction loan
    mechanics + $69M trapped-equity proof + NI 45-106 + NI 52-107 "illustrative comparison")
  - d1-portfolio-architecture-panel-2026-05-24 (4 opus agents: accounting IAS 40 IFRS 8
    segment disclosure + audit-effort reduction, finance class-mix variability + portfolio NOI
    volatility dampening, data science 4-stage computation pipeline + per-building expansion
    + ConstructionPhase enum + ULID identity, banking OSFI B-20/B-21 per-property appraisal
    + NI 45-106 allocation disclosure + NI 51-102 material-change trigger)
  - output-format-ey-panel-2026-05-24 (4 opus agents: accounting CSRS 4250 + EY note
    architecture F/P/C taxonomy + assurance-partner savings, finance IAS 1 statement
    ordering + two-document vs compound + AA12:AM35 placement + NI 52-112, data science
    OutputFormat/ReportProfile/ColumnStyle/NumFmt Rust types + notes-template system +
    RowKind/RowEmphasis + Phase A forward-compat hinge, banking CSRS 4250 practitioner-vs-
    management FOFI + NI 45-106 sign-off gate + NI 51-102 Form 51-102F1 + WORM s.85 readiness)
---

# BRIEF — tool-proforma / Leapfrog 2030

> **Constitutional posture:** all forward-looking projections carry planned/intended/target
> language. BCSC continuous-disclosure posture applies throughout. SYS-ADR-19: no automated
> AI publishing to verified ledgers. This brief is internal planning only.

---

## 1. Product vision

**Leapfrog 2030** is a Goldman Sachs–quality financial proforma engine — not a property
management ERP (Yardi/Sage), not an Excel template library (REFM), not a legacy DCF tool
(ARGUS). It targets institutional-quality outputs at $19/month, closing the gap between
ARGUS at $8,000–$15,000/user/year and nothing.

**Positioning:** ARGUS-fidelity financial modeling with modern UX, audit-grade chain of
custody, and dual-basis outputs (Cash NOI vs. GAAP NOI; Book NAV vs. Adjusted NAV vs.
Market Value) as a first-class default — not an add-on.

**Target market:** real estate developers, small PE/family offices, LP syndicators, CPA
firms serving real estate clients. TAM: $30–60M ARR addressable wedge in Canada + US at
2% conversion. ARPU target $80–$150 (entry tier $19 is marketing hook; Pro tier $99).

**What the incumbents get wrong (panel consensus):**
1. No fair-value model — cost basis only (Yardi/QuickBooks fail IAS 40)
2. CAM/lease-incentive amortization opaque; no audit trace to lease clause (Yardi)
3. Assumptions overwritten in place; no immutable history; cannot survive restatement (ARGUS)
4. No LP waterfall, no IRR/MOIC, no dual-NAV (none of them)
5. Single-scenario only — banker rebuilds in Excel every time

---

## 2. Technology stack (locked)

**Decision: Rust computation core, axum REST API, Svelte SPA frontend.**

Rationale (software architect + accounting + data science panel consensus):
- Rust + `rust_decimal` = no float drift across platforms; deterministic serialisation
- Cargo.lock = reproducible dependency closure; SHA-addressable binary
- Same engine binary serves CLI (now), server (phase 2), WASM (phase 3)
- SSH-signed binary integrates with Foundry identity store
- Audit story: one signed binary → one signed output → one chain-of-custody record

Current crates: `calamine` (Excel), `serde_json`, `clap`, `pulldown-cmark`.

**Crates to add (phased):**
| Crate / component | Purpose | Phase |
|---|---|---|
| `rust_decimal` | Money arithmetic (no f64 for cash flows) | A |
| `toml` or `serde_json` | External DevClassConfig files | A |
| `ulid` | Run IDs for audit records | B |
| Newton-Raphson IRR | Custom — no mature Rust IRR lib | B |
| `proforma-ledger` (new crate) | WORM audit log, hash-chained JSONL | B |
| Sobol sweep harness (~300 LOC) | Sensitivity / Monte Carlo | C |
| `typst` subprocess | Signed PDF bundle generation | C |
| `axum` + `tokio` | REST API server | D |
| Svelte SPA | Frontend | D |
| `je-generator` (new module) | JE templates per entity; CoA constants | B |
| `proforma-tb` (new module) | TrialBalance struct; TB-to-IS/BS bridge | B |
| JE JSONL chain | Parallel WORM chain alongside `proforma-ledger` | B |
| CaseWare export adapter | CSV/JSON with `assumption_ref` field | B |
| `Override::Variable` | Assumption-edit → JE regeneration | B |
| `Override::Journal` | JE-edit → assumption back-solve (enterprise only) | D/E |
| `schemars` | Derive `JsonSchema` from Rust structs; CI diff gate | B |
| `src/snapshot/` (new module) | Calculation snapshot builder per entity | B |
| `tool-proforma-render` (new binary) | HTML/MD renderer; reads snapshot only; zero engine dep | B |
| `data/snapshots/<entity>/<run_id>.json` | Snapshot store (gitignored in deployments) | B |

**Not Leptos/Dioxus** — too immature for a billable product at this stage.

---

## 3. Compliance architecture

### 3a. Audit trail (SOC2 → prospectus level)

Per-run record (WORM JSONL, hash-chained, one entry per output):
```
run_id            ULID (time-sortable)
created_at        RFC 3339 UTC
user_attribution  jwoodfine | pwoodfine (SSH-signed)
input_sha256      hex of input workbook or config file
model_version     semver + git SHA
binary_sha256     compiled engine hash
cargo_lock_sha256 dependency closure hash
as_of_date        ISO date (frozen at run time)
output_sha256     hex of canonical output JSON
compute_ms        integer
signature         SSH-signed manifest (Foundry identity store)
prev_entry_sha256 hash of previous JSONL line (tamper-evident chain)
```

Chain of custody for prospectus: `source_data_hash → assumption_set_hash → engine_binary_sha256
+ git_sha → output_sha256 → RFC 3161 timestamp → SSH signature → auditor co-sign`.

### 3b. Financial standards

**IFRS (mandatory for D2/D3 reporting issuers; D1/D4/D5/D6 may elect ASPE):**

| Standard | Applies to | Requirement |
|---|---|---|
| IAS 40 | D2 Direct-Hold, D3 WCP | Investment property at fair value; opening→closing reconciliation |
| IFRS 13 | D2, D3 | Level 3 hierarchy; cap rate sensitivity table (±25/50/100 bps → NAV impact) |
| IFRS 16 | D1, D2, D3 lease income | Straight-line revenue; TI allowance/free rent amortisation; fixed vs. variable |
| IFRS 9 | All IFRS entities | ECL on tenant receivables; LP unit classification (liability vs. equity, IAS 32.16A–D puttable exception) |
| IFRS 10.27 | D2 PCLP 1 (investment entity) | Investment entity exception: no consolidation; investments at FVTPL; disclosure per IFRS 12.19A–G |
| IAS 23 | D1 construction phase | Borrowing cost capitalisation during active development; suspend on pauses |
| IFRS 15 | D1, D2, D3 | Revenue recognition: management fees, CAM recoveries not in scope of IFRS 16 |
| IAS 1 | D2, D3 (RI) | Comparative periods, Statement of Changes in Equity (IAS 1.106), going-concern disclosure (IAS 1.25–26), capital management notes (IAS 1.134–136) |
| IAS 34 | D2, D3 (RI) | Condensed quarterly IS/BS/CF/SOCE; current quarter + YTD vs. prior-year equivalents |
| IAS 24 | D2, D3 (RI) | Related-party disclosures: WCP↔PCLP 1↔Bencal SPV1/Bencal SPV2↔Bencal Management flows; material for MD&A |
| IFRS 8 | D3 WCP (RI, if publicly traded) | Segment reporting by building class or geography; D1 outputs are natural segment building blocks |
| IFRS 11 | D7 Legacy JV (at investor/partner level) | JV classified as joint venture → equity method at each partner's financial statements; D7 itself presents gross assets/liabilities (LP-level statements) |

**ASPE alternative (D1/D4/D5/D6 private entities only — confirmed private; not available to D2/D3):**

| Standard | ASPE equivalent | Key difference from IFRS |
|---|---|---|
| IAS 40 FV model | ASPE 3061 cost model | Cost less accumulated depreciation/impairment; no Level 3 FV sensitivity table |
| IFRS 9 ECL | ASPE 3856 incurred-loss | No expected-credit-loss model; impairment only on objective evidence |
| IFRS 16 ROU asset | ASPE 3065 capital/operating | Operating/capital dichotomy retained; no right-of-use asset |
| IAS 23 | ASPE 3850 (elective) | Capitalisation permitted but not required |
| IAS 1 SOCE | ASPE: Statement of Retained Earnings | No full statement of changes in equity required |

**Assurance tiers (per reporting tier):**

| Entity tier | Assurance standard | Annual | Interim |
|---|---|---|---|
| Reporting issuer (D2, D3) | CAS audit (NI 52-107 Part 3 — IFRS mandatory) | Full CAS audit | Review (NI 51-102 Part 4) |
| Private IFRS/ASPE (LP external users) | CSRS 2400 review engagement | Review | None required |
| Private ASPE (internal / lender only) | CSRS 4200 compilation | Compilation | None |
| Prospectus pro forma (any tier) | CSRS 4250 / ISAE 3420 (prospectus pro forma) | 3-column format: source / adjustments / pro forma | N/A |

Note: "CSAE 3420" does not exist as a Canadian standard. The correct references are **CSRS 4250** (CPA Canada, Jan 2026 — examination of FOFI) and **ISAE 3420** (international standard for pro forma financial information in a prospectus). Engine outputs feeding a prospectus must produce the CSRS 4250 / ISAE 3420 three-column format.

### 3c. BCSC continuous-disclosure posture

All outputs are forward-looking. Every report carries `planned/intended/target` language.
No proforma output published as verified ledger entry (SYS-ADR-19). Full rules:
`~/Foundry/conventions/bcsc-disclosure-posture.md`.

### 3d. SOC2 Type II minimums (before software.pointsav.com launch)

TLS 1.3 only; HSTS preload; Postgres RLS (tenant_id on every row, enforced at query layer);
OIDC + MFA mandatory; WORM audit log (JSONL, hash-chained); encrypted backups off-region;
Drata/Vanta for GRC; change management traced to signed git commits (existing).

### 3e. Calculation snapshot architecture

Every engine run produces a `<run_id>.snapshot.json` as the **canonical artifact**. HTML,
Markdown, and PDF are renderers that read only the snapshot — zero access to engine internals.
The snapshot is version-controlled and replayable; any renderer can be swapped without
re-running the engine; old snapshots remain renderable across schema versions so long as
`schema_version` compatibility is maintained.

**Five-layer schema:**
- **Layer A** — Engagement metadata: `run_id` (ULID), `schema_version`, `entity`, `created_at`,
  `user_attribution`, `input_sha256`, `model_version`, `binary_sha256`, `worm_anchor.run_id`,
  `distribution` enum (internal | lender | investor_om | regulator | audit_evidence)
- **Layer B** — Assumption register: every TOML input assumption with stable ID (`ASM.*`), value,
  units, source_type, source_id, source_date, support_doc_sha256 (required when
  `distribution: audit_evidence`)
- **Layer C** — Calculation trace: `computed.scalars` + `computed.series[Y1–Y10]` +
  `computed.derivations[]` (one entry per field, not per cell; formula + `inputs[]` of
  ASM/CONST/SR IDs; `trace[y]` present only in `--full` mode)
  - **Portfolio aggregation — three-tier Layer C**: Tier 1 (top-line): vehicle total =
    Σ class contributions; Tier 2 (class): class contribution = Σ `BuildingOutput` for
    that class; Tier 3 (leaf): each building's `ConstructionPhase`, area, rent/sf, draw
    schedule, capex. Slim snapshot omits Tier 3; `--full` includes all three tiers for
    auditor recomputation.
- **Layer D** — Statements: IS/BS/CF rows as `values_ref` pointers into Layer C (never
  duplicated numbers); every row carries `gl_account` + `source_line_item` linking to
  JE/TB shared ID space
- **Layer E** — Compliance overlay: `ifrs_classifications` per entity, `divergences[]`
  (known IFRS flags — LP5/LP6 FX anomaly, D2 income-continuity Y1–Y3, D5 going-concern,
  etc.), `disclosure_grade` boolean (true requires all Layer B `_basis` fields populated)

**Paired artifacts** (required when `disclosure_grade: true`):
- `snapshot.json` — canonical artifact
- `basis_of_preparation.md` — templated from Layer E + `basis_overlay.toml` judgmental overlay
- `management_representation.md`
- `sensitivity_report.md`
- `change_log.md` — diff vs prior baseline at same `as_of_date`

**WORM relationship:** WORM JSONL is the chain-linked index; snapshot is the content artifact.
WORM record carries `snapshot_sha256` + `je_merkle_root`; snapshot Layer A carries
`worm_anchor.run_id`. Symmetric binding.

**Provenance tiers (under uniform $19 FSL-1.1-Apache-2.0 license, v0.15.7):**
| Tier | Integrity | Delivery |
|---|---|---|
| T1 | SHA-256 hash + SSH signature + per-customer chain | Included with $19 license |
| T2 | T1 + RFC 3161 timestamp + `_basis` fields + Rekor anchoring | Included with $19 license; optional services for setup |
| T3 | T2 + auditor co-sign + recomputation attestation + secondary anchor | Included with $19 license; audit-firm partnership services required for sign-off |

Tier delivery differences are documentation, onboarding, and services depth — not pricing or product variants.

**AI readability:** The snapshot is designed for local SLM (Foundry tier) and in-house Big 4
audit-tool consumption. The JSON carries all display intent (labels, units, format hints, row
groupings, footnotes) so a renderer or AI tool needs no implicit engine knowledge. Not intended
for external AI distribution — see BCSC selective-disclosure posture.

**Jennifer decisions (resolved 2026-06-02):**
- **Flag S1** ✓ — TSA provider: **FreeTSA (dev) → DigiCert (production)** two-tier; cost-effective dev path; DigiCert acceptable to Big-4 auditors.
- **Flag S2** ✓ — Rekor anchoring cadence: **nightly batch + manual ad-hoc** anchor available for time-critical deliverables; meets verifiability bar without per-run cost.
- **Flag S3** ✓ — Submit snapshot schema as open standard to CPA Canada: **Yes — post-Phase B** (when v1.0 ships with 2–3 audit-firm pilot deployments validating it). Strongly aligned with FSL/Apache-2.0 licensing posture (Flag P4).
- **Flag S4** ✓ — Audit-firm partnership: **own dedicated sprint post-Phase A** (after at least one PCLP1 proforma delivered end-to-end). Now a services-revenue channel under FSL pricing (see §3f Flag P4).

### 3f. Reporting Issuer vs Private Entity — tier architecture

**Entity classification (Woodfine deployment):**

| Entity | Code | Reporting tier | Accounting standard | Venture/NV? |
|---|---|---|---|---|
| PCLP 1 Direct-Hold Solutions | D2 | Reporting issuer — IFRS | IFRS mandatory (NI 52-107) | TBD (Flag P1) |
| Woodfine Capital Projects Inc. | D3 | Reporting issuer — IFRS | IFRS mandatory (NI 52-107) | TBD (Flag P1) |
| Development Classes | D1 | Private — ASPE default | ASPE 3061/3856/3065 | Not applicable |
| Bencal SPV2-LP | D4 | Private — IFRS or ASPE | Election per LP agreement | Not applicable |
| Bencal Special Purpose 1 Inc. | D5 | Private — ASPE default | ASPE unless bank covenant | Not applicable |
| Bencal Management Corp. | D6 | Private — ASPE default | ASPE unless bank covenant | Not applicable |

**Reporting issuer output surfaces (D2/D3 — beyond private entity base):**
- Comparative financial statements (prior year; IAS 1.38)
- Statement of Changes in Equity (IAS 1.106) — not required for private/ASPE
- IAS 34 condensed interim quarterly outputs (60-day filing deadline, venture; 45-day non-venture)
- MD&A building blocks: results-of-operations Y/Y table, contractual obligations by maturity bucket
  (<1yr / 1–3yr / 4–5yr / >5yr), 8-quarter rolling summary, related-party schedule (IAS 24 / Form 51-102F1)
- IFRS 8 segment disclosure (D3 only — by building class, matching D1 outputs; segment_id tags required)
- Prospectus pro forma 3-column layout (source / adjustments / pro forma) per CSRS 4250 / ISAE 3420
- T5013 partnership slips and capital account rollforward for LP unitholders (D2 PCLP 1 only)

**NI 51-102 filing deadlines (D2/D3 as reporting issuers):**

| Document | Venture issuer | Non-venture |
|---|---|---|
| Annual financial statements + MD&A | 120 days | 90 days |
| Annual Information Form (Form 51-102F2) | Not required | 90 days |
| Interim financial statements + MD&A | 60 days | 45 days |
| Material change report | 10 days | 10 days |

**`ReportingTier` enum (Phase B-RI — Rust):**
```rust
pub enum ReportingTier {
    ReportingIssuerIFRS,   // D2, D3 — BCSC NI 51-102 + NI 52-107; SEDAR+
    PrivateIFRS,           // D4 (by LP agreement election)
    PrivateASPE,           // D1, D5, D6 default
    DevelopmentProject,    // D1 sub-projects pre-stabilisation
}
// Orthogonal overlay — does not collapse into ReportingTier:
pub enum AccountingOverlay { InvestmentEntityIFRS10, None }
```

**Cross-Tier Disclosure Manifest:** when a run includes both reporting-issuer entities (D2/D3)
and private entities that have an ownership relationship with the RI, the engine emits a
manifest listing triggered disclosure obligations: IAS 24 (related-party note), Form 51-102F1
Item 1.9 (MD&A related-party section), MI 61-101 (formal valuation if >25% materiality),
NI 55-104 (insider reporting at 10%+), NI 62-103 (early warning at 10%+, 2% top-ups).
No competitor produces this manifest — strong Reporting Issuer tier differentiator.

**Snapshot Layer E additions for RI tier:**
- `reporting_tier: ReportingTier`
- `accounting_overlay: Option<AccountingOverlay>`
- `comparative_period_required: bool`
- `interim_reporting: bool`
- `sedar_compatible: bool`
- `aspe_eligible: bool`
- `continuous_disclosure_obligations: Vec<CdoFlag>`
- `disclosure_grade`: promoted from `bool` to ordered enum `Draft | Internal | OM | RIQuarterly | RIAnnual | Audited`

**BCSA s.85 snapshot as compliance asset:** the T2/T3 snapshot (RFC 3161 timestamp + Rekor
anchor) constitutes contemporaneous evidence for BCSC continuous-disclosure reviews
(CSA Staff Notice 51-312). Strengthens the issuer's "reasonable investigation" defence
under BCSA Part 16.1 secondary-market civil liability. The T3 auditor co-sign + recomputation
attestation is equivalent to signing the working-paper cover sheet under CSRS 4250. Market
positioning: "BCSC-defence-ready financial statements."

**Jennifer decisions (resolved 2026-06-02):**
- **Flag P1** ✓ — Issuer status: **Venture issuer (TSXV/CSE)** initially for D2/D3; non-venture is post-graduation event. Filing cadence: annual statements + MD&A within 120 days; interim within 60 days; no AIF required at venture tier.
- **Flag P2** ✓ — T5013 / capital-account rollforward: **full CRA-compatible T5013 Partnership Information Return + per-partner T5013 slip + per-unit capital-account rollforward (ACB tracking, at-risk amount, withdrawals, allocations)**. Required for any LP (not optional under the Income Tax Act); schedule-only would force every operator to redo this in tax software.
- **Flag P3** ✓ — Cross-Tier Disclosure Manifest: **available at BOTH Reporting Issuer and Enterprise tiers** (overrides RI-only recommendation). Forward-looking-information governance valuable to private issuers planning future RI transition. Under uniform $19 FSL licensing the manifest is software-included; "tier" refers to documentation/onboarding scope, not pricing.
- **Flag P4** ✓ — Pricing: **$19.00 CAD one-time license under FSL-1.1-Apache-2.0** (replaces $299/$499 SaaS recommendation). Source-available perpetual license; converts to Apache 2.0 two years after each commit. Applies uniformly across ALL product tiers (D2/D3 direct-hold, Enterprise, Reporting Issuer); tier differentiation = documentation, onboarding, and services depth, NOT pricing. Recurring SaaS revenue replaced by one-time license + services revenue (audit-firm partnership, implementation, custom adapters, training). License file at archive root: `/srv/foundry/clones/project-proforma/LICENSE`.

### 3g. Straight-line geometry — base model philosophy

The proforma engine produces **straight-line geometric instruments** — not market forecasts.
Base model assumptions:

| Variable | Base value | Rationale |
|---|---|---|
| Rent escalation | 0% (no escalation) | Isolates development yield geometry |
| CPI / inflation | 0% | No cost inflation on opex |
| Cash interest | 0.5% (EY-calibrated) | Matches PCLP 1 EY FOFI Note 10(c); minor |
| Debt interest | Fixed at each entity's contracted rate | Not variable |
| Cap rate drift | 0% (constant cap rate) | Cap rate is a config parameter, not a forecast |
| Market value Y8+ | DPU / target_yield (formula) | Mechanically derived from distributions |

**Purpose:** Pure geometric analysis isolates the capital structure properties — development
yield, dilution mechanics, fee drag, debt coverage, distribution waterfall — independent of
macro assumptions. A proforma with 3% rent escalation baked in tells you about CPI, not
about the LP structure. Zero escalation tells you the structure's intrinsic geometry.

**Derivative analysis (Phase C):** Once the base model is validated, the Rust engine accepts
market-condition overlays as config parameters — CPI, rent-growth rates, interest-rate
scenarios — and computes the derivative impact. This is always additive to the base; the
base never changes. Three proposed overlay types:
- `cpi_overlay.toml` — inflation escalation on rent and opex
- `interest_rate_scenario.toml` — rate path (flat, rising, inverted)
- `market_cap_rate_path.toml` — cap rate compression/expansion over the hold period

Engine note: `cash_interest_rate = 0.0` is the pure-geometry variant; `cash_interest_rate
= 0.005` is the EY-calibrated baseline. Both are valid; the config drives the choice.
The line item and the Note always appear in output regardless of the rate value.

### 3h. OpexBudget — per-vehicle operating expense infrastructure

Every vehicle (D1–D7) carries its own `OpexBudget` config block. This is the primary input
for cash reserve sizing and for the detailed OpEx Summary section in every output.

**Three-section structure:**

**Section 1 — Origination / Formation (one-time, Y0 or pre-close):**
Costs to form the legal entity before operations begin: incorporation/registration filing fees,
LP/shareholder agreement drafting (if incremental to Y1 legal), initial securities compliance
filings, bank account setup. These are distinct from the Year 1 annual engagement costs.

**Section 2 — Annual operating expenses (Y1–Yn, per year):**
Each line item has a `y1_amount` and a `recurring_amount` (Y2+). The Y1 premium captures
first-year setup work (audit-engagement setup, initial board resolutions) embedded in the
engagement fee. Separation allows the Summary to show setup vs. steady-state costs.

**Section 3 — Reserve sizing (pre-income period only):**

Each vehicle has a `funding_onset_year` — the first year in which income from operations covers
ongoing opex. The reserve only needs to cover the pre-income gap (not the full forecast horizon).

```
required_reserve_at_close = origination_total + Σ(annual_opex, Y1..=funding_onset_year − 1)
```

After `funding_onset_year`, the engine switches from reserve drawdown to income-funded mode.

**Per-vehicle income onset (confirmed 2026-05-24):**

| Vehicle | `funding_onset_year` | Income source | Reserve covers |
|---|---|---|---|
| D2 PCLP 1 | Y4 | NOI from buildings (10.5% yield × generating assets) | Y1–Y3 |
| D4 Bencal SPV2 SPV-LP | Y4 | PCLP 1 DPU × units held (284,700) | Y1–Y3 |
| D5 Bencal SPV1 SPV-WCP | Y4 | Small WCP share sales (book value basis) | Y1–Y3 |
| D6 Bencal Management | Y4 | Bencal SPV2 distributions × 10% holding (33,333 units) | Y1–Y3 |
| D3 WCP | Y1 | AUM advisory fees + offering costs (Revenue Matrix) | None |
| D1 Dev Classes | Y4 (per class) | NOI by class after lease-up | Construction |
| D7 Legacy JV | TBD | JV NOI per class mix | TBD |

**Income-funded opex model (Y >= funding_onset_year):**
```
income_available[y]   = income_from_source[y]       // entity-specific (see below)
opex_covered[y]       = min(income_available[y], opex_annual[y])
opex_gap[y]           = max(opex_annual[y] − income_available[y], 0)
reserve_remaining[y]  = initial_cash_reserve − Σ(opex_covered_by_reserve, Y1..=y)
                      // note: if income_available > opex in a year, no reserve draw
```

Going-concern trigger: `reserve_remaining[y] < 0` for any year → emit Layer E warning.

**D5 Bencal SPV1 — partial WCP share sale model (Y4+):**
Bencal SPV1 sells a small block of WCP shares each year starting Y4 to fund opex. WCP pays no
dividends; share sales are the only cash source.

```
shares_to_sell[y]       = CEIL(opex_annual[y] / wcp_book_per_share[y])
disposal_proceeds[y]    = shares_to_sell[y] × wcp_book_per_share[y]   ≈ opex_annual[y]
wcp_shares_remaining[y] = wcp_shares_remaining[y−1] − shares_to_sell[y]
fvtpl_carrying[y]       = wcp_shares_remaining[y] × wcp_book_per_share[y]
```

IFRS 9 treatment: disposal proceeds = FVTPL carrying value (no incremental gain/loss; prior
unrealized gains already recognized in IS). CF statement: Investing: `+disposal_proceeds[y]`;
Operating: `−opex_annual[y]`. Net cash ≈ $0 for the opex-funded portion.

Estimated WCP shares sold (book method) at representative years:
```
Y4:  $59,544 / $28.81/share ≈ 2,067 shares
Y7:  $59,544 / $55.05/share ≈ 1,082 shares
Y10: $59,544 / $105.15/share ≈ 566 shares
Cumulative Y4–Y10: ~8,249 shares (2.75% of 300,000 total holding)
```

WCP ownership at Y10 (after 7 years of small sales): ~291,751 shares (2.918% of WCP).
Disclose in offering: "WCP shares may be partially liquidated to fund ongoing operating
expenses; ownership percentage decreases gradually from 3.0% to approximately 2.9% over
the forecast horizon (pre-Capital Return Sale)."

**Rust struct:**
```rust
pub struct OpexBudget {
    pub origination: Vec<OpexLineItem>,   // one-time formation items
    pub annual: Vec<AnnualOpexItem>,      // recurring items with optional Y1 premium
    pub reserve_years: u32,              // years of opex to fund at close (default: 10)
}

pub struct OpexLineItem {
    pub label: String,
    pub amount: f64,
    pub asm_id: String,                  // Layer B provenance ID
}

pub struct AnnualOpexItem {
    pub label: String,
    pub y1_amount: f64,                  // Year 1 (may include setup premium)
    pub recurring_amount: f64,           // Year 2+ steady-state rate
    pub asm_id: String,
}

impl OpexBudget {
    pub fn required_reserve(&self) -> f64 {
        let orig: f64 = self.origination.iter().map(|i| i.amount).sum();
        let yr1: f64 = self.annual.iter().map(|i| i.y1_amount).sum();
        let rec: f64 = self.annual.iter().map(|i| i.recurring_amount).sum();
        let recurring_years = self.reserve_years.saturating_sub(1) as f64;
        orig + yr1 + rec * recurring_years
    }
}
```

**Summary output (Format A — all vehicles):**
The OpEx Budget appears as a dedicated table BEFORE the IS in every Format A (operational)
output — even when the formal IFRS/ASPE Income Statement condenses the same costs to 3–5
lines (e.g., "General and Administrative"). The summary table shows:

| Row | Content |
|---|---|
| Origination block | Each formation line item, column = Y0 or "at close" |
| Annual detail | Each opex line item, Y1–Y10 column per year |
| Subtotal: Annual opex | Sum of annual items per year |
| Cumulative opex | Running total from Y1 |
| Reserve remaining | `initial_cash_reserve − cumulative_opex[y]` |
| Reserve exhaustion | First year where reserve < 0 (going-concern trigger) |

Engine emits `going_concern_warning: true` in Layer E when `reserve_remaining[y] < 0`
for any year in the forecast horizon.

**Offering size relationship:**
When `reserve_mechanism = at_close`, the total equity offering = `net_investment +
required_reserve`. Investor shares = `total_offering / share_price`. Manager shares =
`ROUND(investor_shares / 9, 0)`. Net investment target is held constant; the reserve is
additive. Disclose reserve in offering documents.

**Reserve mechanism by vehicle (income onset model, 2026-05-24):**

The reserve relationship to capital structure differs by vehicle type:

| Vehicle | Mechanism | Reserve effect on unit / share count |
|---|---|---|
| Bencal SPV2 SPV-LP (§5d) | Additive to net PCLP1 target ($25M fixed) | `total_equity = $25M + reserve`; investor LP units increase |
| Bencal SPV1 SPV-WCP (§5e) | Additive to net WCP investment ($3M fixed) | `total_offering = $3M + reserve`; investor Inc. shares increase |
| PCLP 1 D2 (§5b) | Deductive from fixed $250M gross | `investable_capital = $250M − reserve`; unit count LPA-locked (2,777,777) |
| Bencal Management D6 (§5f) | Commission-funded (close commissions fund Y1–Y3 reserve; share capital nominal $5/share) | reserve = $41,713; commission_retention = $41,713; share_price = $5.00 (nominal) |
| WCP D3 (§5c) | No reserve — AUM advisory fees cover opex from Y1 | — |

For Bencal SPV2 and Bencal SPV1: `net_investment` is the contractual target amount to deploy into the
underlying asset. Investors subscribe into a gross offering = net_investment + opex_reserve.
The reserve is held in restricted cash; drawn down for pre-income-onset opex (Y1–Y3).

For PCLP 1: gross equity = $250M (fixed by fund mandate and LPA). OpexBudget reserve is
deducted from the gross to determine the building construction budget. More reserve → fewer
buildings. Unit count stays LPA-locked (2,777,777 authorized including Benetti 277,777).

For Bencal Management: no investor pool. The SPV-Manager earns commission at close; that commission funds all
three reserves (Bencal SPV1, Bencal SPV2, Bencal Management). Bencal Management's own reserve ($41,713) is the retained portion of the
commission after the dealer work fee + tax + reserve injections are allocated. Share capital =
$5.00/share × 2 shares = $10 nominal. See §5f Flag 6 commission waterfall.

**Journal entry note (future design flag — §3h-JE):**
When the engine adopts a journal-entry internal model (future flag), each `OpexLineItem`
and `AnnualOpexItem` maps to a GL account code and a recurring JE template. Real supplier
invoices — legal engagement letters, accounting fees, filing receipts — become the source
for `amount` fields, producing auditable computations from source documents rather than
estimates. The OpexBudget struct is designed to accommodate this without refactoring:
`asm_id` links to Layer B, which can reference invoice SHA-256 as `source_doc_sha256`.
Reserve for journal-entry architecture flag.

---

## 4. Entity hierarchy (Woodfine deployment)

```
Layer 3: Woodfine Capital Projects Inc. (WCP) [D3] — REPORTING ISSUER / IFRS
           parent holdco, 42M dev budget
         PCLP 1 LP [D2] — REPORTING ISSUER / IFRS
           250M fund; IFRS 10.27 investment entity exception; carries WCP at FVTPL (Level 3)
         SPV entities [D4/D5/D6] — PRIVATE / ASPE default
           Bencal SPV2-LP | Bencal Special Purpose 1 Inc. | Bencal Management Corp.
              │
Layer 2: Woodfine Direct-Hold Solutions — operating holdco; directly holds properties
              │
Layer 1: Development Classes (4 types) [D1] — PRIVATE / ASPE default
         Professional Centres | Suburban Office | Tech Industrial | Retail Select

D7 Legacy JV [D7] — PRIVATE / ASPE default — COMPARISON ENTITY
         Traditional J/V financing: bank debt first, equity last
         2,298,150 sf portfolio; $250M equity; $750M debt (3.0x); single development round
         Illustrative comparator to D2 — apples-to-apples 10-year return analysis
```

**Roll-up rule:** full consolidation for entities >50% owned (WCP → Dev Classes → SPVs).
PCLP 1 is an investment entity — does NOT consolidate WCP; carries at fair value.
Intercompany eliminations required: IC loans, management fees, dividends, unrealised gains
on asset transfers, NCI (minority interest disclosed separately).

**Scope for this session:** Layer 1 (D1) first, then D2, then D3. SPV derivations
(Ambassadors, Bencal Management) are built on top of D3 — not in scope until D3 is solid.

---

## 5. Financial model specification

### 5a. D1 — Development Classes

**Four classes (unchanged from original spec):**
| Class | Floor plate | Notes |
|---|---|---|
| Professional Centres | 21,000 sqft/floor, 3 floors | Retail ground floor (10,600 sqft) |
| Suburban Office | 19,000 sqft/floor, 3–5 floors | Underground parking option |
| Tech Industrial | 7,200 or 8,400 sqft, single-storey | Always built in pairs |
| Retail Select | 4,500 / 6,700 / 7,700 sqft, single-storey | 3 size variants |

**10-year period structure:**
- Y1: Construction (capitalised; no revenue; interest reserve)
- Y2–Y3: Lease-up ramp (occupancy schedule: 0% / 25% / 75%)
- Y4–Y10: Stabilised (95% occupancy; market rent; rollover reserve)

**Income Statement — required line items (matches Excel CAM + Proforma tabs):**
```
REVENUE
  Rental area summary (per component: Underground / Retail / Office Floor N)
    Units | Sq.ft. | Rate/sqft | Unit amount | Rent at lease start | Rent at sale
  Totals
  Investment valuations
    Office: non-recovery cost (5.5% rate) | Capitalized rent (cap rate 6.25%)
    Retail: non-recovery cost (5.5% rate) | Capitalized rent (cap rate 6.25%)
    Totals
  Ancillary income (pylon signs: 4 panels × $250/month = $12,000/yr)
  Gross rental income
  Recoveries
    Office: 5.5% non-recovery cost applied to gross rent
    Retail: 5.5% non-recovery cost applied to gross rent
    Totals
  TOTAL PROJECTED REVENUE (net of non-recoveries)

OPERATING EXPENSES (CAM budget — itemized, not a flat ratio)
  In-house property manager   (flat $75,000/yr)
  Common area maintenance     ($5.50/sqft leasable)
  HVAC                        ($1.50/sqft leasable)
  Insurance                   ($0.75/sqft leasable)
  Local accountant            (flat $15,000/yr)
  Local lawyer                (flat $15,000/yr)
  Property taxes              ($5.00/sqft leasable)
  Management fee              (3% of gross rental income)
  Vacancy & bad debt          (3% of gross rental income)
  Structural maintenance      (1% of gross rental income)
  Total expenses

NET OPERATING INCOME (Cash)
  → also emit NOI (GAAP) separately when straight-line adjustments added

INTEREST (debt service on construction/term debt)
EBT
```

**Balance Sheet:**
```
  Investment property at fair value (NOI / cap rate at stabilisation)
  Property under development (WIP during construction — IAS 40.20-23)
  Restricted cash (reserves)
  Ending cash
  Total assets
  Construction loan / term debt (LTC 65% on cost)
  Equity (total assets − debt)
```

**Cash Flow:**
```
  CF from operations (NOI − interest)
  Capex / construction draw (Y1: total dev cost; IAS 40 capitalised)
  Debt drawdown (Y1: LTC × total dev cost)
  Distributions (90% of EBT if positive)
  Ending cash
  → also emit: Unlevered CF and Levered CF separately
```

**Development cost structure (matches Excel Proforma tab numbered rows):**
```
ACQUISITION COSTS
  Land costs ($3.47/sqft × acres)

CONSTRUCTION COSTS
  Per floor (Underground / Retail / Office Floor 1–6)
    Sq.ft. | Rate/sqft | Cost
  Total construction
  Contingency (5% of hard costs)

OTHER CONSTRUCTION
  Off-site costs ($1/sqft GFA)
  On-site costs ($1/sqft GFA)

TI COSTS
  Per area type (rate/sqft leasable)

INDIRECT COSTS (professional fees, marketing/leasing, etc.)

TOTAL DEVELOPMENT COST
TOTAL COST PER SQ.FT.
EQUITY REQUIRED
```

**Report summary (matches Excel _Report tab):**
```
Procurement stage:  Land ($/sqft | % of total) | Off-site
Development stage:  Construction | Contingency | On-site | Indirect costs
Management stage:   Inducements | Leasing expense | Vacant space
Total project costs ($/sqft | % of total = 1.000)
Footnotes: off-site definition | on-site definition
```

**Key metrics (header row of summary):**
Leasable Area | GFA | Total Dev Cost | Stabilised NOI | Development Yield | Cap Rate |
Stabilised Asset Value | DSCR (at LTC 65%, 5% rate, 25-year amort)

**D1 as portfolio building blocks:**
D1 class outputs are the primary inputs for all portfolio-holding vehicles (D2, D7). Each
vehicle aggregates per-building D1 outputs — it does not re-derive building economics. The
four-stage pipeline: `compute_dev_class → aggregate_portfolio → apply_capital_stack →
apply_vehicle_adjustments`. D1 must emit `DevClassOutput { buildings: Vec<BuildingOutput> }`
(Phase A-PA gate). Each `BuildingOutput` carries a ULID-stable identity, `ConstructionPhase`,
area, revenue, cost, and NOI. Per-vehicle class mixes differ (D2: 40/30/20/10 configurable;
D7: via `JvConfig`) — each vehicle specifies its own `ClassMix` struct. Array-index references
are prohibited; ULID references are mandatory (index reshuffles invalidate snapshot chains).

### 5b. D2 — Direct-Hold Solutions (Professional Centres Canada LP — canonical legal name; formerly referenced as PCLP 1 / Woodfine Professional Centres LP)

> **Canonical legal name (locked 2026-06-02, v0.15.8): "Professional Centres Canada LP".** This name appears
> in offering documents, LP Agreement, CIM, and all investor-facing artefacts. The shorthand "PCLP 1" is
> retained as the internal display label and the engine config / struct identifier (`Pclp1Config`,
> `pclp1_units`, etc.) for code stability. Anywhere this brief or downstream documents render a
> human-readable LP name, use **Professional Centres Canada LP** (or the formal abbreviation
> **"PC Canada LP"**); reserve "PCLP 1" for engine/code contexts only.


**Portfolio derivation:** D2 derives its IS/BS/CF by aggregating `Vec<BuildingOutput>` from
D1 class computations, applying the D2 class mix (40% Professional / 30% Suburban Office /
20% Tech Industrial / 10% Retail Select), then applying vehicle-level adjustments (LP fee
cascade, offering costs, G&A, debt service per §5b capital structure). The `PortfolioVehicle`
trait governs this interface.

**Source:** `DUE DILIGENCE_PCLP 1_2026_01_06_Forecast_250M_Cash Flow and Valuation_FIN.xlsx`,
sheet `PCLP 1_250M`. Analysed by 3-agent opus panel 2026-05-23.

**Output required:**
1. Full financial statements (IS/BS/CF) — same 10-year landscape format as D1
2. The AA12:AM35 Financial Forecast Summary — exact match; this is the circulated investor table

#### Input parameters (all configurable in `Pclp1Config`)

| Parameter | Value | Cell |
|---|---|---|
| Total equity | $250,000,000 | D15 |
| Unit price | $100 | D16 |
| Diluted units | 2,777,777 | D45 |
| Issuing agents fee | 6% | D17 |
| Issue costs | 1% | D27 |
| Advisory fee (annual % of equity) | 1% | D19 |
| Admin & compliance (annual flat) | $500,000 | D24 |
| Board of directors (annual flat) | $450,000 | D23 |
| Development yield | 10.5% | D10 |
| Cap rate (Public Non-Listed) | 6.25% | D12 |
| Target distribution yield | 8.0% | AC23 |
| Debt rate (debentures) | 5.0% | D29 |
| Debt financing cost | 3.0% | D28 |
| Cash interest rate | 0.5% | D30 |
| Debt buyback rate (of FFO) | 10% | D31 |
| Minimum cash balance | $250,000 | D33 |
| Working capital reserve | 6.25% | D34 |
| Benetti Holdings dilution | 10% | D21 |
| Income continuity Y1/Y2/Y3 | $3.05M / $3.3M / $3.5M | hardcoded |
| Market value Y1-Y7 | $100/$100/$100/$125.8/$132.1/$171.5/$177.3 | hardcoded input |

**Benetti Holdings dilution mechanics (LPA-locked — EY FOFI Notes 2, 3c, 5):**

```
investor_units  = 2,500,000
benetti_units   = 277,777   (11.111% of investor units; fixed per sixth amended LPA, Dec 1 2016)
diluted_total   = 2,777,777 (Note 5: "authorized capital consists of up to 2,777,777 units")
```

Issuance price: $0.001/unit. IFRS 2 fair value = $100/unit. Y1 IFRS 2 expense:
277,777 units × $100 = **$27,777,700** (EY Note 3c; recognized straight-line from initial LPA
date to estimated closing of the Offering — modelled as Y1 bullet in forecast).

**These numbers are LPA-locked** per the sixth amended and restated Partnership agreement and
confirmed in the EY 10-year financial forecast. Do NOT derive via `ROUND(n/9, 0)` — hardcode
277,777 / 2,777,777 as constants in `Pclp1Config`. New entities (Bencal SPV2-LP, Bencal SPV1 Inc.) use the
corrected formula `ROUND(investor_units / 9, 0)` for manager units.

**Benetti escrow / transfer restriction (EY Note 2 — verbatim from LPA):**
> Any units issued to Benetti will be held in escrow, and Benetti will not obtain unrestricted
> ownership of the units and will not have the right to transfer its units to a third-party
> purchaser until the earlier of: (i) the limited partners having received distributions from
> the Partnership equal to 100% of their initial investment in units, (ii) the closing of a
> final sale of all or substantially all of the assets of the Partnership, or (iii) the sale
> of all of Benetti's units to the owner or owners of at least 75% of the outstanding units.

Benetti participates pari passu in all distributions during the escrow period (EY Note 2:
"The units held by Benetti will receive the same per-unit distributions as all units issued
to the investors under the Offering"). There is **no distribution hurdle**; the restriction
applies to transfer / unrestricted ownership only.

This two-part structure — pari passu distributions + escrow transfer restriction — is the
canonical PCLP 1 no-hurdle disclosure. The Bencal SPV2-LP agreement should mirror this (§5d Flag 3).

**Cash interest rate (resolved):** EY Note 10(c) = 0.50%; Excel D30 = 0.05%. Use **0.5%**
(EY-calibrated baseline per §3g). Base model keeps the line item and note at 0.5%; pure-geometry
override uses 0.0%. The Excel D30 value was a data-entry error (10× off).

**Distribution payout ratios:** Y1–Y3 = 0%, Y4–Y7 = 90%, Y8+ = 100%

**Phase schedule:**

| Phase | Build years | Capex total | Annual draw | Start generating rent |
|---|---|---|---|---|
| Phase 1 | Y1–Y3 (3 yrs) | $216,875,000 | $72,291,667/yr | Y4 |
| Phase 2 | Y4–Y5 (2 yrs) | $339,500,000 | $169,750,000/yr | Y6 |
| Phase 3 | Y6–Y7 (2 yrs) | $654,750,000 | $327,375,000/yr (see note) | Y8 |

Phase 3 Y7 debt draw is a **minimum-cash solver**: `debt_draw = phase3_capex_Y7 - prior_cash + min_cash`.
Do NOT replicate the Excel's literal reconciliation constants (`-816 + 80201 - 12030 + 443`).

#### Equity structure (computed once at inception)
```
gross_equity        = $250,000,000   ← fixed by fund mandate (LPA); does not change
issuing_agents_fee  = gross_equity × 0.06  = $15,000,000
issue_costs         = gross_equity × 0.01  = $2,500,000
net_proceeds        = gross_equity - issuing_agents_fee - issue_costs  = $232,500,000
working_capital     = gross_equity × 0.0625 = $15,625,000   ← operating reserve (deductive)
net_equity_funding  = net_proceeds - working_capital = $216,875,000   ← investable for buildings
```

**Reserve mechanism (income onset model, 2026-05-24):** The $250M gross is fixed. The
`working_capital` deduction ($15,625,000 = 6.25% of gross) is the operating reserve; it
reduces the building construction budget to $216,875,000. The $15.625M reserve covers Y1–Y3
management expenses (advisory fee at 1% × $232.5M = $2.325M/yr and other admin) before
building NOI flows from Y4. A future PCLP 1 OpexBudget should validate that 6.25% is adequate
for the Y1–Y3 pre-income gap. Unlike Bencal SPV2/Bencal SPV1, the reserve is **deducted from gross** — the
LP unit count (2,777,777 LPA-locked) does not change; the building portfolio is smaller.

#### Per-year computation order (dependency-safe)

**Step 1 — Capital asset schedule**
```
Phase1_draw[y]     = 72,291,667 if y ∈ {1,2,3} else 0
Phase2_draw[y]     = 169,750,000 if y ∈ {4,5} else 0
Phase3_draw[y]     = 327,375,000 if y=6; solver_result if y=7; else 0
Total_Assets[y]    = sum of all draws 1..y
WIP[y]             = assets not yet generating rent
Generating[y]      = Total_Assets[y] - WIP[y]
```

**Step 2 — Revenue**
```
Net_Proceeds_from_Ops[y]  = Generating[y] × dev_yield   (= 0 for Y1-Y3)
Income_Continuity[y]      = hardcoded Y1-Y3; = Net_Proceeds_from_Ops[y] for Y4+
```
⚠ EBITDA uses `Net_Proceeds_from_Ops` (= 0 for Y1–Y3).
Asset valuation uses `Income_Continuity` (non-zero for Y1–Y3).
These are separate fields — do NOT merge. BCSC note: Y1-Y3 income continuity
is a fair-value/entitlement uplift assumption, not distributable income; emit
as `revenue_construction_phase` with provenance metadata.

**Step 3 — Expenses**
```
Issue_Costs[y]       = 2,500,000 if y=1 else 0
Financing_Costs[y]   = gross_debt_drawn[y] × 0.03
Advisory_Fee[y]      = 2,500,000                           (= equity × 1%)
Admin_Compliance[y]  = 500,000
Board[y]             = 450,000
Total_Expenses[y]    = sum of above
```

**Step 4 — EBITDA**
```
EBITDA[y] = Net_Proceeds_from_Ops[y] - Total_Expenses[y]
```

**Step 5 — Debt schedule (requires solver for Y7)**
```
Opening_Debt[y]     = Closing_Debt[y-1]
Gross_Debt_Draw[y]  = Phase2_gross if y∈{4,5}; Phase3_gross/2 if y=6; solver if y=7; else 0
Net_Interest[y]     = (avg_debt[y] × 0.05) - (avg_cash[y] × 0.005)
  where avg = (opening + closing) / 2  [requires iterative solve or two-pass]
FFO[y]              = EBITDA[y] - Net_Interest[y]
Repayment[y]        = FFO[y] × 0.10 if y≥8 else 0
Closing_Debt[y]     = Opening_Debt[y] + Gross_Debt_Draw[y] - Repayment[y]
```
⚠ `D31` parameter is labelled "% of EBITDA" but the formula uses FFO (row 72).
FFO is the authoritative source. Do not use EBITDA for repayment.

**Step 6 — Cash flow**
```
Opening_Cash[y]     = Ending_Cash[y-1];  Opening_Cash[1] = 0
New_Equity[y]       = 250,000,000 if y=1 else 0
Distributions[y]    = (FFO[y] × payout_ratio[y]) - Repayment[y]
                      where payout: Y1-Y3=0, Y4-Y7=0.90, Y8+=1.0
Ending_Cash[y]      = Opening_Cash + New_Equity + Gross_Debt_Draw
                      - Capex[y] - Repayment[y] - Distributions[y] + FFO[y]
```

**Step 7 — Y7 debt solver (min-cash)**
```
target_ending_cash = min_cash ($250,000)
Gross_Debt_Draw_Y7 = Phase3_capex_Y7 - Opening_Cash_Y7 - FFO_Y7 × payout_Y7 + min_cash
```
Note: FFO_Y7 depends on avg_debt which depends on Gross_Debt_Draw_Y7 — requires
one iterative pass or algebraic solve. A reasonable approximation: solve assuming
interest on Y7 debt applies to half the draw (average-balance method).

**Step 8 — Asset valuation**
```
Asset_Value[y]      = (Income_Continuity[y] / cap_rate) + WIP[y] + Ending_Cash[y]
Asset_Value_per_unit = Asset_Value[y] / diluted_units
```

**Step 9 — NAV**
```
NAV[y]             = Asset_Value[y] - Closing_Debt[y]
NAV_per_unit       = NAV[y] / diluted_units
```

**Step 10 — Per-unit metrics**
```
DPU[y]             = Distributions[y] / diluted_units
Dist_yield_on_cost = DPU[y] / unit_price ($100)

Market_Value_per_unit[y]:
  Y1-Y7: hardcoded input table (config)
  Y8+:   DPU[y] / target_distribution_yield (0.08)

Discount_Premium[y] = (Market_Value[y] - NAV_per_unit[y]) / NAV_per_unit[y]

CAGR_excl_dist[y]  = (Market_Value[y] / unit_price)^(1/y) - 1

Dist_yield_at_mkt[y] = DPU[y] / Market_Value[y]   (= 8% by design for Y8+)
```

**Step 11 — Ratios**
```
ICR[y]             = EBITDA[y] / abs(Net_Interest[y])   (n/a Y1-Y3)
Debt_vs_Dev_Cost[y]= Closing_Debt[y] / Total_Assets[y]
Debt_to_AV[y]      = Closing_Debt[y] / Asset_Value[y]
TER[y]             = (Advisory_Fee + Admin + Board) / NAV[y]
  (TER excludes Issue Costs, Financing Costs, and Interest — operating expenses only)
Total_sqft[y]      = portfolio SF corresponding to Generating[y] assets
```

#### AA12:AM35 Financial Forecast Summary (investor-facing, circulated)

This block is the required D2 summary output. Reproduce exactly — left column is label,
remaining columns are Y1–Y9 (or Y1–Y10 for full version; AA12:AN35 for Y10).

| Row | Label | Formula |
|---|---|---|
| 13 | Revenue | Net_Proceeds_from_Ops / 1M (millions) |
| 14 | Distributions | DPU per unit |
| 16 | Distribution Yield on Initial Capital | DPU / $100 |
| 18 | Asset Value | Asset_Value_per_unit |
| 19 | Total Debt | Closing_Debt / diluted_units |
| 21 | Net Asset Value (NAV) | NAV_per_unit |
| 23 | Market Value | Market_Value_per_unit |
| 24 | (Discount/Premium to NAV) | (MV - NAV) / NAV |
| 25 | Discount / Premium vs. NAV | (label for row 24 values) |
| 27 | Compounded Annual Return (Excl. Dist.) | CAGR on Market Value from $100 |
| 28 | Distribution Yield to Buyers at Market Value | DPU / MV |
| 30 | Interest Coverage Ratio | ICR |
| 31 | Debt vs. Development Cost | Closing_Debt / Total_Assets |
| 32 | Debt to Asset Value | Closing_Debt / Asset_Value |
| 34 | Total Expense Ratio (NAV) | TER (operating only) |
| 35 | Total Square Foot | portfolio_sqft |

Y1-Y3 entries for Revenue, Distributions, ICR, Debt vs Dev Cost, Debt/AV show `"-"` (dash).

**Blank patterns confirmed by cell audit (§14j):**
- Row 22, 26: empty spacers — render as blank rows
- Row 24 (Discount): AE24–AG24 blank (Y1–Y3); AH24+ populated
- Row 27 (CAGR): only AL27 (Y8) populated — single exit scalar; AM27, AN27 blank
- Row 28 (Buyer Yield): AE28–AK28 blank (Y1–Y7); AL28+ populated

**Column extent: AA12:AN35 (Y1–Y10 full).** The circulated version is AA12:AM35 (Y9 cutoff)
but the engine output runs the full 10 years. Confirmed 2026-05-23.

#### Consolidated statement title and mandatory notes

> **Woodfine Professional Centres Limited Partnership**
> Notes to the 10-Year Financial Forecast
> **(Expressed in Canadian Dollars)**

Note 1 text (face of statement — F field, mandatory — matches EY FOFI format):
> Note 1: Share-based compensation — Benetti Holdings Inc. holds 277,777 units (9.9997% of
> 2,777,777 total units; issuance price $0.001/unit; IFRS 2 fair value $100/unit). Benetti
> units participate pari passu in all distributions from first closing. Benetti units are held
> in escrow and may not be transferred until: (i) limited partners have received distributions
> equal to 100% of their initial investment in units; (ii) closing of a final sale of all or
> substantially all of the Partnership's assets; or (iii) sale of all Benetti units to the
> holder(s) of at least 75% of the outstanding units. Per-unit metrics on fully diluted basis
> (2,777,777) unless otherwise noted.

Remaining EY notes 2–10 (abbreviated; see §25 for full text source):
- Note 2: Nature of operations (four WOODFINE BUILDING types, NI 51-102 continuous disclosure)
- Note 3: Accounting policies (a) basis of presentation; (b) cash at FV/amortized cost;
  (c) share-based payments — straight-line from LPA date to closing; (d) investment properties
  **at cost** (EY FOFI uses cost model; depreciation/impairment not considered)
- Note 4: Long-term debt (Series A–D, 5% interest, 3% financing fee)
- Note 5: Partners' units — authorized up to 2,777,777 units
- Note 6: Revenue — 10.5% development yield, four building classes
- Note 7: Operating costs — 1% advisory, 6% referral, 1% issue costs
- Note 8: Construction cost — $310/sqft average
- Note 9: Distributions — minimum 90% of distributable income until investors receive 100%
  of gross proceeds; thereafter 10% applied to redeeming first secured mortgage debentures
- Note 10: Other key assumptions — (b) interest at 5% avg debt; (c) cash income at 0.50%
  opening cash balance

#### Accounting flags (BCSC / IFRS)

1. **Issue costs and financing costs through P&L** — Excel routes these as operating expenses,
   depressing EBITDA. Under IAS 32.37 they should net against equity/debt proceeds. The engine
   replicates the Excel cash-view presentation for now; flag as IFRS divergence in output metadata.

2. **Income Continuity Y1-Y3** — fair-value/entitlement uplift, NOT distributable income.
   Emit as `revenue_construction_phase` (separate field). Do not include in distributable FFO.
   Disclosure language required for BCSC NI 41-101 / 51-102 use.

3. **D31 label error** — parameter says "% of EBITDA" but formula uses FFO. Engine uses FFO.
   Document in config comments.

4. **Investment properties — cost model (EY Note 3d):** The EY FOFI carries investment
   properties at cost (not IAS 40 fair value). The BRIEF §5b formula chain uses cap-rate
   asset valuation for the investor summary block. Both are retained: cost model for IFRS
   formal statements; cap-rate NAV for the AA12:AN35 investor summary. Emit as separate
   fields with `accounting_basis: cost | nav_estimate` metadata.

#### Open items for D2

- [ ] Number audit: engine output vs. Excel cell-by-cell for all 10 years
- [ ] Iterative solver for Y7 debt draw (or algebraic formulation)
- [ ] Two-pass average-balance interest calc (opening + closing avg requires closing to be known)
- [ ] IFRS flag metadata on income continuity lines
- [ ] Confirm AM35 vs AN35 column extent for circulated version (Jennifer: AA12:AM35 = Y9 cutoff)

### 5c. D3 — WCP Inc. (Woodfine Capital Projects Inc.)

**Source Excel:** `COMPLIANCE_WCP_2026_01_08_Forecast_42M_Cash Flow and Valuation_FIN.xlsx`
**Tabs:** `WCP_42M` (main model), `INPUT_PCLP 1_250M` (PCLP1 data mirror)

#### Parameters (fixed config — rows 9-13, F column)

| Parameter | Value | Config key |
|---|---|---|
| Shares outstanding | 10,000,000 | `shares_outstanding` |
| Price per share (CAD) | $20.00 | `price_per_share` |
| CAD-USD rate | 1.3372 | `cad_usd` |
| CAD-EUR rate | 1.4657 | `cad_eur` |
| Total financing raised | $42,000,000 | derived: Y1=$20M + Y2=$22M |
| Tax rate | 27% | `tax_rate` |
| P/E multiple (market val) | 10.72× | `pe_multiple` |
| Dividend yield (div val) | 4.5% | `assumed_dividend_yield` |

#### WCP cap table (Y0 post-Bencal SPV2 founding-bonus allocation; v0.15.8)

Updated 2026-06-02 to reflect the carve-out of 600,000 WCP shares from the Strategic Partner block to Bencal SPV2 in
exchange for Bencal SPV2 completing its minimum CAD 13M investment in Professional Centres Canada LP. Total outstanding
unchanged at 10,000,000; no new shares issued.

| Holder | Shares | % of outstanding | Cost basis (Bencal entities only) | Notes |
|---|---|---|---|---|
| **Bencal Special Purpose 1 Inc.** | 300,000 | 3.0% | $3,000,049.83 ($20.00 × 150K purchased + $0.00033223 × 150K founding-bonus) | per §5e |
| **Bencal Special Purpose 2 Inc. / Bencal SPV2-LP** | **600,000** | **6.0%** | **$199.34** ($0.00033223 × 600K nominal; carve-out from Strategic Partner block; transferred to Bencal SPV2 for completing minimum CAD 13M LP investment) | NEW 2026-06-02; per §5d |
| Strategic Partner block | 1,200,000 | 12.0% | n/a (pre-existing holder) | Reduced from 1,800,000 (18.0%); 600,000 transferred to Bencal SPV2 |
| Other holders (founders, WMC, employees) | 7,900,000 | 79.0% | n/a | Unchanged |
| **Total outstanding** | **10,000,000** | **100.0%** | | |

**Bencal Group aggregate WCP exposure (Y0):** 900,000 shares = 9.0% of outstanding (Bencal SPV1 3.0% + Bencal SPV2 6.0%).

**Carve-out mechanics — Strategic Partner → Bencal SPV2 transfer:**
- Transfer is a secondary sale from the Strategic Partner to Bencal SPV2 at $0.00033223/share nominal consideration ($199.34 total).
- Substance: bonus issuance contingent on Bencal SPV2 completing its minimum CAD 13M LP investment threshold (Bencal SPV2's CAD 25,059,097 subscription exceeds threshold; bonus earned at close).
- Section 69 of the Income Tax Act (Canada) — non-arm's-length transfer rules apply: transfer is deemed to occur at FMV.
  - For Strategic Partner: deemed disposition at FMV; capital gain/(loss) recognised at Strategic Partner level on the difference between FMV and Strategic Partner's cost basis.
  - For Bencal SPV2: cost basis = FMV (not $199.34 consideration). Initial recognition at FVTPL Level 3 proxy (~$4.55/share × 600,000 = ~$2,730,000 at formation).
- Y0 Bencal SPV2 income recognition: the difference between deemed-cost-basis (FMV) and cash paid ($199.34) is recognised as a Y0 bonus-share contribution; treatment as either (a) IFRS 2 share-based payment received for service (subscribing to the LP), or (b) capital contribution from Strategic Partner with offsetting income line, is OPEN — see new Flag in §5d / NEXT.md.
- Document chain: requires (i) a Strategic Partner Donation/Transfer Agreement with WCP, Strategic Partner, and Bencal SPV2 as parties; (ii) consent of WCP under shareholders' agreement (if drag-along / right-of-first-refusal applies); (iii) updated WCP share register at close.


#### Year labelling correction (Jennifer Woodfine, 2026-05-23)

The Excel mislabels two columns "Y0". **Corrected mapping:**

| Excel col | Excel label | Corrected year | Notes |
|---|---|---|---|
| G | Y0 | **Y1** | First capital tranche ($20M) |
| H | Y0 | **Y2** | Second capital tranche ($22M) |
| I | Y1 | **Y3** | |
| J | Y2 | **Y4** | |
| K | Y3 | **Y5** | |
| L | Y4 | **Y6** | |
| M | Y5 | **Y7** | |
| N | Y6 | **Y8** | |
| O | Y7 | **Y9** | |
| P | Y8 | **Y10** | |
| Q | Y9 | (Y11 — outside 10-yr scope) | |
| R | Y10 | (Y12 — outside 10-yr scope) | |

**10-year display scope: columns G:P (corrected Y1–Y10 only).**

#### Revenue Generator C17:R53 — "Revenue and Assets from the Woodfine LPs"

Six LP funds, each contributing three rows: Advisory Fee, Distributions, Net Asset Value.
WCP holds 10% beneficial ownership in each LP; LP1 (WPC Canada) is the seed fund from which all others are derived.

| LP | Fund name | GFV | Currency | Launch | Size vs LP1 | Advisory FX | Dist FX | NAV FX |
|---|---|---|---|---|---|---|---|---|
| LP1 | WPC Canada | C$250M | CAD | Y1 | 1× | ×1 | ×1 | ×1 |
| LP2 | WPC US | US$500M | USD | Y2 | 2× | ×CAD-USD | ×CAD-USD | ×CAD-USD |
| LP3 | WPC Spain | EUR$250M | EUR | Y2 | 1× | ×CAD-EUR | ×CAD-EUR | ×CAD-EUR |
| LP4 | WPC Mexico | US$250M | USD | Y3 | 1× | ×CAD-USD | ×CAD-USD | ×CAD-USD |
| LP5 | WPC Vertical Warehouse | US$250M | USD | Y4 | 1× | ×CAD-USD | ×CAD-EUR ⚠ | ×CAD-USD |
| LP6 | WPC Parking Structures | US$250M | USD | Y5 | 1× | ×CAD-USD | ×CAD-EUR ⚠ | ×CAD-USD |

⚠ LP5 and LP6 distribution rows use the EUR rate (F12) despite being USD funds — confirmed Excel anomaly. Engine replicates exactly and emits `metadata.distribution_fx_anomalies: ["LP5","LP6"]` in JSON output.

**LP1 Advisory Fee source:** `PCLP1.advisory_fees[y]` from INPUT_PCLP1 row 63 with deployment ramp:
- Y1: × 1/3 (partial capital call)
- Y2: × 2/3
- Y3+: × 3/3 (fully deployed)

**LP2–LP6 lag:** each LP sources values from LP1 shifted back by its launch lag:
`LP_n[y] = LP1[y − lag_n] × size_factor × fx_rate`

**Offering Costs Reimbursement (row 53):** WCP fronts LP launch costs; LPs reimburse from advisory fees as they ramp. Closed form (first-difference analytical — no rolling accumulator):

```
offering_costs[Y1] = advisory_fee_total[Y1]
offering_costs[y]  = advisory_fee_total[y] − advisory_fee_total[y−1]   for Y2–Y6
offering_costs[y]  = 0                                                  for Y7+
```
Total recovery cap: ~$21.78M (sum G53:L53). Cuts to zero from Y7 (col M) onward.

#### WCP Income Statement (rows 55-69)

```
Gross_Income[y]     = Σ(advisory_fees[y] + distributions[y]) + offering_costs[y]
Referral_Fees[y]    = if y ≤ 2: financing_tranche[y] × 0.10  else 0
                      (Y1=$2M, Y2=$2.2M, Y3+=0)
WPI_Consulting[y]   = hardcoded: Y1=$2M, Y2=$8.5M, Y3+=0
GnA_total[y]        = if y ≤ 2: hardcoded ($750K NYC + $0/$250K Berlin)
                      else: advisory_fee_total[y] × ga_ramp[y]
                      where ga_ramp = [20%, 25%, 30%, 35%, 40%, 45%, 50%, 55%] for Y3-Y10
Total_OpEx[y]       = Referral_Fees + WPI_Consulting + GnA_total
EBITDA[y]           = Gross_Income − Total_OpEx
Taxes[y]            = 0.27 × EBITDA[y]
Earnings[y]         = EBITDA[y] − Taxes[y]                   (= EBITDA × 0.73)
EPS[y]              = Earnings[y] / 10_000_000
```

EBITDA Y1=−$3.08M, Y2=−$2.30M (negative — capital raise period), Y3=$13.81M+ onward positive.

#### Book Valuation (rows 74-78)

```
financing_activity[y] = {Y1: 20_000_000, Y2: 22_000_000, Y3+: 0}
cumulative_fcf[y]     = financing_activity[y] + earnings[y] + cumulative_fcf[y−1]
lp_ownership[y]       = Σ nav_LP_n[y] (all 6 LPs — zero before each LP's launch year)
book_value[y]         = cumulative_fcf[y] + lp_ownership[y]
book_value_per_share  = book_value / 10_000_000
```

Note: "Woodfine Credit Inc." in the label (row 75) is a legacy label, not a separate legal entity. Row 75 = contributed capital + cumulative retained earnings (cash basis).

#### Four Valuation Methods (rows 80-102)

**1. Market Valuation** (P/E earnings-based):
```
earnings_valuation[y]    = earnings[y] × 10.72
market_valuation[y]      = earnings_valuation[y] + cumulative_fcf[y]
market_val_per_share[y]  = market_valuation[y] / 10_000_000
pe_ratio[y]              = market_valuation[y] / earnings[y]   (verification row)
```
P/E = 10.72 is operator-set early-stage discount (~1/3 of listed peer mean 33.05). Configurable.

**2. Fair Valuation** (PEG-based with FCF floor):
```
earnings_growth[y]         = (EPS[y] − EPS[y−1]) / |EPS[y−1]|
                             special Y1: (0 − EPS[Y1]) / |EPS[Y1]|
forward_growth_avg[y]      = AVERAGE(earnings_growth[y+1 .. Y10])
                             Y9/Y10 edge: window = [earnings_growth[Y10]] for both
fair_val_per_share[y]      = (forward_growth_avg[y] × 100 × peg_ratio × EPS[y])
                             + (cumulative_fcf[y] / 10_000_000)
```
`peg_ratio` = 1.0 (hardcoded). `×100` converts decimal to percentage-point for Lynch formula.
**Warning:** fair_val_per_share is negative for Y1–Y2 (negative EPS). Flag with `valuation_method_warning: "negative_eps"` in JSON.

**3. Dividend Valuation** (yield capitalisation):
```
dividend_val[y]           = earnings[y] / 0.045
dividend_val_per_share[y] = EPS[y] / 0.045
```
Yield-cap at 4.5% with no growth term (bond analogue). Not Gordon Growth. Configurable.

**4. Comparative ratios** (rows 99-102):
```
mv_vs_bv[y]     = market_val_per_share[y] / book_val_per_share[y]
mv_vs_fv[y]     = market_val_per_share[y] / fair_val_per_share[y]
mv_vs_dv[y]     = market_val_per_share[y] / dividend_val_per_share[y]
ronte[y]        = earnings[y] / cumulative_fcf[y−1]   (Y1 = n/a)
```

#### C17:R53 Revenue Generator — required display output

This block is the required D3 summary (parallel to D2's AA12:AN35 summary). It shows all 6 LP funds with their per-year contributions. Column extent: **C:P (label + Y1–Y10)**. Rows 17–53.

Display format spec:
| Row type | Format |
|---|---|
| Advisory Fee, Distributions | `fmt_smart` ($/K/M auto-scale) |
| NAV rows | `fmt_m` (millions, 2 decimals) |
| Offering Costs Reimbursement | `fmt_smart` |
| Gross Income, OpEx totals | `fmt_smart` |
| EBITDA, Earnings | `fmt_smart`, negatives in parentheses |
| Per-share rows (EPS) | `$0.00` (2 decimals) |
| Valuation totals | `fmt_smart` |
| Valuation per share | `$0.00` |
| P/E ratio | `0.00x` |
| Growth YoY rows | `0.0%` |
| RONTE, ratios | `0.0%` and `0.00x` |

Pending Layer 3 additions (not current scope):
- Full 4-tier LP waterfall (ROC → 8% preferred → GP catch-up → 80/20 carry)
- Capital account per LP (contributions, distributions, allocated income, clawback)
- IFRS 9 LP unit classification (liability vs. equity determination)

---

### 5d. D4 — Investor SPV: Bencal Special Purpose 2 Inc. / Bencal SPV2-LP

**Source Excel:** `COMPLIANCE_WCP_2026_01_08_Forecast_Foreign Investment SPVs_Compensations_JW2.xlsx`
**Tab:** `SPV_Compensations_10 percent` (template at $10M; Bencal SPV2 scales to $30M)

#### Structure

Bencal SPV2 is a GP/LP feeder vehicle that holds PCLP1 investment units on behalf of investor groups.
The SPV-Manager receives a non-cash 10% diluted equity stake in the LP at formation.

```
Bencal Special Purpose 2 Inc. (GP, Bencal SPV2-GP)
└── Bencal Special Purpose 2 Limited Partnership (LP, Bencal SPV2-LP)
    └── holds $25M net of PCLP1 units (Woodfine Professional Centres LP)
```

**Consolidated presentation:** Bencal SPV2-GP + Bencal SPV2-LP are presented as a single consolidated
statement to SPV-LP investors. Bencal SPV2-GP controls Bencal SPV2-LP (IFRS 10 — GP has power, variable
returns, and ability to affect returns). The consolidated entity is the investor-facing
reporting unit.

**IFRS 10.27 note:** Bencal SPV2-LP qualifies as an investment entity (obtains funds from investors,
commits to invest for capital appreciation/income, measures performance on FV basis). Bencal SPV2-LP
therefore carries its PCLP1 investment at FVTPL. This does not affect Bencal SPV2-GP's consolidation
of Bencal SPV2-LP — PCLP1 stays at FVTPL within the consolidated statements.

#### Governance — Bencal SPV2-GP (General Partner)

The General Partner shall be governed by a board of three (3) directors:
- **Director A** and **Director B** — appointed by shareholders
- **Director C** — independent director (no financial interest in Professional Centres Canada LP), appointed by unanimous vote of Director A and Director B; may be removed and replaced by shareholders by simple majority vote

All board resolutions require unanimous approval.

**Corporate Secretary:** Rotates annually among the non-independent directors. Serves as sole authorized signatory and spokesperson. No other officers.

#### Parameters (Bencal SPV2 — net $25M PCLP1 investment)

| Parameter | Value | Config key |
|---|---|---|
| Net PCLP1 investment (fixed) | $25,000,000 | `net_pclp1_investment` |
| OpexBudget reserve (Y1–Y3) | `required_reserve_at_close` (see OpexBudget below) | `initial_cash_reserve` |
| Cost per unit | $100 | `cost_per_unit` |
| SPV-Manager dilution | 10% | `dilution_pct` |
| Legal Services (annual flat) | $1,145 | `asm.ad2.opex.legal` |
| Accounting Services (annual flat) | $2,399 | `asm.ad2.opex.accounting` |
| Board of Directors (3 directors × $1,093.34/qtr) | $13,120/yr | `board_expense` |
| Issue costs | 0% | `issue_costs_pct` |
| Interest on debt | 0% | `debt_interest_rate` |
| Interest on cash | 0.5% | `cash_interest_rate` |
| Debt repayment rate | 0% | `debt_repay_pct` |
| Minimum cash balance | $15,000 | `min_cash` |
| ~~Working capital reserve 5.1%~~ | ~~superseded by OpexBudget (§3h)~~ | ~~`wc_reserve_pct`~~ |
| SPV Market Value yield | 8.0% | `spv_market_val_yield` |

**Bencal SPV2 OpexBudget (source: COMPLIANCE_MCorp_2026_05_25_SPV Budget_JW1.xlsx — Tab Bencal SPV2):**

*Setup / Formation (one-time at Y0 — expensed per IAS 38.69(d)):*

| Line item | Amount | ASM ID |
|---|---|---|
| Legal — R&R maintenance + annual + BC Annual Report | $7,730 | `asm.ad2.opex.legal_setup` |
| Accounting — KYC/AML setup | $275 | `asm.ad2.opex.acct_setup` |
| Bank — account setup (LP + Inc.) | $1,100 | `asm.ad2.opex.bank_setup` |
| **Setup total** | **$9,105** | |

*Annual operating expenses (flat Y1–Y10):*

| Line item | Annual | ASM ID |
|---|---|---|
| Legal Services | $1,145 | `asm.ad2.opex.legal` |
| Accounting Services | $2,399 | `asm.ad2.opex.accounting` |
| Board of Directors (3 × $1,093.34/qtr) | $13,120 | `asm.ad2.opex.board` |
| **Annual total** | **$16,664** | |

⚠ Source Excel formula error noted: D&O Officers subtotal cell sweeps Directors row (double-count). Engine uses $13,120/yr director fee (3 × $4,373.37/yr; 2% Work Fee constraint; Excel $36,000 superseded). Jennifer to fix source Excel.

*Reserve sizing (`funding_onset_year = 4` — PCLP1 DPU begins Y4):*

```
required_reserve_at_close = setup_total + annual × 3
                          = $9,105 + $16,664 × 3
                          = $9,105 + $49,992
                          = $59,097
```

**Offering structure with reserve (income onset model, 2026-05-24):**
```
net_pclp1_investment    = $25,000,000 (fixed; buys 250,000 PCLP1 units at $100)
opex_reserve            = required_reserve_at_close  = $59,097
total_equity            = net_pclp1_investment + opex_reserve
investor_units          = total_equity / cost_per_unit ($100)
manager_units           = ROUND(investor_units / 9, 0)
diluted_units           = investor_units + manager_units
```

```
total_equity    = $25,059,097;  investor_units = 250,591;
manager_units   = 27,843;       diluted_units  = 278,434
```

Reserve is **additive** to the PCLP1 investment. PCLP1 units purchased = 250,000 (fixed).
Cash reserve (~$202,737) held in SPV IS; drawn down for Y1–Y3 opex.

**Two-layer valuation framework (Bencal SPV2 Flag 1 resolution, 2026-05-24):**

Both PCLP 1 and Bencal SPV2 use the same 8.0% distribution yield as the market-facing rate, but
the discount to NAV falls out differently at each layer:

| Layer | NAV basis | Market Value basis | Discount driver |
|---|---|---|---|
| PCLP 1 | NOI / 6.25% cap rate | Annual Distributions / 8.0% yield | Cap rate vs. distribution yield spread (~−20%) |
| Bencal SPV2 SPV-LP | PCLP 1 NAV × SPV ownership % + working capital − cumulative fees | SPV Distributions / 8.0% yield | Fee drag (board + admin ≈ $56K/yr; no advisory fee) reduces distributions relative to carried NAV; materially smaller discount than prior model |

Bencal SPV2 carries its Professional Centres Canada LP units **at NAV** (not at PCLP 1 market value). The SPV-LP market
value is then derived from its own distribution capacity at 8.0% yield. The different
discounts between Professional Centres Canada LP and Bencal SPV2 fall out naturally from each entity's cost structure
and are not independently set. 8.0% is the calibration starting point; may be adjusted
in a later pass once both valuation summaries are rendered side by side.

#### Founding-bonus WCP shares (NEW — v0.15.8, 2026-06-02)

Bencal SPV2 receives **600,000 WCP common shares for nominal consideration** as a founding-bonus
allocation in exchange for **completing its minimum CAD 13,000,000 investment in
Professional Centres Canada LP**. Bencal SPV2's subscribed amount ($25,059,097) exceeds the
CAD 13M minimum threshold, so the bonus is earned at close.

| Parameter | Value | Notes |
|---|---|---|
| Bonus shares to Bencal SPV2 | 600,000 | Founding-bonus allocation |
| Nominal price per share | $0.00033223 | Matches Bencal SPV1 founding-bonus precedent |
| Total cash consideration | $199.34 | $0.00033223 × 600,000 |
| Source of shares | Strategic Partner block | Carve-out from existing 1,800,000 (18%); reduces to 1,200,000 (12%) — see §5c WCP cap table |
| WCP shares outstanding (total) | 10,000,000 | UNCHANGED — no new issuance |
| Bencal SPV2 stake in WCP | 6.0% | 600,000 / 10,000,000 |
| Bencal Group aggregate WCP stake | 9.0% | 900,000 / 10,000,000 (SPV1 300K + SPV2 600K) |
| Trigger condition | Completing minimum CAD 13M LP investment in Professional Centres Canada LP | Met at close; Bencal SPV2 subscribed $25.06M |

**Bencal SPV2 is now a DUAL-ASSET SPV** (previously pure-play LP). Two FVTPL holdings:
1. **Professional Centres Canada LP units** (250,591 units × $100 = $25,059,100; FVTPL Level 2/3
   via PCLP 1 NAV calculation; §5b).
2. **WCP common shares** (600,000 shares; FVTPL Level 3 pre-listing via management proxy
   ~$4.55/share = ~$2,730,000; Level 1 at WCP exchange listing; same Phase B trigger as
   Bencal SPV1's holding).

**Bencal SPV2 WCP accounting under locked Path (b) — Y0 capital contribution; subsequent FVTPL movements:**

Per Flag 15 path (b), the ~$2,729,800.66 difference between FMV ($2,730,000) and consideration
paid ($199.34) is credited to **Contributed Surplus equity at Y0** — NOT to Bencal SPV2's income
statement. There is therefore **no Y0 bonus-share income or Y0 FVTPL gain at Bencal SPV2** from
the initial receipt.

The Bencal SPV2 income statement then carries subsequent FVTPL gains/(losses) Y1+ as the WCP
shares revalue (Level 3 management proxy moving with WCP NAV; transitions to Level 1 at WCP
exchange listing via existing `wcp_listing_year` trigger).

For comparison, **Bencal SPV1 records a Y1 unrealised FVTPL loss of ~$1,635,050** because SPV1
paid $3,000,049.83 cash + bonus for 300,000 shares against the same $4.55 Level 3 proxy
($1,365,000 book). The Bencal SPV1 Y1 loss is independent of Bencal SPV2 — SPV1 uses
purchase-and-bonus accounting (cost > Level 3 proxy at formation), while SPV2 uses capital-
contribution accounting (initial recognition at FMV, no Y0 P&L). Both treatments are
internally consistent.

**CIM risk-factor language — disclose at SPV1 only**, not SPV2 (since SPV2 has no Y0 P&L hit):

> Bencal SPV1 will record a substantial Y1 unrealised FVTPL loss (~$1,635,050) on its WCP
> holding owing to the difference between cash purchase price ($20.00/share for 150,000 shares,
> plus founding-bonus shares at nominal) and the Level 3 management-proxy book value at
> formation. This loss is structural and is expected to reverse from Y2 onward as WCP value
> appreciates. Bencal SPV2's WCP holding is received as a founding capital contribution from
> the Strategic Partner block (per Flag 15 path (b)) and is recorded at FMV against contributed
> surplus equity at Y0; subsequent FVTPL gains/(losses) follow the same Level-3-to-Level-1
> transition as SPV1.

**IFRS 2 / IAS 12 / Section 69 treatment — Flag 15 RESOLVED 2026-06-02 → Path (b) Capital Contribution locked:**

The Strategic Partner → Bencal SPV2 transfer is treated as a **founding capital contribution
from the Strategic Partner**, with the WCP shares recorded at FMV against contributed surplus
(equity). No Y0 income recognition at Bencal SPV2. Section 69 of the Income Tax Act treats the
transfer as occurring at FMV regardless of stated consideration; tax effect is recognised at
the Strategic Partner level only.

**Y0 journal entry at Bencal SPV2:**

```
Dr  Investment — WCP shares (FVTPL Level 3)   $2,730,000.00
    Cr  Contributed surplus — founding endowment     $2,729,800.66
    Cr  Cash                                                 $199.34
```

| Path | Status |
|---|---|
| **(a)** IFRS 2 share-based payment received for service | DEFERRED — Bencal SPV2 is not a service provider; substance-over-form analysis favours capital contribution. |
| **(b)** Capital contribution from Strategic Partner via contributed surplus | **LOCKED 2026-06-02 — engine + CIM proceed on this path.** |
| **(c)** Bargain-purchase / cost-basis with subsequent FVTPL adjustment | DEFERRED — IFRS 9 initial recognition prefers FMV when received; cost-basis-then-adjust path inconsistent. |

Tax counsel sign-off requested post-decision (not blocking — working assumption is path (b)).
Strategic Partner's deemed FMV disposal under ITA s.69 produces capital gain/(loss) between
FMV (~$2,730,000) and the Strategic Partner's own cost basis (separate matter at Strategic
Partner level). Bencal SPV2's cost basis for tax = FMV per s.69 (not the $199.34 consideration
paid).

**Document chain required at close:**
1. Strategic Partner Share Transfer Agreement (WCP + Strategic Partner + Bencal SPV2; references CAD 13M LP investment trigger condition).
2. WCP shareholders' consent (if drag-along / right-of-first-refusal applies under WCP Shareholders' Agreement).
3. Updated WCP share register and certificate issuance.
4. Bencal SPV2-LP / Bencal SPV2-GP corporate authorisation to receive transfer (board minutes; LP partners' consent if required by LP Agreement).
5. CRA T2057 / T2058 election if applicable (rollover at tax cost basis — only if structuring permits; unlikely under Section 69).
6. Schedule 50/53 disclosure if Bencal SPV2 crosses any cumulative-stake threshold post-transfer.

**Phase A implementation impact (locked under Flag 15 path (b)):**
- `Ad2Config` / `Pclp1Config` schema additions: `wcp_bonus_shares_received` (default 600_000), `wcp_bonus_price_per_share` (default $0.00033223), `wcp_bonus_proxy_value_per_share` (default $4.55), `wcp_bonus_trigger_lp_minimum` (default CAD 13M).
- `Ad2Data` output additions: dual-asset NAV breakdown; Y0 capital-contribution JE per path (b): `Dr Investment-WCP $2,730,000 / Cr Contributed Surplus $2,729,800.66 / Cr Cash $199.34`; **no Y0 IS impact at Bencal SPV2**; subsequent Y1+ FVTPL movements on WCP component flow through Bencal SPV2 IS as usual.
- `Block A-F` valuation matrix at §5d: add WCP component column to dual-asset NAV showing cost ($199.34) / Level 3 proxy ($2.73M) / Level 1 post-listing.
- MOIC presentation: Block F renders side-by-side **per-share AND aggregate** MOIC columns per Flag 3 / Block F decision; header note explains 10/90 manager/investor dilution mechanics.
- Engine struct names retain `pclp1_*` identifiers per §5b code-stability note.

**Consolidated statement title update (§5d) when this is wired through:**
> Note 5 (NEW): WCP — Bencal SPV2 holds 600,000 WCP common shares (6.0% of 10,000,000 outstanding;
> received as founding bonus from Strategic Partner block for nominal consideration of $199.34
> on completion of minimum CAD 13,000,000 investment in Professional Centres Canada LP).
> Shares carried at FVTPL; Level 3 management proxy pre-WCP listing; Level 1 post-listing.

#### Dilution mechanics

```
total_equity            = net_pclp1_investment + opex_reserve
investor_units          = total_equity / cost_per_unit
                        = ($25,000,000 + $59,097) / $100
                        = 250,591
manager_units           = ROUND(investor_units / 9, 0)       = 27,843
diluted_units           = investor_units + manager_units     = 278,434
manager_pct_of_total    = 27,843 / 278,434                   ≈ 10.0000%
issuance_dilution       = manager_units / investor_units     = 1/9 = 11.1111̄%
```

The LP issues manager units equal to 1/9 of investor units (11.111111̄% issuance dilution),
resulting in the manager holding ≈ 10.0000% of total units on a fully diluted basis.
All per-unit metrics use the fully diluted total (= 278,434) as denominator.

Unit-based compensation under IFRS 2: manager_units × $100 ≈ $2,784,300 (example).
On consolidation this collapses to a capital transaction (not P&L), since Bencal SPV2-GP
and Bencal SPV2-LP are consolidated as one reporting group.

**Bencal Management cascade:** Bencal Management holds the manager units (≈ 27,843 Bencal SPV2-LP units). The Bencal SPV2
diluted unit count (≈ 278,434) used in Bencal Management §5f calculations. All Bencal Management Bencal SPV2 constants
in bencal.rs updated to v0.15.6.

#### Derived parameters

```
initial_cash_reserve    = opex_reserve = $59,097
net_equity_funding      = net_pclp1_investment                       = $25,000,000
pclp1_units_held        = net_equity_funding / cost_per_unit         = 250,000
total_expenses_annual   = legal + accounting + board                 = $16,664/yr (flat)
total_sqft              = net_equity_funding / $310_gross_cost_sqft  = ~80,645 sqft
```

Cash reserve ($59,097) earns 0.5% interest annually ≈ $295/yr — book in SPV IS (§3g baseline).
Income onset: PCLP1 DPU × 250,000 units begins Y4; reserve covers Y1–Y3 opex ($16,664/yr flat).

#### Income Statement formula chain

```
net_proceeds_from_ops[y] = pclp1_units_held × PCLP1.dpu[y]
  = 250,000 × PCLP1.dpu[y]   (was (D43/100) × PCLP1_row122 at 284,700 units)
  Y1–Y3: 0 (construction phase; no PCLP1 DPU)
  Y4+: 250,000 × PCLP1.dpu[y]

income_continuity[y]     = net_proceeds_from_ops[y]   (no construction-phase split)

expenses[y]:
  issue_costs[Y1]        = 0
  financing_costs[y]     = 0   (no debt)
  legal_services[y]      = $1,145 (flat)
  accounting_services[y] = $2,399 (flat)
  board_of_directors[y]  = $13,120   (3 directors × $1,093.34/qtr)
  total_expenses[y]      = $16,664 (flat)

ebitda[y]                = net_proceeds_from_ops[y] − total_expenses[y]
net_interest[y]          = −avg_cash[y] × 0.005  (no debt; interest income on WC at 0.5% per §3g)
funding_from_ops[y]      = ebitda[y] + net_interest[y]

distribution_to_lp[y]:
  Y1–Y3: 0
  Y4–Y7: funding_from_ops[y] × 0.90
  Y8+:   funding_from_ops[y] × 1.00

dpu[y]                   = distribution_to_lp[y] / diluted_units   (≈ 280,030 — TBD)
distribution_yield[y]    = dpu[y] / cost_per_unit
```

#### Asset Valuation + NAV

Asset value is the SPV's proportional interest in PCLP1 NAV — no real property held directly:
```
opening_assets[y]        = pclp1_units_held × PCLP1.nav_per_unit[y]
  = 250,000 × PCLP1.nav_per_unit[y]   (was (D43/100) × PCLP1_row115 at 284,700 units)

total_capital_assets[y]  = opening_assets[y]   (no construction)
asset_value_total[y]     = total_capital_assets[y] + closing_cash[y]
nav_total[y]             = asset_value_total[y]  (closing_debt = 0)
nav_per_unit[y]          = nav_total[y] / diluted_units   (≈ 280,030)
```

#### Four valuation rows in summary (AA6:AN35)

```
asset_value_per_unit[y] = asset_value_total[y] / diluted_units    (row 18)
nav_per_unit[y]         = nav_total[y] / diluted_units             (row 21)

market_value_per_unit[y]:
  Y1–Y3: $100 (subscription price)
  Y4–Y7: operator config (hardcoded in Excel: $125.8, $132.1, $171.5, $177.3)
  Y8+:   dpu[y] / spv_market_val_yield  (= dpu[y] / 0.080)

discount_premium[y]    = (market_value − nav) / nav    (emergent)
cagr_y8                = (mv[Y8] / 100)^(1/8) − 1      (single scalar, Y8 only)
buyer_yield[y ≥ Y8]    = dpu[y] / market_value[y]      (tautologically = 0.080)
```

#### SPV compensation summary block (AA6:AN11, right-hand totals)

```
total_advisory_fees_Y1_Y8         = 0   (advisory fee removed 2026-05-25; line retained for schema compat)
unit_based_comp_at_mv_Y8          = mv_per_unit[Y8] × manager_units
total_distributions_to_manager    = SUM(distribution_to_lp[Y1..Y8]) × dilution_pct
```

#### AA6:AN35 summary — required display output

Columns AE=Y1 through AN=Y10 (same header pattern as D2 AA12:AN35).

| Row | Label | Formula |
|---|---|---|
| 13 | Revenue | net_proceeds_from_ops / diluted_units |
| 14 | Distributions | dpu |
| 16 | Dist Yield on $100 | dpu / 100 |
| 18 | Asset Value | asset_value_per_unit |
| 19 | Total Debt | 0 |
| 21 | NAV | nav_per_unit |
| 23 | Market Value | see above (Y1-Y7 config, Y8+ yield-cap) |
| 24 | Discount/Premium | (MV − NAV) / NAV (emergent) |
| 25 | *(label for row 24)* | "Discount / Premium vs. Net Asset Value (NAV)" |
| 27 | CAGR (excl. dist.) | single scalar at Y8 only |
| 28 | Buyer Yield at MV | = 0.074 tautologically (Y8+) |
| 30 | ICR | 0 (no debt) |
| 31 | Debt vs Dev Cost | 0 |
| 32 | Debt to AV | 0 |
| 34 | TER | (legal + accounting + admin + board) / nav_total |
| 35 | Total Sqft | net_equity_funding / $310 |

**Blank patterns:** Y1–Y3 entries for Revenue, Distributions, ICR show `"-"`.
Rows 24/27/28 blank for Y1–Y7 (per PCLP1 pattern).

**Denominator for per-unit rows:** ⚠ See open flag below.

#### Investor yield vs. direct PCLP1

| Investor route | DPU at Y8 | Yield on $100 |
|---|---|---|
| Direct PCLP1 | $24.09 | 24.09% |
| Via Bencal SPV2-LP (diluted basis) | TBD (engine) | TBD (engine) |
| Yield drag | TBD (engine) | TBD (engine) |

Drag decomposition (revised — standardized opex 2026-05-25): OpEx ≈ $59,544/yr on $25M net (≈ −24 bps), manager dilution 10% (−64 bps). Total ≈ −88 bps vs direct. Exact DPU values engine-derived.
⚠ WC reserve drag (5.1% / −41 bps) eliminated — reserve now funded from offering gross-up (additive, not deducted from investable capital).

TER at $25M net ≈ $59,544 / NAV ≈ 0.24% (Y2+; no advisory fee; board + admin + legal/accounting only).

#### ⚠ Open flags for Jennifer's decision (raised by 4-agent panel)

**Flag 1 — 7.4% SPV Market Value yield (AC23):**
The Excel uses 7.4%, which produces a **higher** Market Value for Bencal SPV2 than for PCLP1 (PCLP1 = 8%). Economically, Bencal SPV2 carries more fee drag and dilution than PCLP1 — it should trade at a **wider** cap rate (lower MV), not tighter. The banking agent notes the defensible range is **8.5%–9.5%**. The 7.4% is not arm's-length defensible for secondary transfer or estate valuation. **Confirm whether this is intentional or a model carry-over from the template.**

**Flag 2 — Per-diluted denominator (RESOLVED 2026-05-24):**
Fully diluted total (333,333) used as denominator in all per-unit metrics. No dual-column display. Also resolved in this flag: unit count corrected from 333,333 → 333,333 (drop `−1` floor; formula `ROUND(investor_units / 9, 0)`; issuance dilution = 1/9 = 11.111111̄%). Applies uniformly to PCLP 1, Bencal SPV2 SPV-LP, and Bencal SPV1 SPV-WCP.

**Flag 3 — No-hurdle disclosure (RESOLVED 2026-05-24):**
Confirmed: face-of-statement disclosure. Two required components:

1. **Pari passu participation (no distribution hurdle):** SPV-Manager holds 33,333 units
   (≈ 10.0000% fully diluted). No preferred return, no hurdle rate, no performance waterfall,
   no clawback. Manager units receive the same per-unit distributions as all investor units
   from the first closing.

2. **Escrow / transfer restriction (mirror PCLP 1 LPA Benetti clause):** Manager units to be
   held in escrow under the LP agreement; unrestricted transfer not permitted until the earlier
   of: (i) limited partners receiving distributions equal to 100% of their initial investment in
   units; (ii) closing of a final sale of all or substantially all of the Partnership's assets;
   or (iii) sale of all manager units to the holder(s) of at least 75% of outstanding units.
   Draft this clause into the Bencal SPV2-LP agreement at formation.

Engine Note 1 text (face of consolidated statement — F field, mandatory):
> Note 1: Manager Dilution — SPV-Manager holds 33,333 units (≈ 10.00% of 333,333 total
> fully diluted units). No preferred return, no hurdle rate, no clawback. Manager units
> participate pari passu in all distributions from first closing. Manager units are held in
> escrow and may not be transferred until: (i) limited partners have received distributions
> equal to 100% of their initial investment in units; (ii) closing of a final sale of all
> or substantially all of the Partnership's assets; or (iii) sale of all manager units to
> the holder(s) of at least 75% of the outstanding units. Per-unit metrics on fully diluted
> basis (≈ 280,030) unless otherwise noted.

#### Consolidated statement title

> **Bencal Special Purpose 2 Limited Partnership**
> *(administered by Bencal Special Purpose 2 Inc., General Partner)*
> **Consolidated Statement of Cash Distributions and Unit Valuation — Years 1–10**
> Note 1: Manager Dilution — SPV-Manager holds ≈ 27,843 units (≈ 10.00% of LP; fully diluted ≈ 278,434). Per-unit metrics on diluted basis.

---

### 5e. D5 — Investor SPV: Bencal Special Purpose 1 Inc.

**Source Excel:** None (no sample; derived from WCP 42M model + SPV template parameters)
**Specification basis:** 4-agent opus panel analysis, 2026-05-23 (see §18)

#### Structure

Bencal SPV1 is a simple Canadian federal corporation (CBCA) whose sole asset is 150,000 common shares
of Woodfine Capital Projects Inc. Bencal SPV1 is a **pure capital appreciation vehicle** — WCP pays no
dividends; investors benefit only through WCP share appreciation at eventual exit.

```
Bencal Special Purpose 1 Inc.
└── holds 300,000 WCP common shares (3.0% of 10,000,000 outstanding)
    (150,000 purchased at $20.00/share + 150,000 bonus at $0.00033223/share)
```

**No LP structure.** Bencal SPV1 is a standalone corporation issuing common shares to investors.
There is no consolidated subsidiary, no GP/LP relationship.

#### Governance — Bencal Special Purpose 1 Inc.

The corporation shall be governed by a board of three (3) directors:
- **Director A** and **Director B** — appointed by shareholders
- **Director C** — independent director (no financial interest in WCP), appointed by unanimous vote of Director A and Director B; may be removed and replaced by shareholders by simple majority vote

All board resolutions require unanimous approval.

**Corporate Secretary:** Rotates annually among the non-independent directors. Serves as sole authorized signatory and spokesperson. No other officers.

#### Parameters

| Parameter | Value | Config key |
|---|---|---|
| WCP purchased shares | 150,000 at $20.00/share = $3,000,000 | `investment.wcp_purchased_shares` |
| WCP bonus shares | 150,000 at $0.00033223/share = $49.83 (founding investor bonus) | `investment.wcp_bonus_shares` |
| Total WCP shares held | 300,000 (3.0% of 10,000,000 outstanding; Bencal Group total exposure 900,000 = 9.0% via SPV2 founding-bonus carve-out — see §5c + §5d) | `investment.wcp_shares_held` |
| Total WCP cost basis | $3,000,049.83 | `investment.wcp_cost_basis` |
| Investor shares | derived from total_offering ÷ $1.00 (≈ 3,198,892) | `shares.investor_shares` |
| Investor share price | $1.00 | `shares.investor_share_price` |
| SPV-Manager dilution | 10% (structural; no IFRS 2 charge at Bencal SPV1 level) | `shares.dilution_pct` |
| Cash reserve at close | $54,832 (only cash on BS) | `investment.initial_cash_reserve` |
| FVTPL unrealised tax rate | 13.5% (27% blended × 50% cap-gains inclusion) | `tax.fvtpl_unrealised_rate` |
| Revenue | nil | WCP pays no dividends |
| Debt | nil | no borrowings |

**Bencal SPV1 OpexBudget (source: COMPLIANCE_MCorp_2026_05_25_SPV Budget_JW1.xlsx — Tab Bencal SPV1):**

*Setup / Formation (one-time at Y0 — expensed per IAS 38.69(d)):*

| Line item | Amount | ASM ID |
|---|---|---|
| Legal — R&R maintenance + annual + BC Annual Report | $4,950 | `asm.ad1.opex.legal_setup` |
| Accounting — KYC/AML setup | $275 | `asm.ad1.opex.acct_setup` |
| Bank — account setup | $350 | `asm.ad1.opex.bank_setup` |
| **Setup total** | **$5,575** | |

*Annual operating expenses (flat Y1–Y10):*

| Line item | Annual | ASM ID |
|---|---|---|
| Legal Services | $900 | `asm.ad1.opex.legal` |
| Accounting Services | $2,399 | `asm.ad1.opex.accounting` |
| Board of Directors (3 × $1,093.34/qtr) | $13,120 | `asm.ad1.opex.board` |
| **Annual total** | **$16,419** | |

⚠ Source Excel formula error noted: D&O Officers subtotal cell sweeps Directors row (double-count). Engine uses $13,120/yr director fee (3 × $4,373.37/yr; 2% Work Fee constraint; Excel $36,000 superseded). Jennifer to fix source Excel.

*Reserve sizing (`funding_onset_year = 4` — WCP share sales begin Y4; reserve covers Y1–Y3):*

```
required_reserve_at_close = setup_total + annual × 3
                          = $5,575 + $16,419 × 3
                          = $5,575 + $49,257
                          = $54,832
```

Cumulative opex milestones: Y1 = $68,649; Y2 = $128,193; Y3 = $187,737.
Reserve covers full Y1–Y3 gap. From Y4: WCP share sales fund ongoing opex (§3h income onset model).

**Offering structure with reserve (Flag 1 — RESOLVED 2026-05-24 — Option 1: reserve at close):**
```
wcp_purchased       = $3,000,000 (150,000 WCP shares at $20.00/share)
wcp_bonus           = $49.83     (150,000 WCP shares at $0.00033223 founding bonus)
wcp_cost_total      = $3,000,049.83
opex_reserve        = required_reserve_at_close  = $54,832
total_offering      = wcp_cost_total + opex_reserve
investor_shares     = total_offering / share_price ($1.00)
manager_shares      = ROUND(investor_shares / 9, 0)
diluted_shares      = investor_shares + manager_shares
```

Capital deployment at close: $3,000,049.83 → WCP shares (300,000 total, 3.0% ownership);
$54,832 → cash reserve (only cash on BS; funds Y1-Y3 opex gap).

```
total_offering  = $3,054,882;  investor_shares = 3,054,882;
manager_shares  = 339,431;     diluted_shares  = 3,394,313
```

The reserve is **additive** to the WCP investment — WCP shares held constant at 150,000 (1.5%).
Engine emits `going_concern_warning` if `initial_cash_reserve − cumulative_opex[y] < 0`.
Offering documents disclose the reserve amount and its purpose.

#### Dilution mechanics

```
total_offering      = wcp_cost_total + opex_reserve
                    = $3,000,049.83 + $54,832
                    = $3,054,882
investor_shares     = total_offering / share_price     = 3,054,882
manager_shares      = ROUND(investor_shares / 9, 0)    = 339,431
diluted_shares      = investor_shares + manager_shares = 3,394,313
manager_pct         = 339,431 / 3,394,313              ≈ 10.0000%
issuance_dilution   = manager_shares / investor_shares = 1/9 = 11.1111̄%
```

Reserve confirmed from Excel (COMPLIANCE_MCorp_2026_05_25_SPV Budget_JW1.xlsx Tab Bencal SPV1). The legacy
3,000,000 / 333,333 / 3,333,333 figures are superseded by the 300K WCP / reserve-inclusive model above.
All IS/BS/CF denominators use investor_shares / diluted_shares from this formula.

#### WCP per-share value series (source data, corrected Y1-Y10)

Source: WCP_42M rows 78/85/92/97, cols G:P (corrected year mapping — see §15a; G=Y1, P=Y10).
All values in CAD per WCP share.

| Method | Y1 | Y2 | Y3 | Y4 | Y5 | Y6 | Y7 | Y8 | Y9 | Y10 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| Book | $4.55 | $18.10 | $22.84 | $28.81 | $37.74 | $42.78 | $55.05 | $65.91 | $91.46 | $105.15 |
| Market | −$0.64 | $2.01 | $15.62 | $20.41 | $22.60 | $24.67 | $25.71 | $33.22 | $54.67 | $64.53 |
| Fair | −$17.18 | −$11.32 | $27.30 | $34.21 | $40.29 | $47.77 | $56.49 | $73.22 | $63.59 | $71.46 |
| Dividend | −$5.00 | −$3.73 | $22.40 | $29.56 | $31.20 | $32.47 | $31.66 | $43.19 | $80.18 | $92.04 |

Negative early-year values (Market Y1, Fair Y1-Y2, Dividend Y1-Y2) are **floored at $0** for carrying-value
purposes per IFRS 13 (limited liability). Unfloored values retained in JSON as `*_unfloored` fields.
Dividend Valuation is notional — WCP pays no dividends. Disclosed with mandatory footnote in output.

#### IFRS / accounting treatment

**IFRS 9 classification — FVTPL (mandatory):** FVOCI election not appropriate because gains
trapped in OCI permanently cannot recycle to P&L on disposal under IFRS 9.B5.7.1 — destroying the
investor's ability to see capital gains as income. FVTPL aligns IS with NAV movement. Designation
irrevocable; document at incorporation in accounting policies note.

**IFRS 10 — Investment entity status:** Bencal SPV1 does NOT qualify (single-asset SPV; IFRS 10.28
typical characteristics not met). Standard corporate entity; WCP measured at FVTPL via IFRS 9.

**Manager dilution — structural (IFRS 2 not applicable at SPV level — Flag 3 RESOLVED 2026-05-25):**
Bencal SPV1 is the buyer of WCP shares, not a formation participant in WCP. No IFRS 2 service-cost expense
arises at the Bencal SPV1 entity level. Manager shares (≈ 339,431 TBD) are issued at the same $1.00
subscription price as investor shares; the economic dilution is reflected in the diluted share count
and per-share metrics only. No IS charge. No share-based compensation line in IS or CF.

**Deferred tax:**
- DTL: `max(cum_unrealised_book[y], 0) × 13.5%`; year-on-year change = deferred tax expense
- DTA on operating losses: unrecognised (probable-recovery test conservative default)

#### Income Statement structure (Y1-Y10)

```
Net change in fair value of investments (WCP FVTPL, Book method)   [line 1]
  Y1: $1,365,000 − $3,000,049.83 = −$1,635,050 (large Y1 loss; recovers from Y2)
+ Dividend income from WCP                                          [nil — WCP pays no dividends]
+ Interest income on cash reserve                                   [0.5% × cash_balance[y]]
─────────────────────────────────────────────────────────────────
Total investment income (loss)
− Legal Services                                                    [Y1: $8,875 / Y2+: $1,145]
− Accounting Services                                               [Y1: $3,774 / Y2+: $2,399]
− Board of Directors fees                                           [$13,120/yr — 3 directors × $1,093.34/qtr]
− Administration                                                    [$20,000/yr]
─────────────────────────────────────────────────────────────────
Income (loss) before tax
− Deferred tax expense (recovery)                                   [DTL change: 13.5% × max(cum_gain,0)]
─────────────────────────────────────────────────────────────────
Net income (loss)
EPS — basic (÷ investor_shares ≈ 3,198,892)
EPS — diluted (÷ diluted_shares ≈ 3,554,324)
```

`unrealised_gain[y] = 300,000 × wcp.book[y] − cost_basis_prev`
where `cost_basis_prev[Y1] = $3,000,049.83`; `cost_basis_prev[Y2+] = 300,000 × wcp.book[y-1]`.
Y1: 300,000 × $4.55 = $1,365,000; gain = −$1,635,050 (below cost — no DTL in Y1).
Y2: 300,000 × $18.10 = $5,430,000; gain vs Y1 FV = +$4,065,000.

#### Balance Sheet (Y1-Y10)

```
Assets
  Cash and cash equivalents                  [≈ reserve only at Y0; erodes by cum_opex[y];
                                              interest 0.5% p.a. on balance; going_concern if < 0]
  Investment in WCP shares (FVTPL)           [300,000 × wcp.book[y], floored at $0;
                                              cost $3,000,049.83; Y1 FV = $1,365,000]
  Deferred tax asset                         [unrecognised by default; disclose in notes]
Liabilities
  Accrued expenses                           [opex accruals]
  Deferred tax liability                     [max(300K×wcp.book[y] − $3,000,049.83, 0) × 13.5%]
Equity
  Share capital — investor common            [investor_shares × $1.00 ≈ $3,054,882]
  Share capital — manager common             [manager_shares × $1.00 ≈ $339,431; structural dilution;
                                              no IFRS 2 charge]
  Retained earnings / accumulated deficit
NAV per investor share = (Assets − Liabilities) / investor_shares  (≈ 3,198,892)
NAV per diluted share  = (Assets − Liabilities) / diluted_shares   (≈ 3,554,324)
```

#### Cash Flow Statement

```
Operating: net income/loss + add-back non-cash items (FVTPL change; DTL change)
  Y1 cash opex: −$68,649; Y2+ cash opex: −$59,544
  Interest income on cash: +0.5% × cash_balance[y] (cash item; no add-back)
Investing: Y0 WCP purchase: −$3,000,049.83 (150K at $20 + 150K at $0.00033223)
  Capital Return Sale year: +proceeds_from_wcp_sales
Financing: Y0 investor issuance: +total_offering (≈ +$3,198,892); manager shares: nil cash
  Capital Return Sale dividend year: −capital_returned_to_investors
Net cash change[y] = interest_income[y] − opex[y]  (pre-exit; no other cash flows)
Cash balance[y] = initial_cash_reserve + cum_interest[y] − cum_opex[y]
```

Engine emits `going_concern_warning: true` in JSON metadata when `cash_balance[y] < 0` in any year.

#### Capital Return Sale mechanics (exit — within Y1-Y5 FOFI window)

**Trigger:** WCP exchange listing. Phase B config: `wcp_listing_year: Option<u8>`.

**Capital Return Sale:** At listing, Bencal SPV1 sells sufficient WCP common shares to generate proceeds
= total investor capital (≈ $3,198,892) at the prevailing listing price. Proceeds distributed
as dividend to investors pro-rata.

**Residual Position:** WCP shares not sold in Capital Return Sale; effective cost basis ≈ $0.
Voted annually; absent two-thirds majority to hold, Residual sold that year. Hard cap: 10 years
post-listing.

**Example at $20/share listing price:**
- Need $3,198,892 ÷ $20 = 159,945 WCP shares for Capital Return Sale
- Residual = 300,000 − 159,945 = 140,055 WCP shares at zero effective cost basis

**Example at $40/share listing price:**
- Need $3,198,892 ÷ $40 = 79,972 WCP shares
- Residual = 220,028 WCP shares (73% of position retained as pure upside)

**IFRS 9 hierarchy at listing:** Level 3 Book Value → Level 1 market-quoted price. Switch is
irrevocable from listing date. Phase A: Level 3 Book Value throughout (pre-listing assumed).

**FOFI scope (Flag 4 RESOLVED 2026-05-25):** Bencal SPV1 OM-facing FOFI = Y1-Y5 only.
Capital Return Sale expected within this window. Y6-Y10 not included in investor FOFI.
Internal management view retains full Y1-Y10 for Residual Position modelling.

#### Summary block (comparable to D4 AA6:AN35)

Column extent: Y1-Y10 (same header pattern as D2/D4 summaries). Primary = Book method.

**Section A — WCP per-share pass-through:**
A1: Book Value per share · A2: Market Value per share · A3: Fair Value per share ·
A4: Dividend Value per share (notional — footnote mandatory)

**Section B — Bencal SPV1 Asset Value (Book primary):**

| Row | Formula |
|---|---|
| B1 Total asset value | `300,000 × wcp.book[y]` |
| B2 Per investor share | `B1 / investor_shares` (≈ ÷ 3,198,892) |
| B3 Per diluted share | `B1 / diluted_shares` (≈ ÷ 3,554,324) |

**Section C — Operating cost drag:**

| Row | Formula |
|---|---|
| C1 Annual opex | `opex[y]` |
| C2 Cumulative opex | `Σ opex[1..=y]` |
| C3 Annual drag % of cost basis | `opex[y] / 3,000,049.83` |

**Section D — NAV (Book primary; deferred tax deducted):**

| Row | Formula |
|---|---|
| D1 Debt | `0` (retained for structural parity with D4) |
| D2 NAV total | `B1 − C2 − DTL[y]` |
| D3 NAV per investor share (primary) | `D2 / investor_shares` (≈ ÷ 3,198,892) |
| D4 NAV per diluted share | `D2 / diluted_shares` (≈ ÷ 3,554,324) |
| D5 Discount / Premium to par | `(D3 / 1.00) − 1` |

**Section E — Valuation method comparison (per-investor-share, gross of DTL):**

| Row | Formula |
|---|---|
| E1 Book NAV / inv share | `(max(300K × wcp.book[y], 0) − C2) / investor_shares` |
| E2 Market NAV / inv share | `(max(300K × wcp.market[y], 0) − C2) / investor_shares` |
| E3 Fair NAV / inv share | `(max(300K × wcp.fair[y], 0) − C2) / investor_shares` |
| E4 Dividend NAV / inv share | `(max(300K × wcp.div[y], 0) − C2) / investor_shares` (notional) |

**Section F — Returns (scalars at Y8, investor-share basis):**

| CAGR | Formula | Computed value |
|---|---|---|
| F1 Book | `(D3[Y8] / 1.00)^(1/8) − 1` | **≈22.9%** |
| F2 Market | `(E2[Y8] / 1.00)^(1/8) − 1` | **≈14.2%** |
| F3 Fair | `(E3[Y8] / 1.00)^(1/8) − 1` | **≈26.6%** |
| F4 Dividend | `(E4[Y8] / 1.00)^(1/8) − 1` | **≈18.2%** |
| F5 TER | `opex[y] / (300,000 × wcp.book[y])` | Y1≈6.1%; Y10≈0.23% |

Note: D3[Y8] = (300,000 × $65.91 − $597,457 − $2,264,348) / 3,198,892 ≈ $5.287/share.
DTL[Y8] = ($19,773,000 − $3,000,049.83) × 0.135 ≈ $2,264,348. CAGR values are pre-engine
estimates; Phase A engine will recompute. Supersedes prior 5-agent verified values (based on 150K shares).
CAGR returns `None` if D3[Y8] ≤ 0 (not triggered by any method at Y8).

**Bencal SPV1 vs WCP direct (WCP purchase price $20/share; Book method):**

| | Y8 | Y10 |
|---|---:|---:|
| WCP direct per $1.00 invested | $3.296 | $5.258 |
| Bencal SPV1 per investor share (Book, net of opex + DTL) | ≈$5.218 | ≈$8.315 |
| Bencal SPV1 premium over WCP direct | **+$1.922** | **+$3.057** |
| WCP exposure per $1.00: Bencal SPV1 = 0.0926 shares vs direct = 0.0500 (85% more WCP per $1 invested) | | |

The bonus 150,000 WCP shares (acquired at $49.83) effectively double Bencal SPV1's WCP exposure,
producing structural outperformance vs direct WCP investment net of all costs and dilution.

Engine should assert: `ad1_premium = d3_ad1 - wcp_direct_value` ≥ 0 at Y5-Y10 (unit-test sanity).

#### Engine implementation (new; supersedes legacy ambassadors_d1.rs)

- **Config:** `Ad1Config` TOML — new `src/config/ad1.rs` (or `src/excel/ad1_config.rs`)
- **Data:** `Ad1Data` struct — new `src/spv/ad1.rs` (replaces `spv/ambassadors_d1.rs`)
- **Renderer:** `src/report/ad1.rs` (distinct layout from d3_wcp and d2_direct_hold)
- **CLI:** extend `spv-bencal` with `--ad1-config <toml>` flag; legacy 30%-scale path kept behind
  flag until ratified. When `--ad1-config` supplied, new `Ad1Data` path runs.
- **WCP source fields:** pull from `WcpData` — `wcp.book.book_value_per_share`,
  `wcp.market.market_value_per_share`, `wcp.fair_div.fair_value_per_share`,
  `wcp.fair_div.dividend_value_per_share`; do not hard-code arrays in TOML.
- **JSON `_derivation`:** `source_model = "wcp_42m"`, `method = "direct_equity_spv"`,
  `scale_factor = 0.030` (= 300,000 / 10,000,000 WCP shares; 150K purchased + 150K bonus)
- **Left-gutter row IDs** on all D5 output rows (legal-pleading-paper style, per §14)

#### ⚠ Open flags for Jennifer's decision

**Flag 1 — Cash reserve / going concern (RESOLVED 2026-05-24 — Option 1: reserve at close):**
Mechanism: raise additional equity equal to `required_reserve_at_close` (≈ $198,842 from OpexBudget
§3h — Y1–Y3 only; income onset at Y4 via WCP share sales); add to total offering on top of WCP costs.
WCP: 150,000 purchased at $20 + 150,000 bonus at $0.00033223 = 300,000 shares total (3.0%).
Investor shares, manager shares, and diluted shares derived from total offering ÷ $1.00.
Reserve is the ONLY cash on BS (WCP costs + reserve = total offering). Disclose in offering documents.

**Flag 2 — FVTPL IS basis (RESOLVED 2026-05-25):**
Book Value is the primary IS and NAV basis (Level 3 proxy pre-listing; switches to Level 1 market
price at WCP exchange listing — Phase B `wcp_listing_year: Option<u8>` config parameter).
Phase A: Level 3 Book Value throughout. Investors should note large Y1 FVTPL loss (cost $3,000,049.83
vs Y1 book value $1,365,000); recovers to profit from Y2 onward as book value exceeds purchase price.
Market/Fair/Dividend shown in summary Section E as supplemental scenario rows.

**Flag 3 — IFRS 2 vesting schedule (RESOLVED 2026-05-25 — no IFRS 2 charge):**
Bencal SPV1 is the buyer of WCP shares, not a WCP formation participant. No IFRS 2 service cost at Bencal SPV1 level.
Manager dilution is structural only — reflected in diluted share count and per-share metrics.
No share-based compensation line in IS, CF, or BCSC disclosure metadata.

**Flag 4 — FOFI truncation (RESOLVED 2026-05-25 — Y1-Y5 OM):**
Bencal SPV1 OM-facing FOFI = Y1-Y5. Capital Return Sale expected within this window at WCP listing.
Y6-Y10 not in investor FOFI; internal management view retains full Y1-Y10 for Residual Position.
`fofi_truncate_flag = true; fofi_years = 5` in BCSC metadata.

**Flag 5 — Structural reconciliation (RESOLVED 2026-05-25 — this brief is canonical):**
Bencal SPV1 holds 300,000 WCP shares (150K purchased at $20 + 150K founding bonus at $0.00033223 = $49.83).
Bencal SPV1 does NOT invest in PCLP1. `scale_factor = 0.030`; total cost basis $3,000,049.83.
Legacy `outputs/ambassadors-d1.{md,json,html}` (30%-scale, 3M WCP shares) is superseded.
Rust rewrite targets this spec; legacy 30%-scale path kept behind `--legacy` flag until ratified.

#### Consolidated statement title

> **Bencal Special Purpose 1 Inc.**
> **Statement of Financial Position and Share Valuation — Years 1–5 (FOFI) / 1–10 (internal)**
> Note 1: Manager Dilution — SPV-Manager holds ≈ 339,431 shares (≈ 10.00% fully diluted; total diluted
> ≈ 3,394,313 — exact pending TBD formation fees). Manager dilution is structural; no IFRS 2 charge.
> Per-share metrics on investor-share basis (÷ investor_shares) for primary; diluted basis noted where shown.
> Note 2: WCP holdings: 300,000 common shares (150K at $20 + 150K bonus at $0.00033223; cost $3,000,049.83).
> FVTPL (IFRS 9 Level 3 — Book Value proxy pre-listing; switches to Level 1 at WCP exchange listing).
> Investors should note expected Y1 FVTPL loss as Book Value recovers above purchase price.
> Note 3: Capital Return Sale: at WCP listing, sell sufficient WCP shares to return pro-rata investor
> capital; Residual Position retained at effective zero cost basis. Annual hold vote; 10-year listing cap.
> Note 4: Forward-looking information. Actual results may differ materially. Planned / intended / target
> language throughout — no assurance of values stated.

---

### 5f. D6 — SPV Manager: Bencal Management Corp.

**Source Excel:** None — derived from D4 Bencal SPV2-LP + D5 Bencal SPV1 Inc. outputs
**Specification basis:** 4-agent opus panel analysis, 2026-05-23 (see §19)

#### Structure

Bencal Management Corp. is the SPV-Manager vehicle. It holds the management stakes across
the two investor SPV vehicles. Bencal Management is a **CBCA limited liability corporation** (not an LP).

```
Bencal Management Corp.
├── ≈ 27,843 Bencal SPV2-LP units       (10% diluted; manager stake in the direct-hold feeder; v0.15.6)
├── ≈ 339,431 Bencal SPV1 Inc. shares   (10% diluted; manager stake in the WCP equity SPV; v0.15.6)
└── 1 Bencal SPV2-GP share          (nominal; GP shell equity — nil value)
```

#### Governance — Bencal Management Corp.

The corporation shall be governed by a board of two (2) directors: **Director A** and **Director B**. All board resolutions require unanimous approval.

**Corporate Secretary:** Director B shall serve as Corporate Secretary and sole authorized signatory and spokesperson. No other officers.

**Share capital (revised — commission-funded model, 2026-05-25):**
Share price = **$5.00/share** × 2 shares = **$10.00 total** (nominal).
Bencal Management's Y1–Y3 opex reserve is funded by commission income earned at close — not by share capital.
This supersedes the prior "share capital = opex reserve" mechanism (Flag 5 updated).

Bencal Management OpexBudget (source: COMPLIANCE_MCorp_2026_05_25_SPV Budget_JW1.xlsx — Tab Bencal):

*Setup / Formation (one-time at Y0 — expensed per IAS 38.69(d)):*

| Line item | Amount | ASM ID |
|---|---|---|
| Legal — R&R maintenance + annual + BC Annual Report | $4,950 | `asm.bencal.opex.legal_setup` |
| Accounting — KYC/AML setup | $275 | `asm.bencal.opex.acct_setup` |
| Bank — account setup | $350 | `asm.bencal.opex.bank_setup` |
| **Setup total** | **$5,575** | |

*Annual operating expenses (flat Y1–Y10):*

| Line item | Annual | ASM ID |
|---|---|---|
| Legal Services | $900 | `asm.bencal.opex.legal` |
| Review Engagement / Accounting | $2,399 | `asm.bencal.opex.review_engagement` |
| Board of Directors (2 × $1,093.34/qtr) | $8,747 | `asm.bencal.opex.board` |
| **Annual total** | **$12,046** | |

⚠ Source Excel formula error noted: D&O Officers subtotal cell sweeps Directors row (double-count). Engine uses $8,747/yr director fee (2 × $4,373.37/yr; 2% Work Fee constraint; Excel $24,000 superseded). Jennifer to fix source Excel.

*Reserve sizing (`funding_onset_year = 4` — Bencal SPV2 distributions to Bencal Management begin Y4):*
```
required_reserve = setup_total + annual × 3
                 = $5,575 + $12,046 × 3
                 = $5,575 + $36,138
                 = $41,713
```

Bencal Management Y1–Y3 opex reserve (funded by commission — see Flag 6 waterfall):
```
reserve_needed      = setup_total + annual × 3 = $41,713
share_price         = $5.00/share (nominal; 2 shares = $10 total share capital)
commission_retention = $41,713 (Bencal Management reserve retained from commission waterfall — see Flag 6)
```

No investor pool. No LP structure. Sole owner: the SPV-Manager individual(s).

#### Parameters

| Parameter | Value | Config key |
|---|---|---|
| Bencal Management shares outstanding | 2 | `shares.outstanding` |
| Bencal Management share price | $5.00 (nominal; reserve funded by commission, not share capital) | `shares.price_per_share` |
| Bencal SPV2-LP units held | ≈ 27,843 (v0.15.6) | `holdings.ad2_lp_units` |
| Bencal SPV2-LP total diluted units | ≈ 278,434 (v0.15.6) | `holdings.ad2_diluted_units` |
| Bencal SPV1 Inc. shares held | ≈ 339,431 (v0.15.6) | `holdings.ad1_shares` |
| Bencal SPV1 Inc. total diluted shares | ≈ 3,394,313 (v0.15.6) | `holdings.ad1_diluted_shares` |
| Bencal SPV1 WCP shares held (by Bencal SPV1) | 300,000 (150K purchased + 150K bonus) | `holdings.ad1_wcp_shares_held` |
| Bencal SPV1 cost basis | $3,000,049.83 ($3M purchased + $49.83 bonus) | `holdings.ad1_cost_basis` |
| Bencal SPV1 deferred tax rate | 13.5% | `holdings.ad1_deferred_tax_rate` |
| Bencal SPV2-GP share held | 1 | `holdings.ad2_inc_share` |
| Commission income (SPV-Manager advisory) | $0 by default (see Flag 6) | `income.commission_enabled` |
| Cash (at formation) | $41,713 (commission retention after dealer work fee + tax + reserve injections; = Y1–Y3 reserve, no surplus) | `cash.initial` |

**Bencal Management ownership fractions:**
- Bencal SPV2-LP: ≈ 27,843 / ≈ 278,434 ≈ 10.0000% — Bencal SPV2 is **dual-asset** as of v0.15.8 (LP units + 600,000 WCP founding-bonus shares; see §5d); Bencal Management's 10.0000% claim therefore proportionally captures Bencal SPV2's WCP holding via Bencal SPV2.nav_total
- Bencal SPV1 Inc.: ≈ 339,431 / ≈ 3,394,313 ≈ 10.0000%
- Bencal SPV2-GP: 1 share at **$10.00 nominal** (Flag 2 resolved 2026-06-02; see §5f Flag list)
- **Indirect WCP exposure at Bencal Management level:** 10.0% × Bencal SPV2's 600K WCP holding (60,000-share lookthrough) + 10.0% × Bencal SPV1's 300K (30,000-share lookthrough) = 90,000-share indirect WCP exposure (0.9% of WCP outstanding via 10% manager-of-manager dilution)

#### IFRS / accounting treatment

**IFRS 10.27 — Investment entity election (YES — recommended):** Bencal Management holds stakes in
Bencal SPV2-LP and Bencal SPV1 Inc. purely for capital appreciation/return measurement. It has no operational
staff, no direct property holding, no employees. Management reporting is performance-on-FV basis.
Meets IFRS 10.27 criteria; investment entity election exempts Bencal Management from consolidating Bencal SPV2/Bencal SPV1.
Measurements: all holdings at FVTPL. See §19a for full accounting agent analysis.

**If investment entity election NOT made:** Bencal Management must consolidate Bencal SPV2-LP (via Bencal SPV2-GP GP
interest) and Bencal SPV1 Inc. — complex consolidation requiring IC elimination of all intercompany flows.
Not recommended for an entity with $10 paid-in capital.

**Bencal SPV2-GP 1 share — Path B (nil value):** Bencal SPV2-GP is the GP shell, whose only asset is its
carried interest in Bencal SPV2-LP. At $30M fund scale, GP interest FV = management fee × multiplier
minus costs. The banking agent recommends booking at nil unless legal requires otherwise; an
annual review-engagement note disclosing "GP interest carried at cost ($1 nominal)" is sufficient.

**IFRS 9 — Holdings classification:** Both Bencal SPV2-LP units and Bencal SPV1 Inc. shares are equity instruments
held for capital appreciation → FVTPL mandatory (FVOCI would trap gains permanently per IFRS 9.B5.7.1).

**IAS 12 — Deferred tax on Bencal Management:**
- Bencal SPV2-LP units: distributions received are return-of-capital then capital gain at disposal.
  Unrealised FV gains → DTL at 13.5% (27% × 50% cap-gains inclusion).
- Bencal SPV1 shares: indirect WCP exposure. Bencal Management's DTL computed on its *own* carrying value of Bencal SPV1
  (not Bencal SPV1's internal DTL — no double-counting). Bencal Management books DTL on `max(FV_AD1 - cost_AD1, 0) × 0.135`.
- Bencal SPV2-GP share: nil → no DTL.

#### Portfolio composition (Y1–Y10)

```
BenCal_NAV[y] = FV_AD2_LP[y] + FV_AD1[y] + FV_AD2Inc[y]

FV_AD2_LP[y]  = (ad2_lp_units / ad2_diluted_units) × Bencal SPV2.nav_total[y]
              = (≈ 27,843 / ≈ 278,434) × Bencal SPV2.nav_total[y]   (≈ 10% of Bencal SPV2 NAV)

FV_AD1[y]     = (ad1_shares / ad1_diluted_shares) × Bencal SPV1.nav_book[y]
              = (≈ 339,431 / ≈ 3,394,313) × Bencal SPV1.nav_book[y]   (≈ 10% of Bencal SPV1 NAV)

FV_AD2Inc[y]  = 0   (nil; Path B)
```

#### Income Statement (Y1–Y10)

```
Net change in FV — Bencal SPV2-LP units    [FVTPL gain/loss on Bencal SPV2 NAV movements]
Net change in FV — Bencal SPV1 Inc. shares [FVTPL gain/loss on Bencal SPV1 NAV movements]
Net change in FV — Bencal SPV2-GP share  [nil]
Distributions received from Bencal SPV2-LP [return-of-capital; reduce cost basis; NOT income under FVTPL]
─────────────────────────────────────────────────────────────────
Total investment income (loss)
− Commission income (SPV-Manager fee — see Flag 6)
− Operating expenses (legal + review engagement + board + admin; see OpexBudget above)
─────────────────────────────────────────────────────────────────
Income (loss) before deferred tax
− Deferred tax expense (DTL change on FV gains)
─────────────────────────────────────────────────────────────────
Net income (loss)
```

**Distributions from Bencal SPV2-LP** reduce Bencal Management's cost basis in the Bencal SPV2-LP units (IFRS 9 treatment
for FVTPL equities — distributions that exceed cost are reclassified as income, but at this scale
cost basis ($0 — units received as compensation) → all distributions are income in Bencal Management's IS).
See Flag 7 for authoritative treatment confirmation.

#### Balance Sheet (Y1–Y10)

```
Assets
  Investment in Bencal SPV2-LP units (FVTPL)    [Bencal Management % of Bencal SPV2.nav_total]
  Investment in Bencal SPV1 Inc. shares (FVTPL)  [Bencal Management % of Bencal SPV1.nav_book]
  Investment in Bencal SPV2-GP (1 share)       [nil / $1 nominal]
  Cash and cash equivalents              [initial $10; + distributions received − opex]
  Deferred tax asset                     [unrecognised pending probable-recovery test]
Liabilities
  Deferred tax liability                 [max(cum_unrealised,0) × 13.5% on each holding]
  Shareholder loan payable               [if cash funding gap option chosen — see Flag 5]
Equity
  Share capital (2 shares × $5.00)         [$10.00 nominal]
  Retained earnings — commission income    [$41,713 at close after work fee + tax + reserve injections]
  Retained earnings / accumulated deficit
NAV per Bencal Management share = (Assets − Liabilities) / 2
```

#### Cash Flow Statement

```
Operating: distributions received from Bencal SPV2-LP + commission income (if any) − opex
Investing: $0 acquisition of Bencal SPV2-LP units (Y0 formation; cost basis = $0 IFRS 2 compensation)
Financing: share capital issuance (Y0) = $10.00 (nominal)
Operating (Y0): commission income $1,805,488 − sales fee $1,000,000 − dealer legal $30,000 − work fee $562,280 − tax $57,566 − Bencal SPV1 reserve injection $54,832 − Bencal SPV2 reserve injection $59,097 = $41,713 Bencal Management reserve retained
Net cash change[y] ≈ distributions_from_AD2 − opex[y]
```

#### Valuation Matrix (summary output — Block A-F)

Bencal Management has no separate "summary block" comparable to D2 AA12:AN35. The summary is the
portfolio composition + valuation matrix. Structure per finance agent spec (§19b):

**Block A — Portfolio composition (Y1–Y10):**
| Row | Label | Formula |
|---|---|---|
| A1 | Bencal SPV2-LP position (Bencal Management units) | ≈ 27,843 (constant; v0.15.6) |
| A2 | Bencal SPV2 NAV (10% share) | FV_AD2_LP[y] |
| A3 | Bencal SPV1 position (Bencal Management shares) | ≈ 339,431 (constant; v0.15.6) |
| A4 | Bencal SPV1 NAV (10% share) | FV_AD1[y] |
| A5 | Total Bencal Management NAV | A2 + A4 |
| A6 | Bencal SPV2 % of NAV | A2 / A5 |
| A7 | Bencal SPV1 % of NAV | A4 / A5 |

**Block B — Bencal SPV2-LP position (Y1–Y10):**
| Row | Label | Formula |
|---|---|---|
| B1 | Distributions received from Bencal SPV2 | Bencal SPV2.dpu[y] × ≈ 27,843 |
| B2 | Cumulative distributions received | Σ B1 |
| B3 | Yield on cost (units at $0 IFRS 2 basis) | undefined — show absolute $ |

**Block C — Bencal SPV1 Inc. position (Y1–Y10):**
| Row | Label | Formula |
|---|---|---|
| C1 | Implied WCP exposure (effective shares) | 300,000 × 10% = 30,000 WCP shares |
| C2 | Bencal SPV1 NAV (10% share) | FV_AD1[y] |
| C3 | Wrapper drag vs WCP direct (10% stake) | `30,000 × wcp.book[y] − FV_AD1[y]` |

**Block D — Valuation Matrix (4 sub-tables, Y1–Y10):**
For each method [Book, Market, Fair, Dividend]:
| Row | Label | Formula |
|---|---|---|
| D.n.1 | Bencal SPV2-LP NAV (10%) | as above |
| D.n.2 | Bencal SPV1 NAV (10%, method-n) | 10% × Bencal SPV1.nav_[method][y] |
| D.n.3 | Bencal SPV2-GP | $0 |
| D.n.4 | Total Bencal Management NAV | D.n.1 + D.n.2 |
| D.n.5 | NAV per Bencal Management share | D.n.4 / 2 |
| D.n.6 | MOIC | D.n.4 / total_equity_at_formation (= $10 share capital + $41,713 commission retention = $41,723) |

**Block D.V — Comparative ratios (Y1–Y10):**
| Row | Label |
|---|---|
| DV.1 | Bencal SPV2/Bencal SPV1 NAV ratio |
| DV.2 | MOIC Book |
| DV.3 | MOIC Market |
| DV.4 | MOIC CAGR (Book, Y8) |

**Block E — Financial Forecast (Y1–Y10):**
Rows E.1–E.21 mirror the PCLP1/Bencal SPV2 summary pattern:
E.1 = Bencal SPV2 DPU received; E.2 = Bencal SPV2 distributions to Bencal Management; E.3 = Bencal SPV1 FV change;
E.4 = Bencal Management total investment income; E.5 = opex; E.6 = EBT; E.7 = deferred tax;
E.8 = net income; E.9 = Bencal Management NAV; E.10 = NAV/share; E.11 = MOIC cumulative;
E.12 = cash; E.13 = DTL; E.14–E.21 = per-method NAV comparisons.

**Block F — MOIC headline card (Y10; Flag 3 + Block F decision RESOLVED 2026-06-02):**

Block F renders **side-by-side per-share AND aggregate MOIC columns** plus a header note
explaining the 10/90 manager/investor dilution mechanics so per-share figures are interpretable
to LP investors. Both views ship in CIM materials. Engine output struct adds:

```rust
struct BlockF {
    moic_aggregate: Decimal,      // total portfolio NAV / total invested capital
    moic_per_share: Decimal,      // per-share NAV / per-share cost basis
    cagr: Decimal,                // realised CAGR over the hold period
    portfolio_nav_total: Decimal,
    portfolio_nav_per_share: Decimal,
}
```

Header note text (rendered above the table in HTML/MD/PDF):

> The per-share MOIC reflects the manager's $5.00 share-capital basis at Bencal Management.
> Because Bencal Management's two shares carry $10 of paid-in capital while the manager's
> 10% allocation at Bencal SPV1 + Bencal SPV2 carries economic claims on a much larger NAV,
> the per-share MOIC is mechanically very high and should be read alongside the aggregate
> MOIC, which reflects total invested capital across all Bencal entities. The 10/90
> manager/investor dilution at SPV1 and SPV2 is described in §5d–§5e.

This supersedes the earlier "do not lead with per-share" guidance — Flag 3 lock requires
both views to be shown in investor-facing materials, with the header note providing
interpretive context.

#### Engine implementation (rewrite of existing bencal.rs)

The existing `src/spv/bencal.rs` uses incorrect constants (see §19c for full diff). Required changes:

```rust
// New constants block (replaces entire old const block):
// Updated v0.15.6 — director fee $4,373.37/yr/director; 2% Work Fee constraint
const BENCAL_AD2_UNITS: f64      = 27_843.0;     // ROUND(250,591/9,0); reserve=$59,097; total_equity=$25,059,097
const AD2_DILUTED_UNITS: f64     = 278_434.0;    // 250,591 investor + 27,843 manager
const BENCAL_AD1_SHARES: f64     = 339_431.0;    // ROUND(3,054,882/9,0); reserve=$54,832; offering=$3,054,882
const AD1_DILUTED_SHARES: f64    = 3_394_313.0;  // 3,054,882 investor + 339,431 manager
const AD1_WCP_SHARES_HELD: f64   = 300_000.0;    // 150K purchased at $20 + 150K bonus
const AD1_COST_BASIS: f64        = 3_000_049.83; // $3,000,000 purchased + $49.83 bonus
const AD1_DEFERRED_TAX_RATE: f64 = 0.135;        // 27% × 50% cap-gains inclusion
const BENCAL_SHARES_OUTSTANDING: f64 = 2.0;
const BENCAL_PRICE_PER_SHARE: f64 = 5.0;          // nominal $5/share; reserve funded by commission retention
const BENCAL_COMMISSION_RETENTION: f64 = 41_713.0; // Bencal Management reserve after dealer work fee $562,280 + tax $57,566 + Bencal SPV1/Bencal SPV2 injections

// Remove: BENCAL_AD1_STAKE, AD1_WCP_STAKE, COMMISSION_PER_YEAR (old)
```

Bencal SPV1 inline helpers (interim until Ad1Data struct exists):
```rust
fn ad1_book_nav_per_diluted_share(wcp: &WcpData, y: usize) -> f64 {
    let book_per_wcp_share = wcp.book.book_value_per_share[y];
    let asset_value = AD1_WCP_SHARES_HELD * book_per_wcp_share;
    let dtl = (asset_value - AD1_COST_BASIS).max(0.0) * AD1_DEFERRED_TAX_RATE;
    let cum_opex = 5_575.0 + (y as f64 * 16_419.0).max(0.0);  // Y0: setup=$5,575; Y1+: $16,419/yr flat
    (asset_value - cum_opex - dtl) / AD1_DILUTED_SHARES
}
```

Revenue Generator: reduce from current 7-row output to **2 rows** (Bencal SPV2-LP + Bencal SPV1 Inc.).
All per-share fields populated (was `[0.0; 10]`). `price_per_share = 5.00` (was 0.0).

#### Jennifer decisions — all flags resolved (final sweep 2026-06-02)

**Flag 1 ✓ — Investment entity election (IFRS 10.27 — RESOLVED 2026-05-25 — YES):**
Election confirmed YES. Bencal Management meets all three criteria: (1) obtains funds to provide investment
management services; (2) business purpose = capital appreciation on Bencal SPV2-LP + Bencal SPV1 Inc. holdings;
(3) measures performance on fair-value basis. Bencal SPV2-LP units and Bencal SPV1 shares both measured at FVTPL;
Bencal SPV2-LP is NOT consolidated despite GP control via Bencal SPV2-GP (IFRS 10.32 does not apply — Bencal SPV2-GP
provides no services). Accounting-policy note required at incorporation documenting the election
and the three criteria met.

**Flag 2 ✓ — Bencal SPV2-GP 1 share: $10.00 nominal (RESOLVED 2026-06-02):**
GP share issued for **$10.00 nominal consideration** (overrides nil recommendation). Small GP capital account
($10.00) tracked through LP Agreement distribution/dissolution allocations. Bencal SPV2-GP balance sheet
remains empty other than this nominal equity; no operating liabilities. Update Note 3 of §5f consolidated
statement title accordingly.

**Flag 3 ✓ — MOIC optics: show BOTH per-share and aggregate (RESOLVED 2026-06-02):**
Block F renders BOTH per-share AND aggregate MOIC in investor materials (overrides suppress recommendation).
CIM must carry supplementary text explaining manager/investor 10/90 dilution mechanics when interpreting
per-share figures. Engine `Block F` renderer must emit both views; documentation explains the distinction.

**Flag 4 ✓ — DTL computation: Option (a) Bencal Management-level (RESOLVED 2026-06-02):**
DTL computed at Bencal Management Corp. level on its own commission income (27%) and on FVTPL gains realised
at Bencal Management when SPV1/SPV2 distribute. Consistent with the IFRS 10.27 investment-entity election —
SPV1/SPV2 are FVTPL investments, not consolidated subsidiaries. No look-through to underlying WCP / PCLP1
holdings. Simpler IS presentation; clean DTL roll-forward.

**Flag 5 — Cash funding gap (RESOLVED v0.15.4 — commission-funded reserve; nominal share capital):**
Commission income at close funds all three entity reserves + dealer work fee. Share price = **$5.00/share**
(nominal; total share capital = $10.00). After dealer legal ($30K), work fee ($562,280), and corporate
tax ($57,566), Bencal Management retains $41,713 — exactly covers Y1–Y3 opex reserve (no surplus). No shareholder
loan needed. Reserve amounts (v0.15.6): Bencal Management $41,713; Bencal SPV1 $54,832; Bencal SPV2 $59,097. Director fee: $4,373.37/yr/director.

**Flag 6 — Commission waterfall (updated v0.15.4, 2026-05-26 — all reserves from commission):**
Bencal Management earns commissions at close from two events:
- WCP close (Bencal SPV1): **10% × $3,054,882 Bencal SPV1 offering** = **$305,488** → Work Fee = Dealer fee
- PCLP1 close (Bencal SPV2): **6% × $25,000,000** = **$1,500,000** → Sales Fee = Agents fee (4% × $25M)

**Commission waterfall (27% corporate tax; all three entity reserves funded from commission):**

| Step | Item | Amount |
|---|---|---|
| 1 | WCP gross commission (10% × $3,054,882) | +$305,488 |
| 2 | PCLP1 gross commission (6% × $25,000,000) | +$1,500,000 |
| 3 | **Total gross commission** | **$1,805,488** |
| 4 | Less: Sales fee to Agents (4% × $25M PCLP1) | −$1,000,000 |
| 5 | **Net commission to Bencal Management** | **$805,488** |
| 6 | Less: Dealer legal expense (pre-tax deduction) | −$30,000 |
| 7 | Less: Work Fee (Dealer) | −**$562,280** |
| 8 | Less: Corporate tax (27% × $213,208) | −$57,566 |
| 9 | Less: Bencal SPV1 reserve injection (Bencal Management → Bencal SPV1) | −$54,832 |
| 10 | Less: Bencal SPV2 reserve injection (Bencal Management → Bencal SPV2) | −$59,097 |
| 11 | Less: Bencal Management reserve retention | −$41,713 |
| 12 | **Remaining balance** | **$0** |

**Work Fee (Dealer) = $562,280 = 2.00% of total investor subscriptions** ($28,113,979).
All three entity reserves are funded from Bencal Management's commission — NOT from investor offering gross-up.

**Algebraic derivation (27% tax; dealer legal pre-tax deductible):**
```
Net commission = $805,488
Work Fee (target 2.00%): w = 2% × $28,113,979 = $562,280
Taxable income = net_commission − dealer_legal − work_fee = $775,488 − $562,280 = $213,208
Tax = 0.27 × $213,208 = $57,566
Verification: $805,488 − $30,000 − $562,280 − $57,566 − $54,832 − $59,097 − $41,713 = $0 ✓
```

**Terminology:**
- **Work Fee** = Dealer fee (Bencal Management's arrangement/structuring fee; residual after all deductions)
- **Sales Fee** = Agents fee (4% × PCLP1 investment; paid to placement agents; separate from Bencal Management)

**Flag 6 ✓ — Work Fee recipient: Altas One Digital Securities Inc. keeps the Work Fee (RESOLVED 2026-06-02).**
The registered EMD retains the $562,280 Work Fee as compensation for dealer services. Cleanest NI 31-103 path —
the dealer that performs the work is paid for it; avoids related-party complications in CIM §D and IAS 24.17
enhanced disclosure. 27% assumes general corporate rate; SBD allocation may reduce to 11% — see §26 finance agent
§2.2.1. Build at Phase A-D6.

**Flag 7 ✓ — Distributions from Bencal SPV2 to Bencal Management treated as income, cost basis $0 (RESOLVED 2026-06-02):**
Bencal SPV2-LP units carried at FVTPL on Bencal Management's balance sheet (IFRS 9). Cash distributions hit the IS
as investment income — not return-of-capital. Original cost basis (nominal $0 paid when received as 10% manager
allocation) remains at $0. Standard FVTPL treatment under IFRS 9. Same treatment applied to Bencal SPV1 share
dividends. Confirmed.

**Flag 8 ✓ — Management Services Agreement: embedded in LP opex, no separate fee line (RESOLVED 2026-06-02):**
GP services compensated through the existing LP opex line ($16,664/yr = $1,145 legal + $2,399 accounting +
$13,120 board fees). No standalone management fee to GP. LP investors see one annual opex number, not a
separate management fee. Standard for small SPV structures where the GP performs no substantive ongoing
investment management beyond board duties.

**Flag 9 ✓ — FOFI scope: REVERSED 2026-06-02 → publish Y1–Y3 ONLY (overrides earlier same-day decision to publish Y1–Y5):**
Bencal Management Y1–Y3 FOFI published in CIM (covers commission income period). Y4–Y5
NOT published. Rationale: avoid speculative-language disclosure issues — Y4–Y5 manager
economics depend on WCP listing timing + PCLP1 distribution onset, both of which are too
uncertain to forecast with regulatorily-acceptable precision. Engine BM forecast remains at
Y1–Y3 (no struct extension needed). Saves engine work; reduces CIM regulatory surface.

**Flag 10 ✓ — Transfer restrictions / manager-change event: majority-LP consent right (RESOLVED 2026-06-02):**
LP Agreement to include a manager-change event clause requiring affirmative consent of LPs holding >50% of
investor units. Triggers on change of Bencal SPV2-GP OR change of control of Bencal Management Corp. Standard
LP minority-protection clause; parallels the existing Benetti escrow/transfer restriction already in PCLP 1
LPA §2.3 (investors receive 100% of initial investment before transfer). Low overhead; high investor comfort.

**Flag 11 ✓ — Assurance engagement: CSRE 2400 review engagement (RESOLVED 2026-06-02):**
Bencal Management Y1–Y3 financials prepared under CSRE 2400 — Engagements to Review Historical Financial
Statements. Lower cost than full CAS audit. Appropriate given (a) Bencal Management is small; (b) SPV1/SPV2
carried at FVTPL; (c) no public listing yet. Same firm reviews SPV1, SPV2, and Bencal Management together.
Audit may be triggered later by CIM-disclosed listing milestone or covenant requirements.

**Flag 12 ✓ — Reserve sizing: 3 years (RESOLVED 2026-06-02):**
Y1–Y3 opex reserves: SPV1 $54,832; SPV2 $59,097; Bencal Management $41,713; total $155,642. Matches the
Y4 income onset (WCP share-sales window opens; PCLP1 distributions material). 4 years would over-reserve
by one year and consume ~$47K additional commission, reducing the Work Fee headroom. 3-year sizing
remains baked into the waterfall and v9 SPV operating budget.

**Flag 13 ✓ — Reserve injection mechanism: direct commission rebates from Altas One (RESOLVED 2026-06-02 — supersedes Option A):**
Altas One distributes commission rebates DIRECTLY to each Bencal entity (SPV1 $86,364 gross / $63,046 net;
SPV2 $92,207 gross / $67,311 net; BM Corp $64,637 gross / $47,185 net). Already implemented in v9 of the
SPV operating budget. Manager diluted % stays exactly 10.0000% at both SPV1 and SPV2 — no Option A
subscription dilution. Note: §C waterfall steps 9–11 retain pre-tax labels showing Bencal Management
"injecting" Y1–Y3 reserves; under v9 the mechanism is direct rebate, not subscription.

**Flag 14 ✓ — Setup costs Y0 treatment: IAS 38.69(d) expense at Y0 (RESOLVED 2026-06-02):**
Formation legal, accounting, and banking costs expensed at Y0 as operating loss. Mandatory under IAS 38.69(d) —
formation costs are explicitly excluded from intangible-asset capitalisation. Y0 IS shows a clean operating-loss
line equal to setup costs ($5,575 each at SPV1 + Bencal Management; $9,105 at SPV2), before Y1 drawdown
commences. Standard treatment; no auditor pushback expected.

#### Consolidated statement title

> **Bencal Management Corp.**
> **Statement of Financial Position and Portfolio Valuation — Years 1–10**
> Investments carried at FVTPL. Investment entity election per IFRS 10.27.
> Note 1: Bencal SPV2-LP — Bencal Management holds ≈ 27,843 units (10.00% diluted; v0.15.6) of Bencal SPV2-LP.
> Note 2: Bencal SPV1 Inc. — Bencal Management holds ≈ 339,431 shares (10.00% diluted; v0.15.6) of Bencal Special Purpose 1 Inc.
> Note 3: Bencal SPV2-GP — Bencal Management holds 1 common share (nominal $10.00; carried at $10.00 in capital account; Flag 2 resolved 2026-06-02).
> Note 4: These projections constitute forward-looking information. Actual results may differ
> materially. Planned / intended / target language throughout — no assurance of values stated.

---

### 5g. Journal entry + trial balance

Research consolidated in §20 for review. See §20 for CoA, GL account tagging, JE mappings
(D2–D6), TB format, bidirectionality constraints, IFRS gaps, competitive moat, and 4-agent
panel findings. Pending Jennifer's review and ratification before promotion to implementation
spec.

---

### 5h. D7 — Legacy JV (Traditional Joint Venture) — illustrative comparator

**Portfolio derivation:** D7 derives its IS/BS/CF by aggregating `Vec<BuildingOutput>` from
D1 class computations, applying D7's class mix (configurable via `JvConfig`; default mirrors
D2 at 40/30/20/10 for apples-to-apples comparison; override allowed to model land-constraint
or JV-mandate scenarios), then applying vehicle-level adjustments (construction loan →
permanent loan transition, JV partner capital accounts, ASPE 3061 cost model). The
`PortfolioVehicle` trait governs this interface.

**Source:** `DUE DILIGENCE_MCorp_Tear Sheet_Alternative Real Estate_FIN.xlsx` (V3, 2026-01-06)
**Subtitle:** "Traditional J/V Financing vs. the Woodfine LPs"
**Purpose:** Apples-to-apples 10-year comparison against D2 (PCLP 1 Direct-Hold).

### Capital structure (from tear sheet)

| Parameter | D7 Legacy JV | D2 PCLP 1 (comparison) |
|---|---|---|
| Equity contributions | $250,000,000 | $250,000,000 |
| Bank debt (construction / permanent) | $750,000,000 | $1,025,000,000 |
| Debt-to-equity ratio | 3.0× | 4.1× |
| Total capital deployed | $1,000,000,000 | $1,275,000,000 |
| Construction cost/sf | $326.35 | $326.35 |
| Total portfolio sf | 2,298,150 sf | 3,906,855 sf |
| Development yield on total debt | 10.5% | — |
| NOI at stabilization | $78,750,000 | — |
| Cap rate | 6.25% | 6.25% |
| Stabilized asset value | $1,260,000,000 | ~$2,041,000,000 (Y10) |
| LTV at permanent financing | 59.52% | variable (phased) |

### Legal structure and reporting tier

**Structure:** Limited Partnership (BC). Nominee GP holds bare legal title (PTT exemption);
LP equity partners hold LP units. LP agreement governs distributions, capital accounts,
promote, and transfer restrictions.

**Reporting tier:** `PrivateASPE` — banks prefer ASPE 3061 cost model; no IFRS FV swings
on covenanted real estate. D7 is not a reporting issuer. ASPE 3061 depreciation: 50-year
straight-line on building component.

**IFRS 11 at investor level:** each equity partner records their D7 interest using the
equity method (joint venture classification per IFRS 11) in their own financial statements.
D7 itself presents gross LP-level statements.

### 10-year timeline

| Phase | Years | Description |
|---|---|---|
| Construction | Y1–Y3 | $1,000M capital deployed; S-curve draw (Y1: 20%, Y2: 50%, Y3: 30%); equity-last |
| Stabilization | Y4 | Construction loan → $750M permanent loan; full NOI begins; distributions start |
| Operating (flat) | Y4–Y10 | No new development rounds; 59.5% LTV leaves only $69M refinancing headroom |

**Single-shot constraint (quantitative proof):**
- Stabilized asset value: $1,260M
- Max permanent debt at 65% LTV covenant: $1,260M × 65% = $819M
- Existing debt: $750M
- **Refinancing headroom: $69M** (6.9% of a second $1B round — structurally insufficient)
- New round required debt: ~$750M new construction loan, new take-out commitment, new equity
- CMHC MLI+: residential-only — not applicable to commercial/industrial portfolio

### Portfolio composition (from D1 development classes)

The 2,298,150 sf is allocated across the 4 D1 development classes using a fixed mix.
This is a TOML input (not derived) — the operator sets the allocation at configuration time.

**Default mix (Jennifer decision flag D7-3 to confirm):**
| Class | Allocation % | SF | Building count | Approx buildings |
|---|---|---|---|---|
| Professional Centres | 40% | 919,260 sf | 63,000 sf/bldg | 15 buildings |
| Suburban Office | 30% | 689,445 sf | 76,000 sf/bldg avg | 9 buildings |
| Tech Industrial | 20% | 459,630 sf | 7,800 sf/pair | 30 pairs (59 bldgs) |
| Retail Select | 10% | 229,815 sf | 6,300 sf avg | 37 buildings |
| **Total** | **100%** | **2,298,150 sf** | | **~100+ buildings** |

Note: "geometric allocation" in Jennifer's framing means using the same D1 class mix
as the Woodfine LP portfolio — this is a fixed proportional mix at single-round deployment,
not a time-series compounding series.

### Income statement structure (Y4–Y10 stabilized, ASPE)

| Line | Amount | Derivation |
|---|---|---|
| Gross rental revenue | $78,750,000 | $750M debt × 10.5% development yield |
| Operating expenses (~20% of gross) | ($15,750,000) | CAM + management + insurance + taxes |
| Net Operating Income | $63,000,000 | — |
| Interest on $750M @ 5% | ($37,500,000) | Permanent loan |
| Depreciation (50-yr SL on $1,055M building cost) | ($21,100,000) | ASPE 3061 |
| G&A | ($2,000,000) | — |
| **Net income** | **~$2,400,000** | — |
| Add back depreciation | $21,100,000 | Non-cash |
| **Distributable cash** | **~$23,500,000/yr** | ~9.4% cash-on-cash on $250M equity |

Note: NOI figure ($78.75M) in the tear sheet is gross of operating expenses applied to the
10.5% development yield on total debt. Engine must compute operating expenses from the D1
CAM parameters applied to 2,298,150 sf.

### Partners' Capital Account (BS equity section)

ASPE uses Partners' Capital Accounts (per-partner ledgers), not Share Capital:
```
Opening Capital | + Contributions | − Distributions | + Allocated Net Income | = Closing Capital
```

### 10-year financial statement summary (illustrative)

| Metric | Y3 (end construction) | Y4 | Y7 | Y10 |
|---|---|---|---|---|
| Total assets (investment property) | $1,055M (at cost) | $1,260M (if IFRS FV) / $1,034M (ASPE net) | ASPE: $971M | ASPE: ~$897M |
| Construction/permanent loan | $750M | $750M | ~$706M | ~$648M |
| Partners' capital (ASPE) | $250M | $255M | $270M | $296M |
| NOI | — | $63M | $63M | $63M |
| Distributable cash | — | ~$23.5M | ~$23.5M | ~$23.5M |
| Cumulative distributions (from Y4) | — | $23.5M | $94M | $164M |
| DSCR | — | 2.10× | 2.10× | 2.10× |
| LTV (debt / asset book) | 71.1% | 59.5% | ~62.9% | ~66.3% (rising on ASPE) |

### D7 vs D2 — headline comparison metrics

| Metric | D7 Legacy JV (Y10) | D2 PCLP 1 (Y10) | D2 advantage |
|---|---|---|---|
| Total sf delivered | 2,298,150 | 3,906,855 | +70% |
| Total development capital | $1,000M | $1,275M | — |
| Equity in | $250M | $250M | same |
| Stabilized asset value | $1,260M | ~$2,041M | +62% |
| Equity value (asset − debt) | $510M | ~$1,016M | +2× |
| **MOIC (pre-tax, gross)** | **2.04×** | **~4.06×** | **+2×** |
| Cumulative distributions Y4–Y10 | ~$164M | ~$320–360M | +2× |
| Sf per $1 initial equity (10-yr) | 9.19 sf/$ | 15.63 sf/$ | +70% |
| Refinancing headroom at stabilization | $69M | phased / structural | structural advantage |
| Continuous development rounds possible? | No (single-shot) | Yes (3 tranches) | compounding |

Note: tear sheet Row 13 (108.78 vs 63.99 sf/$equity) measures single-round leverage
efficiency, not 10-year compounded output. The corrected 10-year metric is 9.19 vs 15.63
sf/$ equity — D2 is 70% more capital-efficient over the full period.

### Comparison output label

Per NI 52-107: the term "pro forma" is restricted to defined GAAP-compliant constructions.
All D7 comparison outputs must be labelled **"illustrative comparison"** (not "pro forma")
and carry BCSC forward-looking-information safe-harbour language.

### Jennifer decisions — §5h (resolved 2026-06-02)

- **Flag D7-1 ✓ — IFRS 11 + IFRS 9 fair-value model** (OVERRIDES ASPE 3061 recommendation).
  Rationale: D7 must be compared apples-to-apples against D2/D3 direct-hold solutions, which are
  Reporting Issuers and IFRS-mandated under NI 52-107. ASPE would create a framework mismatch
  in the comparison output. D7 is reported under IFRS 11 (joint arrangement) with the financial
  asset measured at FVTPL/FVOCI per IFRS 9. Quarterly fair-value re-measurement burden accepted.
- **Flag D7-2 ✓ — Construction draw S-curve: 20 / 50 / 30** (Y1 / Y2 / Y3). Typical mid-size
  commercial profile: Y1 site prep + foundations + permitting; Y2 superstructure + envelope + MEP;
  Y3 finishing + commissioning + occupancy. Per-deal override remains available via DevClassConfig TOML.
- **Flag D7-3 ✓ — Portfolio class mix locked against WMC tear-sheet matrix**. D7 modelling uses
  the building-count + SF matrix straight from the WMC Building Portfolio: 15 Professional /
  9 Suburban Office / 59 Tech Industrial (30 pairs) / 37 Retail Select buildings; 919,260 sf /
  689,445 sf / 459,630 sf / 229,815 sf = 2,298,150 sf total. Source: `DUE DILIGENCE_MCorp_Tear
  Sheet_Alternative Real Estate_FIN.xlsx` V3 (2026-01-06). The 40 / 30 / 20 / 10 percentages
  emerge from this geometric matrix rather than being inputs themselves. `JvConfig` TOML in
  tool-proforma must reference this canonical matrix; apples-to-apples comparison with D2/D3
  direct-hold (same WMC portfolio reference) is preserved.
- **Flag D7-4 ✓ — NOI basis CORRECTED: $78.75M is NET development yield (~10.5%)**, already
  net of building-level CAM/taxes/operating costs (tenant pass-through). Engine MUST NOT apply
  D1 CAM opex on top — that would double-deduct what tenants already pay. Engine may still
  apply portfolio-level overhead (asset mgmt, audit, governance). Dual-NOI architecture
  clarified: "gross NOI" = tenant payment (base rent + CAM recovery); "net NOI" = base rent −
  non-recoverable opex; **$78.75M sits at the net-NOI line**. Update DevClassConfig and
  JvConfig accordingly.
- **Flag D7-5 ✓ — Comparison output label: "illustrative comparison"**. NI 52-107 reserves
  "pro forma" for specific accounting contexts (e.g., business-combination adjustments); D7
  comparison does not qualify. All D7 comparison outputs labelled **"illustrative comparison"**
  with BCSC forward-looking-information safe-harbour language.

---

## 6. CAM operating cost parameters (Woodfine defaults)

Source: `DUE DILIGENCE_TitleCo 3_...FIN.xlsx` tabs `Test Site_CAM` + `Test Site_Underground_CAM`.

| Expense | Rate type | Rate | Notes |
|---|---|---|---|
| In-house property manager | flat | $75,000/yr | per building regardless of size |
| Common area maintenance | per sqft leasable | $5.50 | |
| HVAC | per sqft leasable | $1.50 | |
| Insurance | per sqft leasable | $0.75 | |
| Local accountant | flat | $15,000/yr | |
| Local lawyer | flat | $15,000/yr | |
| Property taxes | per sqft leasable | $5.00 | |
| Management fee | % gross rent | 3.0% | |
| Vacancy & bad debt | % gross rent | 3.0% | |
| Structural maintenance | % gross rent | 1.0% | |

Non-recovery cost rate: **5.5%** applied to gross rent per area type.
Cap rate (investment valuation): **6.25%**
Pylon sign income: **$12,000/yr** (4 panels × $250/month)

These are Woodfine-specific defaults. In the generic engine they live in `DevClassConfig`
(TOML file, tenant-supplied). Generic engine has no hardcoded rates.

---

## 7. Generic engine vs. deployment boundary

**Rule (software architect panel):** if removing the customer doesn't change a single line
in the engine crate, the boundary is correct.

**Generic core** (`vendor/pointsav-monorepo/tool-proforma-engine`):
- Computation primitives: DCF, waterfall, NOI/NAV/IRR math, schedule generation, formatters
- Takes a `Scenario` struct in; returns `ProformaOutput`
- Zero customer names, zero hardcoded rates, zero asset-class assumptions

**Woodfine deployment config** (`inputs/dev-class-config.toml`):
- Class definitions (floor counts, sqft, names)
- CAM parameters (9 expense lines + rates)
- Cap rate, construction rate, LTC, debt rate
- Ancillary income definitions
- Entity names and report headers

**SaaS tenant config** (future, `tenant_config` Postgres table):
- Same schema as TOML file, stored per-tenant in the database

**Current gap:** `d1_dev_classes.rs` has hardcoded constants (`LTV_ON_COST`, `DEBT_RATE`,
`OPEX_RATIO`, `DEV_CLASSES` array). These must move to `DevClassConfig` before launch.

---

## 8. Sensitivity analysis (phased implementation)

Phase C only — not blocking D1 delivery.

| Technique | Priority | Notes |
|---|---|---|
| OAT tornado chart | Must-have | ±10%/±20% on each driver; IC memo standard |
| 2-variable grid | Must-have | cap rate × rent growth; LTV × DSCR heatmap |
| Monte Carlo (Sobol) | Phase C | 10k paths, fixed seed; P5/P50/P95 + CVaR |

Key sensitivity dimensions: exit cap rate, rent growth, vacancy, lease-up velocity,
construction cost overrun, interest rate stress (+200 bps), TI/LC at rollover,
opex inflation vs. rent inflation spread.

---

## 9. Output format requirements

**Canonical artifact: `snapshot.json`**
Every engine run produces a calculation snapshot JSON as the primary output. HTML, Markdown,
and PDF are renderers that read only the snapshot — they have zero access to engine internals.
This means: (a) the snapshot is version-controlled and replayable; (b) any renderer can be
swapped without re-running the engine; (c) old snapshots remain renderable with new HTML
renderers so long as `schema_version` compatibility is maintained.

**Two canonical reference formats (both must be reproduced exactly):**

**Format A — Operational HTML summary** (`d1-dev-classes_sample.html`):
Internal/operational use (D1 classes; internal dashboards). Structure per entity/class:
valuation metrics summary row FIRST (Leasable Area | GFA | Dev Cost | Stab. NOI | Dev Yield |
Cap Rate | Stab. Asset Value), then IS → BS → CF. Numbers: `fmt_smart` (28.51M, 0.50M);
column headers: Y1–Y10; parenthetical negatives: (0.35M); em-dash for zero.

**Format B — Formal EY financial forecast** (`CORPORATE_Woodfine LPs_Forecast_Financial_10 Year.pdf`):
Investor-facing FOFI for D2/D3 (reporting issuers); variant used for D4/D5/D6 (private);
illustrative-comparison variant for D7. This format was designed by Ernst & Young and the
language/structure is fixed. Statement order (IAS 1): BS → IS → SCE → CF → Notes 1–10.
Document section order: Cover → EY compilation report → Management Forecast Summary
(non-GAAP valuation metrics, one page) → AA12:AM35 Financial Forecast Summary →
BS → IS → SCE → CF → Notes 1–10. Numbers: full dollars with commas (229,050,000); no M/K
abbreviation; parenthetical negatives (48,727,700); zero shown as `-`; ratios to two decimals.
Column headers: "Projected 1" through "Projected 10" with "$" currency sub-row (not Y1–Y10).
Page header on every page: `{entity_name_full}` bold | `{statement_title}` | `10-Year
Financial Forecast` bold | `(Expressed in Canadian Dollars)` bold. Cover uses lowercase
`Canadian dollars`; interior pages use `Canadian Dollars` — both preserved verbatim (EY quirk).

**EY note structure (fixed by EY; reproduced exactly; language is CSRS 4250 standard prose):**
| Note | Title | Type |
|---|---|---|
| 1 | Purpose of the financial forecast | F — fixed CSRS 4250 boilerplate + P entity name |
| 2 | Nature of operations | P — entity name, jurisdiction, LPA date, product-class descriptions |
| 3 | Significant accounting policies: (a) Basis of presentation; (b) Cash; (c) Share-based payments; (d) Investment properties; (e) Obligations | F + P |
| 4 | Long-term debt | C — computed from debt schedule; Series A/B/C/D table + ratios |
| 5 | Partners' units | P — authorized unit count |
| 6 | Revenue | P — dev yield 10.5%; construction cost basis |
| 7 | Operating costs | P — advisory 1%, referral 6%, issue costs 1% |
| 8 | Construction cost | P — $/sqft average |
| 9 | Distributions | P — payout ratio, distribution waterfall |
| 10 | Other key assumptions | F+P — contingencies; interest calc 5%; interest income 0.50% |

Type: F = fixed prose (CSRS 4250 standard); P = entity-parameter substitution; C = computed
from snapshot. Note 1 closing disclaimer ("Actual results achieved may be significantly
different…") is F and mandatory — never omitted. D7 requires a distinct Note 1 variant
("financial illustration … not a financial forecast … not an offering") — see §25d.

**Three render profiles (one `tool-proforma-render` binary, `--format` flag):**
| Profile | Entities | Notes depth | Compilation report |
|---|---|---|---|
| `internal-operating` | D1 | Methodology only | No |
| `private-forecast` | D4/D5/D6/D7 | Notes 1–6 (abbreviated) | Optional |
| `issuer-fofi` | D2/D3 | Notes 1–10 (full EY) | EY or successor |

Notes templates stored in `templates/notes/<entity_slug>/<lang>.toml`; prose is not hardcoded
in Rust source (bilingual hygiene; EY review-cycle round-tripping).

**`cover.variant` (regulatory):**
`management-prepared | compilation | examination` — engine must NOT emit EY attribution in
`management-prepared` mode. D7 always uses `management-prepared` with illustrative disclaimer.

**Snapshot hash on PDF cover** — regulatory-mandatory for BCSec Act s.85 examination readiness
(see §25d). One line in colophon; enables deterministic recomputation proof.

**Every output carries:**
- Left-gutter legal-pleading row IDs (existing convention, universal across all 10 items)
- Report header: entity name, date, version
- Option label (e.g., "OPTION 1: TEST SITE NO UNDERGROUND PARKING")

**Output files per run:**
- `<run_id>.snapshot.json` — canonical artifact (all tiers)
- `<run_id>.html` — rendered from snapshot (all tiers)
- `<run_id>.md` — rendered from snapshot (all tiers)
- `<run_id>.json` — legacy summary JSON (deprecated in Phase B; snapshot supersedes)
- `<run_id>.tb.json` + `<run_id>.tb.csv` — TB export (Pro and up, when `--with-tb`)
- `basis_of_preparation.md` etc. — when `--disclosure-grade` flag set (T2/T3)

**Slim vs full snapshot:**
- Slim (default): derivations carry formula + input IDs; no per-year trace. D2 ≈ 28 KB.
- Full (`--full`): adds `trace[y]` per derivation with evaluated inputs. D2 ≈ 64 KB.
- Full SPV pack (D2+D4+D5+D6) slim ≈ 110 KB, gzip ≈ 25 KB.

**AI readability:** The snapshot is designed for local SLM (Foundry tier) and in-house Big 4
audit-tool consumption. The JSON carries all display intent (labels, units, format hints, row
groupings, footnotes) so a renderer or AI tool needs no implicit engine knowledge.

**Comparison output (`compare` subcommand, Phase B-C):** side-by-side D7 Legacy JV vs D2 PCLP 1
across 10 rows (Total SF, Total Capital, Asset Value, Debt Balance, LTV, NOI, Distributable
Cash, Cumulative Distributions, Equity Value, MOIC). Reads two snapshots; does not re-run
engines. Label: "illustrative comparison" — NOT "pro forma" (NI 52-107). Output formats:
MD (table) | HTML (sortable, stacked-bar equity visualisation) | JSON.

**Signed PDF bundle** (Phase C): `typst` or `weasyprint` subprocess; SHA-256 of output
embedded in footer; version + git SHA stamped; RFC 3161 timestamp.

---

## 10. Pricing and market position (confirmed by research panel)

| Tier | Price | Limits | JE / TB | Provenance | RI features |
|---|---|---|---|---|---|
| Entry | $19/month | 1 active proforma; watermarked PDFs | None — IS/BS/CF only | T1 (hash + SSH sig) | None |
| Pro | $99/month | Unlimited proformas; white-label PDF; scenario library | JE export (CSV/JSON) + annual TB + TB-to-IS/BS bridge | T2 (+ RFC 3161 + Rekor) | None (private entity only) |
| Reporting Issuer | $299–$499/month | Unlimited; SEDAR-compatible export; RI compliance suite | Same as Pro | T2 | IAS 34 quarterly; comparatives; SOCE; MD&A building blocks (F1 §§3, 5, 8); CSRS 4250 pro forma 3-column; T5013/capital account; Cross-Tier Disclosure Manifest |
| Team | $499/month | 5 seats; LP investor portal | Same as Pro | T2 | None (may upgrade to RI) |
| Enterprise | Custom | Unlimited + audit bundle + EY-ready exports | Bidirectional Override::Journal + CSRS 4250/ISAE 3420 working-paper export pack | T3 (+ auditor co-sign) | Full RI suite + recomputation attestation |

Note: "CSAE 3420 working-paper export pack" corrected to CSRS 4250/ISAE 3420 (CSAE 3420 is not a Canadian standard).

**SAM for Reporting Issuer tier (Canada):** ~38 TSX REITs + ~25–40 venture real-estate RIs +
~200–400 BC unlisted reporting issuers = ~75–120 realistic targets. At $299/month, 15–25
logos = ~$54K–$90K ARR. Positioned as gateway to Enterprise; ARGUS gap is real (no NI 51-102
deliverable at any price point).

ARPU target: $80–$150. $19 is the marketing hook, not the revenue driver.

Competitors: ARGUS ($8K–$15K/user/year), Dealpath (enterprise), REFM (Excel templates).
**The $19–$99 institutional-quality gap is unoccupied.** ARGUS remains the institutional
lingua franca — "ARGUS-fidelity outputs with modern UX" is the exact positioning.

---

## 11. Current implementation status

| Component | State | Notes |
|---|---|---|
| D1 dev-classes engine | Partial — gaps listed in §12 | 4 classes, flat OPEX_RATIO, no CAM itemisation |
| D2 direct-hold | Implemented | Number audit pending |
| D3 WCP | Implemented | |
| D4 Bencal SPV2-LP/Inc. | Implemented | Stage 6 pending (017a8f2d, 05b0cce6); Flags 1-3 pending |
| Bencal SPV1 legacy (30% WCP scale) | Implemented but superseded | §5e spec redefines as 1.5% / 150K shares; full rewrite required |
| Bencal Management Corp. | Partial — bencal.rs uses wrong constants | §5f spec replaces; constants + Bencal SPV1 inline helpers must be rewritten; Flags 1-11 pending |
| `fmt_smart` formatter | Implemented | G&A rows in d3_wcp |
| WORM audit ledger | Not started | Phase B |
| `DevClassConfig` TOML | Not started | Phase A gate |
| Dual NOI output | Not started | Phase A (fields added, values identical until IFRS 16) |
| Sensitivity / tornado | Not started | Phase C |
| axum server | Not started | Phase D |
| Svelte SPA | Not started | Phase D |
| Calculation snapshot (`src/snapshot/`) | Not started | Phase B |
| `tool-proforma-render` binary | Not started | Phase B |
| Disclosure-grade paired artifacts | Not started | Phase B (T2/T3) |
| `ReportingTier` enum + ASPE/IFRS dispatch | Not started | Phase B-RI |
| IAS 34 quarterly / comparative outputs | Not started | Phase B-RI |
| Statement of Changes in Equity (SOCE) | Not started | Phase B-RI |
| MD&A building blocks (F1 §§3, 5, 8) | Not started | Phase B-RI |
| CSRS 4250 pro forma 3-column format | Not started | Phase B-RI |
| T5013 / capital account rollforward | Not started | Phase B-RI |
| Cross-Tier Disclosure Manifest | Not started | Phase B-RI |
| IFRS 8 segment tags on D1 outputs | Not started | Phase B-RI (gate for D3 consolidation) |
| D7 Legacy JV entity (`legacy-jv` subcommand) | Not started — stub Phase A | Phase A stub → Phase B full |
| `JvConfig` TOML + construction draw schedule | Not started | Phase B-D7 |
| D7 permanent loan schedule (Y4–Y10, 25-yr amort) | Not started | Phase B-D7 |
| D7 ASPE 3061 cost model + Partners' Capital Accounts | Not started | Phase B-D7 |
| `compare` subcommand (D7 vs D2, 10 comparison rows) | Not started | Phase B-C |

**Binary location:** `/srv/foundry/cargo-target/jennifer/debug/tool-proforma-engine`
**Output location:** `/srv/foundry/clones/project-proforma/outputs/`
**Monorepo branch:** `main` — 3 commits ahead of origin/main (Stage 6 pending: 017a8f2d + 05b0cce6 + e624f27)

---

## 12. Open items — priority-ordered

### Phase A — D1 dev classes: match Excel structure (current focus)

- [ ] **A1** Replace `OPEX_RATIO` constant with `CamBudget` struct (9 itemized fields)
- [ ] **A2** Add recoveries revenue line (5.5% non-recovery cost per area type)
- [ ] **A3** Add investment valuations section (capitalised rent per area type at 6.25%)
- [ ] **A4** Add ancillary income (`pylon_sign_income: f64` on config)
- [ ] **A5** Add `underground_parking` as optional component on `DevClass`
- [ ] **A6** Emit `noi_cash` and `noi_gaap` dual fields (identical value until IFRS 16 added)
- [ ] **A7** Emit `unlevered_cf` and `levered_cf` separately in cash flow
- [ ] **A8** Add Report summary renderer ($/sqft + % of total, three stages, with footnotes)
- [ ] **A9** Externalize `DevClassConfig` to TOML file (move all hardcoded constants out)
- [ ] **A10** Update `titleco.rs` reader to also read CAM tab parameters
- [ ] **A11** Left-gutter row IDs on all D1 output rows (consistent with PCLP convention)
- [ ] **A12** DSCR metric in class summary header (NOI / (P+I) at 65% LTC, 25-yr amort, 5%)

### Phase A-Portfolio Architecture gate (gate: A9 TOML externalisation complete)

- [ ] **A-PA-1** Define `BuildingOutput` struct (`building_id: Ulid`, `class_type`,
                  `phase: ConstructionPhase`, `area_sf`, `revenue[Y1–Y10]`, `noi[Y1–Y10]`,
                  `cost[Y1–Y10]`); `ConstructionPhase` enum:
                  `PreDev | Construction | LeaseUp | Stabilised`.
- [ ] **A-PA-2** Update D1 engine to return `DevClassOutput { buildings: Vec<BuildingOutput> }`
                  from each class computation (replaces current scalar array returns). All
                  existing D1 outputs must remain identical post-refactor.
- [ ] **A-PA-3** Define `PortfolioVehicle` trait with `aggregate_portfolio(&[DevClassOutput],
                  &ClassMix) -> PortfolioAggregates` and `apply_vehicle_adjustments(
                  &PortfolioAggregates, &VehicleConfig) -> VehicleOutput` — both
                  `unimplemented!()` stubs; trait only, no implementation in Phase A.
- [ ] **A-PA-4** `ClassMix` struct: four `f64` fields (professional, suburban_office,
                  tech_industrial, retail_select) summing to 1.0; validated in constructor.

### Phase A-D2 — D2 direct-hold engine (new, concurrent with A-D1)

- [ ] **A-D2-1** Implement `Pclp1Config` TOML (all parameters from §5b input table)
- [ ] **A-D2-2** Capital asset schedule: phase draws, generating vs. WIP split
- [ ] **A-D2-3** Revenue: `net_proceeds_from_ops` (EBITDA input) vs. `income_continuity` (valuation input) — separate fields
- [ ] **A-D2-4** Expenses: issue costs, financing costs, advisory, admin, board — per year
- [ ] **A-D2-5** EBITDA: uses `net_proceeds_from_ops` (= 0 Y1–Y3), not income continuity
- [ ] **A-D2-6** Debt schedule: phase draws + Y7 min-cash solver + FFO-based repayment (Y8+)
- [ ] **A-D2-7** Net interest: average-balance method (avg_debt × 5% − avg_cash × 0.5%)
- [ ] **A-D2-8** FFO = EBITDA − net_interest
- [ ] **A-D2-9** Distributions: payout_ratio schedule (0% / 90% / 100%) × FFO − repayment
- [ ] **A-D2-10** Cash flow continuity: full 10-year walk
- [ ] **A-D2-11** Asset valuation: (income_continuity / cap_rate) + WIP + cash
- [ ] **A-D2-12** NAV = asset_value − closing_debt; all per-unit
- [ ] **A-D2-13** Market Value: Y1–Y7 config table; Y8+ = DPU / 0.08
- [ ] **A-D2-14** All ratios: ICR, Debt/Dev Cost, Debt/AV, TER, CAGR, DY at MV
- [ ] **A-D2-15** AA12:AM35 summary renderer (exact layout match to circulated investor table)
- [ ] **A-D2-16** BCSC provenance metadata on `revenue_construction_phase` field
- [x] **A-D2-17** Column extent confirmed: AA12:AN35 (Y10 full). Circulated version stops at AM35 (Y9) but engine produces Y10. 2026-05-23.

### Phase A-D3 — D3 WCP engine (new, researched 2026-05-23)

- [ ] **A-D3-1** `WcpConfig` TOML: FX rates, P/E multiple, dividend yield, tax rate, LP fund configs (stagger, FX, size factor)
- [ ] **A-D3-2** LP1 from PCLP1 output: advisory_fee × deployment_ramp (1/3, 2/3, 1.0); distributions × 1/10; NAV × 1/10
- [ ] **A-D3-3** LP2–LP6 cascade: `LP_n[y] = LP1[y − lag] × size_factor × fx_rate` via `checked_sub` bounds-safe indexing
- [ ] **A-D3-4** Offering Costs Reimbursement: first-difference closed form (no rolling accumulator), cutoff after Y6
- [ ] **A-D3-5** G&A ramp: hardcoded Y1–Y2; Y3–Y10 = advisory_fee_total × ramp_pct (20%–55%)
- [ ] **A-D3-6** IS: Gross Income → Referral Fees → WPI → G&A → Total OpEx → EBITDA → Taxes (27%) → Earnings
- [ ] **A-D3-7** Cumulative FCF: prefix scan of (financing_activity + earnings)
- [ ] **A-D3-8** 10% LP ownership NAV rollup: sum of all 6 LP NAVs per year
- [ ] **A-D3-9** Book Valuation: CumFCF + LP ownership; per-share
- [ ] **A-D3-10** Market Valuation: earnings × 10.72 + CumFCF; per-share
- [ ] **A-D3-11** Fair Valuation: PEG formula with FCF floor; forward moving average edge case (Y9/Y10); negative-eps warning flag
- [ ] **A-D3-12** Dividend Valuation: earnings / 4.5%; per-share
- [ ] **A-D3-13** Comparative ratios: MV/BV, MV/FV, MV/DV, RONTE (Y1 = n/a)
- [ ] **A-D3-14** C17:R53 Revenue Generator renderer (exact 6-LP layout, Y1–Y10 col scope)
- [ ] **A-D3-15** WCP IS + valuation section renderer (4 methods + comparables block rows 105-117)
- [ ] **A-D3-16** LP5/LP6 distribution FX anomaly: replicate Excel exactly; emit `metadata.distribution_fx_anomalies` in JSON
- [ ] **A-D3-17** Left-gutter row IDs on all D3 output rows (consistent with D1/D2 convention)

### Phase A-D4 — D4 investor SPV engine (Bencal SPV2, researched 2026-05-23)

- [ ] **A-D4-1** `Ad2Config` TOML: total_equity, dilution_pct, legal/accounting/board/admin expenses (no advisory_fee_pct), opex_reserve, spv_market_val_yield (8.0% — Flag 1 resolved)
- [ ] **A-D4-2** Derived scalars: diluted_units (ROUND formula), manager_units, pclp1_units_held
- [ ] **A-D4-3** IS: net_proceeds_from_ops = pclp1_units_held × PCLP1.dpu; expenses constant; EBITDA → interest income on WC → funding_from_ops
- [ ] **A-D4-4** Cash flow continuity with distribution fixed-point solver (mirror PCLP1 pattern)
- [ ] **A-D4-5** Asset valuation: opening_assets = pclp1_units_held × PCLP1.nav_per_unit; no construction additions
- [ ] **A-D4-6** Distribution schedule: 0/90%/100% payout × funding_from_ops
- [ ] **A-D4-7** DPU, dist_yield, nav_per_unit, asset_value_per_unit
- [ ] **A-D4-8** Market Value: Y1-Y7 config hardcoded; Y8+ = dpu / spv_market_val_yield (0.074)
- [ ] **A-D4-9** Summary block AA6:AN35 renderer (Y1-Y10, per-diluted-unit denominator ≈ 280,030; TBD formation)
- [ ] **A-D4-10** CAGR single scalar Y8; buyer yield tautology; TER; sqft row
- [ ] **A-D4-11** Compensation summary (AL7-AL9): total advisory fees Y1-Y8 ($0; no advisory fee), unit-based comp at MV_Y8, distributions to manager
- [ ] **A-D4-12** Consolidated entity label + `_derivation` JSON block (source=PCLP1, method=SPV_wrap, dilution_pct=10%, manager_units=33333)
- [ ] **A-D4-13** IFRS 2 unit-based compensation disclosure note in output metadata
- [ ] **A-D4-14** Left-gutter row IDs on all D4 rows; blank patterns match §5d spec
- [ ] **A-D4-15** BCSC disclosure flag: manager dilution %, no hurdle, related-party Note

### Phase A-D5 — D5 investor SPV engine (Bencal SPV1, researched 2026-05-23)

**Pre-conditions:** Jennifer must confirm Flags 1-5 from §5e before any of these items ship.
Flags 1 (cash reserve) and 5 (structural reconciliation vs legacy) are blockers.

- [ ] **A-D5-1** `Ad1Config` TOML struct: investment_amount, wcp_share_price_at_purchase, wcp_shares_held, initial_cash_reserve, investor_shares, investor_share_price, dilution_pct, opex fields (legal_setup/legal_annual/accounting_setup/accounting_annual/board_annual/admin_annual), tax fields (fvtpl_unrealised_rate, realisation_rate). Sample at `inputs/ad1-config.toml`.
- [ ] **A-D5-2** Derived scalars: `compute_total_shares(investor_shares, dilution_pct)` using ROUND-HALF-UP then −1 (matches Excel ROUND); manager_shares; dilution_drag_pct; initial_cost_basis
- [ ] **A-D5-3** WCP per-share arrays: pull from `WcpData` fields (`book.book_value_per_share`, `market.market_value_per_share`, `fair_div.fair_value_per_share`, `fair_div.dividend_value_per_share`) — do NOT hard-code in config
- [ ] **A-D5-4** Asset value per method: `av[method][y] = wcp_shares_held × wcp.per_share[method][y]`; retain unfloored value in JSON; carry floored (`max(av,0)`) for all NAV and IS computations
- [ ] **A-D5-5** Unrealised gain: `ug[Y1] = av_book[Y1] − initial_cost_basis`; `ug[y] = av_book[y] − av_book[y-1]` for y>1. Cumulative: `cum_unrealised[y] = av_book[y] − initial_cost_basis` (identity check).
- [ ] **A-D5-6** Opex schedule: Y1 = legal_setup + accounting_setup + legal_annual + accounting_annual + board_annual + admin_annual; Y2+ = recurring only. Assert Y1 = $82,649, Y2+ = $73,544 in tests.
- [ ] **A-D5-7** Cash balance: `cash[y] = initial_cash_reserve − cum_opex[y]`. Emit `going_concern_warning: true` in JSON metadata if any cash[y] < 0.
- [ ] **A-D5-8** Deferred tax liability: `dtl[y] = max(cum_unrealised_book[y], 0) × fvtpl_unrealised_rate`. Deferred tax expense: `dte[y] = dtl[y] − dtl[y-1]` (Y1: = dtl[0]).
- [ ] **A-D5-9** IS: net_income[y] = unrealised_gain[y] − opex[y] − ifrs2_charge[y] − dte[y]. IFRS 2 charge: flag-3-dependent (immediate = Y1 full $333,333; cliff = amortised).
- [ ] **A-D5-10** NAV and per-share: `nav_book[y] = av_book[y] − cum_opex[y] − dtl[y]`. Per-investor = nav / investor_shares; per-diluted = nav / total_shares. Both series output.
- [ ] **A-D5-11** Summary block renderer (§5e Section A-F): WCP per-share pass-through, Bencal SPV1 asset value, opex drag, NAV ladder, 4-method comparison, CAGR scalars at Y8, TER, cost-of-intermediation table. Left-gutter row IDs on every row.
- [ ] **A-D5-12** CAGR: `(nav_per_inv[Y8] / investor_share_price)^(1/8) − 1`. Return `Option<f64>` when final ≤ 0. Emit all 4 method CAGRs.
- [ ] **A-D5-13** Cost of intermediation assertion: `|wcp_direct_per_dollar[y] − ad1_inv_per_share[y] − opex_drag[y] − dilution_drag[y]| < 0.01` as debug_assert
- [ ] **A-D5-14** CLI: `spv-bencal --ad1-config <toml>` flag; when supplied, run new `ad1::compute`; when omitted, legacy 30%-scale path retained (deprecated, one-cycle). Bencal Management stake recomputed as `bencal_ad1_pct × wcp_shares_held / wcp.shares_outstanding` after Bencal SPV1 change.
- [ ] **A-D5-15** JSON `_derivation` block: source_model="wcp_42m", method="direct_equity_spv", scale_factor=0.015, configured_inputs map (per §18c), derived_from_wcp field paths
- [ ] **A-D5-16** BCSC disclosure flags in metadata: manager_shares, manager_pct_diluted, no_hurdle=true, no_clawback=true, no_preferred_return=true, fofi_truncate_flag (pending Flag 4 decision)
- [ ] **A-D5-17** Smoke test `tests/ad1_smoke.rs`: hand-constructed WcpData fixture using exact per-share arrays from §5e; assert total_shares=3,333,333, manager_shares=333,333, Y1_opex=$82,649, Y2_opex=$73,544, cagr_book≈15.18%, cum_opex_Y8=$597,457

### Phase A-D6 — D6 SPV-Manager engine (Bencal Management, researched 2026-05-23)

**Pre-conditions:** Jennifer must confirm Flags 1, 5, 6 from §5f before any of these items ship.
Flag 1 (investment entity election) and Flag 6 (commission income DROP) are design blockers.
Flag 5 (cash funding) determines whether a shareholder loan payable appears on BS.

- [ ] **A-D6-1** Replace entire const block in `src/spv/bencal.rs`: BENCAL_AD2_UNITS=27_843, AD2_DILUTED_UNITS=278_434, BENCAL_AD1_SHARES=339_431, AD1_DILUTED_SHARES=3_394_313, AD1_WCP_SHARES_HELD=300_000, AD1_COST_BASIS=3_000_049.83, AD1_DEFERRED_TAX_RATE=0.135, BENCAL_SHARES_OUTSTANDING=2, BENCAL_PRICE_PER_SHARE=5.0, BENCAL_COMMISSION_RETENTION=41_713.0. Remove: BENCAL_AD1_STAKE, AD1_WCP_STAKE, COMMISSION_PER_YEAR. (Source: v0.15.6; 2% Work Fee; $4,373.37/yr/director.)
- [ ] **A-D6-2** Add inline Bencal SPV1 helper functions to bencal.rs (interim until Ad1Data struct exists): `ad1_book_nav_per_diluted_share(wcp, y)` computing asset value − cum_opex − DTL; all 4 methods (book/market/fair/div).
- [ ] **A-D6-3** FV_AD2_LP[y]: `(BENCAL_AD2_UNITS / AD2_DILUTED_UNITS) × Bencal SPV2.nav_total[y]`. Requires Ad2Data to be available in the compute chain (pass as argument or compute inline using PCLP1 data).
- [ ] **A-D6-4** FV_AD1[y]: `(BENCAL_AD1_SHARES / AD1_DILUTED_SHARES) × ad1_book_nav_per_diluted_share(wcp, y) × AD1_DILUTED_SHARES`. All 4 methods.
- [ ] **A-D6-5** Bencal Management NAV[y] = FV_AD2_LP[y] + FV_AD1[y] (+ $0 Bencal SPV2-GP)
- [ ] **A-D6-6** IS: net_change_fv_ad2[y] + net_change_fv_ad1[y] − opex[y] − dte[y] = net_income[y]. Commission income: gated on `commission_enabled` flag (default false per Flag 6 decision).
- [ ] **A-D6-7** BS: Investment in Bencal SPV2-LP (FVTPL) + Investment in Bencal SPV1 (FVTPL) + Bencal SPV2-GP (nil) + Cash + DTL.
- [ ] **A-D6-8** Cash[y]: initial $10 + distributions_from_AD2[y] + commission (if enabled) − opex[y]. Flag shareholder_loan if cash < 0 (pending Flag 5 decision).
- [ ] **A-D6-9** DTL[y]: sum of `max(FV_AD2_LP[y] − 0, 0) × 0.135` + `max(FV_AD1[y] − 0, 0) × 0.135`. (Cost basis $0 — units/shares received as compensation at Bencal Management level per Flag 7.)
- [ ] **A-D6-10** Revenue Generator: reduce to **2 rows** (Bencal SPV2-LP position + Bencal SPV1 Inc. position). Remove old 7-row Revenue Generator. Per-share fields populated using BENCAL_SHARES_OUTSTANDING=2.
- [ ] **A-D6-11** Valuation Matrix: Block A (portfolio composition) + Block D (4-method sub-tables: Bencal SPV2 + Bencal SPV1 + total + MOIC). `price_per_share = 5.00`. Per-share NAV shows correctly (large number — add footnote per Flag 3: internal-use only).
- [ ] **A-D6-12** Block B (Bencal SPV2 distributions to Bencal Management Y1-Y10) + Block C (Bencal SPV1 NAV 10% share + wrapper drag vs WCP direct).
- [ ] **A-D6-13** Block F headline card: **total portfolio NAV + MOIC + CAGR only**. Suppress per-share from primary display.
- [ ] **A-D6-14** `_derivation` JSON block: source_models=["ad2_lp","ad1_inc","wcp_42m"], method="spv_manager_vehicle", bencal_pct_ad2=0.09999, bencal_pct_ad1=0.09999, ad2_inc="nil_gp_shell".
- [ ] **A-D6-15** Smoke test `tests/bencal_smoke.rs`: assert BENCAL_AD2_UNITS=33_332, BENCAL_AD1_SHARES=333_332, price_per_share=5.00; compare Revenue Generator row count = 2; verify bencal_nav[Y8] > 0.

### Phase B — Audit ledger + number validation

- [ ] **B1** Number audit: D1 computed values vs. Excel tab-by-tab
- [ ] **B2** Number audit: D2 direct-hold vs. PCLP 1 Excel (all 10 years, every row)
- [ ] **B3** `proforma-ledger` crate: WORM JSONL, hash-chained, per-run record
- [ ] **B4** IRR/XIRR implementation (Newton-Raphson with explicit convergence + failure mode)
- [ ] **B5** Stage 6 for 017a8f2d + 05b0cce6 (Command Session)
- [ ] **B6** Bencal SPV2 expense review (currently scaled PCLP overhead; may need standalone SPV costs)
- [ ] **B7** D1 entity name override (TitleCo source entity vs. target report header)

### Phase C — Sensitivity + PDF bundle

- [ ] **C1** OAT tornado chart renderer
- [ ] **C2** 2-variable scenario grid (cap rate × rent growth)
- [ ] **C3** Signed PDF bundle via typst subprocess
- [ ] **C4** Monte Carlo / Sobol harness (~300–500 LOC new code)

### Phase D — Web product (software.pointsav.com)

- [ ] **D1** `axum` server wrapping engine as library
- [ ] **D2** Svelte SPA: grouped scenario form + live proforma + comparison panel
- [ ] **D3** SOC2 minimums: Postgres RLS, OIDC+MFA, Drata/Vanta
- [ ] **D4** Tenant configuration model (Postgres table replaces TOML for SaaS)

### Phase B-JE — Journal entry + trial balance engine

**Pre-condition:** B-JE-1 (GL account tagging) is a Phase A gate — it must complete during
Phase A before any JE template is written. Tagging D2/D5/D6 can begin immediately; defer D1
tag until A12 (D1 report summary renderer) is complete.

- [ ] **B-JE-1**  GL account tagging: add `gl_account_id` constant to every IS/BS/CF field in
                   Pclp1Data, WcpData, TitleCoData, and all SPV structs. One constant per line
                   item; zero rendering change; zero new output. **Phase A prerequisite.**
- [ ] **B-JE-2**  `je/types.rs`: Account, JournalEntry, GeneralLedger, TrialBalance,
                   TrialBalanceRow, Direction, AccountType, SourceKind, BalancedJE newtype,
                   Override enum (Variable / Journal / Balance), LedgerError.
- [ ] **B-JE-3**  `je/coa.rs`: canonical 4-digit CoA constants per entity type (D1–D6) per §5g-i.
- [ ] **B-JE-4**  `je/d2_pclp1.rs`: 9 annual JE templates → GeneralLedger × 10 years.
- [ ] **B-JE-5**  `je/d3_wcp.rs`: JE templates for GP/AM (advisory income, tax, LP equity pickup).
- [ ] **B-JE-6**  `je/d5_ad1.rs`: 5 annual JE templates per §5g-iv (FVTPL, DTL, opex, close).
- [ ] **B-JE-7**  `je/d6_bencal.rs`: JE templates (FV changes on Bencal SPV2/Bencal SPV1 stakes, distributions, DTL).
- [ ] **B-JE-8**  `je/trial_balance.rs`: aggregate GeneralLedger → TrialBalance; validate
                   Σ Dr = Σ Cr; derive IS/BS/CF line items from TB rows; assert CF closing cash
                   = TB 1110 closing balance.
- [ ] **B-JE-9**  `je/render.rs`: markdown TB section (appended after IS/BS/CF tables when
                   `--with-tb` flag set); CSV/JSON export with `assumption_ref` field linking
                   each TB row back to its TOML input assumption.
- [ ] **B-JE-10** `je/worm.rs`: parallel JSONL chain (`journals.jsonl`); each row carries
                   `run_id`, `je_id` (ULID), `prev_hash`, `entry_hash`. Run record in
                   `proforma-ledger` gains `je_merkle_root` field (Merkle root of run's JEs).
- [ ] **B-JE-11** CLI: `--with-tb` flag on all subcommands; new subcommand:
                   `proforma trial-balance --entity <d2|d3|d5|d6> --year <1-10> [--format md|json|csv]`.
- [ ] **B-JE-12** TB-to-IS/BS bridge CSV: every account → IS/BS/CF line mapping; emitted
                   alongside TB on `--with-tb`. Enables CaseWare / Working Papers ingestion.
- [ ] **B-JE-13** Override::Variable: `Ad1Config` / `Pclp1Config` TOML gains `[[overrides]]`
                   table; year-specific assumption overrides regenerate downstream JEs only;
                   WORM override log captures `{year, assumption_name, original, override, reason}`.
- [ ] **B-JE-14** Smoke tests `tests/je_balance.rs`: balanced-JE invariant (Σ Dr = Σ Cr) for
                   all entities Y1–Y10; TB-to-IS tie-out; TB 1110 closing balance = CF closing
                   cash. D1 JE deferred until Phase A CAM itemisation complete.

### Phase B-SS — Calculation snapshot

- [ ] **B-SS-1**  `src/snapshot/schema.rs`: typed structs for all 5 layers; `schemars` derive;
                  `schema/snapshot.v1.0.json` committed; CI diff-gate blocks schema drift.
- [ ] **B-SS-2**  `src/snapshot/ids.rs`: stable ID namespaces (ASM.*, CONST.*, SR.*, S.*,
                  DER.*, IS.*, BS.*, CF.*); newtype wrappers to prevent cross-namespace errors.
- [ ] **B-SS-3**  `src/snapshot/derivations.rs`: `Derivation` struct (formula, inputs[], scope,
                  optional trace[]); `BalancedDerivation` validator (sum of inputs = output,
                  within f64 tolerance).
- [ ] **B-SS-4**  `src/snapshot/builders/`: one builder per entity (d1–d6); pure projection
                  `EntityData -> SnapshotLayer{B,C,D,E}`; no side effects; no Excel reads.
- [ ] **B-SS-5**  `src/snapshot/layout.rs`: `FmtTag` enum (fmt_m/k/pct/dollar/ratio/smart);
                  row `emphasis` (body/subtotal/header/spacer); `layout.notes[]`; bilingual
                  `label_es` field; `section_order[]`.
- [ ] **B-SS-6**  `tool-proforma-render` binary: reads `snapshot.json` only; renders HTML + MD;
                  zero dependency on `src/excel/` or `src/engine/`; handles `schema_version`
                  N-1 MAJOR via `migrate-snapshot` codepath.
- [ ] **B-SS-7**  WORM integration: WORM record gains `snapshot_sha256` + `snapshot_path` fields;
                  snapshot Layer A gains `worm_anchor.run_id`; symmetric binding verified in tests.
- [ ] **B-SS-8**  `--disclosure-grade` CLI flag: requires all Layer B `_basis` fields populated;
                  emits paired `basis_of_preparation.md` + `management_representation.md` +
                  `sensitivity_report.md` + `change_log.md` (diff vs `--baseline <run_id>`).
- [ ] **B-SS-9**  Deprecate legacy `--json` summary output; emit snapshot slim as default `--json`;
                  retain `--json-legacy` for one release cycle.
- [ ] **B-SS-10** Smoke tests `tests/snapshot_roundtrip.rs`: render HTML from snapshot; compare
                  key cells to engine-direct output; assert `schema_version` present; assert
                  slim mode < 50 KB for D2; assert full mode derivation traces sum correctly.

### Phase B-RI — Reporting Issuer tier

**Gate:** `reporting_tier` field added to entity TOML in Phase A (one field, default `PrivateASPE`).

- [ ] **B-RI-1**  `src/tier/mod.rs`: `ReportingTier` enum (4 variants); `AccountingOverlay` enum;
                  serialize/deserialize to/from TOML; add `reporting_tier` + `accounting_overlay`
                  fields to all entity config structs.
- [ ] **B-RI-2**  `src/tier/policy.rs`: `AccountingPolicySet` struct; strategy traits for 4
                  accounting domains: `InvestmentPropertyValuation` (IFRS FV vs ASPE cost),
                  `ReceivableImpairment` (ECL vs incurred-loss), `LpUnitClassification`
                  (puttable exception vs ASPE), `BorrowingCostTreatment` (IAS 23 mandatory vs
                  ASPE elective). `AccountingPolicySet::from_tier()` factory.
- [ ] **B-RI-3**  `src/output/traits.rs`: `StatementBundle` trait (IS/BS/CF — all tiers);
                  `ReportingIssuerBundle` trait extends with `comparative_is()`, `socie()`,
                  `notes_schedule()`, `segment_disclosure()`. `ComparativeStatement<T>` wrapper
                  (current + prior; `values_ref` pointers — no recomputation).
- [ ] **B-RI-4**  `src/output/mdna.rs`: `MdAndA` struct with `YoYTable` (derived from IS Y/Y
                  delta), `ContractualObligationsTable` (debt maturity ladder + lease commits
                  by bucket <1yr/1–3yr/4–5yr/>5yr), `RelatedPartySchedule` (IC flows). Three
                  new TOML input sections: `[mdna.contractual_obligations]`, `[mdna.off_balance_sheet]`,
                  `[mdna.subsequent_events]`. All other fields derived from Layer C/D.
- [ ] **B-RI-5**  `src/output/prospectus.rs`: CSRS 4250 / ISAE 3420 three-column pro forma layout
                  (source statement / adjustments / pro forma); `--prospectus` CLI flag; adjustments
                  TOML table `[[proforma.adjustments]]`; footnote tie numbers.
- [ ] **B-RI-6**  `src/output/partnership.rs`: T5013 partnership information return data (D2 PCLP 1
                  only); per-unit capital account rollforward (opening capital, income allocation,
                  distributions, closing capital); ACB tracking per unit class.
- [ ] **B-RI-7**  Snapshot Layer E: add `reporting_tier`, `accounting_overlay`,
                  `comparative_period_required`, `interim_reporting`, `sedar_compatible`,
                  `aspe_eligible`, `continuous_disclosure_obligations: Vec<CdoFlag>`;
                  promote `disclosure_grade` from bool to ordered enum
                  `Draft|Internal|OM|RIQuarterly|RIAnnual|Audited`.
- [ ] **B-RI-8**  `src/output/cross_tier.rs`: Cross-Tier Disclosure Manifest — when run includes
                  both RI (D2/D3) and private entities with ownership edge, emit manifest of
                  triggered obligations: IAS 24 note, Form 51-102F1 Item 1.9, MI 61-101 flag,
                  NI 55-104 flag, NI 62-103 flag.
- [ ] **B-RI-9**  IFRS 8 segment tags: add `segment_id: Option<String>` to D1 output row structs;
                  D3 WCP consolidation roll-up aggregates by `segment_id`; segment reconciliation
                  to consolidated totals.
- [ ] **B-RI-10** Smoke tests `tests/reporting_issuer.rs`: assert D2/D3 with `ReportingIssuerIFRS`
                  tier emits SOCE; assert comparative columns present; assert ASPE entities
                  (D1/D5/D6) use cost model for investment property; assert cross-tier manifest
                  triggered when D6 Bencal Management owns D3 WCP shares; assert `disclosure_grade` ordering
                  (Audited > RIAnnual > OM > Internal).

### Phase A-FMT — Output format forward-compat hinge (gate: A9 TOML; concurrent with A-PA)

- [ ] **A-FMT-1** Add `rust_decimal = "1"` to `Cargo.toml`; convert `f64` engine outputs to
                  `Decimal` at snapshot-emission boundary. Engine math may remain `f64`
                  internally; boundary conversion must be explicit. `f64` in snapshot JSON
                  fails EY arithmetic tie-out (silent drift).
- [ ] **A-FMT-2** Introduce `Snapshot` struct (Phase B schema stub); serialise alongside current
                  HTML output as `<run_id>.snapshot.json`; content may be sparse in Phase A.
- [ ] **A-FMT-3** Refactor current `src/html.rs` to consume `Snapshot` instead of raw engine
                  structs. Output must be byte-identical to current HTML. No inline
                  `"Y1"` column strings or `format!("{:.2}M", x/1e6)` — go through
                  `ColumnStyle` / `NumFmt` enums reserved in this phase.
- [ ] **A-FMT-4** Reserve all four `OutputFormat` variants in source:
                  `OperationalHtml | FormalIfrsForecast | FormalAspeForecast | InvestorSummary`.
                  Phase B implements; Phase A only reserves. No dead-code removal.
- [ ] **A-FMT-5** Reserve `RowKind { Data(Row) | Section | Spacer | Subheader }` and
                  `RowEmphasis { Body | SectionHeader | Subtotal | Total | GrandTotal |
                  Spacer | Header | NoteReference | NoteCallout }`. `note_refs:
                  SmallVec<[u8; 2]>` on `Row` struct (a row may be both `Total` AND carry
                  note refs 3+7). Forward-compat integration test: construct
                  `ReportProfile::preset(FormalIfrsForecast)` — allowed to `panic!(
                  "not yet implemented")` but types must compile.

### Phase B-FMT — Full EY-format renderer (gate: B-SS snapshot stable; rust_decimal in use)

- [ ] **B-FMT-1** `tool-proforma-render` binary: single binary with `--format` flag
                  (`internal-operating | private-forecast | issuer-fofi`). PDF output
                  behind Cargo feature `pdf` (typst), initially `unimplemented!()`.
                  HTML preview is the primary QA path for EY format.
- [ ] **B-FMT-2** `ReportProfile` struct: bundles `OutputFormat`, `ColumnStyle`, `NumFmt`,
                  `language`, `disclaimer_variant`, `notes_template`. `ReportProfile::preset(
                  fmt: OutputFormat)` is the single auditable function encoding preset choices
                  (e.g., `FormalIfrsForecast` → `ProjectedWithDollarRow + FullDollars +
                  IssuerFofi notes`).
- [ ] **B-FMT-3** `ColumnStyle` enum: `YearsAbbreviated | ProjectedWithDollarRow |
                  CalendarYears`. Default driven by `ReportProfile`; CLI override with warning.
- [ ] **B-FMT-4** `NumFmt` enum: `FullDollars | FullDollarsCents | SmartAbbrev | Percent |
                  PerUnit | Count`. `NegativeStyle: Parenthetical | Minus`.
                  `ZeroDisplay: EmDash | Dash | Zero | Blank`. Per-cell `Option<NumFmt>`
                  override required (Note 4 debt table mixes dollar and percent rows).
- [ ] **B-FMT-5** EY page-header block: `{entity_name}` bold / `{statement_title}` /
                  `10-Year Financial Forecast` bold / `(Expressed in Canadian Dollars)` bold.
                  Cover variant uses lowercase `Canadian dollars` — both preserved verbatim;
                  neither is a typo to fix.
- [ ] **B-FMT-6** Statement ordering per `ReportLayout { statements: Vec<ReportSection>,
                  valuation_summary_position: SummaryPosition, notes_position: NotesPosition }`.
                  `SummaryPosition { Before | After | Suppressed }`.
                  EY issuer order: ValuationSummary (before BS, non-GAAP labelled) → BS →
                  IS → SCE → CF → Notes. Operational HTML order: ValuationSummary → IS →
                  BS → CF (no SCE in D1).
- [ ] **B-FMT-7** Notes template system: TOML per entity at
                  `templates/notes/<entity_slug>/<lang>.toml`. Use `tinytemplate` crate.
                  `NoteSource { Fixed | Templated | Computed }` discriminates per-note type.
                  Note 4 is `NoteComputation` trait impl returning typed `NoteBody` rows
                  (same `NumFmt`/`NegativeStyle` rules as statements).
- [ ] **B-FMT-8** `regulatory_status` TOML per entity:
                  `fofi_in_scope / is_offering / is_reporting_issuer / is_illustrative`.
                  Gates Note 1 template selection. D7 always `is_illustrative=true`;
                  render fails lint if `is_illustrative=false` for a D7 entity.
- [ ] **B-FMT-9** Snapshot-hash colophon on every EY-format PDF — regulatory-mandatory
                  for BCSec Act s.85 readiness (see §25d). One line in footer template.
- [ ] **B-FMT-10** `cover.variant` field: `management-prepared | compilation | examination`.
                  `management-prepared` lint: engine fails if EY logo/attribution present
                  in template. Sign-off gate: OM-flagged renders require signed-off snapshot
                  manifest in `outputs/signed/` before emitting document.
- [ ] **B-FMT-11** Smoke tests `tests/format_roundtrip.rs`: assert `issuer-fofi` format
                  emits BS before IS; assert `FullDollars` format for 229,050,000 → `"229,050,000"`
                  (no `$` per cell); assert D7 `is_illustrative=true` selects illustration
                  Note 1 variant; assert snapshot hash present in EY-format colophon;
                  assert `SmartAbbrev` for 28,510,000 → `"28.51M"`.

### Phase A-D7 — Legacy JV stub (gate: D1 class specs stable)

- [ ] **A-D7-1** `src/legacy_jv/config.rs`: `JvConfig` struct with all TOML fields (equity,
                  debt_to_equity, construction_loan_rate, permanent_loan_rate/amort, development_yield,
                  cap_rate, construction_cost_per_sf, total_sf, stabilization_year, draw_curve,
                  portfolio_class_mix[4], reporting_tier). Validate: sum of allocations = 1.0,
                  debt_to_equity in [0, 5].
- [ ] **A-D7-2** `legacy-jv` subcommand wired through `clap`; placeholder `JvData` with zeros;
                  one sample `inputs/d7_jv_config.toml` committed with default class mix.
- [ ] **A-D7-3** Bullet-list MD renderer (capital structure + portfolio mix summary only; no IS/BS/CF).

### Phase B-D7 — Legacy JV full (gate: ReportingTier enum landed; rust_decimal in use)

- [ ] **B-D7-1**  `src/legacy_jv/construction.rs`: `ConstructionDraw` struct; S-curve draw
                  schedule (Y1: 20%, Y2: 50%, Y3: 30% of total capital); equity-last waterfall;
                  IAS 23 / ASPE 3850 interest capitalisation during construction;
                  `interest_reserve_balance` tracked Y1–Y3.
- [ ] **B-D7-2**  `src/legacy_jv/transition.rs`: `PermanentLoanTransition` at Y4; construction
                  loan → $750M permanent loan; ASPE 3061 path: property under development → 
                  investment property at cost ($1,055M incl. capitalised interest);
                  IFRS path: IAS 40 FV uplift to $1,260M through P&L (if `PrivateIFRS`).
- [ ] **B-D7-3**  `src/legacy_jv/loan.rs`: `LoanSchedule` struct; 25-year amortisation at 5%
                  (annual rows Y4–Y10); DSCR computed each year (NOI / debt_service);
                  LTV computed each year (closing_balance / asset_value).
- [ ] **B-D7-4**  `src/legacy_jv/statements.rs`: full IS/BS/CF Y1–Y10 from construction through
                  operating phase. ASPE: depreciation 50-yr SL on building cost; Partners' Capital
                  Account ledger (opening + contributions − distributions + net_income = closing).
                  IS: gross revenue, opex (CAM parameters from D1 config), NOI, interest, depreciation,
                  G&A, net income. CF: distributions capped at distributable_cash.
- [ ] **B-D7-5**  `src/legacy_jv/refinancing.rs`: `RefinancingHeadroom` struct — at each
                  year Y4–Y10, compute `max_debt_at_65_ltv`, `existing_debt`, `headroom`.
                  Headroom printed as first-class output row to make the single-shot constraint
                  visible: Y4: $69M. Flag if headroom < minimum_new_round_cost (from config).
- [ ] **B-D7-6**  Snapshot integration: `JvData` produces snapshot Layers A–E; `reporting_tier:
                  PrivateASPE`; Layer E divergences include `construction_phase_no_revenue: Y1-Y3`,
                  `single_shot_constraint: true`, `refinancing_headroom_y4: $69M`.
- [ ] **B-D7-7**  Smoke tests `tests/legacy_jv.rs`: assert Y4 permanent loan = equity_contributions
                  × debt_to_equity; assert DSCR Y4 ≈ 2.10; assert LTV Y4 ≈ 0.595; assert
                  capitalised interest Y1–Y3 non-zero; assert refinancing_headroom Y4 ≈ $69M;
                  assert Partners' Capital Account closes = opens + income − distributions.

### Phase B-C — Comparison output (gate: D7 snapshot + D2 snapshot both stable)

- [ ] **B-C-1**   `src/compare/mod.rs`: `ComparisonRow` struct (label, unit, `d7_jv: [Decimal; 10]`,
                  `d2_pclp1: [Decimal; 10]`, `variance: [Decimal; 10]`, `variance_pct`). Reads two
                  snapshots (no re-running engines); dispatches from Layer C `computed.series`.
- [ ] **B-C-2**   `D7vsD2Comparison` struct: 10 `ComparisonRow`s (Total SF, Total Capital, Asset Value,
                  Debt Balance, Debt-to-Equity, LTV, NOI, Distributable Cash, Cumulative
                  Distributions, Equity MOIC). Plus `equity_irr_d7/d2`, `sf_per_equity_d7/d2`,
                  `narrative_summary` (auto-generated; planned/intended language).
- [ ] **B-C-3**   `compare` subcommand: `--left d7 --left-config inputs/d7_jv_config.toml
                  --right d2 --right-config inputs/d2_pclp1_config.toml --format md|html|json`.
                  Generic (not D7-specific): reusable for D2-vs-D4, D3-vs-D6, etc.
- [ ] **B-C-4**   HTML renderer: sortable table + stacked-bar equity composition chart (cash
                  distributed vs equity in asset) at Y4/Y6/Y8/Y10. Output header: "Illustrative
                  Comparison — D7 Legacy JV vs D2 PCLP 1 Direct-Hold". BCSC FLI disclaimer
                  in footer. NOT labelled "pro forma" (NI 52-107).
- [ ] **B-C-5**   Snapshot for comparison output: Layer A `entity: "d7-vs-d2-comparison"`;
                  Layer B includes both D7 and D2 assumption registers by reference; Layer E
                  `distribution: internal | lender | investor_om` controls suppression of
                  sensitive D2 assumptions in D7-visible output.

---

## 13. TOPIC and GUIDE drafting agenda

These will be staged to `.agent/drafts-outbound/` → project-editorial at relevant milestones.

### TOPICs (doctrine / architecture / background — bilingual EN + ES)

| Slug | Source material | Milestone |
|---|---|---|
| `topic-tool-proforma-architecture.md` | §§ 2, 7, 9 of this brief | Phase B complete |
| `topic-financial-statement-structure-d1.md` | § 5a + Excel analysis | Phase A complete |
| `topic-cam-recovery-methodology.md` | § 6 + accounting panel | Phase A complete |
| `topic-proforma-audit-chain-of-custody.md` | §§ 3a, 3b + data science panel | Phase B |
| `topic-development-yield-and-dscr.md` | §§ 5a, banking panel | Phase A |
| `topic-lp-waterfall-mechanics.md` | §§ 4, 5c + M&A panel | Phase D |
| `topic-leapfrog-2030-product-vision.md` | §§ 1, 10 + market research panel | Phase D |

### GUIDEs (operational runbooks — English only)

| Slug | Covers | Milestone |
|---|---|---|
| `guide-tool-proforma-engine-cli.md` | Subcommands, flags, output locations | Phase A |
| `guide-dev-class-config-toml.md` | How to edit DevClassConfig for new deployments | Phase A |
| `guide-proforma-number-audit.md` | Tab-by-tab verification procedure | Phase B |
| `guide-proforma-ledger-worm.md` | How to read + verify the audit ledger | Phase B |
| `guide-tool-proforma-scenario-workflow.md` | Base/upside/downside scenario procedure | Phase C |
| `guide-proforma-loan-submission-package.md` | Producing banker-ready output package | Phase C |

---

## 14. Research panel findings — D2 formula resolution (2026-05-23)

Three opus agents (accounting, finance, data science) analysed the PCLP 1 Excel workbook
cell-by-cell. The following findings are authoritative for the Rust engine implementation.

### 14a. EBITDA split — the most important engine constraint

Row 57 (`Net_Proceeds_from_Ops`) and Row 58 (`Income_Continuity`) are **two different values**
that look the same from Y4 onward but diverge completely in Y1–Y3:

| Year | Net_Proceeds_from_Ops (row 57) | Income_Continuity (row 58) |
|---|---|---|
| Y1 | $0 | $3,050,000 |
| Y2 | $0 | $3,300,000 |
| Y3 | $0 | $3,500,000 |
| Y4+ | same | same |

- **EBITDA uses row 57** (= $0 for Y1–Y3). EBITDA Y1 = $0 − $20.95M = −$20.95M.
- **Asset valuation uses row 58** (income continuity for Y1–Y3). AV Y1 = $3.05M/6.25% + $72.29M + $156.76M = $277.85M.
- If the engine merges these fields, it overstates distributable income in construction years
  AND misstates EBITDA. They must remain separate throughout.

### 14b. Income continuity — BCSC disclosure requirement

The $3.05M/$3.3M/$3.5M income continuity in Y1–Y3 represents land entitlement uplift
(zoning, permits, partial advance leases) — **not** operating income. It is used only for
asset valuation. Under BCSC NI 41-101 / 51-102:
- Must be disclosed as a fair-value/entitlement assumption, not distributable revenue
- Cannot be aggregated into distributable FFO
- Engine field: `revenue_construction_phase` with `provenance: "entitlement_uplift"` metadata
- Issue costs and financing costs are routed through P&L in the Excel (depresses EBITDA);
  under IAS 32.37 these should net against proceeds. Engine replicates the cash-view
  presentation but flags as IFRS divergence in output metadata.

### 14c. Market Value — hybrid mechanic confirmed

| Period | Formula | Rationale |
|---|---|---|
| Y1–Y3 | $100/unit (subscription price) | Fixed; capital-raising primary market |
| Y4–Y7 | Hardcoded config inputs: $125.8 / $132.1 / $171.5 / $177.3 | Sponsor-set transition assumptions; no closed-form formula; development-phase discount to NAV |
| Y8+ | DPU / 0.08 (target distribution yield) | Bond-analogue: yield-capitalised unit price |

Y8 verification: $24.09 / 0.08 = $301.15 ✓. Y9: $24.21/0.08 = $302.65 ✓. Y10: $24.33/0.08 = $304.17 ✓.
Discount/Premium = (Market Value − NAV) / NAV (labelled in row 25, values in row 24).

### 14d. Compounded Annual Return formula confirmed

`CAGR_excl_dist[y] = (Market_Value_per_unit[y] / 100)^(1/y) − 1`

This is **Market Value appreciation only** — not NAV growth, not total return including
distributions. The "Excluding Distributions" label is precise. Y8 verification:
(301.15/100)^(1/8) − 1 = 14.775% ✓.

### 14e. Three formula corrections vs. Excel parameter labels

| Parameter | Excel label | Actual formula | Correction |
|---|---|---|---|
| D31 — Debt repayment rate | "% of EBITDA" | `FFO[y] × 0.10` (row 72, not row 68) | Use FFO. Label is wrong. |
| Distributions Y4–Y7 | not labelled | `FFO × 0.90 − 0` (90% payout, retains 10%) | Payout schedule: 0%/90%/100% |
| Y7 debt draw | hardcoded `$299,729,637` | `phase3_capex_Y7 − prior_cash + min_cash` (solver) | Do not replicate Excel's literal constants |

### 14f. Distribution formula — full spec

```
payout_ratio[y]:  0.0  for Y1–Y3
                  0.9  for Y4–Y7
                  1.0  for Y8+

Dist[y] = (FFO[y] × payout_ratio[y]) − Repayment[y]

where Repayment[y] = FFO[y] × 0.10  if y ≥ 8,  else 0
```

Y8 verification: ($74.36M × 1.0) − $7.44M = $66.92M ✓.
Y4 verification: ($4.45M × 0.9) − $0 = $4.00M ✓.

### 14g. Y7 minimum-cash debt solver

Phase 3 construction ($327.375M capex) is partially funded by cash on hand from Y6
($27.96M) plus Y7 operations ($13.12M FFO × 90% net of distributions). The solver finds
the minimum gross debt draw such that Ending_Cash_Y7 ≥ min_cash ($250K):

```
Gross_Debt_Draw_Y7 = Phase3_capex_Y7 − Opening_Cash_Y7 − Net_Available_Y7 + min_cash
```

where `Net_Available_Y7` accounts for FFO minus distributions in Y7. The Excel literal
(`-816 + 80201-12030+443 = +67,798`) is a manual reconciliation plug from rounding in the
original Excel. The Rust engine must implement the principle, not the plug. Any residual
should be flagged as a reconciliation item in the audit record.

### 14h. Total Expense Ratio — operating expenses only

`TER[y] = (Advisory_Fee + Admin + Board) / NAV[y]`

Excludes: Issue Costs, Financing Costs, Net Interest. This is the institutional convention
(CFA Institute / IFRS): TER measures recurring management overhead as a fraction of NAV,
not total cost burden. Y1 verification: ($2.5M + $0.5M + $0.45M) / $277.85M = 1.24% ✓.

### 14i. Interest Coverage Ratio formula

`ICR[y] = EBITDA[y] / abs(Net_Interest[y])`

n/a for Y1–Y3 (show `"-"`). Y4 verification: $8.82M / $4.37M = 2.02 ✓.
Note: Debt Service Ratio (row 75) diverges from ICR at Y8+ when principal repayments begin.
Both are required outputs; DSR = EBITDA / (interest + principal repayment).

### 14j. AA21:AN28 investor-facing valuation block — cell-level audit (2026-05-23)

Verified by dedicated Excel/data opus agent against PCLP 1 workbook. This block is the
investor-facing summary. The Rust renderer must match it exactly.

**Column mapping (AA–AN for this summary block):**
- AA = row labels
- AB–AD = parameter / reference cells (e.g., AC23 = 0.08 target distribution yield)
- AE–AK = Y1–Y7 data columns
- AL = Y8 | AM = Y9 | AN = Y10

**Row-by-row cell audit:**

| Row | Content | Cell details |
|---|---|---|
| 21 | Net Asset Value (NAV per unit) | Formula `= (Asset_Value − Debt) / diluted_units`; all years populated |
| 22 | *(empty spacer)* | No content; visual separation between NAV and MV triad |
| 23 | Market Value per unit | AE23–AK23: **hardcoded plain numbers** (Y1–Y7); AL23–AN23: formula `=AL14/AC23` (DPU ÷ target_yield) |
| 24 | Discount/Premium values | `=(row23 − row21)/row21` each year; AE24–AG24 blank (Y1–Y3, MV = $100 = NAV → no meaningful discount); AH24–AN24 populated |
| 25 | *(label: "Discount / Premium vs. NAV")* | Label cell aligned to row 24 values; no numeric content |
| 26 | *(empty spacer)* | No content; visual separation between MV triad and return triad |
| 27 | Compounded Annual Return (Excl. Dist.) | **Only AL27 populated**: `=((AL23/100)^(1/8))-1` — single scalar exit CAGR at Y8; AM27, AN27 blank |
| 28 | Distribution Yield to Buyers at Market Value | AE28–AK28 blank (Y1–Y7); AL28–AN28: `=AL14/AL23` — tautologically 0.08 (self-consistency check, since MV = DPU/0.08) |

**Critical directionality — this is a hard engine constraint:**

Market Value is **not** derived from NAV via a discount rate. The causal chain is:

```
DPU[y]  ──÷ 0.08──►  Market_Value[y]   (Y8+, formula-driven)
NAV[y]  ──────────►  (Market_Value[y] − NAV[y]) / NAV[y]  =  Discount[y]   (emergent)
```

Discount/Premium is an *observation* — the spread between yield-capitalised MV and the
cost-basis NAV. It is never an input. Any engine implementation that treats discount as
a parameter and derives MV from `NAV × (1 + discount)` inverts the model.

**AC23 — live parameter cell:** The value 0.08 sits in cell AC23 (not hardcoded in formulas).
Y8–Y10 Market Value formulas reference AC23 directly. In the Rust engine this maps to
`target_distribution_yield: Decimal = 0.08` in DevClassConfig / DirectHoldConfig.

**Row 27 — single exit scalar, not a time series:** The CAGR formula uses exponent `1/8`
(years from capital raise to Y8). Only AL27 is emitted. The Rust renderer must leave
AM27, AN27 blank — do not extend the formula across Y9/Y10.

**Row 28 — tautological check:** `DPU/MV = DPU/(DPU/0.08) = 0.08` always for Y8+.
This is a display assertion that the yield-capitalisation is internally consistent,
not new information. Render it as a percentage; leave Y1–Y7 blank.

**Number format spec (for renderer):**
- Row 23 (Market Value): accounting format, `$000.00` per unit
- Row 24 (Discount): percentage, one decimal place, parentheses for negative (premium shown positive)
- Row 27 (CAGR): percentage, two decimal places
- Row 28 (Buyer Yield): percentage, two decimal places

---

## 15. Research panel findings — D3 WCP formula resolution (2026-05-23)

Three opus agents (accounting, finance, data science) analysed the WCP 42M Excel workbook
cell-by-cell. The following findings are authoritative for the Rust engine implementation.

### 15a. LP cascade — the core derivation chain

LP1 (WPC Canada C$250M) is the seed fund; every other LP is a FX/size/lag transform of it.
LP1 itself derives from PCLP1 (`INPUT_PCLP 1_250M` tab = mirror of the PCLP1 workbook):
- `advisory_fee_LP1[y]` = PCLP1 row 63 × deployment_ramp[y]
- `distributions_LP1[y]` = PCLP1 row 120 × 0.10 (WCP holds 10% of PCLP1)
- `nav_LP1[y]` = PCLP1 row 114 × 0.10 (row 114 = Asset_Value − Debt, total not per-unit)

Derivation for LP2–LP6: `LP_n[y] = LP1[y − launch_lag_n] × size_factor_n × fx_rate_n`

The lag means LP2 in Y2 pulls LP1's Y1 value — each LP effectively re-runs LP1's 3-year
deployment ramp from its own launch year. Out-of-bounds (pre-launch): array index = 0.

### 15b. Advisory fee deployment ramp — economic basis

The 1/3 → 2/3 → 3/3 ramp in LP1 Y1-Y2-Y3 reflects partial capital call mechanics:
only ~1/3 of committed capital is deployed in year 1, ~2/3 in year 2, fully deployed from
year 3. Standard institutional private-fund fee convention (fees on invested capital, not
committed). LP2–LP6 inherit the same ramp from their respective launch years.

### 15c. Offering Costs Reimbursement — closed form

WCP fronts LP launch costs; the LPs reimburse from advisory fees over years Y1–Y6.
The Excel formula is a rolling subtractor; the **analytical equivalent is the first difference**
of total advisory fees, floored at zero:

```
offering_costs[Y1] = advisory_fee_total[Y1]
offering_costs[y]  = advisory_fee_total[y] − advisory_fee_total[y−1]   (Y2–Y6)
offering_costs[y]  = 0                                                  (Y7+)
```

Total recovery: ~$21.78M (verifiable as sum of G53:L53). Do NOT replicate the rolling
`SUM(G53:G53)` accumulator — use first-difference form which is deterministic and testable.

### 15d. G&A ramp — full schedule

Y1–Y2: hardcoded lump sums ($750K NYC, $0/$250K Berlin respectively).
Y3–Y10: `advisory_fee_total[y] × ga_pct[y]` where:

| Year | G&A % of advisory fees | NYC split | Berlin split |
|---|---|---|---|
| Y3 | 20% | 14% | 6% |
| Y4 | 25% | 17.5% | 7.5% |
| Y5 | 30% | 21% | 9% |
| Y6 | 35% | 24.5% | 10.5% |
| Y7 | 40% | 28% | 12% |
| Y8 | 45% | 31.5% | 13.5% |
| Y9 | 50% | 35% | 15% |
| Y10 | 55% | 38.5% | 16.5% |

No cap is visible in the Excel beyond Y10. Engine caps at 55% for any year > Y10.

### 15e. P/E multiple — derivation and positioning

F81 = 10.72 is **not** a peer-derived average. Listed peer P/E data from the comparables table:
KKR=53.06, Carlyle=26.16, Blackstone=45.22, Brookfield=33.41, Apollo=19.98, Fiera=20.46.
Peer mean ≈ 33.05; peer low = 19.98. The 10.72 is approximately **one-third of peer mean**,
representing a pre-IPO / illiquidity / early-stage discount. Treat as an operator-configurable
TOML parameter (`pe_multiple`). Surface the peer comparables table alongside the valuation
output so the discount is visible to management.

### 15f. Fair Valuation formula — PEG with FCF floor

Standard Lynch PEG Fair Value = `P/E_fair = earnings_growth_pct` → `Fair Price = EPS × growth%`.
WCP formula adds two modifications:

```
fair_val_per_share[y] = (forward_growth_avg[y] × 100 × peg_ratio × eps[y])
                        + (cumulative_fcf[y] / shares)
```

1. `× 100`: decimal-to-percentage conversion (growth=0.842 → 84.2 Lynch points). Correct.
2. `+ CumFCF_per_share`: tangible-book FCF floor. Prevents collapse to zero at negative EPS.
   Represents "even if earnings are zero, the accumulated cash is worth something."

**Y1–Y2 negative result:** the formula produces negative fair values when EPS < 0. This is
correct mechanics but not suitable for investor presentations. Engine must emit
`valuation_method_warning: "negative_eps"` in the JSON for Y1 and Y2.

**Forward moving average edge case:** Y9 window = `[earnings_growth[Y10]]` (single element).
Y10 window: degenerate — reuse Y10 growth rate (matches Excel cells Q91=R91).

### 15g. Dividend Valuation — yield capitalisation

`dividend_val_per_share[y] = EPS[y] / 0.045`

This is **yield capitalisation, not Gordon Growth** (no growth term in denominator).
Economically equivalent to: "what price would a buyer pay to achieve a 4.5% yield on
WCP distributions?" Assumes 100% earnings payout — aggressive but appropriate for a
GP/asset-manager flow-through vehicle. The 4.5% is configurable (`assumed_dividend_yield`).
Bond analogue: same structure as PCLP1's `DPU / 0.08` Market Value formula.

### 15h. LP5/LP6 distribution FX anomaly

Cells J42/K47 (LP5/LP6 distributions) use the CAD-EUR rate (F12), not CAD-USD (F11),
despite both funds being denominated in USD. Advisory Fees and NAV for the same funds
correctly use F11. This is a confirmed Excel authoring error.

**Engine behaviour:** replicate the Excel exactly for audit fidelity. Emit:
```json
"metadata": {
  "distribution_fx_anomalies": ["LP5", "LP6"],
  "note": "LP5/LP6 distributions use CAD-EUR rate; expected CAD-USD for USD funds"
}
```
Override via TOML: `distribution_fx = "CadUsd"` on each LpFundConfig to correct.

### 15i. Book Value — "Woodfine Credit Inc." label

Row 75 label references "Woodfine Credit Inc." — this is a **legacy label, not a legal entity**.
Mechanically, row 75 is contributed capital (share sales) + cumulative retained earnings.
Together with row 76 (10% LP NAV), Book Value (row 77) is a hybrid:
- Parent level: cost/cash basis (contributed capital + cumulative earnings)
- LP stakes: fair-value (LP cap-rate NAV, Level 3)

This is acceptable for a GP/asset-manager. Flag in output metadata as `book_value_basis: "hybrid_parent_cost_lp_nav"`.

### 15j. Return on Net Tangible Equity — denominator choice

`RONTE[y] = earnings[y] / cumulative_fcf[y−1]`

Uses prior-year CumFCF as the equity base (not total book value including LP NAV).
Rationale: measures return on the parent's **deployed cash capital only**, isolating GP-level
operating return from LP mark-to-model gains. Defensible convention.
Y1 = `"n/a"` (no prior period); emit as dash in renderer.

---

## 16. Research panel findings — D4 Bencal SPV2 SPV formula resolution (2026-05-23)

Four opus agents (accounting, finance, data science, banking) analysed the SPV_Compensations_10 percent
Excel tab. The following findings are authoritative for the Rust engine.

### 16a. IFRS consolidation — confirmed required

Bencal SPV2-GP consolidates Bencal SPV2-LP under IFRS 10 (GP has power, variable returns, ability to affect returns).
Bencal SPV2-LP qualifies as an **investment entity** under IFRS 10.27 — it obtains funds from investors,
commits to invest for capital appreciation/income, measures performance on FV basis. Therefore:
- Bencal SPV2-LP carries PCLP1 at FVTPL (not cost basis)
- Bencal SPV2-GP still consolidates Bencal SPV2-LP (investment-entity exception applies to LP's investees, not GP's consolidation of LP)
- Single consolidated statement is appropriate and required

### 16b. Unit-based compensation — IFRS 2 treatment

33,333 manager units at $100/unit = $3,333,300 grant-date IFRS 2 charge in Bencal SPV2-LP's standalone IS.
**On consolidation: collapses to a capital transaction, not P&L.** Bencal SPV2-GP's issuance of LP units
to itself (as SPV-Manager) is an intragroup equity transaction that eliminates in consolidation.
Consolidated statement shows the 10% allocation as an income allocation line below net income, not
as an operating expense.

**Open question:** if reporting orientation is Bencal SPV2-LP with 300,000 investor units as "owners" and
33,333 manager units as NCI, the IFRS 2 charge may remain in P&L with NCI credit. EY consultation
recommended before finalising.

### 16c. Dilution formula — `ROUND(investor_units / 9, 0)` (updated 2026-05-24)

Prior formula used `ROUND(investor_units / (1 − dilution_pct), 0) − 1 = 333,333`, which biased
the manager below 10%. Corrected: `manager_units = ROUND(investor_units / 9, 0) = 33,333`;
`diluted_units = investor_units + manager_units = 333,333`. Issuance dilution = 1/9 = 11.111111̄%.
In Rust: `let manager_units = (investor_units as f64 / 9.0).round() as u64;`

### 16d. Yield waterfall — drag components at $30M

An investor's $100 in Bencal SPV2 earns ~$19.51 DPU at Y8 vs $24.09 direct PCLP1 = **19% relative drag**.

| Drag source | Mechanism | Bps lost |
|---|---|---|
| WC reserve (5.1%) | $1.53M not deployed → fewer PCLP1 units | −41 bps |
| SPV OpEx ($354,700/yr) | deducted before distributions | −123 bps |
| Manager dilution (10%) | 10% of distributions to SPV-Manager | −64 bps |
| **Total** | | **−228 bps** |

TER at $30M = **1.23%** (vs 1.70% at $10M template — fixed costs dilute over larger equity).

### 16e. 7.4% SPV Market Value yield — flag as potentially wrong

AC23 = 0.074 in the Excel. This produces an Bencal SPV2 MV that is **~8.1% higher** than PCLP1's MV
(since 0.074 < 0.08). Economically Bencal SPV2 carries more fee drag and dilution — it should trade at
a **wider** cap rate (lower MV per unit) than PCLP1, not tighter. Defensible range per banking
agent: **8.5%–9.5%**.

Possible explanations for 7.4%: (a) accidental carry-over from another template, (b) deliberate
premium for foreign-investor access or single-ticket convenience, (c) marketing assumption.
Whatever the reason, 7.4% is not arm's-length defensible for secondary transfer or estate
valuation under BC Reg. 49 / NI 51-102. **Jennifer must confirm before the engine commits to this.**

### 16f. Per-unit denominator — fully diluted (resolved 2026-05-24)

Fully diluted total (333,333) used as denominator in all per-unit metrics. No dual-column display.
The fully diluted denominator correctly shows the manager's pari-passu share in all economic
outcomes. Per-diluted and per-investor are the same economic fact — a single diluted denominator
is the clean disclosure without requiring a second column.

### 16g. Working capital interest income

D30 = **0.5%** interest earned on cash (updated per §3g — EY-calibrated baseline; was 2% in
prior Excel draft). Working capital of $1.53M earns **$7,650/yr**. This is booked in the SPV IS
(reduces effective net OpEx from $354,700 to ~$347,050). In the Rust engine,
`net_interest[y] = −avg_cash[y] × cash_interest_rate` captures this since the WC remains in the
closing_cash pool. Pure-geometry override: set `cash_interest_rate = 0.0` (line item still emitted).

### 16h. Scale invariance and engine parameterisation

All formulas scale linearly with `D15 / D43` except the fixed costs ($70K/yr Board + Admin).
`pclp1_units_held = net_equity_funding / cost_per_unit` is the central scaling variable.
The `Ad2Config` TOML must include `total_equity` as the primary operator input; all other derived
quantities computed at runtime. The `spv_market_val_yield` is a separate config field — never
aliased to or derived from PCLP1's `target_distribution_yield`.

### 16i. Related-party and BCSC disclosure requirements

- NI 45-106 Form 45-106F2/F3: manager's 10% interest = related-party compensation → disclose under Item 8
- NI 45-106: "use of available funds" must show dilution effect explicitly
- NI 33-105: single-asset feeder into a related-party LP likely triggers related-issuer disclosure
- No hurdle, no preferred return, no clawback: prominent disclosure required; do not bury in footnotes
- All forward-looking DPU projections require planned/intended/target language (BCSC posture)

---

## 17. Archived: superseded briefs

### From BRIEF-proforma-engine (.agent/briefs/) — archived 2026-05-23

> Original current-state summary. Superseded by §§ 11 + 12 above.
> Key facts preserved: binary at `/srv/foundry/cargo-target/jennifer/debug/tool-proforma-engine`;
> outputs at `/srv/foundry/clones/project-proforma/outputs/`; 2 Stage-6-pending commits
> `017a8f2d` (SPV costs) + `05b0cce6` (fmt_smart).

### From BRIEF-proforma-V2 (briefs/) — archived 2026-05-23

> Original three-deliverable spec (Jennifer's brief, 2026-05-22):
> D1 = 4-class IS/BS/CF, one landscape page per class, internal only, no sensitivity.
> D2 = Direct-Hold IS/CF/BS + Financial Forecast verbatim AA12:AN35.
> D3 = WCP IS/BS/CF + Revenue Generator + Valuation Matrix.
>
> **Delta from current spec:** flat OPEX_RATIO is replaced by itemized CAM budget (§ 6);
> simple summary is replaced by Excel-matched Proforma+Report+CAM format (§ 5a);
> internal-only posture expanded to generic + deployment separation (§ 7);
> no sensitivity expanded to Phase C (§ 8). Core 3-deliverable structure preserved.

---

## 18. Research panel findings — D5 Bencal SPV1 SPV formula resolution (2026-05-23)

4 opus agents dispatched: accounting (IFRS 9/10/2/IAS 12), finance (summary block + CAGR),
data science (Rust struct design + pipeline), banking (BCSC/NI-45-106 investor presentation).
All formulas verified against WCP_42M Excel (openpyxl read, rows 78/85/92/97 cols G:P).

### 18a. IFRS 9 classification — FVTPL is mandatory for Bencal SPV1

**Finding (accounting agent):** FVTPL is the correct classification under IFRS 9.4.1.4.
The FVOCI election (IFRS 9.5.7.5) is not appropriate because:
- FVOCI traps realised gains in OCI permanently (IFRS 9.B5.7.1); gains can never recycle to P&L
- Destroying the IS income signal defeats the purpose of an investor-facing SPV whose return is capital appreciation
- FVTPL aligns IS with NAV movement — the relevant performance metric for Bencal SPV1 investors

Irrevocable election: document FVTPL designation in accounting policies at incorporation.

**IFRS 10 investment entity status:** Bencal SPV1 does NOT qualify. IFRS 10.28 typical characteristics:
single-asset SPV with one investor pool = below threshold. Standard corporate entity (IAS 27
separate financial statements). Net accounting outcome is identical to investment-entity treatment
because WCP is measured at FVTPL in either case — but the legal framing differs.

**P&L impact:** all unrealised changes in WCP fair value flow through IS as "Net change in fair
value of investments." Transaction costs on WCP acquisition are expensed immediately (IFRS 9.5.1.1)
— not capitalised. Dividends from WCP (nil in current model) recognised in P&L when right established.

### 18b. IFRS 2 manager share grant

**Measurement:** two defensible methods at grant date:
1. **Subscription-price approach (recommended):** 333,333 × $1.00 = $333,333. Observable
   arm's-length transaction price; preferred under IFRS 2.B2.
2. **WCP look-through approach:** WCP equity at purchase × Bencal SPV1 ownership (1.5%) ÷ total Bencal SPV1 shares
   = $3,000,000 / 3,333,333 = $0.90/share × 333,333 = $300,000. Slightly lower.

**Recognition:** if immediate vesting → $333,333 expensed in Y1. If 5-year cliff → $66,666/yr Y1-Y5.
Accounting entries: Dr. Share-based compensation expense; Cr. Share capital / contributed surplus.
Net equity impact is zero (expense reduces retained earnings; credit increases paid-in capital).
**This is not a consolidation issue** — Bencal SPV1 is standalone; IFRS 2 charge is real and stays.

### 18c. Deferred tax on FVTPL unrealised gains (IAS 12)

Canadian capital gains inclusion rate = 50%; blended federal+provincial rate = 27% (BC/ON general).
Effective rate on FVTPL unrealised gains = 27% × 50% = **13.5%**.

Under ITA s. 10(1) and IFRS (IAS 12), unrealised fair-value gains on capital property are NOT
taxable until realised. This creates a temporary difference → DTL each year.

```
DTL[y]     = max(cum_unrealised_book[y], 0.0) × 0.135
           where cum_unrealised_book[y] = 300,000 × wcp.book[y] − 3,000,049.83

DTE[y]     = DTL[y] − DTL[y-1]   (positive = expense, negative = recovery)
```

⚠ DTL schedule updated for corrected 300K shares / $3,000,049.83 cost basis (Flag 5 resolved 2026-05-25):

| Year | WCP book carrying ($) | Cum unrealised ($) | DTL ($) |
|---:|---:|---:|---:|
| Y1 | 1,365,000 | −1,635,050 | 0 (clamp; no DTA) |
| Y2 | 5,430,000 | 2,429,950 | 327,743 |
| Y3 | 6,852,000 | 3,851,950 | 520,013 |
| Y4 | 8,643,000 | 5,642,950 | 761,798 |
| Y5 | 11,322,000 | 8,321,950 | 1,123,463 |
| Y6 | 6,417,000 | 3,417,000 | 461,295 |
| Y7 | 8,257,500 | 5,257,500 | 709,763 |
| Y8 | 9,886,500 | 6,886,500 | 929,678 |
| Y9 | 13,719,000 | 10,719,000 | 1,447,065 |
| Y10 | 15,772,500 | 12,772,500 | 1,724,288 |

DTA for Y1-Y2 cumulative losses unrecognised (IAS 12.24 probable-recovery test; conservative
default). Disclosed in notes. Recognised only if Jennifer confirms future taxable gains
probability assertion.

Operating expense losses accumulate as non-capital loss carryforward (20-yr per ITA s.111(1)(a))
at 27% — DTA similarly unrecognised pending probable-recovery assertion.

### 18d. Cash funding gap — material going-concern issue

**Finding (all four agents independently flagged this):**

$3M raised − $3M deployed to WCP = $0 cash. Annual opex ≈ $73-83K. No funding source.

Year-by-year cash (no reserve):
- Y0 close: $0 (post-WCP-purchase)
- Y1: −$82,649
- Y2: −$156,193
- Y10: ≈ −$744,545 cumulative

**IAS 1.25 going-concern disclosure triggered in Y1.** Auditor will likely issue emphasis-of-matter.

**Four remediation options:**

| Option | Mechanism | Impact on structure |
|---|---|---|
| (a) Reserve at close | Raise $3.75M; invest $3M in WCP; retain $750K | Investor share price stays $1.00; fewer shares or higher price |
| (b) Shareholder loan | WCP or related party advances opex; repayable at exit | Creates related-party payable; NI 61-101 related-party disclosure if applicable |
| (c) Annual capital call | Investors fund pro-rata each year | Subscription agreement clause; investor default risk |
| (d) Management fee from WCP | WCP board approves fee to Bencal SPV1 | Unusual; related-party transaction at WCP level |

The engine `initial_cash_reserve` field accepts any amount. Governance decision belongs with Jennifer.

### 18e. Summary block design — rationale for Section E four-method comparison

**Finding (finance agent):**

Primary metric is Book Value because:
1. Auditable (WCP audited equity ÷ share count; no multiple/discount-rate assumption)
2. Avoids negative Y1-Y2 income under Market/Fair/Dividend methods
3. Consistent with Bencal SPV2 pattern (D2 PCLP1 NAV drives D4 Bencal SPV2 primary; Book drives D5 Bencal SPV1)

Negative Market/Fair/Dividend values in early years: `max(150K × per_share[y], 0)` for
carrying-value. Engine retains unfloored values in JSON. IFRS 13 comment: equity FV cannot
be negative under limited liability.

Dividend method is notional — WCP pays no cash dividends. Present as supplemental with footnote:
*"Dividend Valuation is shown for analytical context. WCP does not pay cash dividends; this value
represents the implied price if WCP's earnings were distributed at the assumed yield multiple."*

**CAGR computed values (corrected for 300K shares, exact opex, Y8):**
- Cumulative Y8 opex = $82,649 + 7 × $73,544 = $597,457 (unchanged)
- D3[Y8] formula: (300,000 × wcp.book[Y8] − $597,457 − DTL[Y8]) / investor_shares (≈ 3,240,892)
- DTL[Y8] = ($19,773,000 − $3,000,049.83) × 0.135 ≈ $2,264,348

⚠ Values below supersede earlier 150K/3M panel findings:

| Method | wcp/share[Y8] | Total asset | Cum opex | DTL | NAV/inv share | CAGR |
|---|---:|---:|---:|---:|---:|---:|
| Book | $65.91 | $19,773,000 | $597,457 | $2,264,348 | ≈$5.218 | **≈22.9%** |
| Market | $33.22 | $9,966,000 | $597,457 | $923,969 | ≈$2.890 | **≈14.2%** |
| Fair | $73.22 | $21,966,000 | $597,457 | $2,561,339 | ≈$5.797 | **≈24.6%** |
| Dividend | $43.19 | $12,957,000 | $597,457 | $1,341,751 | ≈$3.395 | **≈16.5%** |

### 18f. Rust struct design highlights (data science agent)

**Consistency finding:** existing engine uses `f64` throughout (not `rust_decimal::Decimal`).
Adopting Decimal for Bencal SPV1 alone would require conversion wrappers everywhere Bencal SPV1 data touches
WcpData. Recommendation: keep `f64` for Bencal SPV1 matching the rest of the engine; open a separate
Phase B migration ticket for whole-engine Decimal adoption if Jennifer requires it.

**Existing ambassadors_d1.rs conflict:** current module computes Bencal SPV1 as 30% of WCP
(`scale_factor = 0.30`, 3M WCP shares). New spec: 1.5% (`scale_factor = 0.015`, 150K WCP shares).
These are materially different. The existing module is a **complete rewrite target** — not an extension.
Bencal Management also references the Bencal SPV1 stake; must be recalculated after Bencal SPV1 change.

**Dilution formula (exact Rust):**
```rust
fn compute_total_shares(investor_shares: u64, dilution_pct: f64) -> u64 {
    let raw = investor_shares as f64 / (1.0 - dilution_pct);
    // Excel ROUND = round-half-away-from-zero; (raw + 0.5).floor() matches for positive inputs
    (raw + 0.5).floor() as u64 - 1
}
// compute_total_shares(3_000_000, 0.10) == 3_333_332  ✓
```

**TOML config dependency:** `toml = "0.8"` must be added to `Cargo.toml` (Bencal SPV1 is the first TOML-
configured module in the engine). Alternative: JSON (already in deps). TOML recommended for its
comment-friendly format which supports audit-trail self-documentation.

**Negative-base CAGR:** return `Option<f64>`; `None` when Y8 per-share NAV ≤ 0. Matches existing
engine convention (`Pclp1Year::interest_coverage` uses `Option<f64>` for undefined ratios).

**Key files to modify:**
- Add: `src/config/ad1.rs` (Ad1Config + read fn)
- Add: `src/spv/ad1.rs` (Ad1Data, compute, derivation_json)
- Add: `src/report/ad1.rs` (renderer, distinct layout)
- Add: `tests/ad1_smoke.rs` (fixture-based smoke tests)
- Add: `inputs/ad1-config.toml` (sample configuration)
- Modify: `src/main.rs` (SpvBencal arm: add --ad1-config flag)
- Modify: `Cargo.toml` (add toml = "0.8")
- Delete/supersede: `src/spv/ambassadors_d1.rs` (legacy 30%-scale; one-cycle deprecation flag)

### 18g. BCSC / NI 45-106 compliance highlights (banking agent)

**Offering exemption:** AI exemption (NI 45-106 s.2.3) → file Form 45-106F1 within 10 days
of distribution only. OM exemption (s.2.9) not required if all investors qualify as AIs.
Form 45-106F2 and F3 are NOT triggered by AI-only distribution.

**Item 8 compensation disclosure (manager shares):**
- 333,333 shares at $1.00 = $333,333 implicit non-cash compensation
- Look-through WCP value at issuance: 333,333 × ($3M/3,333,333) = $300,000
- Disclose both; lead with higher ($333,333); characterise as non-cash share-based compensation

**NI 61-101 related-party:** does not strictly apply to private issuers not qualifying as reporting issuers. Best-practice disclosure of SPV-Manager's relationship to WCP management recommended regardless.

**FOFI boundary:** Y1-Y10 projections are at the outer edge of BCSC NI 51-102 s.4B.2 FOFI
tolerance for private placements. Consider truncating OM-facing FOFI to Y1-Y5, with Y6-Y10
clearly labelled "illustrative scenario only — not future-oriented financial information."
Full Y1-Y10 retained in management reports.

**Required footnote on Dividend Valuation:**
> "The Dividend Valuation row is shown for analytical context only. WCP does not pay cash
> dividends; this represents an implied value if WCP earnings were distributed at the assumed
> multiple. It is not a realisable cash-flow projection and should not be treated as a yield."

**Risk flags (JSON fields for engine to emit):**

| ID | Risk | Severity | JSON field |
|---|---|---|---|
| R1 | Illiquidity (no WCP public market) | High | `risk_flags.illiquidity` |
| R2 | No income / opex drag | High | `risk_flags.opex_drag` |
| R3 | Concentration (single asset, 1.5%) | High | `risk_flags.concentration` |
| R4 | Dilution (manager 333,333 shares) | Medium | `risk_flags.dilution` |
| R5 | Valuation uncertainty (Level 3, four methods) | Medium | `risk_flags.valuation_uncertainty` |

**No-hurdle mandatory disclosure language:**
> "Bencal SPV1 has no preferred return, no hurdle rate, no waterfall, and no clawback. The manager's
> 333,333 shares participate pari passu with investor shares in all economic outcomes.
> The manager's economic interest does not improve with stronger investment performance —
> it scales linearly with NAV at a fixed 10.00% share."

---

## 19. Research panel findings — D6 Bencal Management Corp. formula resolution (2026-05-23)

4 opus agents dispatched: accounting (IFRS 10.27/IAS 12/investment-entity election),
finance (portfolio composition + valuation matrix Block A-F), data science (bencal.rs rewrite
spec + Bencal SPV1 inline helpers), banking (BCSC/Form-45-106F1 compliance + commission review).
All four agents operated from the §5f spec and the existing bencal.rs source.

### 19a. IFRS 10.27 investment entity election — accounting agent

**Finding:** Investment entity election is appropriate and recommended for Bencal Management.

The IFRS 10.27 three-criteria test:
1. Obtains funds from investors to provide investment management services — **YES** (Bencal Management
   manages the manager stakes in Bencal SPV2/Bencal SPV1 on behalf of the Bencal Management shareholders).
2. Commits to invest for capital appreciation, investment income, or both — **YES** (Bencal Management's
   return comes entirely from FV appreciation of Bencal SPV2 and Bencal SPV1 stakes).
3. Measures and evaluates performance of substantially all investments on a FV basis — **YES**
   (NAV-based reporting is the only rational measurement for FVTPL equities).

Additional IFRS 10.28 typical characteristics (not all required, but support the conclusion):
- Multiple investments: NO (only 2 operating + 1 nominal) — this is the sole weakness; with
  only 2 meaningful holdings, IFRS 10.27 is closer to the boundary. Legal counsel review advised.
- Unrelated investors: NO (Bencal Management has only the SPV-Manager individual). However, no unrelated-
  investor requirement exists in 10.27 — this is a 10.28 indicator only.
- Conclusion: election is supportable; auditor will likely require a policy memo at formation.

**If election fails or Jennifer rejects it:** Bencal Management must consolidate Bencal SPV2-LP (via Bencal SPV2-GP GP
power + variable returns) and Bencal SPV1 Inc. (majority effective control via manager seat + veto).
This would require IC elimination of: advisory fees, distributions, management fee (if any),
unrealised gains on intragroup equity — vastly more complex than investment-entity treatment.

**Bencal SPV2-GP 1 share — Path B justification:**
Bencal SPV2-GP is the GP shell. Its assets are: (a) the right to earn management fees from Bencal SPV2-LP
(embedded in the fund agreement, nil separate carrying value); (b) carried interest (nil at
$30M scale given no performance waterfall). Path B: nil carrying value + annual review-engagement
note "GP interest in Bencal SPV2-LP carried at cost of $1.00 nominal." Deconsolidation from Bencal Management
justified because GP rights embedded in Bencal SPV2-LP agreement, not Bencal SPV2-GP balance sheet.

**IAS 12 — Two-layer DTL structure:**
Bencal Management has its own DTL computed at Bencal Management's own cost basis (not passing through Bencal SPV1's DTL):
- Bencal Management's cost in Bencal SPV1 shares: $0 (received as IFRS 2 compensation from Bencal SPV1 — cost basis at
  Bencal Management's own books is the grant-date FV = $333,333 × $1.00 = $333,333. See Flag 7 for confirmation).
- Bencal Management's cost in Bencal SPV2-LP units: $0 (received as IFRS 2 compensation from Bencal SPV2-LP — cost basis
  = 33,333 × $100 = $3,333,300, but the actual cash outflow was $0; IFRS 2 charge is recognised
  by Bencal SPV2-LP not Bencal Management).
- Accounting agent flags: the IFRS 2 cost basis question is non-trivial. If Bencal Management's cost basis
  in Bencal SPV1 shares is $333,333 (grant-date FV of IFRS 2 charge), then DTL = max(FV_AD1 − $333K, 0) × 0.135.
  If basis is $0 (Bencal Management had zero cash outflow), DTL = max(FV_AD1, 0) × 0.135. **This is Flag 4.**

### 19b. Portfolio composition and valuation matrix — finance agent

**Block A — Portfolio composition design:**
The primary output for Bencal Management is a portfolio composition table, not an income statement cascade.
Both holdings are passive; the IS adds little information. Block A shows the year-by-year
NAV for each holding, the total, and the % split:

At Y8 (illustrative, Book method):
- FV_AD2_LP ≈ 10% × Bencal SPV2.nav_total[Y8]
- FV_AD1 ≈ 10% × Bencal SPV1.nav_book[Y8] = 10% × ($9,886,500 − $597,457 − $929,678) = 10% × $8,359,365 = **$835,937**
- Total Bencal Management NAV ≈ (Bencal SPV2 component) + $835,937

**Block D — 4-method valuation matrix:**
Each of the 4 valuation methods (Book/Market/Fair/Dividend) propagates from Bencal SPV1's per-share
series. For Bencal SPV2-LP, only NAV (Book method) is available — PCLP1 has only one Book NAV track.
Block D therefore has:
- Bencal SPV2 sub-columns: Book only (repeated in all 4 sub-tables with Market/Fair/Div rows marked "—")
- Bencal SPV1 sub-columns: 4 methods (from WCP per-share × 150K × 10% ownership)

**MOIC caution:** Bencal Management was formed for $10.00. MOIC = total NAV / $10 = enormous multiple
with no economic meaning (the $10 was not the economic investment — the IFRS 2 compensation
at Bencal SPV2/Bencal SPV1 level was the economic cost). Block F should show CAGR on the *economic cost*
($333,333 + $3,333,300 implicit IFRS 2 value), not the $10 par value. Flag 3 confirmed.

**Wrapper drag vs WCP direct — Block C:**
The WCP effective exposure via Bencal Management is 15,000 WCP shares (150K × 10%):
```
direct_wcp_nav = 15,000 × wcp.book[y]
bencal_ad1_nav = FV_AD1[y]   (= 10% × Bencal SPV1.nav_book)
wrapper_drag_C = direct_wcp_nav - bencal_ad1_nav
```
At Y8 (Book): $65.91 × 15,000 = $988,650 direct; $835,937 via Bencal SPV1 at Bencal Management; drag = $152,713.
This drag captures Bencal SPV1 opex + DTL + manager dilution, measured from Bencal Management's 10% stake.

### 19c. Rust implementation — data science agent

**Current bencal.rs errors (must fix before running):**

| Current constant | Wrong value | Correct value |
|---|---|---|
| `BENCAL_AD1_STAKE` (0.10) | Conceptually OK but unused correctly | Replace with inline formula |
| `AD1_WCP_STAKE` (0.30) | Wrong — Bencal SPV1 holds 1.5% of WCP (150K shares) | Remove; use AD1_WCP_SHARES_HELD |
| `BENCAL_AD2_UNITS` (25_000) | Old $25M fund | 33_332 |
| `COMMISSION_PER_YEAR` (100_000) | Model artefact — dropping | Remove (Flag 6) |
| `shares_outstanding` (2.0) | Correct | Keep |
| `price_per_share` (0.0) | Wrong | 5.00 |

**Bencal SPV1 inline helper — full spec (4 methods):**
```rust
fn ad1_cum_opex(y: usize) -> f64 {
    // y is 0-indexed; Y1 = y=0
    82_649.0 + (y as f64) * 73_544.0
}

fn ad1_nav_by_method(wcp: &WcpData, method: ValuationMethod, y: usize) -> f64 {
    let per_share = match method {
        ValuationMethod::Book     => wcp.book.book_value_per_share[y],
        ValuationMethod::Market   => wcp.market.market_value_per_share[y],
        ValuationMethod::Fair     => wcp.fair_div.fair_value_per_share[y],
        ValuationMethod::Dividend => wcp.fair_div.dividend_value_per_share[y],
    };
    let asset_value = (AD1_WCP_SHARES_HELD * per_share).max(0.0);
    let cum_unrealised = (asset_value - AD1_COST_BASIS).max(0.0);
    let dtl = cum_unrealised * AD1_DEFERRED_TAX_RATE;
    (asset_value - ad1_cum_opex(y) - dtl).max(0.0)
}
```

**Revenue Generator — reduce to 2 rows:**
```
Row 1: "Bencal SPV2-LP Position (33,333 units)"   — FV_AD2_LP[y] for Y1-Y10
Row 2: "Bencal SPV1 Inc. Position (333,333 shares)" — FV_AD1[y] (Book method) for Y1-Y10
```
Remove all legacy rows referencing LP funds (those belong to D3 WCP Revenue Generator).

**Per-share fields:** `[BENCAL_SHARES_OUTSTANDING as usize; 1]` → no longer `[0.0; 10]`.
At Y10, per-share NAV = total_bencal_nav[Y10] / 2.0. Large number; add footnote in renderer.

**Integration order in main.rs SpvBencal arm:**
1. Compute WcpData (D3)
2. Compute PCLP1Data (D2) [needed for Bencal SPV2]
3. Compute Ad2Data (D4) [needs PCLP1Data]
4. Compute BenCalData (D6) [needs WcpData + Ad2Data]
When Ad1Data (D5) struct exists, pass it directly; until then, use inline helpers.

### 19d. BCSC / Form-45-106F1 compliance — banking agent

**Offering exemption for Bencal Management shares:** Bencal Management is an internal vehicle (2 shares; owners are
the SPV-Manager individuals). No distribution of Bencal Management shares to outside investors.
No Form 45-106 filing triggered by formation. If Bencal Management shares are ever transferred to an
unrelated party, Form 45-106F1 required within 10 days (AI exemption).

**Commission income — strong DROP recommendation:**
If Bencal Management charges Bencal SPV2/Bencal SPV1 a management/commission fee, this creates:
- Related-party compensation between a controlled entity (Bencal Management) and the funds it manages
- NI 45-106 Item 8 escalation: "compensation" to the manager increases required disclosure
- Economic conflict: Bencal SPV2/Bencal SPV1 investors bear the fee; they already bear the 10% dilution
- The fee is economically duplicative — the 10% manager stake *is* the compensation

Banking agent finding: the 10% dilution stake is the intended total economic return to Bencal Management.
Adding a commission layer creates a disclosure problem without commensurate economic logic.
**Engine default: `commission_enabled = false`. Override requires explicit Jennifer confirmation.**

**GP ring-fencing (Bencal SPV2-GP):**
The banking agent recommends a formal Management Services Agreement between Bencal SPV2-GP and
Bencal Management (or the SPV-Manager's personal operating company). This ensures:
- Personal liability of Bencal Management's shareholders is not exposed to Bencal SPV2-LP investor disputes
- D&O insurance flows to the correct entity
- Any future management fee is properly documented as an arm's-length MSA charge

**Transfer restrictions on Bencal Management shares:**
If one of the two Bencal Management shareholders transfers their share to a third party, the new holder
becomes the SPV-Manager for Bencal SPV2 and Bencal SPV1. Bencal SPV2-LP and Bencal SPV1 Inc. investor agreements should
include a "manager-change event" clause requiring investor approval (majority or unanimous).
This protects investors from undisclosed changes in the manager identity.

**Annual review engagement:**
At Bencal Management's scale ($10 paid-in, FV assets), a full audit is unwarranted. Annual review
engagement under CSRE 2400 (or CSRS 4200) is appropriate and sufficient. Estimated cost:
$5K–$10K/year. Include in Bencal Management opex model as `review_engagement_fee` TOML field.

---

## 20. Journal entry + trial balance — consolidated research (2026-05-23)

*All JE/TB research consolidated here for Jennifer's review. Pending ratification before §5g
is promoted to implementation spec. Panel sources: `je-tb-panel-2026-05-23` (4 opus agents:
accounting, finance, data science, banking) plus synthesis from D2–D6 entity panels.*

### 20a. Panel verdict — phase placement and key decisions

| Decision | Answer | Rationale |
|---|---|---|
| Add JE/TB to engine? | YES | Genuine moat; assurance workflow accelerant |
| Phase placement? | Phase B | Phase A IS/BS/CF correctness must complete first |
| Bidirectional JE ↔ macro? | Deferred Phase D/E | Anti-pattern without Svelte UI + `rust_decimal` |
| Override::Variable (macro → JE)? | Phase B YES | Safe direction; TOML `[[overrides]]` table |
| Entry tier ($19) gets JE? | NO | Noise, not value, for small-dev community members |
| Pro tier ($99) gets JE? | YES | CPA-firm wedge; CSAE 3420 engagement accelerant |
| Enterprise gets Override::Journal? | Phase D/E | Requires Svelte UI + exact arithmetic |
| Regulatory requirement? | NO | NI 45-106, NI 41-101, CSAE 3420 = presentation-level |
| Competitive moat? | YES | Forward-looking double-entry bookkeeping — nobody else does it |

Phase A gate: GL account tagging only (one `gl_account_id` field per IS/BS/CF line; zero
rendering change). Phase B: full JE templates + TB generation. See B-JE-1 through B-JE-14 in §12.

### 20b. Chart of accounts — 4-digit canonical scheme

```
1000–1999  Assets
  1110  Cash and equivalents
  1310  Rents receivable
  1320  SL rent receivable (IFRS 16 straight-line)
  1410  Investments FVTPL — LP units / Bencal SPV2-LP position
  1510  Investments equity-method (WCP LP stakes)
  1610  Investment — WCP common shares (FVTPL) / Bencal SPV1 position
  1810  Investment property (IAS 40 FV model)

2000–2999  Liabilities
  2110  Accounts payable
  2120  Accrued advisory fee
  2210  Mortgage / construction loan
  2310  Org costs payable
  2510  Deferred tax liability (IAS 12)
  2610  Distributions payable
  2810  Shareholder loan payable

3000–3999  Equity
  3110  LP capital — Class A units / Share capital
  3120  LP capital — Class B units (manager / GP)
  3210  Retained earnings
  3310  AOCI (reserved; currently N/A)

4000–4999  Revenue
  4110  Base rent income / Distribution income received
  4120  Lease incentive / SL adj.
  4210  Advisory income / Interest income
  4220  Offering costs reimbursement
  4310  LP equity pickup income
  4910  Unrealised FV gain (FVTPL / IAS 40)
  4920  Realised FV gain (IAS 40 disposal)

5000–6999  Expenses
  5210  Management fee
  5220  Advisory fee / Referral fees (WPI)
  5310  CAM / G&A operating expenses
  5410  Org cost amortisation
  5910  Admin / legal / accounting

7000–7999  Tax (LP entities — flow-through; not used)
  7210  Deferred tax expense (IAS 12)
  7310  Current income tax expense

8000–8999  OCI
  8110  AOCI movements (reserved)
```

**Entity-type variations:**
- LP entities (D2 PCLP 1, D4 Bencal SPV2-LP): no 7xxx accounts — flow-through; tax at investor level.
- Operating company (D3 WCP Inc.): 7xxx active; LP stakes held at equity-method (1510).
- FVTPL corporations (D5 Bencal SPV1 Inc., D6 Bencal Management): 7xxx active; 3310 AOCI reserved (not used —
  FVTPL elections eliminate OCI recycling).
- Investment property entities (D1, D2): 1810 active; bifurcate 4910 unrealised / 4920 realised.

### 20c. GL account tagging — Phase A prerequisite

Every IS/BS/CF line item in every entity data struct receives a companion constant:

```rust
pub const GL_UNREALISED_GAIN:  &str = "4910-unrealised-fv-gain";
pub const GL_ADVISORY_FEE:     &str = "5220-advisory-fee";
pub const GL_DEFERRED_TAX_EXP: &str = "7210-deferred-tax-expense";
pub const GL_DTL:              &str = "2510-dtl";
// one constant per IS/BS/CF line item
```

One-line annotation per field — no rendering change, no new output file. Phase A gate for
Phase B JE generation. Tag D2/D5/D6 during Phase A (IS structures stable); defer D1 until A12
(report summary renderer) completes. See B-JE-1.

### 20d. Rust type definitions — JE / GL / TB

```rust
pub struct JournalEntry {
    pub je_id:            String,      // ULID (Phase B)
    pub period_year:      u16,
    pub account_id:       String,      // e.g. "1610-investment-wcp-fvtpl"
    pub direction:        Direction,
    pub amount:           f64,
    pub description:      String,
    pub source_line_item: String,      // "unrealised_gain[y=3]"
    pub source_kind:      SourceKind,
}

pub struct GeneralLedger { pub entity: String, pub period_year: u16, pub entries: Vec<JournalEntry> }

pub struct TrialBalance {
    pub entity: String, pub period_year: u16, pub as_of: String,
    pub rows: Vec<TrialBalanceRow>, pub total_debits: f64, pub total_credits: f64,
}

pub struct TrialBalanceRow {
    pub account_id: String, pub account_name: String, pub kind: AccountType,
    pub debit_balance: f64, pub credit_balance: f64,
}

#[derive(Debug, Clone, Copy)] pub enum Direction   { Debit, Credit }
#[derive(Debug, Clone, Copy)] pub enum AccountType { Asset, Liability, Equity, Revenue, Expense }
#[derive(Debug, Clone, Copy)] pub enum SourceKind  { Derived, OverrideManual, OverrideCsv }

/// BalancedJE invariant: Σ Dr = Σ Cr within $0.005. Constructor rejects unbalanced input.
pub struct BalancedJE(Vec<JournalEntry>);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Override {
    Variable { name: String, year: u16, value: f64 },
    Journal  { je_id: String, new_amount: f64 },
    Balance  { account_id: String, year: u16, value: f64 },
}
```

### 20e. Journal entry mappings — all entities

Annual period: one JE set per entity per fiscal year (10 total). Not monthly.

**D1 dev classes — deferred.** JE template shapes depend on final IS line structure from Phase A
CAM itemisation (A1–A12). Tag GL accounts during Phase A; write JE templates in Phase B after A12.

**D2 PCLP 1 — 9 annual journal entries:**

| # | Description | Dr | Cr |
|---|---|---|---|
| 1 | Capital call | 1110 Cash | 3110 LP Capital Class A |
| 2 | Deploy capital | 1410 Investments FVTPL | 1110 Cash |
| 3 | Distribution income from assets (Y4+ only) | 1110 Cash | 4210 Interest / Distribution Income |
| 4 | FV step-up (IAS 40 / FVTPL) | 1410 Investments FVTPL | 4910 Unrealised FV Gain |
| 5 | Mgmt fee accrual | 5210 Mgmt Fee | 2110 AP |
| 6 | Advisory fee accrual | 5220 Advisory Fee | 2120 Accrued Advisory Fee |
| 7 | Distribution declared to investors | 3210 Retained Earnings | 2610 Distributions Payable |
| 8 | Distribution paid | 2610 Distributions Payable | 1110 Cash |
| 9 | Org cost amortisation | 5410 Org Cost Amortisation | 1410 Investments FVTPL (contra) |

D2 is an LP — no 7xxx tax accounts. Y1–Y3 (construction): entry 3 = $0; entries 1 + 2 dominate.

**D3 WCP Inc. — 7 annual journal entries:**

| # | Description | Dr | Cr |
|---|---|---|---|
| 1 | Advisory fee income received | 1110 Cash | 4210 Advisory Income |
| 2 | Offering costs reimbursement received | 1110 Cash | 4220 Offering Costs Reimb. |
| 3 | Referral fees (WPI) expense accrual | 5220 Referral Fees | 2110 AP |
| 4 | G&A expense accrual | 5310 G&A Expenses | 2110 AP |
| 5 | Current income tax | 7310 Current Tax Expense | 2110 Tax Payable |
| 6 | LP equity pickup (10% of each LP NAV increase) | 1510 Investments equity-method | 4310 LP Equity Income |
| 7 | Year-end close | 4xxx–7xxx balances | 3210 Retained Earnings |

D3 is a 27% corporate tax entity. Entry 6: cash distributions from LP funds reduce the 1510
carrying value when received (Dr 1110 / Cr 1510), not separate income.

**D4 Bencal SPV2-LP — 8 annual journal entries:**

| # | Description | Dr | Cr |
|---|---|---|---|
| 1 | Capital call from investors | 1110 Cash | 3110 LP Capital Class A |
| 2 | Purchase PCLP1 units | 1410 Investments FVTPL | 1110 Cash |
| 3 | Distributions received from PCLP1 | 1110 Cash | 4110 Distribution Income |
| 4 | FV step-up (NAV of PCLP1 units) | 1410 Investments FVTPL | 4910 Unrealised FV Gain |
| 5 | Advisory fee accrual | 5220 Advisory Fee | 2120 Accrued Advisory Fee |
| 6 | Admin / board expenses | 5910 Admin-Legal | 2110 AP |
| 7 | Distribution declared to investors | 3210 Retained Earnings | 2610 Distributions Payable |
| 8 | Distribution paid | 2610 Distributions Payable | 1110 Cash |

D4 is an LP — no 7xxx tax accounts. Pattern mirrors D2; D4 invests in PCLP1 units rather than
property assets directly. Manager units (33,333) in 3120 LP Capital Class B.

**D5 Bencal SPV1 Inc. — 5 annual journal entries:**

| # | Description | Dr | Cr | Amount |
|---|---|---|---|---|
| 1a | WCP purchased shares (Y1 only) | 1610 Investment-WCP-FVTPL | 1110 Cash | $3,000,000 |
| 1b | WCP bonus shares (Y1 only) | 1610 Investment-WCP-FVTPL | 1110 Cash | $49.83 |
| 2 | FVTPL remeasurement gain | 1610 Investment-WCP-FVTPL | 4910 Unrealised-FV-Gain | unrealised_gain[y] |
| 2b | FVTPL remeasurement loss | 4910 Unrealised-FV-Gain (contra) | 1610 Investment-WCP-FVTPL | \|loss[y]\| |
| 3 | DTL increase | 7210 Deferred-Tax-Expense | 2510 DTL | dte[y] |
| 4 | Opex accrual | 5910 Admin-Legal | 2110 AP | opex[y] |
| 5 | Year-end close | 4xxx–7xxx balances | 3210 Retained-Earnings | net_income[y] |

**D6 Bencal Management Corp. — 7 annual journal entries:**

| # | Description | Dr | Cr | Amount |
|---|---|---|---|---|
| 1 | Formation (Y1 only) | 1110 Cash | 3110 Share Capital | $10 |
| 2 | FV change — Bencal SPV2-LP position | 1410 Investment-Bencal SPV2-LP-FVTPL | 4910 Unrealised-FV-Gain | fv_ad2[y] − fv_ad2[y−1] |
| 3 | FV change — Bencal SPV1 position | 1610 Investment-Bencal SPV1-FVTPL | 4910 Unrealised-FV-Gain | fv_ad1[y] − fv_ad1[y−1] |
| 4 | Distributions from Bencal SPV2 (income; cost basis $0) | 1110 Cash | 4110 Distribution Income | dist_ad2[y] |
| 5 | DTL increase | 7210 Deferred-Tax-Expense | 2510 DTL | dte[y] |
| 6 | Opex accrual | 5910 Admin-Legal | 2110 AP | opex[y] |
| 7 | Year-end close | 4xxx–7xxx balances | 3210 Retained-Earnings | net_income[y] |

Bencal Management uses 1410 (Bencal SPV2-LP units, FVTPL) and 1610 (Bencal SPV1 shares, FVTPL) separately to track
the two FVTPL positions. Bencal SPV2-GP share carried at $nil — no JE.

### 20f. Trial balance format

- **Annual point-in-time:** 10 TBs per entity (one per fiscal year).
- **Columns:** Account# | Account name | Type | Opening Dr/Cr | Period Dr/Cr | Closing Dr/Cr | IS/BS tag.
- **Invariants enforced:**
  - Σ(Closing Dr) = Σ(Closing Cr) per year (BalancedJE newtype).
  - IS = period activity of 4xxx–7xxx accounts.
  - BS = closing balances of 1xxx–3xxx accounts.
  - CF closing cash = TB 1110 closing balance.
- **TB-to-IS/BS bridge:** every account maps to exactly one IS/BS/CF line; emitted as CSV/JSON
  alongside TB on `--with-tb`. Enables CaseWare / Working Papers ingestion.

### 20g. Bidirectionality — Override::Variable vs Override::Journal

**Override::Variable (Phase B):** operator edits a macro assumption in TOML `[[overrides]]`
table; engine regenerates all downstream JEs for affected years. Safe direction — always
supported. WORM override log captures `{year, assumption_name, original, override, reason}`.

**Override::Journal (Phase D/E, enterprise only):** operator edits a JE amount; engine
back-solves the affected assumption for that year only. Requires: (1) Svelte UI with explicit
override confirmation; (2) `rust_decimal` migration (f64 errors compound in back-solve);
(3) WORM log entry per JE edit. Deferral rationale: back-solve without UI creates ambiguous
intent — operator cannot confirm which assumption was recalibrated in a CLI-only flow.

### 20h. IFRS mechanics currently unrepresented without GL

Eight cases where IS/BS/CF cannot cleanly represent IFRS mechanics without distinct GL accounts.
Priority order for Phase B JE implementation:

1. IFRS 16 straight-line rent receivable (D1): Dr 1320 SL-Rent-Receivable / Cr 4110 Base-Rent
2. IAS 40 FV gain bifurcation (D1, D2): separate 4910 Unrealised from 4920 Realised
3. IAS 12 DTL movements (D5, D6): opening DTL + current movement + closing (3-line TB pattern)
4. Equity-method pickup vs distribution (D3, D6): pickup increases 1510; distribution reduces it
5. Org cost amortisation (D2): capitalised contra; amortised 5-year SL
6. IFRS 9 ECL on receivables (D1): Dr 5xxx ECL / Cr 1xxx Allowance
7. Performance allocation clawback (D2): accrual + reversal in liability account
8. AOCI vs RE separation: 3310 AOCI required if FVOCI ever elected (currently N/A; reserved)

### 20i. Competitive positioning and stakeholder value

**Moat:** ARGUS ($8K–$15K/user/year) produces GAAP/cash IS/BS/CF but no GL or JE. No known
real estate proforma tool generates forward-looking double-entry bookkeeping from model outputs.

**Woodfine:** JE/TB converts the proforma into a full accounting-cycle deliverable. A CPA firm
can tie every TB balance back to a TOML assumption in one lookup. Reduces review-engagement
preparation from days to hours.

**Pro-tier CPA firms ($99/month):** `assumption_ref` field on each TB row. ROI: $15,000–$40,000/
year in recovered CSAE 3420 billable hours at $99/month — 12–34× return on the subscription.

**Entry tier ($19/month):** JE/TB adds noise, not value. Correctly excluded from Entry tier.

**Assurance partners:** CSAE 3420 `assumption_ref` creates direct linkage from assurance subject
(the projection) to evidence (TOML assumption file, version-controlled + hash-linked). Reduces
documentation burden materially.

### 20j. Accounting agent findings

**BalancedJE constructor:**
```rust
impl BalancedJE {
    pub fn new(entries: Vec<JournalEntry>) -> Result<Self, LedgerError> {
        let debits:  f64 = entries.iter().filter(|e| e.direction == Direction::Debit ).map(|e| e.amount).sum();
        let credits: f64 = entries.iter().filter(|e| e.direction == Direction::Credit).map(|e| e.amount).sum();
        if (debits - credits).abs() > 0.005 {
            return Err(LedgerError::Unbalanced { debits, credits });
        }
        Ok(BalancedJE(entries))
    }
}
```
Enforces Σ Dr = Σ Cr within $0.005 at constructor time, before any TB computation.

**D2 construction vs operating years:** In Y1–Y3, entry 3 (distribution income) = $0 because
`net_proceeds_from_ops = 0`. TB pattern: large 1410 + 3110, no 4210 income — matches IS/BS.

**IFRS 16 SL rent receivable:** highest Phase B priority after D1 IS structure finalises;
D1 NOI splits depend on it. IAS 12 DTL 3-line TB pattern (D5, D6) needed for CSAE 3420.

### 20k. Finance agent findings

**Pro-tier ROI:** 10 CSAE 3420 engagements/year × 2–3 days saved at $150–$250/hr =
$15,000–$40,000/year recovered at $99/month SaaS cost. 12–34× ROI. High willingness to pay;
low churn once integrated into the engagement workflow.

**Verdict:** JE/TB is not a regulatory requirement but a genuine competitive moat. The
assurance-workflow case alone justifies Phase B inclusion.

### 20l. Data science agent findings

**Phase gate:** B-JE-1 is the only Phase A item. Tag D2/D5/D6 during Phase A; defer D1 until A12.

**Override::Variable sketch:**
```rust
#[derive(Deserialize)]
pub struct Ad1Config {
    #[serde(default)]
    pub overrides: Vec<OverrideRecord>,
}
#[derive(Deserialize)]
pub struct OverrideRecord { pub year: u16, pub field: String, pub value: f64, pub reason: String }
```

**`je_merkle_root`:** Merkle root of all JE hashes for a run stored in the run record.
Re-running with different assumptions produces a different root even if IS/BS totals are
identical — per-JE tamper evidence.

**Module layout:**
```
tool-proforma-engine/src/je/
├── types.rs  coa.rs  d2_pclp1.rs  d3_wcp.rs  d5_ad1.rs
├── d6_bencal.rs  trial_balance.rs  render.rs  worm.rs
```
D1 deferred until Phase A CAM itemisation finalises IS line structure.

### 20m. Banking / regulatory agent findings

**NI 45-106 / NI 41-101:** JE/TB not mandated. Required CSAE 3420 contents: (a) assumptions,
(b) accounting policies, (c) basis of preparation, (d) financial statements. No JE required.

**CSAE 3420 accelerant:** `assumption_ref` makes engagement evidence directly traceable.
30–50% reduction in engagement time per filing estimated.

**Competitive landscape:** ARGUS — no GL/JE. Dealpath — no accounting output. REFM — manual
JE entry in separate system. JE/TB is an unoccupied position in the competitive landscape.

**Override::Journal deferral — correct:** a JE-level override that silently recalibrates a
macro assumption without explicit UI confirmation is a BCSC disclosure risk. Phase D/E with
Svelte UI is the correct gate.

---

## 21. Research panel findings — calculation snapshot architecture (snapshot-panel-2026-05-23)

Panel: 4 opus agents. Question: should every engine run produce a `*.snapshot.json` capturing
all inputs, intermediate calculations, and outputs? Should the HTML be rendered purely from
that JSON? What is the accounting, finance, data science, and regulatory verdict?

**Panel verdict:**
| Decision | Answer |
|---|---|
| JSON as canonical artifact (HTML = pure renderer)? | YES |
| Ship JSON at all tiers or gate it? | All tiers get JSON; gate provenance richness only |
| "Give to any AI" phrasing? | DROP — replace with "structured for local SLM + Big 4 audit tools" |
| CSAE 3420 sufficient from JSON alone? | NO — needs paired basis-of-preparation + mgmt rep docs |
| IP risk from full derivation chain in JSON? | LOW — math is public-domain IFRS/ASC |
| Phase placement? | Phase B (snapshot generation); HTML renderer decoupling = Phase A gate |
| Competitive moat? | YES — unoccupied position; 3 years uncontested |

### 21a. Accounting agent — CSAE 3420 + 5-layer schema

**CSAE 3420 requirements:** The standard requires (a) a basis of preparation, (b) a list of
assumptions and their source/authority, (c) accounting policies, (d) financial statements, and
(e) a practitioner's independent report. A `snapshot.json` alone is not sufficient for a CSAE
3420 engagement — it must be paired with `basis_of_preparation.md`, `management_representation.md`,
and practitioner's working papers.

**What the snapshot enables:** Layer B (assumption register) directly populates CSAE 3420
Exhibit A (assumption list). Layer E (compliance overlay) surfaces divergences and flags
requiring disclosure. The `--disclosure-grade` flag can enforce completeness gates before
the snapshot is handed to an assurance partner.

**Five-layer schema rationale:**
- Layer A metadata allows re-identification of the exact binary, input file, and git commit
  that produced any output — critical for restating a report produced two years ago.
- Layer C derivation chain makes the "black box" of the Rust engine auditable without
  requiring the auditor to read Rust code. Formula strings + input IDs are sufficient for
  a working-paper reference.
- Layer D statement values as `values_ref` pointers (not duplicated numbers) prevents
  data integrity errors where IS total ≠ TB subtotal.
- Layer E compliance overlay is the bridge to IFRS disclosure notes — it lists known
  divergences so an auditor's checklist can be generated programmatically.

**IFRS 13 / Level 3 hierarchy:** cap rate sensitivity table in Layer E (±25/50/100 bps →
NAV impact per entity) is required IFRS 13 disclosure. Having it as structured JSON rather
than a prose footnote means it can be validated programmatically and linked back to the
assumption that drives it.

**`disclosure_grade: true` gate:** all Layer B `source_type`, `source_id`, and `source_date`
fields must be populated before the flag can be set to true. This prevents an engagement
from being handed to EY/Deloitte with an incomplete assumption register.

### 21b. Finance agent — stakeholder value + IP risk

**Stakeholder value by tier:**
- *Woodfine internal:* replayable snapshots allow scenario comparison without re-running
  the engine. Baseline snapshot vs. upside snapshot can be diffed to show exactly which
  assumption changed and what the IS/BS impact was. This is the "version control for
  financial models" positioning.
- *Lender package:* Layer A metadata (binary_sha256, cargo_lock_sha256, run_id) gives a
  lender the ability to independently verify the output was produced by a specific signed
  binary — stronger chain of custody than an Excel file.
- *LP investor OM:* `distribution: investor_om` in Layer A triggers suppression of
  confidential internal assumptions (management fee structures, cost basis). Investors see
  a clean output; internal snapshot retains full detail.
- *EY/Deloitte working papers:* Layer B `assumption_ref` fields link every JE to its
  source assumption — estimated 30–50% reduction in engagement time per filing.
- *Community members (Entry tier):* T1 provenance (SHA-256 + SSH signature) provides more
  chain-of-custody than any current tool in the $0–$500/month RE proforma space.

**IP risk assessment — LOW:** The mathematical content of the derivation chain (cap rate
yield, IRR Newton-Raphson, IFRS 9 ECL mechanics, IAS 40 reconciliation) is public-domain
financial mathematics. Publishing the formula strings in the snapshot does not reveal a
protectable trade secret. The competitive moat comes from the integration, the audit chain,
and the pricing — not from secrecy of the formulas.

**"Give to any AI" phrasing — DROP:** the correct framing is "structured for local SLM
(Foundry tier) and in-house Big 4 audit tools." External AI distribution would trigger
BCSC selective-disclosure risk: if an investor's AI system can extract forward-looking
projections from a snapshot and act on them before public disclosure, that is potential
selective disclosure under NI 51-102. The Foundry local SLM tier is the safe boundary.

**Competitive moat:** no current RE proforma tool ships a structured, replayable, auditable
JSON snapshot as a first-class artifact. ARGUS ships binary `.xcf` files; REFM ships Excel.
The snapshot architecture is an unoccupied position. Panel estimate: 3 years before
meaningful competition appears, assuming Phase B ships within 12 months.

### 21c. Data science agent — Rust schema design + sizing

**Module layout:**
```
tool-proforma-engine/src/snapshot/
├── schema.rs        — typed structs for all 5 layers; schemars derive
├── ids.rs           — stable ID namespaces; newtype wrappers
├── derivations.rs   — Derivation struct; BalancedDerivation validator
├── builders/
│   ├── d1.rs        — D1 DevClass snapshot builder
│   ├── d2.rs        — D2 PCLP1 snapshot builder
│   ├── d3.rs        — D3 WCP snapshot builder
│   ├── d4.rs        — D4 Bencal SPV2 snapshot builder
│   ├── d5.rs        — D5 Bencal SPV1 snapshot builder
│   └── d6.rs        — D6 Bencal Management snapshot builder
└── layout.rs        — FmtTag enum; row emphasis; bilingual labels

tool-proforma-render/src/
├── main.rs          — reads snapshot.json; routes to HTML/MD renderer
├── html.rs          — HTML renderer; zero engine dependency
└── migrate.rs       — schema_version N-1 MAJOR migration codepath
```

**Sizing estimates (D2 as reference entity, 9 JEs, 10 years):**
| Mode | Raw JSON | gzip |
|---|---|---|
| Slim (no per-year trace) | ~28 KB | ~7 KB |
| Full (with `trace[y]`) | ~64 KB | ~16 KB |
| Full SPV pack D2+D4+D5+D6 slim | ~110 KB | ~25 KB |

These are well within browser single-page-load budgets and suitable for email attachment
(lender package) even without compression.

**`schemars` CI diff-gate:** `cargo test --test schema_diff` compares `schema/snapshot.v1.0.json`
against the live `schemars`-derived schema. If they diverge, CI fails. This prevents silent
schema drift between releases and gives Big 4 partners a stable JSON schema to integrate against.

**`BalancedDerivation` validator:** every derivation that represents an accounting identity
(e.g., `net_income = revenue - expenses`, `assets = liabilities + equity`) gets a validator
that asserts `|sum(inputs) - output| < f64::EPSILON * scale`. Caught at build time, not at audit.

**Migration path:** `migrate-snapshot` function accepts a snapshot at schema version N-1 MAJOR
and transforms it to the current version. This means a snapshot generated in Phase B remains
renderable by the Phase D `tool-proforma-render` binary without re-running the engine. Critical
for legal hold scenarios where the original engine binary may no longer be available.

**Phase A gate item:** `B-SS-1` (schema.rs + schemars derive) should be listed as a Phase A
gate for `tool-proforma-render` — the HTML renderer cannot be built until the snapshot schema
is stable. This means schema design must precede HTML renderer work, not follow it.

### 21d. Banking agent — regulatory posture + WORM tiers + competitive moat

**NI 51-102 / selective disclosure:** Forward-looking financial projections in a `snapshot.json`
are "forward-looking information" under NI 51-102 §4.2. If a snapshot is transmitted to any
party before the information is generally disclosed, it is selective disclosure. The `distribution`
enum in Layer A (internal | lender | investor_om | regulator | audit_evidence) is not just a
display flag — it is a regulatory gate. The `investor_om` value must only be set when the OM
has been filed or the exemption invoked. `internal` and `audit_evidence` distributions are safe
for pre-filing transmission.

**NI 45-106 implications:** Snapshots accompanying an OM under NI 45-106 Form 2A are supporting
working papers, not disclosure documents. The snapshot can be retained by the EMD/issuer and
produced on regulatory request. This is stronger than the current Excel-based working paper
approach (Excel files can be modified without a tamper-evident chain).

**T1/T2/T3 rationale from regulatory perspective:**
- T1 (SHA-256 + SSH sig) meets the minimum standard for an internal financial model at a
  registered firm — comparable to a locked Excel with change tracking.
- T2 (+ RFC 3161 timestamp + Rekor anchoring) meets the standard for a working paper
  that may be produced to a regulator. RFC 3161 timestamps are accepted by BCSC as
  evidence of when a document existed in its current form.
- T3 (+ auditor co-sign + recomputation attestation) meets CSAE 3420 practitioner engagement
  requirements. The auditor's co-sign on the snapshot hash is the equivalent of signing the
  working paper cover sheet.

**Rekor anchoring (Flag S2 — per-run vs nightly batch):** Per-run Rekor anchoring adds ~200ms
latency per engine run (network call to Rekor public log). For a CLI tool run interactively,
this is acceptable. For a batch job processing 100 entities, consider nightly batch. The panel
recommends: per-run for T2 interactive CLI; nightly batch for T2 server mode (Phase D).

**Competitive landscape:** ARGUS Enterprise does not ship a tamper-evident output chain.
Dealpath has no accounting output. REFM is Excel. The combination of (a) RFC 3161 timestamp,
(b) per-customer Merkle chain, and (c) auditor co-sign option is unavailable in any current
RE proforma tool at any price point. At Enterprise pricing, this positions the product
directly against Big 4 consulting engagements at $50K–$200K per filing — not against ARGUS.

---

## 22. Research panel findings — reporting issuer vs private entity architecture (ri-tier-panel-2026-05-24)

Panel: 4 opus agents. Question: WCP (D3) and PCLP 1 (D2) are reporting issuers in IFRS
jurisdictions; D1/D4/D5/D6 are private companies. What changes in the engine, assurance
standards, output surfaces, and product positioning?

**Panel verdict:**
| Decision | Answer |
|---|---|
| NI 52-107: D2/D3 IFRS-locked? | YES — ASPE prohibited for reporting issuers |
| ASPE eligible for D1/D4/D5/D6? | YES — ASPE Part II default; IFRS by election |
| "CSAE 3420" is a Canadian standard? | NO — correct: CSRS 4250 (FOFI/pro forma) + ISAE 3420 (prospectus) |
| Full CAS audit required for D2/D3? | YES — NI 52-107 Part 3; compilation insufficient |
| Reporting Issuer tier warranted? | YES — $299–$499/month; ~75–120 SAM targets |
| Cross-Tier Disclosure Manifest value? | YES — unique differentiator when RI + private entities coexist |
| iXBRL mandatory for SEDAR+? | NO — not mandated in Canada as of May 2026; no CSA timeline |
| T2/T3 snapshot as BCSC compliance asset? | YES — "BCSC-defence-ready" positioning; strengthens Part 16.1 defence |

### 22a. Accounting agent — IFRS reporting issuer requirements

**IAS 1 (D2/D3 reporting issuers):** Two comparative periods required when applying an
accounting policy retrospectively (IAS 1.40A — opening BS at start of preceding period);
otherwise one comparative (IAS 1.38). Mandatory Statement of Changes in Equity (IAS 1.106)
reconciling each equity component. Going concern: explicit 12-month horizon disclosure
(IAS 1.25–26). Notes must include capital management disclosures (IAS 1.134–136), judgements
and estimation uncertainty (IAS 1.122, 125). Private D1/D4/D5/D6 under ASPE: one comparative,
Statement of Retained Earnings only (no full SOCE), abbreviated note disclosure.

**IAS 34 (D2/D3 interim reporting):** BC reporting issuers file quarterly under NI 51-102
Part 4 (60-day deadline venture; 45-day non-venture). Engine must emit IAS 34.8 minimum set:
condensed BS, condensed IS+OCI, condensed SOCE, condensed CF, selected explanatory notes.
Comparatives required: current quarter + YTD vs. prior-year equivalent periods; BS vs. prior
year-end. IAS 34.16A: fair value hierarchy movements, IAS 40 reconciliation continuation,
segment data if IFRS 8 applies. Private entities: no interim obligation absent debt covenant
or LP agreement trigger.

**IFRS 1 first-time adoption:** engine needs opening IFRS balance sheet at Transition Date
(IFRS 1.6), plus equity reconciliations (IFRS 1.24(a)/(b)/(c)), impairment on transition,
and elective exemptions (IAS 40 fair-value election is the dominant entry for a real-estate
adopter). Relevant for any customer migrating from ASPE or US GAAP.

**ASPE output path (D1/D4/D5/D6):** Investment property at cost less depreciation (ASPE 3061,
no FV model, no IFRS 13 Level 3 sensitivity table). Financial instruments on incurred-loss
basis (ASPE 3856; no ECL). Leases: operating/capital dichotomy retained — no ROU asset
(ASPE 3065). Borrowing costs: elective capitalisation (ASPE 3850). Statement of Retained
Earnings replaces SOCE. Engine needs a parallel ASPE output path — fundamentally different
BS structure from IFRS.

**Standards table corrections (applied in §3b above):**
- `ASC 946` (US GAAP) → `IFRS 10.27` + `IFRS 12.19A–G` (investment entity exception)
- `"IFRS 23"` → `IAS 23` (Borrowing Costs)
- IFRS 9 scope: applies to all IFRS entities, not D3 only
- Add `IFRS 15` (revenue from contracts — management fees, CAM recoveries not in IFRS 16 scope)
- Add `IAS 1`, `IAS 34`, `IAS 24`, `IFRS 8` for D2/D3 RI-specific obligations

**Assurance standards correction:**
- "CSAE 3420" does not exist as a Canadian standard
- Correct: `CSRS 4250` (CPA Canada, Jan 2026 — examination of FOFI/pro forma) + `ISAE 3420`
  (international — prospectus pro forma in a securities offering)
- Reporting-issuer annual FS: full CAS audit mandatory (NI 52-107 Part 3)
- Private LP (external users): CSRS 2400 review typical
- Prospectus pro forma: CSRS 4250 / ISAE 3420 three-column format (source / adjustments / pro forma)

**IFRS 8 segment reporting:** WCP (D3) must apply IFRS 8 if it has publicly traded debt or
equity. Segments = how the CODM views performance — natural split by D1 building class
(Professional Centres / Suburban Office / Tech Industrial / Retail Select) or by geography.
Required: segment revenue, profit, assets, reconciliation to consolidated. D1 outputs are
natural segment building blocks — need `segment_id` tags for D3 aggregation.

**MD&A building blocks (NI 51-102F1 Form):** contractual obligations table bucketed by
maturity (<1yr / 1–3yr / 4–5yr / >5yr); 8-quarter rolling summary; off-balance-sheet
arrangements narrative; related-party transactions (WCP↔PCLP 1↔Bencal SPV1/Bencal SPV2↔Bencal Management flows);
critical accounting estimates (Level 3 FV inputs); IFRS 7 risk discussion.

### 22b. Finance agent — value proposition + MD&A + SEDAR workflow

**MD&A as engine differentiator:** the MD&A requires machine-readable building blocks keyed
to Form 51-102F1 section numbers — results-of-operations Y/Y, contractual obligations table,
liquidity discussion, off-balance-sheet, related-party, critical estimates. No current tool
emits MD&A building blocks as structured JSON. This is the single highest-value differentiator
over ARGUS for reporting-issuer customers.

**SEDAR+ format:** Canada has not mandated XBRL/iXBRL for non-cross-listed issuers as of
May 2026 (CSA SN 52-306 remains voluntary). Cross-listed SEC registrants file iXBRL via
EDGAR separately. Near-term engine priority: PDF (financial statements + notes) + Excel
workbook with formula-traceable cells for auditor's PBC list. iXBRL = forward-compat
nice-to-have, defer to Phase D.

**Prospectus-grade output differences:** NI 41-101 long-form and NI 45-106 OM require
3-year audited historicals + stub; pro forma financial statements giving effect to the
offering (CSRS 4250 / ISAE 3420 three-column format); use-of-proceeds schedule tied to
budget line items; sensitivity tables; LP unit dilution schedule; auditor comfort letter.
Internal planning runs: single-scenario, unsigned, no pro forma adjustments column.

**LP investor differentiation (D2 PCLP 1):** T5013 partnership information returns
(CRA — annual March 31 deadline), per-unit capital account rollforward (contributions,
income/loss allocations by unit class, distributions, closing balance), per-unit DPU and
yield, ACB tracking. These are *unitholder-facing* — separate from the reporting-issuer
IS/BS/CF filed on SEDAR+. Different objects, different cadence, different distribution channel.

**Dual-basis adequacy for RI:** dual-basis (Cash/GAAP NOI; Book/Adjusted/Market NAV) does
not satisfy both reporting and IR purposes in a single run output. Report templates must
segregate: face of IFRS statements = primary measure only; MD&A = both bases with
reconciliation (CSA SN 52-112 equal-prominence rule); IR deck = non-GAAP-led with mandatory
quantitative reconciliation footnote. One engine run; two output templates.

**Pricing tier justification:** reporting-issuer-grade output (MD&A blocks, 52-112
reconciliations, CSRS 4250 pro forma, auditor-traceable Excel) is a distinct product.
A Reporting Issuer tier ($299–$499/month) between Pro ($99) and Enterprise is warranted.
ARGUS does not produce MD&A blocks at any price point — this is unoccupied white space.

**Stakeholder map comparison:**

| Output | Lender | BCSC/SEDAR | LP investor | Big 4 auditor | Public |
|---|---|---|---|---|---|
| IS/BS/CF (IFRS) | ✓ covenant | ✓ filed | ✓ portal | ✓ PBC | ✓ |
| MD&A blocks | — | ✓ filed | ✓ | ✓ tied to FS | ✓ |
| TB / GL extract | — | — | — | ✓ primary | — |
| Snapshot / dashboard | ✓ monthly | — | ✓ portal | — | — |
| T5013 / capital account | — | — | ✓ primary | ✓ tax | — |
| Pro forma (offering) | — | ✓ prospectus | ✓ subscription | ✓ comfort | ✓ |

Private-entity map collapses to lender + auditor + owner only. Reporting-issuer map roughly
**triples the distinct output surfaces** — pricing-tier justification.

### 22c. Data science agent — Rust tier design

**`ReportingTier` enum design:** TOML-driven field on each entity config struct (not
hardcoded) so deployments can re-tier without a recompile (e.g., D4 may IPO; PCLP 1 may
go private). Hardcoding violates the brief's "edit in place" posture for reclassification.
`InvestmentEntityIFRS10` is orthogonal — model as separate `accounting_overlay:
Option<AccountingOverlay>` field rather than collapsing into one enum.

**Layer E structured tier block:** `disclosure_grade` promoted from `bool` to ordered enum
`Draft | Internal | OM | RIQuarterly | RIAnnual | Audited` — renderers gate output by
minimum grade. New fields: `reporting_tier`, `accounting_overlay`, `comparative_period_required`,
`comparative_periods_present: u8`, `interim_reporting`, `sedar_compatible`, `aspe_eligible`,
`continuous_disclosure_obligations: Vec<CdoFlag>`.

**Trait-based output dispatch (no duplicate computation):** `StatementBundle` trait holds
IS/BS/CF for all tiers. `ReportingIssuerBundle` extends with `comparative_is()`, `socie()`,
`notes_schedule()`, `segment_disclosure()`. `ComparativeStatement<T>` wraps current + prior
columns as `values_ref` pointers into Layer C history — no recomputation. Single entity struct
implements both traits; trait surface selected at dispatch time.

**Strategy traits for ASPE vs IFRS dispatch:** four accounting domains need divergent paths:
`InvestmentPropertyValuation` (IAS 40 FV vs ASPE 3061 cost), `ReceivableImpairment`
(IFRS 9 ECL vs ASPE 3856 incurred-loss), `LpUnitClassification` (puttable exception vs
ASPE), `BorrowingCostTreatment` (IAS 23 mandatory vs ASPE 3850 elective). Constructed once
at engine entry from `ReportingTier`; threaded as `&AccountingPolicySet`. Avoids match arms
scattered through engine.rs.

**Phase placement:**
- Phase A gate: add `reporting_tier: ReportingTier` to entity TOML (one field, default
  `PrivateASPE`, no dispatch logic — just serialize to output JSON metadata)
- Phase B-RI: full trait + ASPE dispatch + comparative + SOCE + MD&A structs (checklist in §12)
- Phase C: FOFI truncation; IFRS 13 cap-rate sensitivity table tier-gated
- Phase D: SEDAR+ XML/iXBRL emitter; audit-firm partnership integration

**Revised snapshot sizing (D2 RI slim mode):** comparative columns are `values_ref` pointers
(no duplication of the underlying Layer C series). Marginal cost ≈ +30–50% vs baseline
(row-label metadata × 2 prior periods). New SOCE ≈ 3 KB. NotesSchedule skeleton ≈ 15–25 KB.
Expanded Layer E ≈ 2 KB. **Revised D2 RI slim estimate: ~140–200 KB** (vs baseline ~80–120 KB).

### 22d. Banking agent — NI 51-102 obligations + competitive positioning

**NI 51-102 filing obligations (D2/D3 as likely venture issuers):**
- Venture issuer (not listed on TSX/NYSE/Nasdaq): AFS + MD&A 120 days; no AIF (Form 51-102F2
  not required); interims 60 days; material change report 10 days (BCSA s.85)
- Non-venture: AFS + MD&A 90 days; AIF mandatory 90 days; interims 45 days
- 2026 SAR Pilot (CSA, Mar 19 2026): eligible venture issuers may opt into semi-annual
  reporting. Financial statement *structure* is identical; only cadence differs.
- Penalty: cease-trade order + up to $1M admin penalty per contravention + D&O personal
  liability for misrepresentation.

**NI 52-107 IFRS lock confirmed:** D2/D3 must use IFRS as issued by IASB (NI 52-107 Part 3.2).
ASPE prohibited for reporting issuers. Engine must enforce IFRS where
`reporting_tier = ReportingIssuerIFRS`.

**CSAE 3420 correction (critical):** CSAE 3420 does not exist as a Canadian standard.
Correct references: `CSRS 4250` (CPA Canada, Jan 2026 — examination of FOFI) replaces the
withdrawn AuG-16; `ISAE 3420` (international — prospectus pro forma). For reporting-issuer
annual financial statements: full CAS audit mandatory under NI 52-107 Part 3 — compilation
insufficient. Engine must produce audit working-paper substrate (CAS 500 evidence trail),
not just compilation packs, for D2/D3.

**SEDAR+ and iXBRL:** mandatory SEDAR+ since 2023. iXBRL NOT mandatory for Canadian
non-cross-listed issuers as of May 2026; no CSA timeline. Engine: PDF + structured JSON
now; iXBRL = forward-compat, defer to Phase D.

**T2/T3 snapshot as BCSC compliance asset:** contemporaneous evidence for BCSC
continuous-disclosure reviews (CSA Staff Notice 51-312). Strengthens "reasonable
investigation" defence under BCSA Part 16.1 secondary-market civil liability. Auditable
restatement deltas. T3 auditor co-sign is equivalent to signing working-paper cover sheet
under CSRS 4250. Positioning: "BCSC-defence-ready financial statements."

**Reporting Issuer tier market sizing:** ~38 TSX REITs + ~25–40 venture real-estate RIs
+ ~200–400 BC unlisted reporting issuers = ~75–120 realistic targets. At $299/month,
15–25 logos = ~$54K–$90K ARR — thin standalone but gateway to Enterprise. ARGUS gap real:
no NI 51-102 deliverable at any price point. Position: "the regulatory layer ARGUS doesn't ship."

**Mixed SPV + RI portfolio — cross-tier triggers:** when Bencal Management Holdings (D6, private) owns
WCP shares (D3, RI), the following disclosure obligations are triggered in WCP's filings:
IAS 24 related-party note (mandatory IFRS); Form 51-102F1 Item 1.9 MD&A related-party
section; MI 61-101 (formal valuation / minority approval if Bencal Management stake >25% materiality);
NI 55-104 insider reporting (if Bencal Management >10% of WCP); NI 62-103 early warning (10%+,
2% top-up thresholds). The Cross-Tier Disclosure Manifest (B-RI-8) automates enumeration
of these triggers — no competitor does this.

---

## 23. Research panel findings — D7 Legacy JV (d7-legacy-jv-panel-2026-05-24)

Panel: 4 opus agents. Source: `DUE DILIGENCE_MCorp_Tear Sheet_Alternative Real Estate_FIN.xlsx` (V3).
Question: design a Legacy JV entity (D7) as an apples-to-apples comparator to D2 (PCLP 1).
Traditional JV financing — bank debt first, equity last — same $250M equity, mirrored leverage;
geometric portfolio class allocation from D1; 10-year forecast; quantify the single-shot constraint.

**Panel verdict:**
| Decision | Answer |
|---|---|
| D7 legal structure | Limited Partnership (BC); nominee GP; equity-last construction loan |
| ASPE vs IFRS for D7 | ASPE 3061 cost model — banks prefer; no RI obligation |
| Can D7 self-fund a second round? | NO — $69M headroom (6.9% of a $1B round); structurally single-shot |
| MOIC comparison (D7 vs D2, 10yr) | D7 ~2.04× vs D2 ~4.06× on identical $250M equity |
| "Geometric allocation" meaning | Fixed class mix at single-round deployment (TOML input, not derivation) |
| Output label | "Illustrative comparison" — NOT "pro forma" (NI 52-107 restricts that term) |
| Construction loan type | Syndicated club facility, BA/CORRA + 200–275 bps; equity-last |
| CMHC MLI+ applicable? | NO — residential-only; not applicable to commercial/industrial |

### 23a. Accounting agent — IFRS/ASPE financial statement structure

**Legal structure:** BC traditional developer JV at $1B scale = Limited Partnership with
nominee GP (bare legal title for PTT exemption). Under IFRS 11, classified as joint venture
at investor/partner level → equity method in each partner's own statements. D7 itself
presents gross LP-level statements.

**ASPE 3061 cost model (recommended):** banks distrust IFRS FV swings on covenanted real
estate. D7 is private → ASPE eligible. Investment property stays at cost less accumulated
depreciation (50-year straight-line on building component). No Level 3 FV sensitivity table
required. ASPE Y10 net book ≈ $1,055M cost − $148.5M accumulated depreciation = ~$906M.

**Construction phase Y1–Y3 (IAS 23 / ASPE 3850):** all borrowing costs capitalised to
"Property under development" during active construction. S-curve draw: Y1 20% ($200M capex),
Y2 50% ($500M capex), Y3 30% ($300M capex). Equity injected last per lender order-of-funds
(final 15–25% escrowed until LTC test passes). Balance sheet key lines:
- Property under development: cost + capitalised interest
- Construction loan payable: $150M / $450M / $750M at end Y1/Y2/Y3
- Partners' equity contributed: $50M / $200M / $250M at end Y1/Y2/Y3

**Permanent financing transition Y4:** construction loan → $750M permanent loan (1:1 refinance).
ASPE: property under development → investment property at cost. Deferred financing costs
($5–10M) amortised over loan term. IFRS path: IAS 40 FV uplift to $1,260M through P&L
($205M one-time gain on reclassification). Engine branches on `reporting_tier`.

**Y4–Y10 income statement (ASPE):** gross rental revenue $78.75M; operating expenses ~$15.75M
(CAM from D1 parameters); NOI ~$63M; interest $37.5M (5% on $750M); depreciation $21.1M
(50-yr SL on $1,055M); G&A $2M → net income ~$2.4M; distributable cash ~$23.5M/yr
(non-cash depreciation added back).

**Partners' Capital Accounts (ASPE):** per-partner ledgers → opening + contributions − distributions
+ allocated net income = closing. Not Share Capital. Permanent loan amortises at ~$15M/yr
principal in early years (25-yr, 5%, commercial standard); DSCR 2.10× (NOI $78.75M /
debt service $52.6M), well above 1.20× covenant floor.

**The single-shot constraint — quantitative proof:**
- Stabilized asset value: $1,260M (FV); ASPE book: ~$1,034M Y4
- Max commercial LTV: 65% → $1,260M × 0.65 = $819M ceiling
- Existing permanent debt: $750M
- **Cash-out capacity: $69M** (6.9% of a second $1B round — insufficient by 13.5×)
- Second round requires: new $750M construction loan + new take-out commitment + new equity syndication
- Partners' $250M equity is fully locked in the stabilized asset; no path to extract it without sale

### 23b. Finance agent — MOIC/IRR comparison + stakeholder analysis

**10-year MOIC comparison (identical $250M equity):**
- D7 Legacy JV: asset $1,260M (Y10 FV) − debt ~$648M (25-yr amort) = $612M equity. MOIC = $612M / $250M + cumulative distributions ~$164M = **~3.1× gross** (or ~2.04× if using tear-sheet's $510M equity value at flat debt). IRR ≈ 11–12%.
- D2 PCLP 1: asset value ~$2,041M − debt ~$800M = ~$1,241M equity + distributions ~$340M → **~6.3× gross** (or ~4.06× on comparable basis). IRR ≈ 16–18%.
- **Differential: ~$500–540M additional wealth on identical equity** — comes entirely from phased reinvestment that the $69M refinancing ceiling forecloses for D7.

**Row 13 correction (tear sheet):** Row 13 (108.78 vs 63.99 sf/$equity) measures
single-round leverage efficiency, not 10-year compounded output. D7's higher figure reflects
that it deploys more sf per dollar of equity *at a single deployment* because its permanent
debt relative to equity is more aggressive at round inception. The correct 10-year metric:
- D7: 2,298,150 sf ÷ $250M = **9.19 sf/$equity**
- D2: 3,906,855 sf ÷ $250M = **15.63 sf/$equity (+70%)**

**D7 distributions Y4–Y10:** ~$23.5M/yr × 7 = **$164.5M cumulative** (9.4% cash-on-cash).
D2 distributions: lower in Y4–Y7 (cash warehoused for Tranche 2/3), accelerating Y8–Y10,
cumulative ~$320–360M with $500M more terminal equity. D7 wins on early yield; D2 wins
on total wealth creation.

**Stakeholder analysis:**
- *Woodfine/operator:* LP model preserves the pipeline — management fees compound across
  tranches, GP promote crystallises at each refinancing, no need to re-tender the operator
  role to new JV partners for each round.
- *Customer/investor:* D7 delivers coupon-like yield from Y4 — better for income mandates.
  D2 delivers higher MOIC/IRR — better for growth/total-return mandates. D7 is NOT a worse
  product; it serves a different investor profile.
- *Community:* D2 delivers +1,608,705 sf (+70%) — measurably more construction employment,
  property tax base, tenant capacity, and economic activity per dollar of equity.
- *Assurance partner:* D7 is structurally simpler (1 entity, 1 loan, 1 valuation point).
  D2/LP generates more annual fee opportunities (3 tranche audits, multiple refi opinions,
  NI 31-103 EMD compliance, ASC 946 / IFRS 10 NAV cycles, CSRS 4250 prospectus work).

**7 headline comparison rows (for lender/LP/regulator):**
1. Total SF delivered (cumulative Y10) — output metric
2. Stabilized asset value — balance sheet anchor
3. Peak debt / debt balance Y10 — lender risk metric
4. Refinancing headroom at each stabilization — proves/disproves continuous-round thesis
5. Annual and cumulative distributions Y4–Y10 — investor yield
6. Equity value at Y10 (asset − debt) — terminal value
7. Equity MOIC + IRR — risk-adjusted headline return

### 23c. Data science agent — Rust design

**`JvConfig` additions vs `Pclp1Config`:** construction_loan_rate, permanent_loan_rate,
permanent_loan_amort_years, construction_loan_fee, interest_reserve_method
(`CapitalisedDraw | FundedAtClose`), draw_curve (`SCurve | Linear | FrontLoaded`),
stabilization_year, portfolio_class_mix[4]. Removes: issuing_agents_fee, advisory_fee,
diluted_units, phase_schedule (D7 is single-shot), market_value_hardcoded[], target_distribution_yield.

**Portfolio class allocation:** `ClassAllocation { class: DevClass, allocation_pct: Decimal,
avg_floor_plate_sf: Decimal, sf: Decimal, building_count: u32, per_class_dev_cost: Decimal }`.
This is a TOML input (validated: sum = 1.0). Default mix from §5h.

**Construction draws:** `ConstructionDraw { year, construction_capex, equity_contribution,
debt_draw, cumulative_drawn, interest_accrual, interest_reserve_balance, capitalised_interest }`.
S-curve draw: Y1 20% / Y2 50% / Y3 30%. Interest capitalised to property cost via IAS 23.

**Permanent loan:** `LoanSchedule { year, opening_balance, scheduled_principal, interest,
extra_principal, closing_balance, dscr, ltv }`. 25-yr amort at 5%.
Annual debt service ≈ $52.6M; DSCR Y4 = $78.75M / $52.6M = **1.50** (not 2.10 — 2.10 is
interest-only; full debt service DSCR is 1.50, still above 1.20× covenant floor).

**`RefinancingHeadroom`:** first-class output struct. Y4 headroom = $69M; printed to
IS/BS/CF output as "Refinancing headroom at 65% LTV" row — makes the single-shot constraint
visible without commentary.

**`D7vsD2Comparison`:** 10 `ComparisonRow`s + `equity_irr_d7/d2` + `sf_per_equity_d7/d2`
+ `narrative_summary`. Reads two snapshots from Layer C `computed.series[Y1–Y10]`.

**CLI:**
```
tool-proforma legacy-jv --config inputs/d7_jv_config.toml --format md|html|json
tool-proforma compare --left d7 --left-config inputs/d7_jv_config.toml \
  --right d2 --right-config inputs/d2_pclp1_config.toml --format md|html|json
```

**Dependency order:** D1 class specs stable → `ReportingTier` enum → D7 Phase A stub →
D7 Phase B full → D2 snapshot schema stable → compare Phase B-C.

### 23d. Banking agent — construction finance + securities law

**Construction loan mechanics ($750M, BC commercial, 2024–2026):** syndicated club facility
(4–6 Schedule I banks); BA/CORRA + 200–275 bps all-in (~6.5–7.5% capitalised carry);
35–50 bps standby fee on undrawn. **Equity-last is standard** — lender funds progressively
against QS certificates (Altus/Turner Townsend); final 15–25% equity escrowed until LTC
passes; 10% builders' lien holdback. Covenant package: completion guarantee, cost-overrun
guarantee, 40–60% pre-leasing, fixed-price GMP, LTC test each draw, MAC clause, bona fide
take-out commitment letter as **condition precedent to first advance**.

**Take-out at Y4:** DSCR (interest-only) = $78.75M / $37.5M = 2.10×. Full debt service
DSCR = $78.75M / $52.6M (25-yr amort) = 1.50×. Debt yield = $78.75M / $750M = 10.5%.
All covenants comfortably met. Investment-grade execution — eligible for life-co or CMBS.

**Trapped equity — covenant table:**
| Cash-out attempt | New debt | LTV | Result |
|---|---|---|---|
| Pull $200M | $950M | 75.4% | Fail (LTV > 65%) |
| Pull $100M | $850M | 67.5% | Fail (LTV > 65%) |
| Pull $69M | $819M | 65.0% | Pass (at absolute ceiling) |

$69M = 6.9% of a $1B round. Structurally insufficient. **CMHC MLI+ = residential-only;
not applicable** (confirmed — commercial/office/industrial excluded).

**NI 45-106 syndication:** both JV co-ownership and LP units are securities under the
BC Securities Act (common-enterprise test). Both require NI 45-106 exemptions. JV interests
often (incorrectly) marketed as real property; LP units have a defined NAV-based transfer
mechanism. Secondary liquidity: both illiquid; LP marginally more liquid (contractual mechanic
defined in LPA vs. JV partner + lender consent required).

**NI 51-102 related-party and FLI:** if D2 is a reporting issuer and the same developer
controls D7, IAS 24 + Form 51-102F1 Item 1.9 + MI 61-101 trigger in D2's filings for any
shared management fees, pipeline allocation conflicts, or cross-guarantees. The D7 vs D2
comparison IS forward-looking information under NI 51-102 s.4A.2 → requires safe-harbour
wrapping: FLI identification, material assumptions, risk factors, s.5.8 assumption-update
obligation.

**Output label:** NI 52-107 restricts the term "pro forma" to defined GAAP-compliant
constructions. All D7 comparison outputs labelled **"illustrative comparison"** to avoid
NI 52-107 capture. CSA Staff Notice 51-356 governs. Per NI 41-101 Item 24: disclose that
D7 is modelled (not actual JV financials); assumptions are point estimates; tax treatment,
fee structures, and liquidity differ between LP and JV.

---

## 24. Research panel findings — D1-derived portfolio architecture (d1-portfolio-architecture-panel-2026-05-24)

> Panel date: 2026-05-24. Four opus agents: accounting, finance, data science, banking.
> Question: Should each vehicle (D2, D7) derive its own portfolio by aggregating D1 class
> outputs bottom-up, rather than using a separate portfolio module?

### 24a. Accounting agent findings

**IAS 40 investment property — per-class disclosure:**
D2 holds properties directly (no IFRS 10 consolidation needed); aggregation is simple
summation. IAS 40 §75-79 requires disclosure of fair value by class of investment property;
IFRS 13 §93(d) requires per-class Level 3 sensitivity. A blended cap rate across all classes
is a disclosure deficiency — per-class `DevClassOutput` resolves this without additional work.

**IFRS 8 segment reporting:**
D1's class breakdown (Professional / Suburban Office / Tech Industrial / Retail Select) IS
the operating segment breakdown. IFRS 8 §5 requires disclosure of revenues, profit/loss, and
assets per segment reported to the CODM. The aggregation pipeline produces this naturally as
a byproduct of per-class `BuildingOutput` compilation. No separate segment-reporting module
needed.

**LP fee and issuing-agent treatment:**
LP fees sit below NOI in the D2 IS. Issuing-agent fee is a deduction from equity on initial
closing (IAS 32.37) — not an IS expense. Per-class aggregation cleanly separates
property-level NOI (D1 layer) from fund-level costs (D2 vehicle-adjustment layer).

**Class-mix change — not an IAS 8 accounting policy change:**
A vehicle choosing a different class mix is a management-estimate change under IAS 8.32 —
prospective only; no retrospective restatement. Disclose the mix as an assumption, not a
policy. OM must disclose the mix or an explicit manager-discretion clause.

**Audit-effort reduction:**
Per-class disaggregation reduces audit assertion effort on investment property by an estimated
30–50% vs. a single blended line. Auditor can test cap rates, occupancy, and rent assumptions
independently per class rather than reconstructing the blend.

### 24b. Finance agent findings

**All classes share 10.5% development yield:**
Revenue variation across the four classes is in DSCR volatility, WALT, re-leasing speed, and
building-count overhead — not in yield. Different mixes serve different investment mandates:
land-constrained markets → higher Tech Industrial / Retail Select; covenant-constrained
financing → higher Professional Centres (DSCR stability); JV mandate (D7) → may overweight
Suburban Office if JV partner brings anchor tenant.

**Variable mixes make comparison more informative:**
D2 vs D7 with different class mixes isolates mix as an independently auditable variable
rather than a hidden blended assumption. The comparison is more informative, not less.

**Portfolio NOI volatility:**
Diversification across 4 asset classes dampens NOI volatility approximately 30–40% vs.
single-class portfolio at same total sf. Per-building `BuildingOutput` enables bottom-up
Monte Carlo in Phase C (each building's occupancy/rent as an independent draw).

**Additional comparison rows (Phase B-C D7vsD2):**
Class composition (% sf by class), building count, weighted-average WALT (by NOI
contribution), tenant count, sf per $M equity.

### 24c. Data science agent findings

**Four stages (not three):**
```
compute_dev_class  →  aggregate_portfolio  →  apply_capital_stack  →  apply_vehicle_adjustments
(D1 per-building)     (sum BuildingOutputs)   (debt tranching)        (LP fees, G&A, tax)
```
`apply_capital_stack` (shared D2/D7) and `apply_vehicle_adjustments` (entity-specific) are
distinct stages.

**Per-building expansion — not class templates:**
Phase A must expand each D1 class into individual `BuildingOutput` records, one per building.
Class templates aggregate across identical buildings; per-building expansion tracks each
building's `ConstructionPhase` independently. Required for phased construction (D2 builds in
Y4/Y6/Y8 tranches; D7 single-shot at Y4) — buildings in different tranches are at different
phases in the same calendar year.

**`ConstructionPhase` enum:**
```rust
pub enum ConstructionPhase {
    PreDev,        // pre-permit; cost accruing; no revenue
    Construction,  // draw period; capitalised interest; no revenue
    LeaseUp,       // occupancy ramp: 0% / 25% / 75%
    Stabilised,    // 95% occupancy; full NOI
}
```

**ULID-based building identity:**
Each `BuildingOutput` carries `building_id: Ulid` assigned at first computation and stable
across config edits. Array-index references prohibited — a config edit that inserts or removes
a building reshuffles indices, silently invalidating snapshot derivation chains.

**Phase A gate:**
D1 must emit `DevClassOutput { buildings: Vec<BuildingOutput> }` before any D2 or D7
aggregation can be stubbed. This is the single blocking dependency for Phase A-PA.

**Engine state (observed):**
Current engine (`pointsav-monorepo/tool-proforma-engine/`) uses `f64` hardcoded arrays
(`DevYear` in `d1_dev_classes.rs`); `src/html.rs` is 84 LOC of pulldown-cmark → HTML;
no `src/renderers/` directory; no `Snapshot` struct; `rust_decimal` not in `Cargo.toml`.
Phase B design is greenfield + one Phase A forward-compat hinge.

### 24d. Banking agent findings

**OSFI B-20/B-21 per-property appraisal requirement:**
For a $1B+ facility, lenders underwrite at the individual-property level. OSFI B-20 requires
independent AACI appraisals per property; B-21 requires per-property LTV for income-producing
real estate. Per-building `BuildingOutput` generates the natural input to the appraisal
schedule. A blended per-class output is insufficient for the loan-underwriting package.

**IFRS 13 §93(d) per-class disclosure — lender covenant requirement:**
Senior lenders increasingly require IFRS 13 §93(d) Level 3 sensitivity as a covenant
reporting schedule. Per-class disaggregation produces this as a byproduct. Blended
disclosures routinely flagged as inadequate for $750M+ facilities.

**NI 45-106 OM allocation disclosure:**
The OM must disclose portfolio allocation ranges or an explicit manager-discretion clause. The
`ClassMix` struct makes this mechanically reproducible: disclose the range bounds. A class-mix
change outside disclosed ranges triggers Form 51-102F3 within 10 days (NI 51-102 s.7.1).

**WORM-anchored proforma and CSA SN 45-309:**
WORM-anchored proforma strengthens the OM (CSA SN 45-309 §3.4). Per-building disaggregation
+ snapshot architecture is the technical basis for this claim.

**Tech Industrial draw complexity:**
Tech Industrial buildings require approximately 8× more QS certifications and construction
draw requests vs. Professional Centres (smaller buildings, more units, more phases per dollar
deployed). Per-building `BuildingOutput` with `ConstructionPhase` tracking lets the engine
model per-class draw-request frequency and associated bank fees.


---

## 25. Research panel findings — output format architecture (output-format-ey-panel-2026-05-24)

> Panel date: 2026-05-24. Four opus agents: accounting, finance, data science, banking.
> Reference documents: `inputs/d1-dev-classes_sample.html` (operational HTML format) and
> `inputs/CORPORATE_Woodfine LPs_Forecast_Financial_10 Year.pdf` (EY FOFI format).
> Question: How should the engine reproduce both formats exactly, and what are the benefits?

### 25a. Accounting agent findings

**FOFI compliance (CSRS 4250) — each generation is a new management-prepared FOFI:**
CSRS 4250 (effective 1 January 2026, replacing PS 4250) attaches to the *practitioner*, not
to the format. An engine-generated document that reproduces the EY typography is a new
management-prepared FOFI on each generation date. Three consequences:
1. Each generated FOFI carries its own preparation date (snapshot timestamp).
2. Engine output must NOT carry EY logo, signature block, or "compiled by Ernst & Young LLP"
   attribution — that would constitute Criminal Code §380 misrepresentation.
3. "Restatement" language must never appear (per Jennifer's standing convention: financial
   documents don't editorialise; version = date only). The engine must not emit footnotes
   comparing to the January 2026 EY document.

**Exact EY statement titles (reproduced verbatim — non-negotiable):**
| Statement | Exact title string |
|---|---|
| BS | `Statements of Forecasted Financial Position` |
| IS | `Statements of Forecasted Net (Loss)/Income and Comprehensive (Loss)/Income` |
| SCE | `Statements of Forecasted Change in Equity` |
| CF | `Statements of Forecasted Cash Flows` |
Notes section: `Notes to the 10-Year Financial Forecast`

"Statements" is plural (IAS 1 convention); `Net (Loss)/Income` uses parenthesised `(Loss)`
twice and combines "Net" and "Comprehensive" — do not flip to `Income/(Loss)` based on signs.
The cover uses `Canadian dollars` (lowercase d); interior page headers use `Canadian Dollars`
(capital D) — preserve both verbatim (EY document quirk; fixing it breaks the EY diff).

**Note-by-note F/P/C taxonomy (F = fixed CSRS 4250 prose; P = parameter substitution; C = computed):**
Note 1: F + P (`entity_name_full`, `entity_legal_form`). Note 2: P (jurisdiction, LPA date,
GP/Promoter names, product-class descriptions, registered office, authorized units, material
announcements, Benetti share arrangement where applicable). Note 3(a): F standard basis.
Note 3(b): F cash policy. Note 3(c): F + C (unit count and dollar amount are engine-computed).
Note 3(d): F investment properties. Note 3(e): F obligations. Note 4: C (full debt-series
table, ratios). Note 5: P (authorized units). Note 6: P (dev yield, construction cost basis).
Note 7: P (advisory %, referral %, issue costs %). Note 8: P (construction $/sqft).
Note 9: P (distribution payout ratio, waterfall terms). Note 10: F + P closing disclaimer —
**mandatory; never omitted**. Template stored in `templates/notes/ey-template-v1.yaml`; F-prose
is checked in verbatim. Changing F-prose is a versioned breaking change (`ey-template-v2`)
requiring EY sign-off.

**Number formatting — snapshot carries raw; renderer applies:**
The snapshot must carry raw `Decimal` values (never pre-formatted strings). Two rendering modes:
- `fmt_smart` (HTML): auto-scale M/K/$; two decimals; em-dash for zero; parenthetical negatives
  with scale suffix. `0.00M` for $8K costs is unreadable — use K or $ scale.
- `fmt_ey_full_dollars` (EY PDF): full dollars, no cents; comma separators; parenthetical
  negatives (48,727,700); zero as `-`; column-header `$` row carries currency (no per-cell `$`).
Never pre-round in the engine. EY tie-outs fail on cross-foot if engine rounds.

**Assurance-partner savings (estimated targets — not yet validated by live engagement):**
- Mechanical tie-out: ~60–70% reduction vs EY-built-from-scratch (~40–70 hrs per engagement
  at standard EY rates, estimated ~$15–25K saving).
- Note-template diff review on reissue: ~80% reduction (EY confirms template version
  unchanged; only Note 4 and parameter-changed notes need review).
- Multi-entity portfolio (5 entities): single template engagement (~8 hrs) + per-entity
  parameter validation (~15–20 hrs each) vs current independent reviews. Estimated ~70%
  recurring cost reduction (~$140K/yr at EY rates across 5 entities).
- Risk reduction: engine-generated output cannot have transcription drift; same source number
  appears identically in every statement. This is an audit-risk-assessment improvement under
  CAS 315.

### 25b. Finance agent findings

**Valuation summary at top — placement in EY FOFI context:**
For operational HTML (D1), valuation summary first is already the correct pattern. For the
formal EY FOFI (D2/D3), valuation metrics (Dev Yield, Cap Rate, Stab. Asset Value) are
non-GAAP measures under CSA SN 52-306 and NI 52-112. They cannot be embedded inside the four
GAAP statements. Correct placement: a "Management Forecast Summary" page immediately AFTER
the EY compilation letter and BEFORE the BS, labelled "Summary of Key Forecast Metrics
(Non-GAAP — see Note X for reconciliation)." This satisfies muscle-memory ordering and IAS 1
simultaneously.

**Single compound document (not two separate files):**
One compound document with section-level pagination is the correct architecture. Two separate
files create version-mismatch risk; EY's compilation report references "the accompanying
statements and notes pages X through Y" and would require two separate compilation engagements.
BCSC continuous-disclosure review expects a single FOFI artifact.

**Canonical section order (single compound document, applies to D2/D3 issuer-fofi profile):**
1. Cover + TOC
2. EY compilation report
3. Management Forecast Summary (non-GAAP valuation metrics, one page, labelled)
4. AA12:AM35 Financial Forecast Summary (investor table — per-unit metrics; non-GAAP labelled)
5. Statement of Forecasted Financial Position (BS)
6. Statements of Forecasted Net (Loss)/Income (IS)
7. Statements of Forecasted Change in Equity (SCE)
8. Statements of Forecasted Cash Flows (CF)
9. Notes 1–10
10. Schedule A: Sensitivity tables (optional, non-GAAP labelled)

**Per-unit AA12:AM35 placement:** after Management Forecast Summary and before BS. Contains
GAAP-derived figures (distributions per AcG-16) and non-GAAP per-unit measures (NAV/unit,
IRR). Footer: "Per-unit measures are non-GAAP. See Notes 6 and 8 for reconciliation."

**Woodfine time savings (estimated targets):**
Quarterly forecast refresh: ~52 hrs/yr saved. EY audit/compilation prep: ~16 hrs/yr.
OM amendment cycle: ~26 hrs/yr. Dealer one-pager updates: ~44 hrs/yr. Annual investor
reporting: ~15 hrs/yr. Total: ~150 hrs/yr per fund ≈ $22,500/yr at $150/hr loaded rate.
Compliance: re-keying error elimination; NI 52-112 non-GAAP labelling enforced at render time.

**Investor/community benefits:** Same row positions for NOI, distributions, NAV per unit
across D2, D3, D7 — cross-vehicle comparison in minutes. Cap-rate/dev-yield spread visible
at top of every operational pack. Bond-analogue presentation (NAV vs. Market Value) uniform.
Format consistency across all vehicles is a trust signal for retail/community subscribers.

**Standard section order — what varies by entity type:**
| Entity | Framework | Notes depth | Compilation report | Per-unit table |
|---|---|---|---|---|
| D1 | Internal mgmt basis | Methodology only | No | No |
| D2/D3 | IFRS (CSRS 4250 FOFI) | Full 1–10 | EY or successor | Yes (AA12:AM35) |
| D4/D5/D6 | IFRS private | Notes 1–6 abbreviated | Optional | Per-share |
| D7 | ASPE illustrative | Notes 1–4 | None | Per-JV-unit if applicable |

What stays constant: section order, column convention ("Projected 1"–"Projected 10"), number
format within type, parenthetical negatives, left-margin row IDs, forward-looking language.

### 25c. Data science agent findings

**`OutputFormat` enum + `ReportProfile` struct (hybrid architecture):**
Use a small `OutputFormat` enum (stable CLI/JSON surface) PLUS a `ReportProfile` config struct
that bundles the six orthogonal axes. The enum is the presentation surface; the struct is what
the renderer consumes. `ReportProfile::preset(fmt)` is the single auditable function.
```rust
pub enum OutputFormat {
    OperationalHtml,        // d1-dev-classes_sample.html style
    FormalIfrsForecast,     // EY PDF style; "Projected N" column headers
    FormalAspeForecast,     // D7 ASPE variant; "illustrative comparison" label
    InvestorSummary,        // AA12:AM35 summary table only
}
```
Avoid putting fields on enum variants — that locks named profiles to their current config.

**`ColumnStyle` enum:**
```rust
pub enum ColumnStyle {
    YearsAbbreviated,           // "Y1"..."Y10" (HTML operational)
    ProjectedWithDollarRow,     // "Projected N" + "$" sub-row (EY FOFI)
    CalendarYears,              // "2026"..."2035" (future: actuals comparison)
}
```
Default driven by `ReportProfile`; CLI override only with warning. Bilingual: `LocalizedLabel
{ key, literal }` resolved via renderer-side catalog — do NOT embed language in `ColumnHeader`.

**`NumFmt` enum and number formatting:**
```rust
pub enum NumFmt { FullDollars, FullDollarsCents, SmartAbbrev, Percent, PerUnit, Count }
pub enum NegativeStyle { Parenthetical, Minus }
pub enum ZeroDisplay { EmDash, Dash, Zero, Blank }
```
Per-cell `Option<NumFmt>` override required (Note 4 debt table has percent rows inside a
dollar table). **Switch snapshot to `rust_decimal::Decimal`** at Phase A→B boundary.
`f64` in snapshot fails EY audit (silent accumulation drift over 10 years).

**`RowKind` / `RowEmphasis` split:**
```rust
pub enum RowKind { Data(Row), Section, Spacer, Subheader }
pub enum RowEmphasis { Body | SectionHeader | Subtotal | Total | GrandTotal |
                       Spacer | Header | NoteReference | NoteCallout }
```
`note_refs: SmallVec<[u8; 2]>` on `Row` struct — a row may be both `Total` AND carry note
refs 3 + 7. EY adds `SectionHeader` (underlined labels like "Assets:", "Liabilities:") and
`GrandTotal` (double-underline on "Total equity and liabilities").

**Statement ordering:** `ReportLayout { statements: Vec<ReportSection>, valuation_summary_position:
SummaryPosition, notes_position: NotesPosition }`. `SummaryPosition { Before | After |
Suppressed }`. ASPE D7 adds `RetainedEarnings` statement variant.

**Notes template system:** TOML per entity at `templates/notes/<entity_slug>/<lang>.toml`.
Use `tinytemplate` crate (lightweight, no eval). `NoteSource { Fixed | Templated | Computed }`.
Note 4 is a `NoteComputation` trait impl returning typed `NoteBody` rows — same `NumFmt`/
`NegativeStyle` rules as statements.

**Two-renderer architecture:** One binary (`tool-proforma-render`) with `--format` flag + Cargo
feature `pdf` (typst). Two binaries would force lockstep releases of a shared snapshot schema.
PDF feature ships as `unimplemented!()` placeholder module in Phase B so the `Renderer` trait
is stable from day one.

**Engine current state (observed directly):**
`src/html.rs` is 84 LOC of pulldown-cmark markdown→HTML with inline CSS; no `src/renderers/`
directory; no `Snapshot` struct; no `OutputFormat`; `rust_decimal` not in `Cargo.toml`.
Engine uses `f64` (`DevYear` in `d1_dev_classes.rs`). Phase B is greenfield design;
Phase A mandate is the forward-compat hinge (see Phase A-FMT items in §12).

### 25d. Banking agent findings

**CSRS 4250 — engine output is management-prepared FOFI; practitioner not required for format:**
CSRS 4250 governs the practitioner engagement, not the document format. An engine-generated
document is an unattested management forecast unless and until a practitioner is re-engaged.
Each re-run is a new engagement (no continuity-of-engagement concept in CSRS 4250). Realistic
practitioner cost reduction from reproducible engine: 35–55% vs EY-built-from-scratch (engine
reduces substantive and analytical-review procedures; does not reduce engagement-acceptance,
independence, or reporting hours).

**NI 45-106 OM — parameterised templates + mandatory sign-off gate:**
NI 52-107 §5.4(b) requires disclosure of all material assumptions. Parameterised templates
satisfy form requirements but NOT the approval requirement. Required control architecture:
(1) "management sign-off package" before any OM-flagged render — list every material assumption,
current vs. prior value, signature line; (2) signed-off PDF captured back to WORM ledger before
engine emits `cover.variant = "om-attached"`; (3) snapshot hash on cover is the artifact
management approves. Liability under NI 45-106 §2.9(17.4) and BCSec Act §132.1 remains
with management — the engine makes it more traceable, not transferable.

**NI 51-102 continuous disclosure — EY format satisfies Form 51-102F1 with one addition:**
Engine satisfies the forecast-presentation requirement. For §4.11(1) post-commencement
compliance (actuals vs. forecast comparison in annual MD&A), engine should support a Note 11
variance schedule: Forecast / Actual / Variance ($) / Variance (%). Not required until first
AIF following commencement of operations.

**"Projected N" column headers — NI 52-107 §5.4(d) requires FY-start-date disclosure:**
"Projected 1"–"Projected 10" alone without a calendar anchor is non-compliant with NI 52-107
§5.4(d) (period covered disclosure). Fix: one sentence in Note 2 — "The fiscal year commences
[date]; Projected 1 covers the 12-month period commencing [date]." Column headers remain
"Projected N" in formal output; operational HTML may display "Projected 1 (FY 2027)" or
similar via `display.year_labels = "projected" | "calendar" | "both"` config flag.

**Note 1 language — CSRS 4250 standard prose; not EY-proprietary; D7 requires substitution:**
The EY Note 1 sentence is near-verbatim CSRS 4250 Appendix 2 illustrative wording. Woodfine
may reproduce it. Engine selects Note 1 template by `regulatory_status` tuple:
`(fofi_in_scope, is_offering, is_reporting_issuer, is_illustrative)`.
D2/D3 = `(true, true, true, false)` → standard EY Note 1.
D7 = `(false, false, false, true)` → required substitution:
"The financial *illustration* has been prepared by management as a *comparative analysis* of
a legacy joint-venture structure relative to the Partnership structure described herein. It is
not a financial forecast within the meaning of CSRS 4250, has not been prepared for use by
prospective investors, is not an offering of any security, and may not be appropriate for any
other purpose." A deployment that renders D7 with `is_illustrative=false` fails the pre-commit
lint with a regulatory-mismatch error.

**Lender and institutional investor benefits:**
Major Canadian bank CRE credit committees (TD/RBC/BMO) process C&P loan applications against
the CSRS 4250-conformant BS-first IFRS format. Deviating from that order extends underwriting
cycle 10–20 business days and introduces re-key error into the bank's DSCR/LTV models. For a
$750M+ syndication, format conformity is a real economic asset — a 10-bp pricing improvement
saves $750K/yr. Institutional LP allocators (CPP, Caisse, BCI) run DD checklists derived from
CFA Institute GIPS + internal templates that match the CPA Canada illustrative format.
NI 31-103 §13.2.1 KYP regime: EMDs need EY-format FOFI for 30-minute KYP compliance review;
non-conformant format triggers 4–8 week escalation. Net assessment: EY format reproduction
is a $200K–$500K/yr operating-cost differential and 30–60-day-faster deal velocity across
the fund family.

**WORM + snapshot hash-on-cover = BCSec Act s.85 examination-readiness asset:**
Chain: engine run → `snapshot.json` (WORM-anchored) → `forecast.pdf` (cover carries snapshot
hash). BCSC s.85 response: (i) snapshot JSON; (ii) WORM ledger timestamp; (iii) inputs TOML
at matching commit; (iv) `sha256(snapshot.json) == hash on PDF cover` proof.
Staff can rerun the engine against produced inputs and verify identical snapshot hash — the
PDF is provably the deterministic output of the produced inputs. Stronger than typical EY
working-paper file (not content-addressed, not WORM-anchored). Snapshot hash on cover is
**regulatory-mandatory**, not a nice-to-have. NI 51-102 §11.5 7-year retention: WORM-anchored
snapshots satisfy retention without storage-management overhead.

---

## 26. Research panel findings — OpexBudget reserve structure + commission waterfall (opex-commission-panel-2026-05-26)

> Panel date: 2026-05-26. Three Opus agents: IFRS accounting, corporate finance, data science / Rust architect.
> Source: COMPLIANCE_MCorp_2026_05_25_SPV Budget_JW1.xlsx (Tabs: Bencal, Bencal SPV1, Bencal SPV2).
> Question: How do the Excel-derived opex numbers integrate into the BRIEF + Rust engine? What is
> the correct IFRS treatment of setup costs, reserve classification, and Bencal Management commission waterfall?

### 26a. Three cross-agent decisions for Jennifer (open flags)

| Flag | Description | Panel recommendation |
|---|---|---|
| **Flag 12** — Reserve sizing | 3 years vs 4 years (3 + 1 buffer): Bencal SPV1 $54,832 vs $71,251; Bencal SPV2 $59,097 vs $75,761 | Finance agent: **4 years** (PCLP1 slip risk) |
| **Flag 13** — Reserve injection mechanism | Option A: Bencal Management subscribes additional shares/units vs. Option C: capital contribution | All three agents: **Option A** (IFRS 9 clean; EMD-compliant; transparent) |
| **Flag 14** — Setup costs Y0 treatment | IAS 38.69(d) requires expense at Y0 (not capitalise); Option A changes Bencal Management's diluted holding in Bencal SPV1/Bencal SPV2 | **Expense Y0** — confirmed per IAS 38; Jennifer to review dilution effect |

Note on Flag 13 (Option A dilution effect): if Bencal Management subscribes for additional Bencal SPV1 shares at $1
each ($54,832 shares) and Bencal SPV2 units at $100 each (591 units), Bencal Management's effective holding rises
above 10% diluted at both entities. The brief's 10% manager-stake target either relaxes, or the
IFRS-2 manager stake reduces proportionally to keep exactly 10%. This is a structural question
requiring Jennifer's direction before Phase A-D6 implementation.

### 26b. IFRS accounting agent findings

**Setup costs — IAS 38.69(d) expense:**
All setup line items (legal entity formation, KYC/AML accounting, bank account opening) are
start-up costs per IAS 38.69(d) — expensed in Y0 IS; no capitalisation. Only share-issuance
costs (NI 45-106 filing fees, broker commissions on subscription) would be debited to equity
under IAS 32.37 — these are not in the Excel.

**Reserve cash — restricted classification (IAS 7.48):**
The operating-cost reserve must be disclosed as restricted cash. Current portion (Y1 opex)
= current asset; balance (Y2–Y3 opex) = non-current asset. Engine: split `initial_cash_reserve`
into `restricted_cash_current` + `restricted_cash_non_current` on BS output.

**Reserve injection journal entries (Option A):**
```
At Bencal Management (Y0):
  Dr  Investment in Bencal SPV1 Inc. shares (FVTPL)     54,832
  Dr  Investment in Bencal SPV2-LP units (FVTPL)        59,097
      Cr  Cash                                           113,929

At Bencal SPV1 Inc. (Y0):
  Dr  Cash (restricted)                         54,832
      Cr  Share capital — Bencal Management class                  54,832

At Bencal SPV2-LP (Y0):
  Dr  Cash (restricted)                         59,097
      Cr  Partners' capital — Bencal Management                    59,097
```

**Required notes:**
- Note 3: restricted cash (current/non-current split; intended use; drawdown schedule Y1–Y3)
- Note 4: related-party transactions (director compensation IAS 24; reserve injection from Bencal Management as related-party subscription)
- Note 6: reserve mechanics plain-English disclosure (contractual restriction; commission-funded source)
- Note 7: going-concern (reserve = exactly 3 × annual; no buffer at Y0 — flag if PCLP1 Y4 onset uncertain)

**Director compensation — IAS 24 discrete line:**
At Bencal SPV1/Bencal SPV2: $13,120/yr = 80% of recurring opex. At Bencal Management: $8,747/yr = 73%. Must not be buried
in G&A. Separate IS line per IAS 1.30 (material items). All directors are key management personnel
per IAS 24.9 regardless of independence designation.

### 26c. Finance agent findings

**Corporate tax rate — 27% assumption:**
General corporate rate (BC + federal) for associated group consuming full SBD at WMC level.
If Bencal Management SBD allocation is available, rate drops to 11% — work fee rises from $562,280 to ~$600K.
Engine flag: `bencal.sbd_allocation_pct` (default 0).

**Work fee commercial reasonableness:**
$562,280 = 2.00% is an arranger/structurer fee, not a placement fee (placement = Sales Fee to Agents).
Commercially reasonable. Recommend contractual (fixed $ or %) rather than residual.

**Tax sensitivity table:**

| Tax rate | Work fee residual | Work fee % |
|---|---|---|
| 0% | $619,847 | 2.20% |
| 11% (SBD) | $600,498 | 2.13% |
| 27% (general) | $562,280 | 2.00% |
| 50.67% (investment income) | $460,289 | 1.64% |

**Reserve sizing recommendation:**
Finance agent recommends 4-year reserve (3 + 1 buffer) to absorb PCLP1 stabilization slip:
- Bencal SPV1 (4 yr): $5,575 + 4 × $16,419 = $71,251
- Bencal SPV2 (4 yr): $9,105 + 4 × $16,664 = $75,761
- Bencal Management (3 yr acceptable; lower opex risk): $41,713

### 26d. Data science / Rust architect findings

**`OpexBudget` struct redesign (drop-in replacement):**
- Split `OpexSetup` (Y0 one-time) from `OpexAnnual` (flat Y1–Y10) — no Y1/Y2+ distinction
- `ReserveSource` enum: `CommissionInjection` / `CommissionRetention` / `InvestorGrossUp`
- `required_reserve()` method: `setup.total() + reserve_years × annual.total()`
- `reserve_schedule(horizon)` → `Vec<f64>` of restricted_cash balance per year
- `going_concern_breach(horizon)` → `Option<u32>` (year of depletion)

**`CommissionWaterfall` struct (NEW module `commission.rs`):**
Inputs: `wcp_offering_size`, `pclp1_offering_size`, commission rates, tax rate, `dealer_legal_expense`,
three reserve amounts. `compute()` method derives `work_fee_residual` and `work_fee_pct`.

**Migration constants (all three entity files):**

| Constant | Old value | New value |
|---|---|---|
| Bencal SPV2 legal annual | $1,145 (Y2+ was implicit) | $1,145 flat |
| Bencal SPV1/Bencal Management legal annual | $8,875 Y1 / $1,145 Y2+ | $900 flat |
| All accounting annual | $3,774 Y1 / $2,399 Y2+ | $2,399 flat |
| Bencal SPV1/Bencal Management legal setup | $7,730 | $4,950 |
| Bencal SPV2 legal setup | $7,730 | $7,730 (unchanged) |
| Accounting setup | $1,375 | $275 (KYC only) |
| Bank setup | (absent) | $350 (Bencal SPV1/Bencal Management) / $1,100 (Bencal SPV2) |
| Director fee | $13,120 Y1–Y10 | $13,120 flat (Bencal SPV1/Bencal SPV2); $8,747 (Bencal Management) |
| Administration | $20,000 | **removed** (not in Excel) |

**New module deliverables for Phase A-D6:**
- `src/opex.rs` — `OpexBudget`, `OpexLineItem`, `OpexCategory`, `OpexSetup`, `OpexAnnual`
- `src/commission.rs` — `CommissionWaterfall`
- `src/spv/bencal.rs` — wire `CommissionWaterfall::compute()`; emit updated constants
- `src/spv/ambassadors_d1.rs` — receive `bencal_reserve_injection` argument
- `src/spv/ambassadors_d2.rs` — receive `bencal_reserve_injection` argument
- `tests/spv_opex.rs` — reserve drawdown; going-concern trigger; commission math
- `src/report/opex_summary.rs` — `build_opex_summary()` for IS/BS/CF opex rows
