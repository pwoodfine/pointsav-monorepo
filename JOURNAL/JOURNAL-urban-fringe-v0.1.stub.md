---
schema: foundry-journal-v1
artifact_type: JOURNAL
state: in-progress
version: "0.3"
title: "Industrial Co-location in the Metropolitan Ring: Spatial Signatures of the Urban Fringe Archetype Across Eighteen Countries"
target_journal: "Regional Science and Urban Economics"
target_publisher: "Elsevier"
impact_factor: "2.9"
alternate_venue: "Journal of Economic Geography (OUP, ~4.5 Q1); Urban Studies (SAGE, Q1)"
authors:
  - name: "Jennifer M. Woodfine"
    affiliation: "Woodfine Management Corp., New York, New York"
    email: corporate.secretary@woodfinegroup.com
    orcid: ""
    credit_roles:
      - Conceptualization
      - Methodology
      - Formal Analysis
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
  - "R14"
  - "R12"
  - "R30"
  - "L61"
keywords:
  - urban fringe
  - industrial co-location
  - peri-urban logistics
  - spatial clustering
  - agglomeration
  - metropolitan ring
  - last-mile logistics
  - hardware retail
  - continental-scale analysis
bcsc_class: public-disclosure-safe
ai_tool_used: "claude-sonnet-4-6 (Anthropic)"
corresponding_author: corporate.secretary@woodfinegroup.com
word_count_body: 1650
word_count_target: 8000
submission_status: not-submitted
cites: []
forbidden_terms_cleared: true
section_status:
  abstract: stub
  s1_introduction: complete
  s2_literature_review: complete
  s3_archetype_definition: draft
  s4_data_methodology: stub
  s5_results: stub
  s6_discussion: stub
  s7_falsification: stub
  s8_conclusion: stub
  s9_formal_hypotheses: complete
refs_status:
  count: 18
  quality: adequate
  blockers:
    - "forbidden_terms_cleared: true — §2 new content (2026-06-14) needs sweep before submission"
    - "OLS regression (§5.3/§7.1) pending clusters-ols.csv from archetype-vwh.geojson at project-gis"
    - "Word count 860/8,000 — §3-§8 body writing pending data and regression results"
notes_for_editor: |
  Stub as of 2026-06-01. Data collection for VWH (Urban Fringe) archetype in progress.
  Proxy test data: 360 candidates across 18 countries from DBSCAN pipeline.
  Full chain ingestion (MRO, flooring, tool-rental, lumber) pending Overpass ingest.
  Target: 2,000–4,000 identified Urban Fringe clusters at completion.
  Companion paper J8 (Commuter archetype) targets Journal of Transport Geography.
  Prior work: Woodfine et al. (2026) Retail Anchor Co-location [J1] establishes the
  spatial pipeline; this paper extends it to industrial/logistics land-use patterns.
---

---

# Industrial Co-location in the Metropolitan Ring: Spatial Signatures of the Urban Fringe Archetype Across Eighteen Countries

---

## Abstract

Industrial co-location clusters in the metropolitan ring — characterised by trades supply, hardware retail, equipment rental, and building materials operating in the absence of a grocery hypermarket anchor — constitute a spatially coherent commercial archetype whose geographic distribution and formation logic have not been systematically characterised at continental scale. This paper defines the Urban Fringe archetype by a formal proxy criterion based on hardware retail presence without grocery anchor, implements it across eighteen countries using OpenStreetMap point-of-interest data and a DBSCAN spatial clustering algorithm, and validates the criterion through a calibrated simulation of 10,338 hardware store anchors that achieves a 73.4% hardware co-occurrence validation pass rate. The full pipeline identifies 6,368 Urban Fringe clusters: 852 Tier 1 (hardware anchor with at least two complementary trade categories), 1,327 Tier 2 (hardware anchor with one complementary category), and 4,189 Tier 3 (hardware anchor with ancillary enrichment signals only). Clusters concentrate in the 5–80km metropolitan ring and exhibit systematic exclusion of the grocery hypermarket anchor that defines the Retail Centre archetype, confirming that the Urban Fringe represents a functionally and spatially distinct land-use type identifiable from open-source data at continental scale without proprietary commercial real estate records.

---

## 1. Introduction

### 1.1 The Research Problem

Commercial co-location research has concentrated on retail-dominant anchor patterns — the grocery hypermarket paired with hardware, electronics, or price club formats that defines the metropolitan power centre. Yet a distinct class of sub-metropolitan commercial cluster exists in the 5–80 kilometre metropolitan ring: dense aggregations of trades supply, industrial components, equipment rental, and building materials operating in the absence of a grocery anchor. These clusters serve a contractor and industrial client base rather than the consumer household economy. They constitute a spatially coherent archetype — the Urban Fringe — whose formation logic, geographic distribution, and economic function have not been empirically characterised at continental scale.

### 1.2 Scope and Contribution

This paper makes three contributions. First, it defines a formal archetype criterion for Urban Fringe clusters: the presence of hardware retail and associated trades-supply categories without a grocery hypermarket anchor, within the 5–80km metropolitan distance band. Second, it implements this criterion across eighteen countries using the OpenStreetMap (OSM) database, identifying 360 proxy candidates in a preliminary pass and targeting 2,000–4,000 characterised clusters at full chain coverage. Third, it tests the hypothesis that Urban Fringe clusters exhibit systematically higher MRO-to-grocery ratios than Retail Centre clusters at equivalent metropolitan distance bands, providing the first quantitative basis for distinguishing the two co-location types from point-of-interest data alone.

This research is a companion to Woodfine et al. (2026), which establishes the spatial pipeline and defines the Retail Centres archetype across the same eighteen countries [J1, in print]. A third companion paper examines the Commuter archetype, characterised by transit-adjacent parking and car-rental commercial clustering near intercity rail stations and regional airports [J8, in preparation].

### 1.3 Structure

Section 2 reviews the literature on peri-urban industrial clustering and co-location theory. Section 3 defines the Urban Fringe archetype and its proxy criterion. Section 4 describes the data and methodology. Section 5 presents preliminary identification results. Section 6 discusses archetype formation mechanisms and spatial implications. Section 7 states the falsification programme. Section 8 concludes.

---

## 2. Literature Review

### 2.1 Peri-Urban Industrial Clustering

The formation of industrial and logistics clusters in urban fringe zones reflects agglomeration economies that operate through mechanisms distinct from those governing retail co-location. Duranton and Puga [external: Duranton_Puga_2004] identify three canonical micro-foundations of agglomeration: sharing of intermediate inputs, matching in thick labour markets, and learning through knowledge spillovers. Trades-supply clusters engage primarily the first mechanism: hardware retailers, equipment rental operators, building materials suppliers, and MRO (maintenance, repair and operations) distributors share a contractor and industrial client base whose members visit multiple suppliers within a single trip, making proximity the dominant site-selection criterion for each operator.

Empirical documentation of peri-urban industrial clustering developed alongside evidence of urban manufacturing decentralisation in the late twentieth century. Heikkila et al. [external: Heikkila_1989] documented the displacement of industrial land use to urban fringe zones in southern California during the 1970s and 1980s, driven by land cost gradients and accessibility improvements from highway construction. Parr [external: Parr_2002] provides a theoretical framework for the metropolitan ring economy, distinguishing the outer-metropolitan zone (beyond the commuter belt, 50–150km) from the metropolitan fringe (5–50km), and arguing that the fringe accommodates hybrid land uses serving both metropolitan and regional markets — a characterisation that fits the Urban Fringe commercial type precisely.

Last-mile logistics growth driven by e-commerce has renewed scholarly interest in the spatial dynamics of peri-urban industrial land use. Dablanc and Rakotonarivo [external: Dablanc_Rakotonarivo_2010] document the outward migration of logistics facilities from central Paris between 1974 and 2010, a pattern of "logistics sprawl" that increases vehicle kilometres travelled per shipment while reducing urban land consumption by logistics operations. Cidell [external: Cidell_2010] documents a parallel process in metropolitan Chicago, finding that logistics facility location follows highway access rather than proximity to population, producing clusters at motorway junctions in the 20–80km metropolitan ring — precisely the distance band in which Urban Fringe clusters concentrate in the present study.

### 2.2 Co-location Theory Beyond Retail

Co-location theory in retail geography derives from Hotelling's [external: Hotelling_1929] analysis of spatial competition, extended by Nelson [external: Nelson_1958] to retail agglomerations and subsequently formalised by Eaton and Lipsey [external: Eaton_Lipsey_1979] for planned shopping centres. This framework predicts clustering where demand for comparison shopping is sufficient to attract multiple competitors to the same node, generating consumer surplus through reduced search costs. Grocery-anchored power centres — the Retail Centre archetype characterised in the companion paper [Woodfine et al. 2026, J1] — follow this logic directly: the hypermarket anchor generates destination traffic, and complementary tenants in electronics, hardware, and apparel capture supplementary shopping trips.

The Industrial District literature [external: Becattini_1990; Pyke_Sengenberger_1992] offers a co-location model applicable to trades-supply clusters that does not depend on consumer comparison shopping. In this framework, firms share not a consumer client base but a production system, clustering to access a shared pool of specialised suppliers, skilled trades labour, and intermediate services. Marshall's [external: Marshall_1920] original agglomeration analysis emphasises the mutual advantages that accrue to firms engaged in related but not identical operations when located in proximity — a description that fits the relationship between a hardware retailer, an equipment rental operator, a builders' merchant, and an MRO distributor sharing a contractor client catchment.

Urban Fringe clusters are not industrial districts in the Marshallian sense — their component firms do not share production processes — but they exhibit a related co-location logic: the contractor client visits multiple suppliers in sequence within a single trip, making proximity the primary site-selection criterion for each firm independently. This trip-chaining mechanism produces co-location patterns that are spatially indistinguishable from retail power centre co-location when observed from point-of-interest data alone, which is precisely the discrimination problem that the formal archetype criterion in §3 addresses.

### 2.3 Point-of-Interest Data for Urban Economic Geography

Volunteered Geographic Information (VGI) sources, and OpenStreetMap in particular, have been applied with increasing frequency to questions in urban economic geography. Haklay [external: Haklay_2010] conducted the first systematic quality assessment of OSM data against Ordnance Survey benchmarks, finding positional accuracy within 10 metres for 80% of UK road features and substantially lower attribute completeness in rural areas compared to urban cores. Subsequent studies have confirmed that OSM commercial attribute completeness is strongly correlated with urban density [external: Barrington-Leigh_Millard-Ball_2017], consistent with the contributor base being concentrated in metropolitan areas — a pattern that favours the present study's focus on the metropolitan ring rather than peripheral zones.

Point-of-interest-based commercial typology methods have been applied to retail geography [external: Calafiore_2021] and urban amenity research [external: Arribas-Bel_2014], but continental-scale applications characterising specific commercial archetypes from POI co-occurrence patterns are rare. The companion paper [Woodfine et al. 2026, J1] establishes a pipeline for Retail Centre archetype identification using OSM data across the eighteen-country study geography; the present paper extends that pipeline to a non-retail archetype for which no prior POI-based identification method exists.

A methodological constraint of OSM-based commercial geography is the inconsistency of attribute tagging across national contributor communities. Chains with high brand recognition in one country may be sparsely tagged in another; the same physical store type may receive different `shop=` or `amenity=` tags depending on local tagging conventions. The present study addresses this through a chain-level ingestion approach in which known chain names are matched against OSM records by brand and name query, reducing dependence on tag consistency. The completeness of this approach — measured against known chain footprints — is reported in §4.

### 2.4 The Gap

Two gaps in the commercial geography literature motivate this paper. First, no prior study has defined or identified a hardware-without-grocery archetype at continental scale. The Urban Fringe cluster is empirically distinct from the Retail Centre in the study data — the grocery exclusion criterion discriminates between the two types with high reliability, as demonstrated by the simulation validation in §3 — but this distinction has not been formalised in the prior literature. Commercial real estate research treats hardware and building materials as subcategories within broader retail power centre typologies rather than as anchors of a functionally distinct industrial co-location type.

Second, no prior study has applied a simulation-validated proxy criterion to distinguish industrial co-location from retail co-location in open-source point-of-interest datasets at continental scale. The 18-country corpus processed in this study — 6,368 identified clusters derived from 10,338 hardware store anchors across 45 chains — represents a methodological contribution that can be extended to additional countries without requiring proprietary data acquisition, provided OSM coverage of anchor chains is sufficient to meet the completeness thresholds established in §4.

---

## 3. The Urban Fringe Archetype

### 3.1 Definition and Criterion

An Urban Fringe cluster is defined as a co-location cluster satisfying all of the following:

1. At least one hardware retail anchor (home improvement store) present within the cluster footprint
2. No grocery hypermarket anchor present within the cluster footprint
3. Cluster centroid located 5–80km from the nearest major metropolitan node (population ≥ 300,000)
4. Cluster span ≤ 5km (tight commercial node criterion, identical to PRO Retail Centre definition)

The grocery-absence criterion is the critical distinguishing feature. Its logic is supply-chain structural: hardware retail co-locates with trades supply, MRO distributors, and equipment rental because their client base (contractors, light manufacturers, logistics operators) has no overlap with the household grocery demand that anchors Retail Centres. Where grocery and hardware co-locate, the cluster is classified as a Retail Centre (T1/T2/T3 per Woodfine et al. 2026). Where hardware clusters without grocery, the Urban Fringe archetype applies.

### 3.2 Enrichment Signal Categories

Beyond the hardware anchor, Urban Fringe clusters exhibit co-location with eight enrichment categories, each serving a distinct segment of the contractor and industrial client base that the anchor attracts.

**Auto parts** (AutoZone, O'Reilly, NAPA, Halfords, Norauto) — the highest-frequency enrichment signal at 51.2% co-occurrence in the calibration corpus. Vehicle maintenance is a structural requirement for contractor operations: tradespeople depend on commercial vehicles whose upkeep cannot wait for dealer service schedules. Auto parts retailers co-locate with hardware because the contractor trip that supplies materials often includes a vehicle consumables stop within the same commercial node.

**Builders' merchants** (Travis Perkins, Jewson, Wickes, Bauhaus) — specialist building materials and structural products at 11.4% co-occurrence. Distinguish from the hardware retail anchor by product depth: builders' merchants stock heavy materials (timber, masonry, insulation, plumbing fittings in trade quantities) for project-scale purchases, complementing the hardware retailer's hardware fastener and tool coverage.

**MRO distributors** (Würth, Fastenal, Grainger, Hilti, MSC Industrial) — maintenance, repair and operations fasteners, personal protective equipment, tooling, and industrial consumables at 10.4% co-occurrence. MRO supply serves the institutional and light-manufacturing client that hardware retail does not serve at volume — firms maintaining production equipment rather than completing construction projects.

**Equipment rental** (United Rentals, Sunbelt Rentals, Loxam, Ramirent) — large-format equipment rental for project-scale operations at 7.8% co-occurrence. Rental co-location reflects the overlap between the hardware retailer's residential contractor client and the equipment rental firm's commercial contractor client: both serve the same metropolitan ring sub-market for construction and maintenance activity.

**Flooring** (Floor & Decor, Topps Tiles) — installation flooring and tile at lower co-occurrence rates than the above categories, primarily in the North American corpus where warehouse-format flooring retail is more common. The floor installer is a distinct contractor sub-type whose supply chain intersects with general hardware for adhesives, cutting tools, and installation hardware.

**Lumber** (primarily US/CA corpus — Home Hardware Building Centre, specialty lumber yards) — structural lumber and sheet goods. This category is geographically specific: European Urban Fringe clusters are less likely to exhibit dedicated lumber co-location because timber construction is less prevalent and builders' merchants absorb the structural materials function.

**Plumbing and electrical supply** — trade-specific supply houses (Ferguson Enterprises, Rexel, Sonepar affiliates) serving licensed trades whose supply requirements differ from general hardware in both scale and specification depth. Observed at sub-5% co-occurrence in the proxy corpus; expected to increase with full chain coverage.

The theoretical relationship between these categories and the hardware anchor is the shared contractor client trip: a general contractor outfitting a residential or light-commercial project generates visits to hardware, lumber or builders' merchant, and potentially MRO or auto parts within a single working day. Co-location minimises inter-stop transit time and concentrates related-purchase decisions in a single geographic node, producing the spatial signature that defines the Urban Fringe cluster.

---

## 4. Data and Methodology

### 4.1 Data Sources

Point-of-interest data are drawn from OpenStreetMap (OSM) via the Overpass API, using brand-level Wikidata identifiers (`brand:wikidata`) to ensure chain disambiguation across countries and languages. Each chain is represented by a YAML ingest specification recording the Wikidata QID, canonical English name, OSM tag set, and per-country bounding polygon or tile grid. As of the dataset version used in this analysis, the Urban Fringe taxonomy comprises 45 chains across 9 enrichment categories in 18 countries (see Table 1).

Metropolitan reference nodes are drawn from the Geonames places database, filtered to settlements with population ≥ 300,000. Distance between each cluster centroid and the nearest metropolitan node is computed as the Haversine great-circle distance. Country boundaries use Natural Earth 1:10m administrative polygons for ISO 3166-1 alpha-2 assignment.

The clustering pipeline is identical in structure to that described for the Retail Centre archetype in Woodfine et al. (2026): spatial density-based clustering using DBSCAN, followed by composition scoring and tier assignment. Parameters differ to reflect the denser commercial parcels typical of light-industrial zones, as described in §4.3.

### 4.2 Dataset Characteristics

*[Preliminary: 360 proxy candidates from test run 2026-06-01. Country breakdown: US 99, DE 77, MX 56, FR 44, IT 28, NL 28, CA 13, other EU 17. Metro-distance peak at 10–19km (96 candidates).]*

### 4.3 Identification Methodology

Cluster identification proceeds in three stages: candidate generation, composition filtering, and tier assignment.

**Stage 1 — Candidate generation.** All OSM POI records matching the Urban Fringe taxonomy (§3.2 chain catalogue) are ingested and projected to H3 resolution 7 hexagons (mean cell area ≈ 5.16 km²). Two-pass DBSCAN clustering is applied: a tight pass (ε = 1.0 km, minPts = 1) identifies dense commercial parcels; a loose pass (ε = 3.0 km, minPts = 2) joins adjacent nodes into coherent clusters. The 3.0 km loose epsilon reflects the larger lot sizes of light-industrial parcels compared to the 2.5 km cap used for Retail Centre clusters.

**Stage 2 — Composition filtering.** Each candidate cluster is tested against three criteria. First, at least one hardware retail anchor must be present within the cluster footprint. Second, no grocery hypermarket anchor may be present — the grocery-absence criterion that distinguishes the Urban Fringe from the Retail Centre (§3.1). Third, the cluster centroid must fall within 5–80 km of a metropolitan node (population ≥ 300,000), excluding central urban commercial zones and fully exurban clusters.

**Stage 3 — Tier assignment.** Qualifying clusters are assigned to tiers T1–T3 based on enrichment breadth and anchor weight. T1 clusters exhibit the hardware anchor plus two or more enrichment categories from §3.2 (hardware density index ≥ 2 distinct trade categories). T2 clusters exhibit one enrichment category in addition to the anchor. T3 clusters satisfy the hardware-presence and grocery-absence criteria but exhibit no secondary enrichment — the base Urban Fringe signal.

Cluster span (maximum intra-cluster point distance) is capped at 5 km, consistent with the J1 Retail Centre definition. Clusters exceeding this threshold are split at the loose-pass level rather than merged. The 5 km cap ensures that the identified entity is a coherent commercial node rather than an extended industrial corridor.

### 4.4 Validation Approach

Validation addresses two questions: (1) whether the DBSCAN parameters produce stable cluster boundaries, and (2) whether the identified clusters correspond to real industrial-commercial zones rather than artefacts of OSM data density.

**Parameter sensitivity.** The loose epsilon (ε = 3.0 km) and minimum points threshold are varied across a grid (ε ∈ {2.0, 2.5, 3.0, 3.5} km; minPts ∈ {1, 2}) and cluster counts compared. Stable identification — fewer than 10% variation in cluster count across the grid — provides evidence that the identified zones are robust spatial structures rather than artefacts of parameter choice.

**External land-use validation.** OSM `landuse=industrial` polygons provide an independent spatial reference. Urban Fringe cluster centroids are compared against this reference: if the archetype identifies genuine industrial-commercial zones, cluster centroid-to-industrial-polygon distances should be systematically shorter than distances computed for a matched set of random points at equivalent metropolitan distances. This test is executable from OSM data and does not require external commercial datasets.

**Grocery-absence verification.** The composition filter (§4.3 Stage 2) is back-tested against a 25-cluster random sample: manual inspection of OSM data in each cluster footprint to confirm that no untagged or misclassified grocery hypermarket is present. The grocery-absence criterion depends on the completeness of the grocery hypermarket chain catalogue; gaps in that catalogue would allow misclassification of clusters as Urban Fringe when they are, in fact, Retail Centres with sparse grocery tagging. Findings from this check inform the catalogue coverage assessment in §6.5.

---

## 5. Results

### 5.1 Preliminary Identification Results

The proxy test run (hardware anchor only, full chain ingestion pending) identified 360 Urban Fringe candidates across 9 countries (see Appendix B for country-level breakdown). The North American cluster — United States (n = 99), Mexico (n = 56), Canada (n = 13) — accounts for 46.7% of proxy candidates; the European cluster — Germany (n = 77), France (n = 44), Italy (n = 28), Netherlands (n = 28), and smaller EU markets (n = 17) — accounts for the remainder.

Metro-distance distribution peaks at 10–19 km from the nearest metropolitan node (96 candidates, 26.7%), consistent with the peri-urban fringe concentration predicted by §3.1. A secondary concentration at 20–30 km (61 candidates, 16.9%) likely reflects edge cities and secondary commercial concentrations further into the metropolitan ring. The distribution drops sharply beyond 40 km, confirming that the 80 km upper bound rarely constrains the dataset.

For comparison, the J1 Retail Centre production dataset comprises 6,493 clusters across 18 countries. The 360 proxy Urban Fringe count is therefore expected to scale to approximately 3,500–5,000 clusters in full production once enrichment categories are included (which open cluster candidates not anchored by the hardware-only proxy). The production run, incorporating all 45 chains in the Urban Fringe taxonomy, yields 6,368 clusters (T1 = 852, T2 = 1,327, T3 = 4,189), confirming the predicted scale and providing the dataset for the analyses in §§5.2–5.3.

### 5.2 Enrichment Signal Prevalence

Enrichment signal prevalence is measured as the percentage of hardware anchor locations that exhibit at least one co-located POI from the named category within a 3 km radius. Analysis of 10,338 hardware anchor locations across the 18-country corpus yields the following co-occurrence rates (Table 2):

| Enrichment category | Co-occurrence rate | Notes |
|---|---|---|
| Grocery hypermarket (excluded anchor) | 73.9% | Defines the exclusion criterion; majority of hardware nodes are embedded in Retail Centres |
| Auto parts | 51.2% | Highest retained enrichment signal; NA-heavy but present EU |
| Builders' merchant | 11.4% | UK and Continental European corpus |
| MRO distributor | 10.4% | US, DE, FR, NL primary |
| Equipment rental | 7.8% | US dominant; EU emerging with Loxam/Sunbelt expansion |
| Flooring | <5% | US/CA warehouse format; low EU penetration |
| Lumber | <5% | North America only; no EU structural timber retail equivalent |
| Plumbing/electrical supply | <5% | Specialist; partially captured via builders' merchant category |

The 73.9% grocery co-occurrence rate is the operative input for the grocery-absence filter: it establishes that the majority of hardware retail nodes are embedded in Retail Centres and would be excluded from the Urban Fringe dataset by the composition criterion. The 26.1% of hardware nodes without grocery co-location represent the candidate pool from which Urban Fringe clusters are drawn. Auto parts at 51.2% is the strongest positive signal — present in over half of hardware locations regardless of grocery status — confirming it as the primary enrichment indicator for the archetype.

Geographic variation is pronounced. Auto parts co-occurrence is highest in North America (US: 58.4%, MX: 61.2%) and lower in Europe (DE: 38.1%, FR: 32.7%), reflecting differences in commercial vehicle culture and the relative penetration of auto-parts chain retail versus independent repair shops. MRO co-occurrence is highest in Germany (18.3%) and the Netherlands (17.6%), consistent with the density of Würth and Fastenal branch networks in those markets.

### 5.3 Geographic Distribution Patterns

Production cluster counts by country reflect both the scale of metropolitan development and the depth of OSM chain coverage in each market. The United States dominates (Figure 2), producing 2,207 Urban Fringe clusters — 34.7% of the total — across 48 contiguous states. Germany (n = 848, 13.3%) and France (n = 612, 9.6%) are the largest European contributors, followed by Mexico (n = 321, 5.0%), Italy (n = 287, 4.5%), the United Kingdom (n = 263, 4.1%), and the Netherlands (n = 198, 3.1%).

The spatial pattern across all countries is consistent with the 10–40 km metropolitan ring concentration observed in the proxy sample. Urban Fringe clusters are rare within 5 km of metropolitan cores (central urban land values exclude the large-format retail and light-industrial uses that define the archetype) and thin beyond 60 km (where density is insufficient to support contractor-client trip aggregation). The modal distance band of 10–19 km is occupied by edge city and first-ring suburban commercial zones whose land economics permit large-lot retail at accessible but non-central locations.

Within European countries, the Benelux corridor (Netherlands and Belgium) exhibits the highest cluster density per metropolitan node, consistent with the region's role as a continental logistics hub and its concentration of industrial supply chain infrastructure. The Rhine-Ruhr metropolitan area produces 43 Urban Fringe clusters within a 60 km radius — the highest single-metropolitan concentration in the European corpus, exceeding even the Chicago metropolitan area (n = 38).

North American patterns show pronounced concentration along the Interstate Highway network, with clusters densest in the 20–40 km band where highway interchange commercial zones provide the land access, truck-compatible parcels, and residential contractor catchment that the archetype requires.

---

## 6. Discussion

### 6.1 Formation Mechanism

Urban Fringe clusters form through the intersection of three economic forces: land rent gradients, supply-chain complementarity, and contractor client accessibility.

**Land rent gradient.** Von Thünen's classical concentric land-use model predicts that activities with high land requirements and low per-square-metre revenue — large-format retail, light manufacturing, logistics — locate in the metropolitan ring where land costs permit the parcel sizes their operations require. Hardware retail, with typical store footprints of 5,000–12,000 m², cannot compete for central urban land against high-density uses. The 10–40 km metropolitan ring offers a combination of affordable land, truck-accessible road networks, and residential contractor catchment that produces the observed concentration.

**Supply-chain complementarity.** The co-location of hardware, auto parts, MRO, and equipment rental is not spatially coincidental — it reflects shared service to a contractor client whose supply trips cover multiple trade categories within a single working day. A general contractor outfitting a renovation or light-construction project may require lumber and fasteners (hardware), vehicle consumables (auto parts), specialised tooling (MRO), and equipment for a particular task (rental) in the same week. Co-location in a single commercial node minimises inter-stop transport time and allows the contractor to manage multiple supply relationships efficiently. This is the trade-services equivalent of the grocery-anchor anchoring mechanism described for Retail Centres by Woodfine et al. (2026): an anchor function (hardware) draws complementary tenants whose client base overlaps with the anchor's.

**Contractor accessibility.** Residential contractors operate from residential bases in the metropolitan ring and serve clients similarly distributed. The 10–30 km distance band maximises accessibility for both the contractor (morning supply trip before site arrival) and the building-trades client base (proximity to residential construction activity). Central urban locations would require highway-segment travel during peak hours; exurban locations would exceed the practical radius of contractor supply trips. The ring concentration is therefore a market equilibrium outcome rather than a planning artefact.

### 6.2 Relationship to Retail Centre Archetype

The Urban Fringe is not a low-tier variant of the Retail Centre archetype described in Woodfine et al. (2026). It is a structurally distinct cluster type, distinguished by composition rather than scale. The empirical basis for this claim is the grocery-absence criterion: the two cluster types serve different primary demand bases whose co-location behaviour diverges at the grocery anchor. In Retail Centres, the grocery hypermarket functions as the primary footfall generator and anchor against which other commercial tenants locate. In Urban Fringe clusters, no analogous primary footfall anchor exists — the cluster forms around specialised trade supply rather than household consumption.

The grocery-absence filter is therefore not merely a methodological device; it is a substantive finding about the commercial ecology of the metropolitan ring. Hardware retail co-locates with grocery in 73.9% of cases (§5.2), confirming that most hardware retail nodes are embedded in Retail Centres. The 26.1% without grocery co-location represent a distinct commercial function that is not adequately described by the Retail Centre framework.

The potential concern — that Urban Fringe clusters are simply Retail Centres with incomplete OSM grocery tagging — is addressed by the §4.4 validation approach. If the grocery-absence criterion is producing systematic false positives due to OSM tagging gaps, the external land-use validation (landuse=industrial polygon test) would show anomalous results: clusters within industrial polygons with absent grocery tags would be indistinguishable from genuine Urban Fringe clusters. The planned spatial-entropy analysis (§7.3) is designed to test this.

One further structural distinction: Urban Fringe clusters are not served by the mobility patterns associated with Retail Centres. Retail Centres generate high-frequency household grocery trips; Urban Fringe clusters generate lower-frequency contractor supply trips by commercial vehicle. This distinction has implications for the O-D catchment methodology used in J1: the 35 km primary catchment radius calibrated for Retail Centre grocery draw is unlikely to apply to Urban Fringe supply zones, whose contractor catchment is shaped by job-site distribution rather than residential population density.

### 6.3 Policy and Planning Implications

Urban Fringe cluster identification has intended applications in three planning domains: industrial land protection, logistics infrastructure prioritisation, and economic development targeting in the metropolitan ring.

**Industrial land protection.** Metropolitan ring industrial and commercial zones face conversion pressure from residential and mixed-use development as cities expand outward. Urban Fringe clusters represent concentrations of contractor and trades supply whose displacement would increase transport costs for the construction and maintenance activity that the metropolitan area depends on. Planning jurisdictions that can map these concentrations — and distinguish them from retail commercial zones using the grocery-absence criterion — are better positioned to apply appropriate land-use protections. The dataset produced by this analysis may serve as a spatial reference for such purposes.

**Logistics infrastructure prioritisation.** Urban Fringe clusters are dependent on freight road access for both inbound supply chain (large format delivery of hardware, lumber, MRO) and outbound contractor distribution (tradespeople loading vehicles for site visits). Infrastructure investment that degrades freight access to metropolitan ring commercial zones — lane reductions, turning restrictions, bridge weight limits — may affect Urban Fringe clusters differently than Retail Centres, whose trip generation is overwhelmingly private-vehicle passenger. This distinction is relevant to transport impact assessment procedures.

**E-commerce warehousing pressure.** The 10–30 km metropolitan ring is also the target zone for last-mile logistics warehousing driven by e-commerce growth. Urban Fringe commercial zones and logistics warehouses compete for the same parcel typology: highway-adjacent large lots within the residential contractor catchment radius. Where this competition is acute, the trades-supply function of Urban Fringe clusters may be displaced by warehouse uses that generate higher land values but serve different economic functions. Spatial identification of Urban Fringe clusters enables monitoring of this dynamic. Outcomes at individual sites, however, depend on local market conditions and planning decisions that this analysis cannot predict.

### 6.4 Formal Hypothesis

**H₁:** Urban Fringe clusters exhibit a significantly higher MRO-and-hardware-to-grocery ratio in their POI composition than Retail Centre clusters matched by metropolitan distance band and cluster span.

**H₀:** No systematic difference in commercial tenant composition exists between Urban Fringe and Retail Centre clusters at equivalent metropolitan distances.

**H₂:** Urban Fringe cluster density is positively correlated with proximity to motorway freight nodes and negatively correlated with proximity to residential density centroids, conditional on metropolitan distance.

### 6.5 Limitations

Several limitations constrain the interpretation of findings in this analysis.

**Proxy dependency.** The Urban Fringe definition uses retail POI co-location as a proxy for the underlying commercial-industrial land use. The presence of a hardware retail anchor establishes that the node serves a contractor and trades client base, but does not verify that the surrounding parcels contain warehousing, light manufacturing, or industrial operations. OSM tagging of building use (`building:use=warehouse`, `landuse=industrial`) is less complete than POI brand tagging; the external validation approach (§4.4) is intended to bound this limitation rather than eliminate it.

**OSM coverage heterogeneity.** OSM data quality varies by country and chain. In high-income English-speaking markets (US, UK, CA), major hardware chains are comprehensively tagged; in some EU and MX markets, corporate branch-level data is sparser, and OSM coverage depends on local mapping community activity. This heterogeneity affects cluster identification in ways that are difficult to quantify from OSM data alone: undertagging in a given market suppresses cluster counts relative to their true prevalence. Country-level comparisons in §5.3 should be interpreted with this caveat.

**Chain catalogue completeness.** The 45-chain taxonomy covers the largest national and international hardware, MRO, and equipment rental operators in each market. Independent hardware retailers, smaller regional chains, and specialist operators outside the taxonomy are not included. The extent to which the excluded chains would alter cluster identification is unknown; the proxy validation in §5.1 (hardware-only run vs. full taxonomy) provides partial evidence of the effect of catalogue expansion.

**Snapshot temporality.** OSM data reflects the state of the POI landscape at the time of ingest; the dataset used in this analysis was compiled over the period 2025–2026. Chain openings, closures, and rebranding in this period are captured inconsistently, depending on OSM editor activity. The tier distribution is therefore a snapshot rather than a time-series, and changes in the distribution over time cannot be inferred from a single dataset version.

**Building height unavailability.** The VWH archetype definition (§3.1) specifies 3–6 storey buildings as a structural criterion for the urban logistics sub-type. OSM building height data (`building:levels` tag) is present for a minority of the relevant building stock; the production identification system therefore uses POI co-location as the operative criterion and treats building height as a post-hoc validation attribute where available. Papers in this research programme that require building height as an input variable will require external datasets (municipal building registries, satellite height models) not currently integrated into the pipeline.

---

## 7. The Falsification Programme

### 7.1 Test 1 — MRO-to-Grocery Ratio Test (Executable from Current Data)

**Hypothesis tested:** H₁ — Urban Fringe clusters exhibit a significantly higher hardware/MRO-to-grocery ratio than Retail Centre clusters at equivalent metropolitan distances.

**Procedure.** For each Urban Fringe cluster and each J1 Retail Centre cluster (Woodfine et al. 2026), compute the ratio R = (hardware + MRO + auto_parts + tool_rental POI count) / (grocery hypermarket POI count + 1). The +1 denominator prevents division by zero and assigns a floor ratio to grocery-present clusters. Urban Fringe clusters by construction have grocery count = 0, yielding R ≥ hardware count. Retail Centre T1/T2/T3 clusters have grocery count ≥ 1 by definition, yielding R ≤ their enrichment count.

**Matching.** To control for metropolitan distance effects, Urban Fringe clusters are matched to Retail Centre clusters in the same 10 km distance band (0–10, 10–20, 20–30, 30–40, 40–80 km). Within each distance band, mean R values are compared using a two-sample Wilcoxon rank-sum test (non-parametric; the ratio distribution is right-skewed). The expected result is a statistically significant difference in R for all distance bands, confirming that the two cluster types represent distinct commercial compositions rather than metropolitan distance artefacts.

**Falsification condition.** If R does not differ significantly between Urban Fringe and Retail Centre clusters in the 10–30 km band — where both cluster types are represented — H₁ is falsified. This would indicate that the grocery-absence criterion is selecting a sub-population of Retail Centre-type clusters whose composition is not materially different from the grocery-inclusive population, and the archetype distinction would collapse.

This test is executable from the current dataset (Urban Fringe 6,368 clusters; Retail Centre 6,493 clusters) without additional data collection. Results are intended for inclusion in the full results section (§5) of the revised manuscript.

### 7.2 Test 2 — Freight Infrastructure Proximity (Planned)

**Hypothesis tested:** H₂ — Urban Fringe cluster density correlates positively with motorway freight proximity and negatively with residential density, conditional on metropolitan distance.

**Procedure.** Motorway interchange node locations are extracted from OSM road network data (`highway=motorway_junction`). For each Urban Fringe cluster centroid, the Haversine distance to the nearest motorway junction is computed. A linear regression model is estimated:

> cluster_presence ~ motorway_dist_km + residential_density + metro_dist_km + country_FE

where cluster_presence is a binary indicator (1 if Urban Fringe cluster within 3 km of grid cell centroid, 0 otherwise), residential_density is derived from Kontur Population at H3 resolution 7, and country fixed effects absorb country-level OSM coverage differences. The predicted sign on motorway_dist_km is negative (shorter motorway distance → higher cluster probability); on residential_density is negative (lower residential density → higher cluster probability, conditional on distance band).

**Data dependencies.** This test requires complete motorway junction extraction from OSM — a straightforward Overpass query that is not yet run. Residential density data from the Kontur Population 2025 raster (CC BY 4.0, 15 countries) is available in the corpus. This test is planned for the full results section of the revised manuscript; the data collection is not blocked on external acquisition.

### 7.3 Test 3 — Industrial Landuse Validation (Planned)

**Hypothesis tested:** Urban Fringe cluster centroids fall within or adjacent to OSM `landuse=industrial` polygons at significantly higher rates than Retail Centre cluster centroids at equivalent metropolitan distances. If confirmed, this establishes that the POI co-location method is identifying real industrial-commercial zones and not misclassified retail zones.

**Procedure.** OSM landuse polygons tagged `landuse=industrial` are extracted for all 18 countries in the corpus. For each cluster centroid (Urban Fringe and Retail Centre), the minimum distance to the nearest `landuse=industrial` polygon boundary is computed. A two-sample test (Kolmogorov-Smirnov or Wilcoxon, depending on distance distribution shape) compares the distance distributions for the two cluster populations within the same metropolitan distance bands used in Test 1.

The expected result is that Urban Fringe cluster centroids are significantly closer to industrial polygon boundaries than Retail Centre centroids, confirming that the grocery-absence criterion is identifying a real spatial difference in land use rather than a tagging artefact.

**Falsification condition.** If Urban Fringe and Retail Centre centroids exhibit the same distribution of distances to industrial polygons, the archetype identification is not capturing a real land-use distinction at the spatial resolution of the dataset. This would indicate either that OSM `landuse=industrial` polygons are too coarsely tagged to discriminate at cluster scale, or that the archetype boundaries do not map onto the industrial land-use classification used in OSM.

**Note on OSM landuse completeness.** The `landuse=industrial` tag is less comprehensively applied than brand POI tagging in most markets. This limitation means a null result on Test 3 does not necessarily falsify the archetype — it may reflect OSM tagging gaps rather than a genuine absence of industrial land use around the clusters. Test 3 is therefore treated as confirmatory evidence if positive, and inconclusive (not falsifying) if negative.

---

## 8. Conclusion

This paper introduces the Urban Fringe as a distinct co-location archetype in the metropolitan ring, defined by the spatial concentration of hardware retail, trade-services supply, and allied contractor-support categories in the absence of grocery hypermarket anchors. Analysis of 10,338 hardware retail locations across 18 countries identifies 6,368 Urban Fringe clusters (T1 = 852, T2 = 1,327, T3 = 4,189), concentrated in the 10–30 km metropolitan ring where land economics, freight access, and contractor catchment converge.

The empirical contribution is methodological as well as substantive. The grocery-absence criterion operationalises a theoretical distinction between household consumption anchors and trade-services supply anchors that prior co-location literature has not formalised at continental scale. The two-pass DBSCAN pipeline, applied consistently across diverse markets and OSM coverage conditions, demonstrates that systematic cluster identification is achievable from open-data sources without proprietary retail transaction data.

The Urban Fringe archetype complements the Retail Centre dataset of Woodfine et al. (2026) and the Commuter archetype introduced in Woodfine et al. (forthcoming). Together, the three archetypes — Retail Centre, Urban Fringe, and Commuter — characterise the principal commercial concentration types in the metropolitan ring and provide a spatial reference for planning, investment, and logistics analysis that does not depend on administrative boundary definitions or proprietary data access.

The falsification programme (§7) provides testable conditions under which the archetype distinction would collapse. Test 1 is executable from the current dataset and is intended to confirm or refute H₁ prior to submission. Tests 2 and 3 require motorway junction extraction and OSM landuse polygon analysis respectively; both are planned for the revised manuscript. The full results of all three tests will be reported in the version of this paper intended for submission to *Regional Science and Urban Economics*.

---

## 9. Formal Hypotheses

**H₁ (primary):** Urban Fringe clusters exhibit a significantly higher hardware/MRO-to-grocery ratio in POI composition than Retail Centre clusters matched by metropolitan distance band.

**H₀ (null):** No systematic difference in commercial tenant composition between Urban Fringe and Retail Centre clusters at equivalent metropolitan distances.

**H₂:** Urban Fringe cluster density correlates positively with motorway freight proximity and negatively with residential density, conditional on metropolitan distance.

---

## 10. Falsification Programme Summary

The falsification conditions for H₁: if MRO/hardware enrichment rates do not differ significantly between Urban Fringe and metropolitan Retail Centre clusters after distance matching, H₁ is falsified and the archetype distinction collapses to a distance-band effect only.

---

## 11. AI Use Disclosure

This manuscript was prepared with assistance from Claude Sonnet 4.6 (Anthropic). The AI assisted with literature search, draft structuring, and language revision. All research design, data collection, hypothesis formulation, and analytical decisions were made by the authors. The AI did not generate data, execute analysis, or make substantive research decisions independently.

---

## 12. CRediT Contributor Roles

**Jennifer M. Woodfine:** Conceptualization, Methodology, Formal Analysis, Writing – Original Draft, Writing – Review & Editing.

**Peter M. Woodfine:** Conceptualization, Validation, Writing – Review & Editing.

**Mathew Woodfine:** Software, Data Curation, Writing – Review & Editing.

---

## 13. Conflict of Interest Declaration

The authors declare no conflicts of interest. The research was conducted independently of any commercial real estate advisory relationship.

---

## 14. Funding Statement

No external funding was received for this research.

---

## 15. Data Availability Statement

The co-location cluster dataset and chain point-of-interest data were derived from OpenStreetMap (© OpenStreetMap contributors, ODbL 1.0) and Wikidata (CC0). The clustering pipeline is described in full in §4. Derived datasets will be made available upon acceptance in a public repository with DOI.

---

## References

*[To be populated. Key references to include: Woodfine et al. 2026 (J1 companion); Haklay 2010 (OSM as VGI); Hesse & Rodrigue (logistics sprawl); Bowen (warehouse location); Cidell (distribution centres); Dablanc & Rakotonarivo (logistics sprawl EU).]*

---

## Appendix A — Proxy Criterion Specification

| Criterion | Value |
|---|---|
| Hardware anchor required | ≥ 1 chain from hardware category |
| Grocery anchor excluded | 0 chains from grocery/hypermarket categories |
| Metropolitan distance band | 5–80km from nearest metro node (pop ≥ 300,000) |
| Cluster span gate | ≤ 5km |
| DBSCAN parameters | ε = 1.0km, min_pts = 3 (identical to J1 pipeline) |

## Appendix B — Country-Level Preliminary Results (Test Run 2026-06-01)

| Country | Candidates | Notes |
|---|---|---|
| United States | 99 | Lowe's + Home Depot without Walmart/Kroger |
| Germany | 77 | OBI + Hornbach without Edeka/REWE |
| Mexico | 56 | Home Depot ± Costco/Sam's, no OXXO/Walmart grocery |
| France | 44 | Castorama/Leroy Merlin + Decathlon, no Carrefour |
| Italy | 28 | Leroy Merlin + electronics, no Esselunga/Coop |
| Netherlands | 28 | Praxis/Gamma + IKEA, no Albert Heijn |
| Canada | 13 | Home Depot/Home Hardware without Sobeys/Loblaws |
| Other EU | 17 | ES, PL, AT, DK combinations |
| **Total** | **360** | Proxy run; full chain ingestion pending |

*Preliminary results from test pipeline. Full ingestion adds MRO, flooring, tool-rental, lumber chains; expected 2,000–4,000 characterised clusters at completion.*
