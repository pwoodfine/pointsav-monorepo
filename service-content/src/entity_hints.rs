// SPDX-License-Identifier: LicenseRef-PointSav-Proprietary

//! KoGNER-style entity-hint cache.
//!
//! A handful of known entity names per classification, fetched once from the
//! graph at drain startup and passed to GLiNER as concrete examples appended
//! to its label descriptions. Free quality improvement — no model change, no
//! per-request graph query. Unblocked by `ontology/entity_types.csv` landing
//! (2026-06-30): hints are keyed by the same classification labels the CSV
//! defines, whatever they happen to be at runtime.

use crate::entity_filter::is_noise_entity_name;
use crate::graph::{GraphEntity, GraphStore};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

static ENTITY_HINTS: OnceLock<HashMap<String, Vec<String>>> = OnceLock::new();

/// Max example names cached per classification.
const HINTS_PER_LABEL: usize = 3;

/// Reject candidate hint names that would corrupt the GLiNER label description
/// they get appended to, rather than genuinely illustrating the label.
///
/// Found live 2026-07-02: pre-existing graph noise (`entity_name: "Person"` for
/// classification `Person`; hallucination/meta-commentary remnants like
/// "no named person identified", "different location", "candidate locations")
/// was being cached as hint examples, degrading GLiNER extraction to zero
/// entities on text it otherwise extracts correctly. `entity_filter::is_noise_entity_name`
/// doesn't catch this class (it targets code/path artifacts, not generic
/// descriptive phrases), so this adds two targeted checks: reject a name that
/// is literally its own classification label, and reject names that don't
/// start with an uppercase letter (real entity names in this corpus are proper
/// nouns; the noise observed is lowercase-initial common-noun phrasing).
fn is_valid_hint_candidate(name: &str, classification: &str) -> bool {
    let trimmed = name.trim();
    if is_noise_entity_name(trimmed) {
        return false;
    }
    if trimmed.eq_ignore_ascii_case(classification.trim()) {
        return false;
    }
    trimmed.chars().next().is_some_and(|c| c.is_uppercase())
}

/// Pure bucketing/filtering logic, factored out of [`init_entity_hints`] so it
/// can be unit-tested without touching the process-wide [`ENTITY_HINTS`]
/// singleton (a `OnceLock` can only be set once per process, so tests that
/// go through it directly race against each other under the default
/// parallel test runner).
fn build_hints(entities: Vec<GraphEntity>) -> HashMap<String, Vec<String>> {
    let mut hints: HashMap<String, Vec<String>> = HashMap::new();
    for entity in entities {
        if !is_valid_hint_candidate(&entity.entity_name, &entity.classification) {
            continue;
        }
        let bucket = hints.entry(entity.classification.clone()).or_default();
        if bucket.len() < HINTS_PER_LABEL && !bucket.contains(&entity.entity_name) {
            bucket.push(entity.entity_name);
        }
    }
    hints
}

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

    let hints = build_hints(entities);
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

    fn person_entity(name: &str) -> GraphEntity {
        GraphEntity {
            entity_name: name.to_string(),
            classification: "Person".to_string(),
            role_vector: None,
            location_vector: None,
            contact_vector: None,
            module_id: "test".to_string(),
            confidence: 0.95,
            source_doc: None,
        }
    }

    #[test]
    fn hints_bucket_by_classification_capped_at_three() {
        // All 4 names are valid (uppercase-initial, non-noise) so this genuinely
        // exercises the HINTS_PER_LABEL cap rather than incidental filtering.
        let names = [
            "Jennifer Woodfine",
            "Peter Woodfine",
            "Mathew Foster",
            "Alice Chen",
        ];
        let entities: Vec<GraphEntity> = names.iter().map(|n| person_entity(n)).collect();

        let hints = build_hints(entities);
        let person_hints = hints.get("Person").expect("Person bucket present");
        assert_eq!(person_hints.len(), HINTS_PER_LABEL);
    }

    // ── is_valid_hint_candidate ───────────────────────────────────────────────

    #[test]
    fn rejects_name_equal_to_its_own_classification() {
        assert!(!is_valid_hint_candidate("Person", "Person"));
        assert!(!is_valid_hint_candidate("Company", "Company"));
        assert!(!is_valid_hint_candidate("company", "Company")); // case-insensitive
    }

    #[test]
    fn rejects_lowercase_initial_common_noun_phrases() {
        assert!(!is_valid_hint_candidate(
            "no named person identified",
            "Person"
        ));
        assert!(!is_valid_hint_candidate("different location", "Location"));
        assert!(!is_valid_hint_candidate("candidate locations", "Location"));
        assert!(!is_valid_hint_candidate("commercial locations", "Location"));
    }

    #[test]
    fn accepts_real_proper_noun_names() {
        assert!(is_valid_hint_candidate("American Brass Company", "Company"));
        assert!(is_valid_hint_candidate(
            "Quicksilver Harbor Logistics",
            "Company"
        ));
        assert!(is_valid_hint_candidate("Wellington", "Location"));
        assert!(is_valid_hint_candidate("Callum Fitzgerald", "Person"));
    }

    #[test]
    fn rejects_existing_noise_patterns_via_entity_filter() {
        // Defense-in-depth: code/path artifacts entity_filter::is_noise_entity_name
        // already catches should also be rejected as hint candidates.
        assert!(!is_valid_hint_candidate("build.py", "Project"));
        assert!(!is_valid_hint_candidate(
            "command-20260520-stage6-rebase",
            "Project"
        ));
    }

    #[test]
    fn build_hints_excludes_noise_from_real_graph_data() {
        // Reproduces the live bug found 2026-07-02: pre-existing graph noise
        // ("Person" as a Person-classified entity_name; "no named person
        // identified" as another; "different location" as a Location-classified
        // entity_name) must not end up in the hint cache.
        let entities = vec![
            person_entity("Person"),
            person_entity("no named person identified"),
            person_entity("Prudence Okafor"),
            GraphEntity {
                entity_name: "different location".to_string(),
                classification: "Location".to_string(),
                role_vector: None,
                location_vector: None,
                contact_vector: None,
                module_id: "test".to_string(),
                confidence: 0.75,
                source_doc: None,
            },
        ];

        let hints = build_hints(entities);
        let person_hints = hints.get("Person").expect("Person bucket present");
        assert_eq!(
            person_hints,
            &vec!["Prudence Okafor".to_string()],
            "noise entries must be filtered out, leaving only the real name"
        );
        assert!(
            !hints.contains_key("Location"),
            "Location bucket should be absent — its only candidate was noise"
        );
    }
}
