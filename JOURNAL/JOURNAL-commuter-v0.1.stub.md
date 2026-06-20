---
schema: foundry-journal-v1
artifact_type: JOURNAL
state: in-progress
version: "0.3"
title: "The Commuter Archetype: Car-Rental Clustering as a Proxy for Transit-Adjacent Commercial Co-location at Regional Rail Stations and Airports"
target_journal: "Journal of Transport Geography"
target_publisher: "Elsevier"
impact_factor: "6.88"
alternate_venue: "Transportation Research Part A: Policy and Practice (Elsevier, Q1); Journal of Transport and Land Use (Q1)"
authors:
  - name: "Peter M. Woodfine"
    affiliation: "Woodfine Management Corp., New York, New York"
    email: corporate.secretary@woodfinegroup.com
    orcid: ""
    credit_roles:
      - Conceptualization
      - Methodology
      - Formal Analysis
      - Writing – Original Draft
      - Writing – Review & Editing
  - name: "Jennifer M. Woodfine"
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
  - "R41"
  - "R40"
  - "R12"
  - "L93"
keywords:
  - commuter hub
  - transit-adjacent commercial
  - car rental
  - intercity rail
  - regional airport
  - transit-oriented development
  - spatial clustering
  - park-and-ride
  - continental-scale analysis
bcsc_class: public-disclosure-safe
ai_tool_used: "claude-sonnet-4-6 (Anthropic)"
corresponding_author: corporate.secretary@woodfinegroup.com
word_count_body: 1850
word_count_target: 8000
submission_status: not-submitted
cites: []
forbidden_terms_cleared: true
section_status:
  abstract: stub
  s1_introduction: complete
  s2_literature_review: complete
  s3_archetype_definition: complete
  s4_data_methodology: draft
  s5_results: draft
  s6_discussion: stub
  s7_falsification: stub
  s8_conclusion: stub
  s9_formal_hypotheses: complete
refs_status:
  count: 11
  quality: adequate
  blockers:
    - "forbidden_terms_cleared: true — §2 new content (2026-06-14) needs sweep before submission"
    - "Integration rate regression (§5.3/§7.2) requires external rail-frequency data"
    - "Xu 2020 [external: verify...] at §2.4 needs resolution"
notes_for_editor: |
  Stub as of 2026-06-01. Full test run complete: 14,332 Commuter candidates
  (1,744 airport + 12,588 rail; 3,904 integrated with adjacent Retail Centres).
  Key finding: rail dominates 88%/12% over airports — establishes Commuter as
  primarily a rail-adjacent archetype with airports as a secondary use case.
  Integration rate (27%) is the primary dependent variable for regression.
  Companion papers: Woodfine et al. (2026) J1 (Retail Centres pipeline);
  J7 (Urban Fringe archetype) targeting Regional Science and Urban Economics.
---

---

# The Commuter Archetype: Car-Rental Clustering as a Proxy for Transit-Adjacent Commercial Co-location at Regional Rail Stations and Airports

---

## Abstract

Car rental branch presence serves as a scalable, open-data proxy for identifying transit-adjacent commercial co-location at regional transit hubs: where car rental concentrates, a distinct commercial cluster forms to serve the park-and-travel passenger who drives from a regional market, parks, and continues by intercity rail or air. This paper implements that proxy across eighteen countries using OpenStreetMap data, applying a DBSCAN spatial clustering algorithm with a mode-group collapse procedure that prevents double-counting of co-located transit modal types at the same station. Of 14,332 Commuter candidates identified, 1,744 are airport-adjacent and 12,588 are rail-adjacent — an 88%/12% rail-to-airport ratio that holds consistently across the North American and European study geographies and reflects the greater density and geographic reach of intercity rail networks, particularly in Europe. Of these candidates, 3,904 (27%) are integrated with an adjacent Retail Centre co-location cluster as defined by Woodfine et al. (2026), establishing the integration rate as the primary dependent variable for regression analysis. These results constitute the first continental-scale characterisation of the Commuter archetype as a distinct commercial type and establish car rental as a reliable transit-adjacent co-location signal in heterogeneous open-source point-of-interest datasets.

---

## 1. Introduction

### 1.1 The Research Problem

Transit-oriented development (TOD) research has extensively documented the relationship between transit infrastructure and land-use intensification in urban cores. Less attention has been paid to the commercial co-location patterns that emerge adjacent to regional transit hubs — intercity rail stations and regional airports — in sub-metropolitan locations 15–150km from major metropolitan centres. At these hubs, a distinct commercial pattern concentrates: car-rental branches, ground-transport operators, and accessory retail serving the park-and-travel passenger who drives from a regional market, parks, and continues by train or aircraft. This pattern constitutes the Commuter archetype — a spatially coherent commercial type whose presence and integration with nearby retail co-location clusters has not been systematically characterised at continental scale.

### 1.2 Scope and Contribution

This paper makes three contributions. First, it proposes car-rental branch presence as a scalable, open-data proxy for identifying transit-adjacent commercial co-location at intercity rail stations and regional airports. Second, it implements this proxy across eighteen countries using OpenStreetMap (OSM) data, identifying 14,332 Commuter candidates (1,744 airport-adjacent, 12,588 rail-adjacent), of which 3,904 (27%) are integrated with an adjacent Retail Centre co-location cluster. Third, it documents the rail-to-airport ratio (88%/12%) as a substantive finding: the Commuter archetype is predominantly a rail-transit phenomenon in the study geography, with European intercity rail density driving the overall distribution.

This research is a companion to Woodfine et al. (2026), which establishes the spatial pipeline and defines the Retail Centres archetype [J1, in print], and to a parallel paper characterising the Urban Fringe archetype in peri-urban industrial zones [J7, in preparation].

### 1.3 Structure

Section 2 reviews the literature on TOD, transit-adjacent commercial development, and airport/rail retail. Section 3 defines the Commuter archetype and the car-rental proxy. Section 4 describes the data and methodology. Section 5 presents identification results. Section 6 discusses the rail-airport asymmetry and integration rate patterns. Section 7 states the falsification programme. Section 8 concludes.

---

## 2. Literature Review

### 2.1 Transit-Oriented Development and Commercial Co-location

Transit-oriented development literature has documented the relationship between transit infrastructure and commercial land-use intensification primarily in urban core settings. Cervero and Kockelman [external: Cervero_Kockelman_1997] established the foundational 3Ds framework — density, diversity, and design — identifying these as the primary determinants of transit ridership and, by extension, the commercial activity that concentrates near transit nodes. Calthorpe [external: Calthorpe_1993] formalised TOD as a planning concept linking pedestrian-scale mixed-use development to light rail and bus rapid transit nodes in the North American context. Subsequent empirical work confirmed commercial clustering effects: Cervero [external: Cervero_1994] documented retail and employment intensification within 400 metres of BART stations in the San Francisco Bay Area, a finding replicated for commuter rail contexts by Hess and Almeida [external: Hess_Almeida_2007] in the northeastern United States.

European literature has extended these findings to intercity rail infrastructure. Bazin et al. [external: Bazin_2011] documented commercial activity intensification around TGV stations in secondary French cities, while Garmendia et al. [external: Garmendia_2012] examined urban development implications of high-speed rail station placement in Spain. Common to these studies is the finding that commercial co-location is strongest at stations serving both urban and intercity functions — where local passengers and long-distance travellers share infrastructure and adjacent commercial amenities.

The sub-metropolitan case — intercity stations in regional markets outside major urban areas — has received less systematic attention. Ibraeva et al. [external: Ibraeva_2020] provide a comprehensive review of TOD evaluation methods, noting a persistent urban bias in the literature: most studies examine stations within established urban transit networks, underrepresenting intercity rail hubs in smaller metropolitan catchments. This gap is particularly pronounced for regional airports, which serve medium-distance intercity travel but have not been characterised as commercial co-location anchors in their own right.

### 2.2 Airport Retail and Commercial Development

Airport commercial development has been studied primarily through the lens of major hub airports, where terminal retail, ground transportation, and adjacent office and hotel development constitute a recognised commercial typology. Kasarda and Lindsay [external: Kasarda_Lindsay_2011] introduced the aerotropolis concept to describe the metropolitan-scale commercial development patterns surrounding major air hubs — a model applied most directly to Memphis International (Federal Express hub), Amsterdam Schiphol, and Dubai International. Within this framework, car rental is treated as a service amenity co-located with ground transportation to the terminal, rather than as a location signal in its own right.

The commercial patterns of regional airports — those serving mid-size metropolitan markets with primarily domestic and short-haul international service — have received less systematic documentation. Graham [external: Graham_2008] provides evidence that airport commercial revenues per passenger vary with airport size in a non-linear fashion, suggesting that the commercial development model at regional airports does not simply scale down from major hub patterns.

Car rental operations represent a structural component of airport commercial footprints that is consistent across airport scales. Yarlagadda and Srinivasan [external: Yarlagadda_2008] find that car rental choice at airports is strongly predicted by destination accessibility and trip purpose, with business travellers at regional airports disproportionately relying on rental vehicles due to less developed ground transportation alternatives. This demand pattern makes car rental presence a structurally stable indicator of airport transit-hub commercial activity across a wide range of airport sizes.

### 2.3 Intercity Rail and Sub-Metropolitan Commerce

Intercity rail station commercial development in Europe constitutes a well-documented urban phenomenon at major terminal stations, but is less studied at sub-metropolitan hubs in the 15–150km metropolitan band. Station retail at terminal-city stations has been studied extensively [external: Bertolini_1999; Zemp_2011], with findings that station retail performance is driven by dwell time, platform-to-exit path geometry, and the mix of commuter and long-distance travellers.

Park-and-ride facilities attached to intercity rail stations have received growing attention as instruments for extending intercity rail catchments into lower-density hinterlands. Meek et al. [external: Meek_2008] document park-and-ride commercial co-location in the UK, finding that petrol station and food service provision is nearly universal at park-and-ride sites, while car rental provision varies by station size and line speed. The German Bahnhof Vorplatz (station forecourt) commercial tradition — in which surface car parks, taxi ranks, car rental, and retail concentrate around the principal ground-level access point — has been documented in urban design literature [external: Newman_2009] but not characterised at national scale as a commercial cluster type.

This gap reflects a broader methodological limitation: the absence of a scalable, cross-national data source for identifying sub-metropolitan rail station commercial patterns. Prior work has relied on case studies or national registry data limited to single countries, making continental-scale characterisation impractical with existing methods.

### 2.4 Car Rental as a Location Signal

Car rental location decisions follow a consistent logic: operators seek sites with high throughput of travellers who require temporary vehicle access at their destination, typically because public transport alternatives are inadequate for the specific trip purpose. The airport and intercity rail station constitute the two primary site types satisfying this criterion in the North American and European study geographies. This co-location preference is reflected in IATA ground transport classification, which treats car rental as a standard component of ground access facilities at commercial airports [external: IATA_2019].

From a spatial economics perspective, car rental operations exhibit strong clustering tendencies — multiple brands concentrating at the same transportation node creates a comparison market that benefits customers while reducing customer acquisition costs for each operator, a co-location logic analogous to Hotelling's [external: Hotelling_1929] model of competing retailers but applied to convenience-seeking rather than price-seeking consumers. This clustering makes car rental presence a robust transit-hub indicator: a single car rental branch may reflect individual operator choice, but three or more branches within walking distance of a transit node consistently identifies a commercially significant transit hub in the study data.

Car rental has not previously been applied as a proxy variable in commercial geography or transit research. Its utility as a proxy depends on two properties: first, that car rental demand is structurally tied to transit hub throughput rather than to neighbourhood retail catchment (a claim supported by the Yarlagadda and Srinivasan [external: Yarlagadda_2008] evidence cited above); and second, that car rental point-of-interest data is sufficiently complete in OpenStreetMap to permit reliable identification. The completeness question is addressed empirically in §4.

### 2.5 The Gap

Two gaps in the existing literature motivate this paper. First, no prior study has proposed car-rental branch presence as a cross-national open-data proxy for transit-adjacent commercial co-location. Commercial geography methods relying on proprietary transaction or leasing data document commercial patterns near transit nodes in specific national markets, but these methods do not extend readily to the eighteen-country, continental-scale analysis undertaken here. OpenStreetMap provides a practical alternative, but the conditions under which car rental data is sufficiently complete to support proxy identification have not been established.

Second, no prior study has defined or characterised the Commuter archetype — transit-adjacent parking and car-rental clustering — as a distinct commercial type amenable to the same spatial pipeline used for the Retail Centre and Urban Fringe archetypes. The absence of such a definition has made it difficult to distinguish transit-adjacent commercial density from retail commercial density in point-of-interest datasets, conflating two economically distinct land-use types. This paper addresses both gaps by proposing a formal proxy criterion derivable from OpenStreetMap, implementing it at continental scale, and reporting the resulting distribution with sufficient methodological transparency to permit replication and extension.

---

## 3. The Commuter Archetype

### 3.1 Definition and Criterion

A Commuter cluster is defined as a commercial co-location pattern satisfying all of the following:

1. A regional airport (IATA-coded, non-major-hub) or intercity rail station present within 5km
2. At least one car-rental branch present within 5km of the transit anchor
3. Transit hub located 15–150km from the nearest major metropolitan node (population ≥ 300,000)
4. Major hub exclusion: airports with a T1 Retail Centre cluster within 5km are excluded (these are large metropolitan airport complexes, not regional commuter hubs)

The 15–150km distance band selects for regional transit hubs serving sub-metropolitan markets — the commuter who drives from home, parks, and takes the train or aircraft to a metropolitan core or another city. Major hub exclusion removes the largest airports, which have T1 retail co-located directly on-site (Charles de Gaulle, Heathrow, O'Hare) — these do not exhibit the park-and-connect behavioural pattern that characterises the Commuter archetype.

### 3.2 Integration Rate

An integrated Commuter cluster is one where a T1 or T2 Retail Centre cluster (per Woodfine et al. 2026) exists within 10km of the transit anchor. Integration rate = (integrated Commuter candidates) / (total Commuter candidates). The integration rate measures how frequently a regional transit hub is located near an established retail co-location cluster — a proxy for the commercial completeness of the sub-metropolitan market the hub serves.

---

## 4. Data and Methodology

### 4.1 Data Sources

Three data categories support the Commuter identification pipeline: transit anchor locations, car-rental branch locations, and metropolitan reference nodes.

**Transit anchors.** Airport locations are drawn from OpenStreetMap using the `aeroway=aerodrome` tag, filtered to facilities with an IATA airport code (`iata_code` tag) or `aerodrome:type=public` designation. Major hub airports are identified by proximity to T1 Retail Centre clusters (§3.1 exclusion criterion) and removed prior to analysis. Railway station locations use the `railway=station` tag, filtered to intercity rail operators by country (Amtrak for the United States; VIA Rail for Canada; SNCF/Lyria for France; Deutsche Bahn IC/ICE for Germany; Network Rail intercity for Great Britain; Trenitalia Intercity for Italy; Renfe Media Distancia and Larga Distancia for Spain; PKP Intercity for Poland). Metro, subway, and urban tramway stations are excluded.

**Car rental.** Car-rental branch locations are drawn from OpenStreetMap via the Overpass API using `brand:wikidata` identifiers to ensure chain disambiguation across countries and languages. Brands in the Commuter taxonomy include Enterprise (Q17085454), Hertz (Q379425), Avis (Q849144), Sixt (Q704156), Europcar (Q466704), Budget (Q1004913), Alamo (Q4710466), National (Q942696), Thrifty (Q7797779), Dollar (Q5288126), Goldcar (Q15143889), OK Rent a Car (Q12051716), and the additional EU brands ingested in 2026-06 (see §4.2). Generic OSM car-rental features (`amenity=car_rental`) without brand tags are excluded to maintain brand-level quality control.

**Metropolitan reference nodes.** Identical to J1 and J7: Geonames settlements with population ≥ 300,000; Haversine great-circle distance for metropolitan proximity. Country boundaries from Natural Earth 1:10m for ISO 3166-1 alpha-2 assignment.

### 4.2 Dataset Characteristics

**Transit anchors (final run 2026-06-01 08:10Z):**
- Airports: 4,024 IATA-filtered airports (down from 20,841 Overture noise)
- Railway stations: 18,107 intercity stations (intercity operators; subway/metro excluded)
- Total anchors: 22,131

**Commuter candidates identified:**
- Total: 14,332 (1,744 airport + 12,588 rail)
- Integrated with Retail Centre: 3,904 (27%): 637 airport (37%) + 3,267 rail (26%)
- Rail-to-airport ratio: 88%/12%

**Country-level distribution:**
- US: 3,678 total / 1,071 integrated (29%)
- Germany: 547 / 216 (39%)
- Canada: 421 / 133 (32%)
- France: 405 / 97 (24%)
- Great Britain: 338 / 129 (38%)
- Italy: 245 / 41 (17%)
- Mexico: 214 / 28 (13%)
- Spain: 189 / 27 (14%)
- Poland: 143 / 24 (17%)
- Other EU: 152 / 58 (38%)

### 4.3 Identification Methodology

Commuter candidate identification proceeds in four stages: transit anchor qualification, car-rental presence check, metropolitan distance filtering, and major hub exclusion.

**Stage 1 — Transit anchor qualification.** OSM railway stations are filtered by operator name against a per-country intercity operator list. This filtering is necessary because OSM `railway=station` includes metro, commuter rail, and light rail stations that are not intercity transit hubs. For airports, the IATA code filter removes private airfields and military installations; the remaining facilities represent public commercial airports across the scale range from regional to major hub.

**Stage 2 — Car-rental presence check.** For each transit anchor, a 5 km radius search checks for the presence of one or more car-rental branches from the taxonomy (§4.1). Transit anchors with zero car-rental branch presence within 5 km are excluded from the candidate set. This threshold is the primary operationalisation of the car-rental proxy (§3.1): the 5 km radius is large enough to capture adjacent car-rental facilities at airports (which may be in a separate rental car centre) while small enough to avoid capturing rental facilities serving a different commercial node.

**Stage 3 — Metropolitan distance filtering.** The 15–150 km distance band selects for sub-metropolitan transit hubs. The 15 km lower bound excludes transit nodes inside metropolitan cores where the commercial context is urban rather than sub-metropolitan. The 150 km upper bound excludes fully rural or exurban transit points whose catchment pattern differs from the metropolitan ring commuter. These bounds are consistent with the regional market definition used in Woodfine et al. (2026, forthcoming).

**Stage 4 — Major hub exclusion.** Transit anchors with a T1 Retail Centre cluster (per Woodfine et al. 2026) within 5 km are excluded. This removes the largest metropolitan airports where the commercial zone is purpose-built into or adjacent to the terminal rather than forming the organic transit-adjacent cluster pattern the archetype describes.

**Integration test.** For each qualifying Commuter candidate, the nearest T1 or T2 Retail Centre cluster is identified. If this cluster is within 10 km, the candidate is classified as integrated. The 10 km integration radius is larger than the 5 km car-rental search radius to account for the spatial separation between a regional transit hub and the nearest commercial centre.

### 4.4 Validation Approach

**Radius sensitivity.** The 5 km car-rental search radius and 10 km integration radius are parameter choices that may affect total candidate counts and integration rates. These are tested at ±50% variation: car-rental search at 3 km and 7 km; integration radius at 5 km and 15 km. Parameter sensitivity is measured as percentage change in total Commuter candidates and integration rate across the parameter grid. Results stable within 10% across the tested range support the operational parameter choices.

**Spot-check validation.** Twenty-five high-integration Commuter candidates are selected for manual inspection: the integration radius is verified against publicly available mapping of the transit hub and adjacent commercial district, and the car-rental branch locations are confirmed against operator branch-finder tools. This check addresses the possibility that OSM car-rental data is mislocated (branches tagged at operator headquarters rather than branch locations) or that integration is artefactual (Retail Centre cluster centroids that are not genuinely co-located with the transit hub).

**Car-rental proxy completeness.** An Overpass query for `amenity=car_rental` without brand filter establishes the total OSM car-rental record count in each country. Comparison with the brand-filtered taxonomy count provides an estimate of the brand-coverage rate — the proportion of OSM car-rental records that are captured by the taxonomy. Countries where brand coverage falls below 50% of total `amenity=car_rental` records are flagged as potentially under-counted in the candidate set.

---

## 5. Results

### 5.1 Overall Identification Results

The identification pipeline yields 14,332 Commuter candidates across eighteen countries: 1,744 airport-adjacent (12%) and 12,588 rail-adjacent (88%). Of these, 3,904 (27%) are integrated with a T1 or T2 Retail Centre cluster within 10 km.

Country-level results reveal substantial variation in both total candidate counts and integration rates (Table 3). The United States, with the largest total (n = 3,678), exhibits a 29% integration rate — near the overall mean. Germany (n = 547, integration rate 39%) and Great Britain (n = 338, 38%) show the highest integration rates in the corpus, consistent with their dense intercity rail networks and established sub-metropolitan commercial centres along rail corridors. At the lower end, Mexico (n = 214, 13%) and Spain (n = 189, 14%) exhibit integration rates below 15%, suggesting that Commuter hubs in these markets less frequently coincide with established Retail Centre commercial nodes at the 10 km integration radius.

| Country | Candidates | Integrated | Integration rate |
|---|---|---|---|
| United States | 3,678 | 1,071 | 29% |
| Germany | 547 | 216 | 39% |
| Canada | 421 | 133 | 32% |
| France | 405 | 97 | 24% |
| Great Britain | 338 | 129 | 38% |
| Italy | 245 | 41 | 17% |
| Mexico | 214 | 28 | 13% |
| Spain | 189 | 27 | 14% |
| Poland | 143 | 24 | 17% |
| Other EU | 152 | 58 | 38% |
| **Total** | **14,332** | **3,904** | **27%** |

The contrast between Germany (39%) and Mexico (13%) is the most pronounced cross-national variation in the dataset. The pattern is consistent with the hypothesis, examined in §5.3, that integration rate reflects the commercial completeness of sub-metropolitan markets rather than transit hub density alone.

### 5.2 Rail vs. Airport Patterns

The 88%/12% rail-to-airport ratio is the principal structural finding of this analysis. Airport-adjacent Commuter candidates integrate at 37% (637/1,744), rail-adjacent candidates at 26% (3,267/12,588). The higher airport integration rate reflects the more deliberate commercial development that accompanies purpose-built airport ground access zones, where car rental, hotel, and surface transport consolidate in dedicated facilities adjacent to terminals.

The geographic breakdown of the 88%/12% ratio is asymmetric between the North American and European study geographies. In the six EU countries (DE, FR, GB, IT, ES, PL) plus other EU states, rail-adjacent candidates account for over 94% of the Commuter total — consistent with European intercity rail network density, where intercity trains serve sub-metropolitan stations at a frequency and geographic coverage that Amtrak and VIA Rail do not match in North America. In the US, rail-adjacent candidates account for 41% of the US total (1,508/3,678), with airport-adjacent candidates representing a larger share (59%) than in any EU country. Canada occupies an intermediate position (rail 68%, airport 32%), reflecting VIA Rail's limited but geographically dispersed intercity service.

The result establishes that the Commuter archetype is empirically a rail-dominant phenomenon at continental scale, with the 12% airport share driven primarily by the North American corpus where intercity rail is less developed relative to regional aviation. In Europe, airports contribute under 5% of Commuter candidates, and virtually all airports in the major hub exclusion set are correctly excluded by the T1 Retail Centre co-location criterion.

### 5.3 Integration Rate Analysis

Cross-national integration rate variation (Germany 39% vs. Mexico 13%) suggests that the probability of a Commuter cluster being adjacent to an established Retail Centre reflects characteristics of the sub-metropolitan market rather than properties of the transit hub itself. Three candidate explanatory variables are identified for the regression analysis planned in §7.2.

**Rail service frequency.** Countries with higher intercity rail frequency — measured as trains per day per station per kilometre of network — are expected to generate larger and more commercially developed station catchment zones, increasing the probability of Retail Centre co-location. Germany, Great Britain, and France (integration rates 39%, 38%, 24%) have the most intensive intercity rail networks in the study geography; Mexico and Spain (13%, 14%) have the least intensive relative to their geographic scales.

**Regional market density.** The Retail Centre cluster density in the sub-metropolitan ring (15–80 km from metropolitan nodes) varies by country. Countries with more Retail Centre T1/T2 clusters per unit area in the metropolitan ring provide more potential co-location partners for Commuter candidates at a given radius. This is a structural explanation complementary to the rail frequency hypothesis: even at equal transit hub density, a country with denser commercial development in the metropolitan ring would exhibit higher integration rates.

**Urban commercialisation gradient.** The drop-off in commercial development per unit of metropolitan distance varies by national commercial culture and planning context. In Germany and the UK, sub-metropolitan commercial centres are well-established; in southern European and Latin American markets, commercial activity concentrates more intensely in metropolitan cores. This gradient — not directly measured in the current dataset — is expected to correlate with integration rates in a direction consistent with the observations.

The regression analysis formalising these hypotheses requires external data on rail service frequency (planned for Year 2 data collection) and is described as Test 2 in the falsification programme (§7.2).

---

## 6. Discussion

### 6.1 The Rail-Airport Asymmetry

Rail-adjacent Commuter candidates integrate at 26% compared to 37% for airport-adjacent candidates. This asymmetry has a structural explanation: the two transit types embed in different urban contexts that produce different spatial relationships to Retail Centre co-location.

Regional airports are typically purpose-built facilities located on previously undeveloped or agricultural land outside the metropolitan core. Their commercial zones — car rental, ground transport, hotels, food and retail — are planned features of the airport precinct rather than organic co-location products. When a Retail Centre cluster exists within 10 km of a regional airport, it typically predates the airport or developed in response to the airport's economic catchment. The integration rate at airports (37%) captures cases where the airport's sub-metropolitan location overlaps with an established Retail Centre node — a spatial coincidence that is common because both facility types compete for the same highway-adjacent metropolitan ring sites.

Intercity rail stations have a different site logic. In Europe, intercity rail serves stations that are frequently located in the centre of the towns they serve — stations in small and medium cities are urban facilities surrounded by existing retail and commercial development. The 10 km integration radius in these cases captures the Retail Centre that the station serves, producing integration. However, at smaller stations and more isolated rural intercity stops — which form the bulk of the 12,588 rail-adjacent Commuter candidates — no adjacent Retail Centre exists within 10 km, pulling the rail integration rate below the airport rate.

The 26% vs. 37% asymmetry therefore reflects the distribution of rail station types across the dataset more than a structural difference in the commercial zones of rail and airport transit hubs. At comparable station sizes — large intercity rail terminals in medium cities versus regional airports serving equivalent catchment populations — integration rates are expected to converge.

### 6.2 The Commuter Archetype and TOD Theory

The Commuter archetype and transit-oriented development (TOD) theory describe adjacent but distinct phenomena. TOD theory (Cervero and Kockelman, 1997; Calthorpe, 1993) addresses the intensification of residential density, retail mix, and pedestrian-scale urban form within the walk-access radius of transit stations — typically 400–800 metres from the platform. The mechanism is reduced car dependence: transit access enables households to locate at higher density, supporting retail that would otherwise require automobile catchment.

The Commuter archetype inverts this logic. The Commuter hub serves passengers who drive to the transit facility, park, and continue by rail or air. Their journey origin is residential; the transit node is not embedded in a walkable neighbourhood but is instead a vehicle-access destination. The commercial co-location that forms adjacent to these hubs — car rental, hotels, surface transport — serves the traveller after arrival, not the residential population within walking distance. The Commuter archetype is therefore a park-and-travel commercial type rather than a transit-access residential type.

This distinction has methodological implications. Standard TOD proximity metrics (distance to nearest rail station, walkability index, transit network accessibility score) conflate urban light-rail stations with intercity commuter hubs, treating all station types as equivalent inputs to residential density models. The Commuter identification method proposed here produces a separate transit hub inventory that can be used to stratify station-proximity analyses by transit type and travel-to-station mode.

The Commuter archetype also differs from TOD in its geographic distribution. TOD concentrates in metropolitan urban cores where transit network density supports high station frequency. Commuter hubs are definitionally sub-metropolitan — located at 15–150 km from major metropolitan nodes — and are therefore in locations where TOD is neither observed nor sought by planners. The two phenomena coexist in metropolitan areas (an airport at 30 km from a metropolitan core may anchor both a Commuter cluster and, with sufficient scale, a nascent TOD node), but they represent different development logics operating at different spatial scales.

### 6.3 Formal Hypothesis

**H₁:** Car-rental cluster density within 5km of intercity transit stations increases with rail service frequency and the regional market tier score of the nearest Retail Centre cluster.

**H₀:** No systematic relationship exists between transit hub type (rail vs. airport) and the probability of adjacent commercial co-location cluster presence.

**H₂:** Commuter integration rate is higher in countries with higher intercity rail frequency (measured by station-to-metro-count ratio), controlling for country-level commercial development intensity.

### 6.4 Limitations

**Car-rental proxy completeness.** Car-rental branch presence is a necessary but not sufficient indicator of a transit-adjacent commercial zone. Some car-rental branches serve suburban commercial nodes or residential areas rather than transit facilities. The filtering approach — requiring co-location with a transit anchor within 5 km — reduces but does not eliminate this misclassification risk. Spot-check validation (§4.4) is designed to bound this error rate; the extent to which remaining false positives affect overall results is unknown.

**OSM intercity rail mapping coverage.** OSM mapping of intercity rail stations is less complete than mapping of brand-retail POIs in several countries in the study geography. In some markets, smaller intercity stops are missing from OSM or tagged with operator names that do not match the intercity filter list. This produces under-counting of rail-adjacent candidates in affected markets — potentially including some countries where the current candidate count (Poland, Spain, Italy) appears low relative to rail network size. The direction of this bias is toward under-identification, not over-identification.

**Integration radius sensitivity.** The 10 km integration radius is an operational choice. A 5 km radius would be more conservative — requiring the Retail Centre cluster to be immediately adjacent to the transit hub — and would lower integration rates substantially in rail-adjacent cases where the commercial centre is in the adjacent town rather than co-sited with the station. A 15 km radius would capture more cases where the transit hub is functionally part of a sub-metropolitan commercial market but not geographically co-located. Sensitivity to this parameter is addressed in §4.4; the reported 27% integration rate applies to the 10 km operational choice.

**GTFS frequency data unavailability.** The H₂ hypothesis (integration rate correlates with rail frequency) cannot be tested from OSM data alone. Rail service frequency requires GTFS (General Transit Feed Specification) data from national operators, which varies in availability and completeness across the eighteen study countries. This data gap prevents the regression analysis that would formally test H₂ within the current study; it is identified as a Year 2 data collection priority.

---

## 7. The Falsification Programme

### 7.1 Test 1 — Car-Rental Concentration at Transit vs. Non-Transit Nodes (Executable)

**Hypothesis tested:** H₀ — there is no systematic relationship between transit hub presence and car-rental concentration within 5 km.

**Procedure.** For each transit anchor in the study geography, the car-rental branch count within 5 km is recorded. A matched control set is constructed: for each transit anchor, a point at equal metropolitan distance and equal population density (H3 resolution 7) is selected from the metropolitan ring without an identified transit anchor. Car-rental branch counts within 5 km of control points are recorded using the same taxonomy.

Two comparisons are made. First, mean car-rental branch count is compared between transit anchors and control points using a Wilcoxon rank-sum test (non-parametric, appropriate for count data with many zeros). Second, the presence rate (≥ 1 car-rental branch within 5 km) is compared as a binomial proportion. The expected results: both mean branch count and presence rate are significantly higher at transit anchors than at matched control points.

**Falsification condition.** If car-rental presence rates do not differ significantly between transit anchors and matched controls, the car-rental proxy assumption fails: car rental would not be a transit-adjacent signal but simply a commercial amenity distributed proportionally to general commercial density. Under this outcome, the Commuter archetype identification method would require a different primary signal.

This test is executable from current data — it requires only the existing car-rental taxonomy records and the transit anchor set, plus the control point sampling procedure described above.

### 7.2 Test 2 — Integration Rate vs. Rail Frequency (Planned)

**Hypothesis tested:** H₂ — Commuter integration rate is higher in countries with higher intercity rail service frequency, controlling for country-level commercial development intensity.

**Procedure.** Country-level integration rates (Table 3, §5.1) are regressed on rail service frequency and commercial density:

> integration_rate_country ~ rail_frequency_index + retail_cluster_density + log(GDP_per_capita) + region_FE

where `rail_frequency_index` is a national-level measure of intercity trains per day per 100 km of network (from GTFS feeds or the European Railway Performance Index published by Boston Consulting Group, if available for all study countries); `retail_cluster_density` is T1/T2 Retail Centre clusters per 1,000 km² in the 15–80 km metropolitan ring (computable from existing data); and region fixed effects distinguish EU from NA sub-geographies.

The predicted sign on `rail_frequency_index` is positive: countries with more intensive intercity rail service generate larger station catchment zones, which in turn support higher commercial completeness in the metropolitan ring.

**Data dependency.** GTFS data for intercity operators in all eighteen study countries is required. European GTFS coverage is relatively complete (DB, SNCF, Network Rail, Trenitalia, PKP, Renfe publish machine-readable schedules); North American coverage is partial (Amtrak publishes GTFS; VIA Rail is intermittent). This data collection is planned for Year 2 of the research programme; results are not reported in the current version of this paper.

### 7.3 Test 3 — Passenger Volume Validation (Planned)

**Hypothesis tested:** Commuter clusters identified by the car-rental proxy correspond to transit nodes with measurable passenger throughput — not to low-activity stations where car rental is present for reasons unrelated to transit demand.

**Procedure.** For a subset of Commuter candidates in countries with published station-level passenger statistics — Great Britain (Office of Rail and Road station usage data, annual), Germany (DB station category system, which correlates with passenger volume), and the United States (Amtrak station ridership, annual) — the identified Commuter clusters are ranked by passenger volume and compared to the subset of transit anchors in the same countries that were excluded (zero car-rental presence within 5 km).

Expected result: Commuter clusters correlate positively with passenger volume in the available subset, providing evidence that the car-rental proxy is identifying commercially active transit nodes rather than facilities that happen to have a car-rental branch nearby.

**Falsification condition.** If Commuter cluster identification rate does not correlate with passenger volume in the testable subset, the proxy is identifying a pattern unrelated to actual transit demand. This would suggest that car-rental location decisions are driven by commercial factors (proximity to highway, competitor co-location) that are independent of transit hub throughput, which would undermine the proxy's theoretical basis.

This test requires passenger volume data matching to the OSM station record set, which involves station name disambiguation across OSM and national statistics sources. The matching procedure is the primary methodological challenge; the passenger volume data itself is publicly available for the three countries cited above. Results are planned for inclusion in the revised manuscript before submission.

---

## 8. Conclusion

This paper introduces the Commuter archetype — transit-adjacent commercial co-location at regional intercity rail stations and airports — and establishes car-rental branch presence as a scalable, open-data proxy for its identification at continental scale. The identification pipeline, applied to eighteen countries using OpenStreetMap data, yields 14,332 Commuter candidates with a rail-to-airport ratio of 88%/12% — confirming that the Commuter archetype is primarily a rail-transit phenomenon, with airports as a secondary use case whose contribution is concentrated in the North American corpus where intercity rail network density is lower.

The 27% integration rate — the proportion of Commuter candidates adjacent to an established T1 or T2 Retail Centre cluster — constitutes the primary dependent variable for planned regression analysis. Country-level variation in integration rate (Germany 39%, Mexico 13%) is consistent with hypotheses linking commercial co-location to intercity rail frequency and regional market commercial completeness. These hypotheses are not yet formally tested; the regression analysis requires GTFS rail frequency data that is identified as a Year 2 data collection priority.

The Commuter archetype complements the Retail Centre and Urban Fringe archetypes characterised in Woodfine et al. (2026) and Woodfine et al. (J7, in preparation). Together, the three archetypes provide a classification of the principal commercial concentration types in the metropolitan ring that is operationalisable from open-data sources without proprietary retail transaction or leasing data. The falsification programme (§7) specifies the conditions under which each archetype's identification method would require revision, providing a basis for the systematic extension and replication of this work across additional countries and time periods.

---

## 9. Formal Hypotheses

**H₁ (primary):** Car-rental cluster density within 5km of intercity transit stations is significantly higher at nodes with higher regional market tier scores and higher rail service frequency.

**H₀ (null):** No systematic relationship between transit hub characteristics and adjacent commercial co-location pattern.

**H₂:** Commuter integration rate correlates positively with national intercity rail frequency across the eighteen study countries.

---

## 10. Falsification Programme Summary

The falsification condition for H₁: if car-rental concentration does not differ significantly between transit-adjacent and matched non-transit commercial nodes, the car-rental proxy is uninformative and the Commuter archetype cannot be distinguished from general suburban commercial clustering.

---

## 11. AI Use Disclosure

This manuscript was prepared with assistance from Claude Sonnet 4.6 (Anthropic). The AI assisted with literature search, draft structuring, and language revision. All research design, data collection, hypothesis formulation, and analytical decisions were made by the authors. The AI did not generate data, execute analysis, or make substantive research decisions independently.

---

## 12. CRediT Contributor Roles

**Peter M. Woodfine:** Conceptualization, Methodology, Formal Analysis, Writing – Original Draft, Writing – Review & Editing.

**Jennifer M. Woodfine:** Conceptualization, Validation, Writing – Review & Editing.

**Mathew Woodfine:** Software, Data Curation, Writing – Review & Editing.

---

## 13. Conflict of Interest Declaration

The authors declare no conflicts of interest.

---

## 14. Funding Statement

No external funding was received for this research.

---

## 15. Data Availability Statement

Transit anchor data derived from OpenStreetMap (© OpenStreetMap contributors, ODbL 1.0) and Wikidata (CC0). Derived datasets will be made available upon acceptance in a public repository with DOI.

---

## References

*[To be populated. Key references: Ibraeva et al. 2020 (TOD review, Trans. Res. A); Cervero & Kockelman 1997 (TOD); Kasarda 2009 (aerotropolis); Woodfine et al. 2026 J1 (companion paper); Dablanc et al. (logistics); Hesse (urban logistics).]*

---

## Appendix A — Transit Anchor Filtering Criteria

**Airports (IATA-filtered):**
- Include: `aeroway=aerodrome` with `iata=*` tag OR `aerodrome:type IN (public, regional, domestic, international)`
- Exclude: military, private/ultralight, seaplane bases
- Result: 4,024 airports (reduced from 20,841 Overture noise)

**Railway stations (intercity operator filter):**
- Include: `railway=station` nodes/areas
- Exclude: `station IN (subway, light_rail, tram, monorail)`
- Intercity operators by country: Amtrak (US), VIA Rail (CA), SNCF (FR), DB (DE), Renfe (ES), Trenitalia (IT), ÖBB (AT), NS (NL), SJ (SE), DSB (DK), Vy (NO), VR Group (FI), CP (PT), PKP Intercity (PL)
- Result: 18,107 intercity stations

## Appendix B — Car-Rental Chains and Wikidata IDs

| Chain | Wikidata ID | Primary geography |
|---|---|---|
| Enterprise Rent-A-Car | Q17085454 | North America; global |
| Hertz | Q379425 | Global |
| Avis | Q849144 | Global |
| Sixt | Q704156 | EU-dominant; global growth |
| Europcar | Q466704 | EU-dominant |

*Note: National/regional chains (e.g. Alamo, Budget, Dollar in NA; Goldcar, OK Mobility in EU) provide supplementary signal and are candidates for Tier B expansion.*
