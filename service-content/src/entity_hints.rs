// SPDX-License-Identifier: LicenseRef-PointSav-Proprietary

//! KoGNER-style entity-hint cache.
//!
//! A handful of known entity names per classification, fetched once from the
//! graph at drain startup and passed to GLiNER as concrete examples appended
//! to its label descriptions. Free quality improvement — no model change, no
//! per-request graph query. Unblocked by `ontology/entity_types.csv` landing
//! (2026-06-30): hints are keyed by the same classification labels the CSV
//! defines, whatever they happen to be at runtime.

use crate::graph::GraphStore;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

static ENTITY_HINTS: OnceLock<HashMap<String, Vec<String>>> = OnceLock::new();

/// Max example names cached per classification.
const HINTS_PER_LABEL: usize = 3;

/// Populate the entity-hint cache from the current graph contents. Call once
/// at startup, after the taxonomy load (so seed taxonomy entities are
/// eligible as hints too) and before the drain loop begins. Best-effort: a
/// freshly provisioned or empty graph yields no hints, which downstream is a
/// no-op (GLiNER falls back to its plain, hint-free label descriptions), not
/// an error.
pub fn init_entity_hints(graph_store: &Arc<dyn GraphStore>, module_id: &str) {
    let entities = match graph_store.list_entities(module_id) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("[KOGNER] list_entities failed: {e}; starting with no entity hints");
            let _ = ENTITY_HINTS.set(HashMap::new());
            return;
        }
    };

    let mut hints: HashMap<String, Vec<String>> = HashMap::new();
    for entity in entities {
        let bucket = hints.entry(entity.classification.clone()).or_default();
        if bucket.len() < HINTS_PER_LABEL && !bucket.contains(&entity.entity_name) {
            bucket.push(entity.entity_name);
        }
    }
    let total: usize = hints.values().map(|v| v.len()).sum();
    println!(
        "[KOGNER] entity hints seeded: {total} example(s) across {} label(s)",
        hints.len()
    );
    let _ = ENTITY_HINTS.set(hints);
}

/// Current entity-hint cache, if initialised. `None` only before
/// `init_entity_hints` has run — during normal drain operation this is
/// always `Some`, even if the inner map is empty.
pub fn get_entity_hints() -> Option<&'static HashMap<String, Vec<String>>> {
    ENTITY_HINTS.get()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{GraphEntity, LbugGraphStore};

    fn tmp_store() -> Arc<dyn GraphStore> {
        let dir = std::env::temp_dir().join(format!(
            "sc-entity-hints-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let store = LbugGraphStore::new(dir.to_str().unwrap()).expect("open temp lbug store");
        store.init_schema().expect("init_schema");
        Arc::new(store)
    }

    #[test]
    fn hints_bucket_by_classification_capped_at_three() {
        let store = tmp_store();
        let names = [
            "Jennifer Woodfine",
            "Peter Woodfine",
            "Mathew",
            "A Fourth Person",
        ];
        let entities: Vec<GraphEntity> = names
            .iter()
            .map(|n| GraphEntity {
                entity_name: n.to_string(),
                classification: "Person".to_string(),
                role_vector: None,
                location_vector: None,
                contact_vector: None,
                module_id: "test".to_string(),
                confidence: 0.95,
                source_doc: None,
            })
            .collect();
        store.upsert_entities("test", &entities).expect("upsert");

        init_entity_hints(&store, "test");
        let hints = get_entity_hints().expect("hints initialised");
        let person_hints = hints.get("Person").expect("Person bucket present");
        assert_eq!(person_hints.len(), HINTS_PER_LABEL);
    }
}
