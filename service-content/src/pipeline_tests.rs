// Pipeline integration test: exercises the full chain in one pass.
// filter chain (noise rejection + coerce_classification) →
// LbugGraphStore::upsert_entities → query_context → count_all
//
// Unit tests in graph.rs and entity_filter.rs verify each stage alone.
// This test verifies they compose correctly — the gap D11 identified.
//
// Pattern follows graph.rs tests: std::env::temp_dir() + LbugGraphStore::new.

use crate::entity_filter::{
    coerce_classification, is_allowed_classification, is_noise_entity_name,
};
use crate::graph::{GraphEntity, GraphStore, LbugGraphStore};

/// Run one entity through the full filter chain used in raw_entities_to_graph:
/// noise rejection → word-count gate → coerce_classification → ALLOWED gate.
/// Returns Some(GraphEntity) if the entity survives, None if rejected.
fn apply_filter(name: &str, classification: &str, module_id: &str) -> Option<GraphEntity> {
    if is_noise_entity_name(name) {
        return None;
    }
    if name.split_whitespace().count() > 8 {
        return None;
    }
    let coerced = coerce_classification(name, classification)?;
    if !is_allowed_classification(&coerced) {
        return None;
    }
    Some(GraphEntity {
        entity_name: name.to_string(),
        classification: coerced,
        role_vector: None,
        location_vector: None,
        contact_vector: None,
        module_id: module_id.to_string(),
        confidence: 0.95,
        source_doc: None,
    })
}

#[test]
fn pipeline_filter_graph_query_round_trip() {
    // Inputs: 3 valid entities + 2 noise + 1 misclassification (coercion).
    // Mirrors what raw_entities_to_graph receives from an OLMo extraction response.
    let raw: &[(&str, &str)] = &[
        ("Jennifer Woodfine", "Person"),         // real — survives
        ("PointSav Digital Systems", "Company"), // real — survives
        ("project-totebox", "Project"),          // real — survives
        ("$SLM_DATA_DIR", "Project"),            // noise: env-var prefix → rejected
        ("ops(slm)", "Project"),                 // noise: parens expression → rejected
        ("Portugal", "Company"),                 // coercion: Company → Location; survives
    ];

    let module_id = "pipeline-test";
    let entities: Vec<GraphEntity> = raw
        .iter()
        .filter_map(|(name, cls)| apply_filter(name, cls, module_id))
        .collect();

    // Filter chain: 4 accepted, 2 noise-rejected.
    assert_eq!(
        entities.len(),
        4,
        "filter must accept 4 and reject 2 noise entities"
    );
    assert!(entities
        .iter()
        .any(|e| e.entity_name == "Jennifer Woodfine" && e.classification == "Person"));
    assert!(entities
        .iter()
        .any(|e| e.entity_name == "PointSav Digital Systems" && e.classification == "Company"));
    assert!(entities
        .iter()
        .any(|e| e.entity_name == "project-totebox" && e.classification == "Project"));
    assert!(
        entities
            .iter()
            .any(|e| e.entity_name == "Portugal" && e.classification == "Location"),
        "Portugal (Company→Location coercion) must be accepted with corrected classification"
    );
    assert!(
        !entities
            .iter()
            .any(|e| e.entity_name.contains("SLM_DATA_DIR")),
        "env var must be rejected"
    );
    assert!(
        !entities.iter().any(|e| e.entity_name == "ops(slm)"),
        "parens expression must be rejected"
    );

    // Graph round-trip: upsert filtered entities to a temp LbugDB, then query back.
    let dir = std::env::temp_dir().join(format!("sc-pipeline-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let store = LbugGraphStore::new(dir.to_str().unwrap()).expect("open temp lbug store");
    store.init_schema().expect("init_schema");

    let upserted = store
        .upsert_entities(module_id, &entities)
        .expect("upsert_entities");
    assert_eq!(
        upserted, 4,
        "upserted count must match filtered entity count"
    );

    // Query by name fragment returns the correct entity.
    let results = store
        .query_context(module_id, "Jennifer", 10)
        .expect("query_context");
    assert!(
        !results.is_empty(),
        "query_context must find 'Jennifer Woodfine'"
    );
    assert_eq!(results[0].entity_name, "Jennifer Woodfine");
    assert_eq!(results[0].classification, "Person");

    // Portugal is queryable and has coerced classification.
    let portugal = store
        .query_context(module_id, "Portugal", 10)
        .expect("query_context Portugal");
    assert!(!portugal.is_empty(), "Portugal must be in graph");
    assert_eq!(
        portugal[0].classification, "Location",
        "Portugal must be Location after coercion"
    );

    // Total entity count across all modules must be 4.
    let total = store.count_all().expect("count_all");
    assert_eq!(total, 4);

    // Alias table is queryable; no aliases expected for 4 distinct entities.
    let aliases = store.count_aliases().expect("count_aliases");
    assert_eq!(
        aliases, 0,
        "no ER aliases expected for 4 clearly distinct entities"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
