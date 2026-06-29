---
schema: foundry-journal-v1
artifact_type: JOURNAL
state: stub
version: "0.1"
title: "Top 600 Regional Markets: A Country-Normalized Co-location Scoring System for Multi-Country Retail Site Selection"
target_journal: "Journal of Retailing and Consumer Services"
target_publisher: "Elsevier"
impact_factor: "11.6"
alternate_venue: "Environment and Planning B: Urban Analytics and City Science (SAGE, Q1)"
authors:
  - name: "Jennifer M. Woodfine"
    affiliation: "Woodfine Management Corp., New York, New York"
    email: corporate.secretary@woodfinegroup.com
    orcid: ""
    credit_roles:
      - Conceptualization
      - Methodology
      - Writing – Original Draft
      - Writing – Review & Editing
  - name: "Peter M. Woodfine"
    affiliation: "Woodfine Management Corp., New York, New York"
    email: ""
    orcid: ""
    credit_roles:
      - Conceptualization
      - Validation
      - Writing – Review & Editing
  - name: "Mathew Woodfine"
    affiliation: "Woodfine Management Corp., New York, New York"
    email: ""
    orcid: ""
    credit_roles:
      - Software
      - Data Curation
      - Writing – Review & Editing
subject_codes:
  - "R12"
  - "L81"
  - "R30"
keywords:
  - regional market
  - retail co-location
  - site selection
  - country normalization
  - suburban retail
  - commercial geography
  - proforma coverage
  - OpenStreetMap
bcsc_class: public-disclosure-safe
ai_tool_used: "claude-sonnet-4-6 (Anthropic)"
corresponding_author: corporate.secretary@woodfinegroup.com
word_count_body: 800
word_count_target: 8000
submission_status: not-submitted
paired_with: JOURNAL-retail-colocation-v0.1.draft.md
cites: []
forbidden_terms_cleared: true
section_status:
  abstract: stub
  s1_introduction: stub
  s2_methodology: stub
  s3_country_normalization: stub
  s4_results: draft
  s5_proforma_coverage: draft
  s6_discussion: stub
  s7_falsification: stub
  s8_conclusion: stub
refs_status:
  count: 0
  quality: insufficient
  blockers:
    - "Body sections s1/s2/s3/s6/s7/s8 require full expansion (8,000 word target)"
    - "Literature review (§2) needs CBRE/JLL/Colliers site selection methodology citations"
    - "Country normalization methodology (§3) needs formal statistical justification"
notes_for_editor: |
  Stub as of 2026-06-29. Scoring simulation complete on current OSM data (June 2026):
  NA TOP600: US 515 / CA 56 / MX 29.
  EU TOP600: FR 177 / DE 175 / GB 86 / ES 47 / IT 43 / PL 37 / NL 21 / PT 5 / DK 4 / FI 2 / SE 1 / GR 0.
  Country normalization (v0.4): best site per country reaches index 100 globally.
  Proforma coverage: US ✓, GB ✓ — six markets below 3× target; GR 0 (chain ingest pending).
  Companion papers: Woodfine et al. J1 (retail co-location taxonomy); J2 (Commuter archetype).
---

---

# Top 600 Regional Markets: A Country-Normalized Co-location Scoring System for Multi-Country Retail Site Selection

---

## Abstract

[stub — to expand to 250 words]

We present a system for ranking suburban-regional retail co-location markets across two
continents — North America (Canada, United States, Mexico) and Europe (twelve countries) — using
an open-data spatial pipeline derived from OpenStreetMap. The ranking system assigns each
market a quality × isolation score reflecting co-location depth and geographic independence,
then applies per-country min-max normalization before producing a global ranked list. Country
normalization ensures the highest-ranked market in Greece, Poland, or Mexico competes at equal
footing with the highest-ranked market in Germany or the United States. The Top 600 Regional
Markets system supports the Woodfine Capital Projects Direct-Hold Solution, a multi-country
real estate portfolio requiring 3× development-site candidates per target market. Results
show that 2 of 9 target markets (US, GB) currently meet the 3× proforma target within the
TOP600 list; chain data ingests are planned to close the remaining gaps in Greece, Italy,
Poland, and the Nordic group (DK+SE+NO+FI). The methodology is replicable across any
country with adequate OSM chain-retail coverage and provides a transparent, peer-reviewable
alternative to proprietary retail analytics platforms for multi-country site selection.

---

## 1. Introduction

[stub — to expand]

Retail site selection at continental scale is typically undertaken using proprietary transaction
databases (CBRE RetailCore, JLL Retail Analytics, Colliers RetailNext). These platforms
provide granular consumer demand data but are limited to markets where the data provider
holds contracts, making multi-country coverage uneven and expensive. For investors entering
new markets — particularly in Central and Eastern Europe, Southern Europe, and Latin America —
proprietary data is frequently absent or unaffordable.

This paper presents an open-data alternative: the Top 600 Regional Markets system, derived
entirely from OpenStreetMap point-of-interest data, Wikidata chain identifiers, and public
geographic boundary datasets. The system identifies and scores suburban-regional markets —
towns and cities that serve their own geographic catchment at a minimum distance from major
metropolitan cores — based on two observable signals available in OSM: (1) the co-location
of retail anchor chains from multiple categories (home improvement, sport, electronics, food),
and (2) the geographic isolation of the market from competing retail hubs.

---

## 2. Literature Review

[stub — to expand; ~2,000 words]

Key themes to cover:
- Suburban retail market identification methodology (CBRE, JLL, Colliers)
- Co-location as a quality signal (retail geography literature)
- Country-level retail density variation in multi-country studies
- OpenStreetMap data quality for commercial geography applications
- Site selection scoring systems and their limitations

---

## 3. The Co-location Scoring System

### 3.1 Tier Taxonomy

Markets are built from co-location clusters defined in Woodfine et al. (2026) J1. Clusters
are classified into three tiers:

| Tier | Composition criterion |
|---|---|
| T1 | Two or more anchor categories from different sectors (e.g. home + sport + electronics) |
| T2 | Two or more anchors from one sector category, or T1 composition with tight spatial arrangement |
| T3 | Single anchor category with satellite presence |

A Regional Market is a geographic settlement containing one or more co-location clusters.
Markets are included in the scoring pool when the nearest external T1 hub is between 3 km
and 120 km (suburban-regional classification).

### 3.2 Base Score Formula

```
score = quality × isolation × civic × confidence
quality = (1 + 0.30 × √(depth − floor)) × tightness
floor   = 2 if anchor_d ≤ 40 km, else 4
isolation = 0.65 + 0.35 × min(anchor_d, 90) / 90
anchor_d  = km to nearest T1 hub outside this market
```

Civic multiplier (×1.15) applied when a hospital or university is co-located within the
market boundary. Confidence multiplier based on OSM feature completeness score.

### 3.3 Country Normalization (v0.4)

[stub — key section; to expand with statistical justification]

Raw scores vary systematically by country due to differences in retail chain density,
OSM data completeness, and absolute market size. Germany and France have deep retail chain
penetration; Greece and Poland have fewer large-format chains. Without normalization, the
TOP600 is dominated by German and French markets regardless of relative investment opportunity.

Country normalization applies min-max normalization within each country group:

```
norm_score = (score − country_min) / (country_max − country_min)
```

Nordic countries (DK, SE, NO, FI) are normalized as one group, reflecting their integration
as a single investment region for portfolio planning purposes.

---

## 4. Data and Results

### 4.1 Database Coverage (June 2026)

- 6,493 co-location clusters identified across 18 countries
- 2,110 NA suburban-regional candidates; 1,813 EU candidates
- TOP600 NA and TOP600 EU slices produced per continent

### 4.2 NA TOP600 Distribution

| Country | Markets in TOP600 | Proforma target | 3× needed | Status |
|---|---|---|---|---|
| US | 515 (85.8%) | 44 | 132 | ✓ |
| CA | 56 (9.3%) | 22 | 66 | ✗ gap=10 |
| MX | 29 (4.8%) | 22 | 66 | ✗ gap=37 |

**Top 5 NA (country-normalized):**
1. London, Ontario (CA) — nr. Kitchener 76 km — index 100
2. Lake Charles, Louisiana (US) — nr. Baton Rouge 109 km — index 100
3. Boca del Río (MX) — nr. Puebla 90 km — index 100
4. Mesa, Arizona (US) — nr. Phoenix 18 km — index 95
5. Fort Wayne, Indiana (US) — nr. Indianapolis 107 km — index 93

Country normalization places one market from each country at index 100. Prior to
normalization (v0.3), the top 5 NA were entirely US markets (Fort Wayne IN led at 100).

### 4.3 EU TOP600 Distribution

| Country | Markets in TOP600 | Notes |
|---|---|---|
| France | 177 (29.5%) | non-proforma |
| Germany | 175 (29.2%) | non-proforma |
| Great Britain | 86 (14.3%) | proforma ✓ |
| Spain | 47 (7.8%) | proforma — gap=19 |
| Italy | 43 (7.2%) | proforma — gap=23 |
| Poland | 37 (6.2%) | proforma — gap=29 |
| Netherlands | 21 (3.5%) | non-proforma |
| Portugal | 5 (0.8%) | — |
| Denmark | 4 (0.7%) | Nordic group |
| Finland | 2 (0.3%) | Nordic group |
| Sweden | 1 (0.2%) | Nordic group |
| Norway | 0 | Nordic group |
| Greece | 0 | proforma — gap=66, chain ingest pending |

**Nordic group total: 7** (DK 4 + FI 2 + SE 1 + NO 0). Proforma target: 66 (3× of 22).
Structural gap: limited large-format co-location density in Nordic markets compared to
Central Europe.

**Top 5 EU (country-normalized — one per country at index 100):**
1. Esbjerg (DK) — nr. Odense 118 km — index 100
2. TauntonDeane (GB) — nr. Cardiff 110 km — index 100
3. Rosenheim (DE) — nr. Munich 80 km — index 100
4. Albi (FR) — nr. Toulouse 77 km — index 100
5. Jerez de la Frontera (ES) — nr. Cádiz 76 km — index 100

---

## 5. Proforma Coverage Analysis

[draft — to expand to full section]

The Woodfine Capital Projects Direct-Hold Solution proforma (V2, June 2026) defines per-country
development site targets for the Woodfine Buildings Portfolio. The TOP600 system is evaluated
against the requirement to provide 3× the proforma target as a candidate pool — ensuring three
development sites are available for each site ultimately committed to the portfolio.

**Proforma source:** 
`Proforma_BuildingPortfolio_V2.pdf` (CA/US/ES/MX confirmed; Spain template applied to remaining markets).

| Country | Proforma sites | 3× needed | Currently available | Meets 3× | Gap |
|---|---|---|---|---|---|
| CA | 22 | 66 | 56 | No | 10 |
| US | 44 | 132 | 515 | Yes | — |
| MX | 22 | 66 | 29 | No | 37 |
| ES | 22 | 66 | 47 | No | 19 |
| PL | 22 | 66 | 37 | No | 29 |
| NORDICs | 22 | 66 | 7 | No | 59 |
| GB | 22 | 66 | 86 | Yes | — |
| IT | 22 | 66 | 43 | No | 23 |
| GR | 22 | 66 | 0 | No | 66 |

Key observations:
- US and GB already exceed 3× proforma with significant surplus
- CA gap (10) may be addressable with additional Canadian suburban chain ingests
- GR gap (66) is entirely a data gap — AB Vassilopoulos, Kotsovolos, Leroy Merlin GR
  ingests are planned
- NORDICS gap (59) is structural — limited large-format retail density means the 22-site
  proforma target for the Nordic group may need to be reassessed pending market entry
- MX and PL gaps require both chain ingests and proforma review

---

## 6. Discussion

[stub — to expand]

---

## 7. The Falsification Programme

[stub — to expand; analogous to J1/J2 falsification programs]

**H₁:** Country normalization produces a distribution of top-ranked markets that better
reflects actual retail investment opportunity across all target markets versus raw scoring.
Falsification criterion: if market outcome data (transaction volume, retailer expansion
decisions) shows no correlation with normalized rank but positive correlation with raw rank,
normalization is uninformative.

**H₂:** Chain OSM completeness explains residual gaps in proforma coverage for MX, GR.
Falsification criterion: if ingest of confirmed chains (AB Vassilopoulos, Leroy Merlin GR)
fails to close the GR gap materially, the gap reflects market structure rather than data quality.

---

## 8. Conclusion

[stub — to expand]

---

## 9. AI Use Disclosure

This manuscript was prepared with assistance from Claude Sonnet 4.6 (Anthropic). The AI
assisted with data analysis, draft structuring, and language revision. All research design,
hypothesis formulation, and analytical decisions were made by the authors.

---

## 10. CRediT Contributor Roles

**Jennifer M. Woodfine:** Conceptualization, Methodology, Writing – Original Draft, Writing – Review & Editing.

**Peter M. Woodfine:** Conceptualization, Validation, Writing – Review & Editing.

**Mathew Woodfine:** Software, Data Curation, Writing – Review & Editing.

---

## References

*[To be populated. Key references: CBRE Global Retail Report; JLL Retail Analytics; Colliers
Retail Outlook; Woodfine et al. 2026 J1 (retail co-location taxonomy); Evers et al. (retail
internationalisation); Wood (retail geography); Jones and Simmons (retail geography).]*

---

## Appendix A — Scoring Algorithm Detail

See `app-orchestration-gis/score-regional-markets.py` for the full implementation.

Key constants (v0.4):
```python
TOP_N = 600
NORDIC_ISOS = {'DK', 'SE', 'NO', 'FI'}
PROFORMA_TARGETS = {
    'CA': 22, 'US': 44, 'MX': 22,
    'ES': 22, 'PL': 22, 'NORDICS': 22,
    'GB': 22, 'IT': 22, 'GR': 22,
}
```

Country normalization function:
```python
def apply_country_norm(regional_list):
    groups = {}
    for r in regional_list:
        g = norm_group(r['iso'])
        groups.setdefault(g, []).append(r)
    for g, members in groups.items():
        scores = [m['score'] for m in members]
        g_min, g_max = min(scores), max(scores)
        span = g_max - g_min
        for m in members:
            m['norm_score'] = round(1.0 if span == 0 else (m['score'] - g_min) / span, 4)
    return sorted(regional_list, key=lambda r: r['norm_score'], reverse=True)
```
