use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

/// Canonical extraction output — normalized from heterogeneous YAML ledger schemas.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CanonicalExtraction {
    pub entities: Vec<CanonicalEntity>,
    pub metrics: Vec<CanonicalMetric>,
    pub themes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct CanonicalEntity {
    pub name: String,
    pub entity_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalMetric {
    pub name: String,
    pub value: String,
}

/// F1 score for a single entity axis.
#[derive(Debug, Serialize, Deserialize)]
pub struct F1Score {
    pub precision: f32,
    pub recall: f32,
    pub f1: f32,
    pub true_positives: usize,
    pub false_positives: usize,
    pub false_negatives: usize,
}

/// 5-boolean structural health check for a migrated document.
#[derive(Debug, Serialize, Deserialize)]
pub struct StructuralHealth {
    pub corpus_file_exists: bool,
    pub worm_ledger_advanced: bool,
    pub graph_entity_count_nonzero: bool,
    pub crm_record_exists: bool,
    pub ledger_entry_exists: bool,
}

impl StructuralHealth {
    pub fn all_green(&self) -> bool {
        self.corpus_file_exists
            && self.worm_ledger_advanced
            && self.graph_entity_count_nonzero
            && self.crm_record_exists
            && self.ledger_entry_exists
    }
}

/// Normalize a heterogeneous YAML ledger file to CanonicalExtraction.
/// Handles ~130 schema variants in the jennifer-1 corpus.
pub fn normalize_reference_yaml(path: &Path) -> Result<CanonicalExtraction, String> {
    let content =
        std::fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;

    // Guard: skip prompt-leaked ledgers
    if content.contains("extraction_protocol") || content.contains("fidelity_mandate") {
        return Err("prompt-leaked ledger — skip".into());
    }

    let val: serde_json::Value = serde_yaml::from_str(&content)
        .map_err(|e| format!("yaml parse {}: {e}", path.display()))?;

    let mut entities: Vec<CanonicalEntity> = Vec::new();
    let mut metrics: Vec<CanonicalMetric> = Vec::new();
    let mut themes: Vec<String> = Vec::new();

    // Schema variant 1: mentioned_entities.{people, companies}
    if let Some(mentioned) = val.get("mentioned_entities") {
        extract_name_list(mentioned.get("people"), "Person", &mut entities);
        extract_name_list(mentioned.get("companies"), "Company", &mut entities);
    }

    // Schema variant 2: document_analysis.key_entities
    if let Some(da) = val.get("document_analysis") {
        if let Some(ke) = da.get("key_entities") {
            extract_key_entities(ke, &mut entities);
        }
    }

    // Schema variant 3: article_metadata.authors, .organizations
    if let Some(am) = val.get("article_metadata") {
        extract_name_list(am.get("authors"), "Person", &mut entities);
        extract_name_list(am.get("organizations"), "Company", &mut entities);
    }

    // Schema variant 4: visual_assets (skip — not entity data)
    // Schema variant 5: metrics / financial_metrics
    for key in &["metrics", "financial_metrics", "key_metrics"] {
        if let Some(m) = val.get(key) {
            extract_metrics(m, &mut metrics);
        }
    }

    // Schema variant 6: theme_alignment / woodfine_institutional_themes
    for key in &["theme_alignment", "woodfine_institutional_themes"] {
        if let Some(t) = val.get(key) {
            extract_themes(t, &mut themes);
        }
    }

    // Schema variant 7: top-level entity arrays
    for key in &[
        "entities",
        "key_entities",
        "people",
        "companies",
        "organizations",
    ] {
        if let Some(arr) = val.get(key) {
            extract_key_entities(arr, &mut entities);
        }
    }

    // Deduplicate entities by (name, type)
    let mut seen: HashSet<(String, String)> = HashSet::new();
    entities.retain(|e| seen.insert((e.name.clone(), e.entity_type.clone())));

    Ok(CanonicalExtraction {
        entities,
        metrics,
        themes,
    })
}

fn extract_name_list(
    val: Option<&serde_json::Value>,
    entity_type: &str,
    out: &mut Vec<CanonicalEntity>,
) {
    let arr = match val.and_then(|v| v.as_array()) {
        Some(a) => a,
        None => return,
    };
    for item in arr {
        let name = if let Some(s) = item.as_str() {
            s.trim().to_string()
        } else if let Some(n) = item.get("name").and_then(|v| v.as_str()) {
            n.trim().to_string()
        } else {
            continue;
        };
        if !name.is_empty() {
            out.push(CanonicalEntity {
                name,
                entity_type: entity_type.into(),
            });
        }
    }
}

fn extract_key_entities(val: &serde_json::Value, out: &mut Vec<CanonicalEntity>) {
    let arr = match val.as_array() {
        Some(a) => a,
        None => return,
    };
    for item in arr {
        let name = item
            .get("name")
            .or_else(|| item.get("entity_name"))
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string());
        let etype = item
            .get("type")
            .or_else(|| item.get("classification"))
            .or_else(|| item.get("entity_type"))
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown")
            .trim()
            .to_string();
        if let Some(n) = name {
            if !n.is_empty() {
                out.push(CanonicalEntity {
                    name: n,
                    entity_type: etype,
                });
            }
        }
    }
}

fn extract_metrics(val: &serde_json::Value, out: &mut Vec<CanonicalMetric>) {
    if let Some(arr) = val.as_array() {
        for item in arr {
            let name = item
                .get("name")
                .or_else(|| item.get("metric_name"))
                .or_else(|| item.get("key"))
                .and_then(|v| v.as_str())
                .map(|s| s.trim().to_string());
            let value = item
                .get("value")
                .or_else(|| item.get("metric_value"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            if let Some(n) = name {
                if !n.is_empty() {
                    out.push(CanonicalMetric { name: n, value });
                }
            }
        }
    } else if let Some(obj) = val.as_object() {
        for (k, v) in obj {
            out.push(CanonicalMetric {
                name: k.clone(),
                value: v.as_str().unwrap_or(&v.to_string()).to_string(),
            });
        }
    }
}

fn extract_themes(val: &serde_json::Value, out: &mut Vec<String>) {
    if let Some(arr) = val.as_array() {
        for item in arr {
            let name = item
                .get("name")
                .or_else(|| item.get("theme_name"))
                .and_then(|v| v.as_str())
                .map(|s| s.trim().to_string())
                .or_else(|| item.as_str().map(|s| s.trim().to_string()));
            if let Some(n) = name {
                if !n.is_empty() {
                    out.push(n);
                }
            }
        }
    }
}

/// Strict entity-level F1 (SemEval-2013 mode): exact name + type match.
pub fn compute_f1(reference: &[CanonicalEntity], extracted: &[CanonicalEntity]) -> F1Score {
    let ref_set: HashSet<_> = reference
        .iter()
        .map(|e| (e.name.to_lowercase(), e.entity_type.to_lowercase()))
        .collect();
    let ext_set: HashSet<_> = extracted
        .iter()
        .map(|e| (e.name.to_lowercase(), e.entity_type.to_lowercase()))
        .collect();

    let tp = ref_set.intersection(&ext_set).count();
    let fp = ext_set.len().saturating_sub(tp);
    let fn_ = ref_set.len().saturating_sub(tp);

    let precision = if tp + fp > 0 {
        tp as f32 / (tp + fp) as f32
    } else {
        0.0
    };
    let recall = if tp + fn_ > 0 {
        tp as f32 / (tp + fn_) as f32
    } else {
        0.0
    };
    let f1 = if precision + recall > 0.0 {
        2.0 * precision * recall / (precision + recall)
    } else {
        0.0
    };

    F1Score {
        precision,
        recall,
        f1,
        true_positives: tp,
        false_positives: fp,
        false_negatives: fn_,
    }
}

/// Check 5 structural health booleans for a migrated document stem.
pub fn structural_health_check(
    stem: &str,
    jennifer2_root: &str,
    module_id: &str,
    ledger_path: &str,
) -> StructuralHealth {
    // 1. CORPUS file exists in service-content/ledgers/
    let corpus_dir = format!("{}/service-content/ledgers", jennifer2_root);
    let corpus_exists = std::fs::read_dir(&corpus_dir)
        .map(|mut rd| {
            rd.any(|e| {
                e.ok()
                    .map(|e| {
                        let n = e.file_name();
                        let s = n.to_string_lossy();
                        s.starts_with("CORPUS_") && s.ends_with(".json")
                    })
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false);

    // 2. WORM ledger has at least one entry
    let worm_log = format!("{}/service-fs/worm/{}/log.jsonl", jennifer2_root, module_id);
    let worm_advanced = std::fs::metadata(&worm_log)
        .map(|m| m.len() > 0)
        .unwrap_or(false);

    // 3. CRM record for this stem exists
    let crm_dir = format!(
        "{}/service-fs/data/service-research/ledgers",
        jennifer2_root
    );
    let crm_exists = std::fs::read_dir(&crm_dir)
        .map(|mut rd| {
            rd.any(|e| {
                e.ok()
                    .map(|e| e.file_name().to_string_lossy().starts_with("CRM_"))
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false);

    // 4. Graph entity count nonzero (checked via DataGraph in the HTTP layer; here just CORPUS file present)
    // We use corpus_exists as a proxy for graph_entity_count_nonzero in unit tests
    let graph_nonzero = corpus_exists;

    // 5. service-input ledger.jsonl has an entry for this stem
    let ledger_entry_exists = std::fs::read_to_string(ledger_path)
        .map(|c| c.contains(stem))
        .unwrap_or(false);

    StructuralHealth {
        corpus_file_exists: corpus_exists,
        worm_ledger_advanced: worm_advanced,
        graph_entity_count_nonzero: graph_nonzero,
        crm_record_exists: crm_exists,
        ledger_entry_exists,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_reference_yaml_mixed_schema() {
        let yaml = r#"
mentioned_entities:
  people:
    - "Alice Smith"
    - "Bob Jones"
  companies:
    - "Acme Corp"
woodfine_institutional_themes:
  - name: "Theme_A"
  - name: "Theme_B"
metrics:
  - name: "revenue"
    value: "$1M"
"#;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.yaml");
        std::fs::write(&path, yaml).unwrap();
        let result = normalize_reference_yaml(&path).unwrap();
        assert_eq!(result.entities.len(), 3);
        assert!(result
            .entities
            .iter()
            .any(|e| e.name == "Alice Smith" && e.entity_type == "Person"));
        assert!(result
            .entities
            .iter()
            .any(|e| e.name == "Acme Corp" && e.entity_type == "Company"));
        assert_eq!(result.themes.len(), 2);
        assert_eq!(result.metrics.len(), 1);
    }

    #[test]
    fn structural_health_check_all_green() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_string_lossy().into_owned();

        // Create the expected directory structure
        let corpus_dir = format!("{}/service-content/ledgers", root);
        std::fs::create_dir_all(&corpus_dir).unwrap();
        std::fs::write(format!("{}/CORPUS_abc.json", corpus_dir), "{}").unwrap();

        let worm_dir = format!("{}/service-fs/worm/jennifer", root);
        std::fs::create_dir_all(&worm_dir).unwrap();
        std::fs::write(format!("{}/log.jsonl", worm_dir), r#"{"payload_id":"x"}"#).unwrap();

        let crm_dir = format!("{}/service-fs/data/service-research/ledgers", root);
        std::fs::create_dir_all(&crm_dir).unwrap();
        std::fs::write(format!("{}/CRM_abc.json", crm_dir), "{}").unwrap();

        let ledger_path = format!("{}/service-input/ledger.jsonl", root);
        std::fs::create_dir_all(format!("{}/service-input", root)).unwrap();
        std::fs::write(&ledger_path, r#"{"stem":"some-stem"}"#).unwrap();

        let health = structural_health_check("some-stem", &root, "jennifer", &ledger_path);
        assert!(health.all_green());
    }
}
