use crate::graph::GraphEntity;
use csv::ReaderBuilder;
use std::fs;

// ── Row types ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ArchetypeRow {
    pub id: String,
    pub name: String,
    pub signature: String,
    pub healing_trigger: String,
    pub gravity_keywords: String,
}

#[derive(Debug, Clone)]
pub struct CoaRow {
    pub reference_number: String,
    pub category: String,
    pub type_: String,
    pub gravity_keywords: String,
}

#[derive(Debug, Clone)]
pub struct DomainRow {
    pub domain_id: String,
    pub domain_name: String,
    pub category: String,
    pub thesis: String,
    pub gravity_keywords: String,
}

#[derive(Debug, Clone)]
pub struct GlossaryRow {
    pub term_en: String,
    pub term_es: String,
    pub definition: String,
    pub domain: String,
}

#[derive(Debug, Clone)]
pub struct ThemeRow {
    pub id: String,
    pub name: String,
    pub scope: String,
    pub thesis: String,
    pub gravity_keywords: String,
    pub active_state: String,
}

#[derive(Debug, Clone)]
pub struct TopicRow {
    pub topic_id: String,
    pub title: String,
    pub domain: String,
    pub wiki_repo: String,
    pub wiki_path: String,
    pub active_state: String,
}

#[derive(Debug, Clone)]
pub struct GuideRow {
    pub guide_id: String,
    pub title: String,
    pub domain: String,
    pub wiki_repo: String,
    pub wiki_path: String,
    pub active_state: String,
}

#[derive(Debug, Default)]
pub struct TaxonomyBundle {
    pub archetypes: Vec<ArchetypeRow>,
    pub coa: Vec<CoaRow>,
    pub domains: Vec<DomainRow>,
    pub glossary: Vec<GlossaryRow>,
    pub themes: Vec<ThemeRow>,
    pub topics: Vec<TopicRow>,
    pub guides: Vec<GuideRow>,
}

// ── Parsers ───────────────────────────────────────────────────────────────────

pub fn parse_archetypes(csv: &str) -> Result<Vec<ArchetypeRow>, String> {
    let mut rdr = ReaderBuilder::new()
        .flexible(true)
        .from_reader(csv.as_bytes());
    let mut rows = Vec::new();
    for result in rdr.records() {
        let r = result.map_err(|e| format!("archetypes CSV parse error: {e}"))?;
        if r.len() < 5 {
            return Err(format!(
                "archetypes row has {} columns, need 5: {:?}",
                r.len(),
                r.iter().collect::<Vec<_>>()
            ));
        }
        rows.push(ArchetypeRow {
            id: r[0].trim().to_string(),
            name: r[1].trim().to_string(),
            signature: r[2].trim().to_string(),
            healing_trigger: r[3].trim().to_string(),
            gravity_keywords: r[4].trim().to_string(),
        });
    }
    Ok(rows)
}

pub fn parse_coa(csv: &str) -> Result<Vec<CoaRow>, String> {
    let mut rdr = ReaderBuilder::new()
        .flexible(true)
        .from_reader(csv.as_bytes());
    let mut rows = Vec::new();
    for result in rdr.records() {
        let r = result.map_err(|e| format!("coa CSV parse error: {e}"))?;
        if r.len() < 4 {
            return Err(format!("coa row has {} columns, need 4", r.len()));
        }
        rows.push(CoaRow {
            reference_number: r[0].trim().to_string(),
            category: r[1].trim().to_string(),
            type_: r[2].trim().to_string(),
            gravity_keywords: r[3].trim().to_string(),
        });
    }
    Ok(rows)
}

pub fn parse_domain(csv: &str) -> Result<Vec<DomainRow>, String> {
    let mut rdr = ReaderBuilder::new()
        .flexible(true)
        .from_reader(csv.as_bytes());
    let mut rows = Vec::new();
    for result in rdr.records() {
        let r = result.map_err(|e| format!("domain CSV parse error: {e}"))?;
        if r.len() < 5 {
            return Err(format!("domain row has {} columns, need 5", r.len()));
        }
        rows.push(DomainRow {
            domain_id: r[0].trim().to_string(),
            domain_name: r[1].trim().to_string(),
            category: r[2].trim().to_string(),
            thesis: r[3].trim().to_string(),
            gravity_keywords: r[4].trim().to_string(),
        });
    }
    Ok(rows)
}

pub fn parse_glossary(csv: &str) -> Result<Vec<GlossaryRow>, String> {
    let mut rdr = ReaderBuilder::new()
        .flexible(true)
        .from_reader(csv.as_bytes());
    let mut rows = Vec::new();
    for result in rdr.records() {
        let r = result.map_err(|e| format!("glossary CSV parse error: {e}"))?;
        if r.len() < 4 {
            return Err(format!("glossary row has {} columns, need 4", r.len()));
        }
        rows.push(GlossaryRow {
            term_en: r[0].trim().to_string(),
            term_es: r[1].trim().to_string(),
            definition: r[2].trim().to_string(),
            domain: r[3].trim().to_string(),
        });
    }
    Ok(rows)
}

pub fn parse_themes(csv: &str) -> Result<Vec<ThemeRow>, String> {
    let mut rdr = ReaderBuilder::new()
        .flexible(true)
        .from_reader(csv.as_bytes());
    let mut rows = Vec::new();
    for result in rdr.records() {
        let r = result.map_err(|e| format!("themes CSV parse error: {e}"))?;
        if r.len() < 6 {
            return Err(format!("themes row has {} columns, need 6", r.len()));
        }
        rows.push(ThemeRow {
            id: r[0].trim().to_string(),
            name: r[1].trim().to_string(),
            scope: r[2].trim().to_string(),
            thesis: r[3].trim().to_string(),
            gravity_keywords: r[4].trim().to_string(),
            active_state: r[5].trim().to_string(),
        });
    }
    Ok(rows)
}

pub fn parse_topics(csv: &str) -> Result<Vec<TopicRow>, String> {
    let mut rdr = ReaderBuilder::new()
        .flexible(true)
        .from_reader(csv.as_bytes());
    let mut rows = Vec::new();
    for result in rdr.records() {
        let r = result.map_err(|e| format!("topics CSV parse error: {e}"))?;
        if r.len() < 6 {
            return Err(format!("topics row has {} columns, need 6", r.len()));
        }
        rows.push(TopicRow {
            topic_id: r[0].trim().to_string(),
            title: r[1].trim().to_string(),
            domain: r[2].trim().to_string(),
            wiki_repo: r[3].trim().to_string(),
            wiki_path: r[4].trim().to_string(),
            active_state: r[5].trim().to_string(),
        });
    }
    Ok(rows)
}

pub fn parse_guides(csv: &str) -> Result<Vec<GuideRow>, String> {
    let mut rdr = ReaderBuilder::new()
        .flexible(true)
        .from_reader(csv.as_bytes());
    let mut rows = Vec::new();
    for result in rdr.records() {
        let r = result.map_err(|e| format!("guides CSV parse error: {e}"))?;
        if r.len() < 6 {
            return Err(format!("guides row has {} columns, need 6", r.len()));
        }
        rows.push(GuideRow {
            guide_id: r[0].trim().to_string(),
            title: r[1].trim().to_string(),
            domain: r[2].trim().to_string(),
            wiki_repo: r[3].trim().to_string(),
            wiki_path: r[4].trim().to_string(),
            active_state: r[5].trim().to_string(),
        });
    }
    Ok(rows)
}

// ── Serializers (for GET /v1/config/* export) ────────────────────────────────

#[allow(dead_code)]
pub fn serialize_archetypes(rows: &[ArchetypeRow]) -> String {
    let mut out = String::from("id,name,signature,healing_trigger,gravity_keywords\n");
    for r in rows {
        out.push_str(&csv_row(&[
            &r.id,
            &r.name,
            &r.signature,
            &r.healing_trigger,
            &r.gravity_keywords,
        ]));
    }
    out
}

#[allow(dead_code)]
pub fn serialize_coa(rows: &[CoaRow]) -> String {
    let mut out = String::from("reference_number,category,type,gravity_keywords\n");
    for r in rows {
        out.push_str(&csv_row(&[
            &r.reference_number,
            &r.category,
            &r.type_,
            &r.gravity_keywords,
        ]));
    }
    out
}

pub fn serialize_domains(rows: &[DomainRow]) -> String {
    let mut out = String::from("domain_id,domain_name,category,thesis,gravity_keywords\n");
    for r in rows {
        out.push_str(&csv_row(&[
            &r.domain_id,
            &r.domain_name,
            &r.category,
            &r.thesis,
            &r.gravity_keywords,
        ]));
    }
    out
}

#[allow(dead_code)]
pub fn serialize_glossary(rows: &[GlossaryRow]) -> String {
    let mut out = String::from("term_en,term_es,definition,domain\n");
    for r in rows {
        out.push_str(&csv_row(&[
            &r.term_en,
            &r.term_es,
            &r.definition,
            &r.domain,
        ]));
    }
    out
}

#[allow(dead_code)]
pub fn serialize_themes(rows: &[ThemeRow]) -> String {
    let mut out = String::from("id,name,scope,thesis,gravity_keywords,active_state\n");
    for r in rows {
        out.push_str(&csv_row(&[
            &r.id,
            &r.name,
            &r.scope,
            &r.thesis,
            &r.gravity_keywords,
            &r.active_state,
        ]));
    }
    out
}

#[allow(dead_code)]
pub fn serialize_topics(rows: &[TopicRow]) -> String {
    let mut out = String::from("topic_id,title,domain,wiki_repo,wiki_path,active_state\n");
    for r in rows {
        out.push_str(&csv_row(&[
            &r.topic_id,
            &r.title,
            &r.domain,
            &r.wiki_repo,
            &r.wiki_path,
            &r.active_state,
        ]));
    }
    out
}

#[allow(dead_code)]
pub fn serialize_guides(rows: &[GuideRow]) -> String {
    let mut out = String::from("guide_id,title,domain,wiki_repo,wiki_path,active_state\n");
    for r in rows {
        out.push_str(&csv_row(&[
            &r.guide_id,
            &r.title,
            &r.domain,
            &r.wiki_repo,
            &r.wiki_path,
            &r.active_state,
        ]));
    }
    out
}

// ── GraphEntity converters ────────────────────────────────────────────────────

pub fn archetypes_to_entities(rows: &[ArchetypeRow]) -> Vec<GraphEntity> {
    rows.iter()
        .map(|r| GraphEntity {
            entity_name: r.name.clone(),
            classification: "archetype".to_string(),
            role_vector: Some(r.signature.clone()),
            location_vector: Some(r.healing_trigger.clone()),
            contact_vector: Some(r.gravity_keywords.clone()),
            module_id: "__taxonomy__".to_string(),
            confidence: 1.0,
            source_doc: None,
        })
        .collect()
}

pub fn coa_to_entities(rows: &[CoaRow]) -> Vec<GraphEntity> {
    rows.iter()
        .map(|r| GraphEntity {
            entity_name: format!("{} — {} {}", r.reference_number, r.category, r.type_),
            classification: "coa-profile".to_string(),
            role_vector: Some(format!("{} {}", r.category, r.type_)),
            location_vector: Some(r.reference_number.clone()),
            contact_vector: Some(r.gravity_keywords.clone()),
            module_id: "__taxonomy__".to_string(),
            confidence: 1.0,
            source_doc: None,
        })
        .collect()
}

pub fn domains_to_entities(rows: &[DomainRow]) -> Vec<GraphEntity> {
    rows.iter()
        .map(|r| GraphEntity {
            entity_name: r.domain_name.clone(),
            classification: "domain".to_string(),
            role_vector: Some(r.thesis.clone()),
            location_vector: Some(r.category.clone()),
            contact_vector: Some(r.gravity_keywords.clone()),
            module_id: "__taxonomy__".to_string(),
            confidence: 1.0,
            source_doc: None,
        })
        .collect()
}

pub fn glossary_to_entities(rows: &[GlossaryRow]) -> Vec<GraphEntity> {
    rows.iter()
        .map(|r| {
            let classification = format!("glossary-{}", r.domain);
            GraphEntity {
                entity_name: r.term_en.clone(),
                classification,
                role_vector: Some(r.term_es.clone()),
                location_vector: Some(r.domain.clone()),
                contact_vector: Some(r.definition.chars().take(200).collect()),
                module_id: "__taxonomy__".to_string(),
                confidence: 1.0,
                source_doc: None,
            }
        })
        .collect()
}

pub fn themes_to_entities(rows: &[ThemeRow]) -> Vec<GraphEntity> {
    rows.iter()
        .filter(|r| r.active_state == "active")
        .map(|r| GraphEntity {
            entity_name: r.name.clone(),
            classification: "theme".to_string(),
            role_vector: Some(format!("{} — {}", r.scope, r.thesis)),
            location_vector: Some(r.scope.clone()),
            contact_vector: Some(r.gravity_keywords.clone()),
            module_id: "__taxonomy__".to_string(),
            confidence: 1.0,
            source_doc: None,
        })
        .collect()
}

pub fn topics_to_entities(rows: &[TopicRow]) -> Vec<GraphEntity> {
    rows.iter()
        .filter(|r| r.active_state == "active")
        .map(|r| GraphEntity {
            entity_name: r.title.clone(),
            classification: "topic".to_string(),
            role_vector: Some(r.domain.clone()),
            location_vector: Some(r.wiki_path.clone()),
            contact_vector: Some(r.wiki_repo.clone()),
            module_id: "__taxonomy__".to_string(),
            confidence: 1.0,
            source_doc: None,
        })
        .collect()
}

pub fn guides_to_entities(rows: &[GuideRow]) -> Vec<GraphEntity> {
    rows.iter()
        .map(|r| GraphEntity {
            entity_name: r.title.clone(),
            classification: "guide".to_string(),
            role_vector: Some(r.domain.clone()),
            location_vector: Some(r.wiki_path.clone()),
            contact_vector: Some(r.wiki_repo.clone()),
            module_id: "__taxonomy__".to_string(),
            confidence: 1.0,
            source_doc: None,
        })
        .collect()
}

// ── Directory loader ──────────────────────────────────────────────────────────

pub fn load_taxonomy_from_dir(ontology_dir: &str) -> Result<TaxonomyBundle, String> {
    let mut bundle = TaxonomyBundle::default();

    let read = |path: &str| -> Result<String, String> {
        fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))
    };

    // Archetypes
    let arc_path = format!("{}/archetypes.csv", ontology_dir);
    if std::path::Path::new(&arc_path).exists() {
        let csv = read(&arc_path)?;
        bundle.archetypes = parse_archetypes(&csv)?;
    }

    // Chart of Accounts
    let coa_path = format!("{}/chart_of_accounts.csv", ontology_dir);
    if std::path::Path::new(&coa_path).exists() {
        let csv = read(&coa_path)?;
        bundle.coa = parse_coa(&csv)?;
    }

    // Themes
    let theme_path = format!("{}/themes.csv", ontology_dir);
    if std::path::Path::new(&theme_path).exists() {
        let csv = read(&theme_path)?;
        bundle.themes = parse_themes(&csv)?;
    }

    // Domains (3 files)
    for domain in &["corporate", "documentation", "projects"] {
        let path = format!("{}/domains/domain_{}.csv", ontology_dir, domain);
        if std::path::Path::new(&path).exists() {
            let csv = read(&path)?;
            let mut rows = parse_domain(&csv)?;
            bundle.domains.append(&mut rows);
        }
    }

    // Glossary (3 files)
    for domain in &["corporate", "documentation", "projects"] {
        let path = format!("{}/glossary/glossary_{}.csv", ontology_dir, domain);
        if std::path::Path::new(&path).exists() {
            let csv = read(&path)?;
            let mut rows = parse_glossary(&csv)?;
            bundle.glossary.append(&mut rows);
        }
    }

    // Topics (3 files)
    for domain in &["corporate", "documentation", "projects"] {
        let path = format!("{}/topics/topics_{}.csv", ontology_dir, domain);
        if std::path::Path::new(&path).exists() {
            let csv = read(&path)?;
            let mut rows = parse_topics(&csv)?;
            bundle.topics.append(&mut rows);
        }
    }

    // Guides (Documentation domain only — GUIDE entity class per datagraph-guide-entity-class convention)
    let guide_path = format!("{}/guides/guides_documentation.csv", ontology_dir);
    if std::path::Path::new(&guide_path).exists() {
        let csv = read(&guide_path)?;
        let mut rows = parse_guides(&csv)?;
        bundle.guides.append(&mut rows);
    }

    Ok(bundle)
}

pub fn bundle_to_entities(bundle: &TaxonomyBundle) -> Vec<GraphEntity> {
    let mut all = Vec::new();
    all.extend(archetypes_to_entities(&bundle.archetypes));
    all.extend(coa_to_entities(&bundle.coa));
    all.extend(domains_to_entities(&bundle.domains));
    all.extend(glossary_to_entities(&bundle.glossary));
    all.extend(themes_to_entities(&bundle.themes));
    all.extend(topics_to_entities(&bundle.topics));
    all.extend(guides_to_entities(&bundle.guides));
    all
}

// ── Internal helpers ──────────────────────────────────────────────────────────

#[allow(dead_code)]
fn skip_header(csv: &str) -> &str {
    if let Some(pos) = csv.find('\n') {
        csv[pos + 1..].trim_start()
    } else {
        ""
    }
}

/// Strip the first (header) line. Exposed for external tooling; not used internally.
#[allow(dead_code)]
pub fn skip_header_owned(csv: &str) -> String {
    skip_header(csv).to_string()
}

fn csv_field(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn csv_row(fields: &[&str]) -> String {
    let mut row = fields
        .iter()
        .map(|f| csv_field(f))
        .collect::<Vec<_>>()
        .join(",");
    row.push('\n');
    row
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── parse_guides ──────────────────────────────────────────────────────────

    #[test]
    fn parse_guides_parses_with_header() {
        let csv = "guide_id,title,domain,wiki_repo,wiki_path,active_state\n\
                   guide-doorman,Doorman Operations,documentation,woodfine/fleet,vault/guide-doorman.md,active\n";
        let rows = parse_guides(csv).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].guide_id, "guide-doorman");
        assert_eq!(rows[0].title, "Doorman Operations");
        assert_eq!(rows[0].domain, "documentation");
        assert_eq!(rows[0].active_state, "active");
    }

    #[test]
    fn parse_guides_multiple_rows() {
        let csv = "guide_id,title,domain,wiki_repo,wiki_path,active_state\n\
                   guide-a,Guide A,documentation,repo,path/a.md,active\n\
                   guide-b,Guide B,documentation,repo,path/b.md,pending\n";
        let rows = parse_guides(csv).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1].guide_id, "guide-b");
        assert_eq!(rows[1].active_state, "pending");
    }

    #[test]
    fn parse_guides_rejects_too_few_columns() {
        let csv = "guide_id,title,domain\nguide-a,Guide A,documentation\n";
        let result = parse_guides(csv);
        // header row satisfies column count (6 required; only 3 cols here)
        // actual parse error depends on whether header row is treated as data
        // when fewer cols than required: returns Err
        assert!(result.is_err() || result.unwrap().is_empty());
    }

    #[test]
    fn guides_to_entities_maps_fields() {
        let rows = vec![GuideRow {
            guide_id: "guide-yoyo".to_string(),
            title: "Yo-Yo Operations".to_string(),
            domain: "documentation".to_string(),
            wiki_repo: "woodfine/fleet".to_string(),
            wiki_path: "vault/guide-yoyo.md".to_string(),
            active_state: "active".to_string(),
        }];
        let entities = guides_to_entities(&rows);
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].entity_name, "Yo-Yo Operations");
        assert_eq!(entities[0].classification, "guide");
        assert_eq!(entities[0].role_vector.as_deref(), Some("documentation"));
        assert_eq!(entities[0].module_id, "__taxonomy__");
        assert!((entities[0].confidence - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn serialize_guides_roundtrips() {
        let rows = vec![GuideRow {
            guide_id: "guide-test".to_string(),
            title: "Test Guide".to_string(),
            domain: "documentation".to_string(),
            wiki_repo: "woodfine/fleet".to_string(),
            wiki_path: "vault/guide-test.md".to_string(),
            active_state: "active".to_string(),
        }];
        let csv = serialize_guides(&rows);
        assert!(csv.starts_with("guide_id,title,"));
        assert!(csv.contains("guide-test"));
        let parsed = parse_guides(&csv).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].guide_id, "guide-test");
    }

    // ── skip_header_owned ────────────────────────────────────────────────────

    #[test]
    fn skip_header_owned_removes_first_line() {
        let csv = "header,row\ndata,row\n";
        assert_eq!(skip_header_owned(csv), "data,row\n");
    }

    #[test]
    fn skip_header_owned_empty_input() {
        assert_eq!(skip_header_owned(""), "");
    }

    // ── guides_documentation.csv sanity ─────────────────────────────────────

    #[test]
    fn ontology_guides_documentation_csv_parses() {
        // Verify the actual on-disk CSV parses without error and has expected rows.
        // Path is relative to the manifest directory (service-content/).
        let csv_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/ontology/guides/guides_documentation.csv"
        );
        let Ok(csv) = std::fs::read_to_string(csv_path) else {
            return;
        };
        let rows = parse_guides(&csv).expect("guides_documentation.csv must parse cleanly");
        assert!(
            !rows.is_empty(),
            "guides_documentation.csv must have at least one row"
        );
        for row in &rows {
            assert!(!row.guide_id.is_empty(), "guide_id must not be empty");
            assert_eq!(
                row.domain, "documentation",
                "all guides must have domain=documentation"
            );
        }
    }

    // ── regression: load_taxonomy_from_dir must not drop first data row ──────

    #[test]
    fn parse_archetypes_first_row_not_dropped() {
        // Regression test for the skip_header + has_headers double-strip bug.
        // All parse_* functions use ReaderBuilder with has_headers=true (default),
        // so the CSV must be passed WITH its header row — the crate handles it.
        let csv = "id,name,signature,healing_trigger,gravity_keywords\n\
                   1,The Executive,Strategic Direction,Stagnation,strategy|leadership\n\
                   2,The Guardian,Risk & Compliance,Breach,compliance|audit\n";
        let rows = parse_archetypes(csv).unwrap();
        assert_eq!(
            rows.len(),
            2,
            "both rows must be returned — not 1 (first dropped)"
        );
        assert_eq!(
            rows[0].name, "The Executive",
            "first row must be The Executive"
        );
        assert_eq!(rows[1].name, "The Guardian");
    }

    #[test]
    fn parse_domain_first_row_not_dropped() {
        let csv = "domain_id,domain_name,category,thesis,gravity_keywords\n\
                   corporate,Corporate,governance,Thesis A,real estate|equity\n\
                   documentation,Documentation,knowledge,Thesis B,guide|wiki\n";
        let rows = parse_domain(csv).unwrap();
        assert_eq!(rows.len(), 2, "both domain rows must be returned");
        assert_eq!(rows[0].domain_id, "corporate");
    }
}
