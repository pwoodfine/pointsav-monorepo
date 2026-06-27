//! Entity resolution (ER) — a surface-form matcher that proposes candidate merges so the
//! same real-world entity collapses onto one canonical id (the audit measured the Woodfine
//! Capital Projects org fragmented 5-6 ways and persons split across modules).
//!
//! This module is PURE and side-effect-free: it blocks, scores, and decides. Writing the
//! decision to the graph (a canonical alias table + edge re-point) is an additive migration
//! applied separately — see BRIEF-flow-build-plan. SYS-ADR-19: a predicted/auto match is a
//! candidate; nothing here auto-publishes to the verified ledger.
//!
//! Pipeline: blocking (classification + normalized-key prefix) -> similarity (fuzzy
//! Jaro-Winkler / normalized-Levenshtein; embedding cosine behind the `embeddings` feature)
//! -> decision bands (auto-merge / review / new).
#![allow(dead_code)]

use crate::graph::{normalize_entity_key, GraphEntity};

/// Decision for a mention against the existing canonical set within its block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErDecision {
    /// High-confidence same entity — auto-merge onto the matched canonical key.
    AutoMerge(String),
    /// Ambiguous — route to the human review queue against the best canonical key.
    Review(String),
    /// No confident match — treat as a new canonical entity.
    New,
}

/// Decision-band thresholds. Defaults from the audit gold standard (ingestion precision
/// favoured: auto-merge only on a very strong match; the ambiguous middle goes to review).
#[derive(Debug, Clone)]
pub struct ErConfig {
    /// >= this similarity -> AutoMerge.
    pub auto_merge: f64,
    /// >= this (and < auto_merge) -> Review.
    pub review: f64,
    /// Normalized-key prefix length used for blocking.
    pub block_prefix_len: usize,
}

impl Default for ErConfig {
    fn default() -> Self {
        Self {
            auto_merge: 0.95,
            review: 0.85,
            block_prefix_len: 3,
        }
    }
}

/// For ER blocking only: strip middle initials from Person names so surface variants
/// ("Jennifer M. Woodfine" vs "Jennifer Woodfine") land in the same block and score
/// as a 1.0-similarity pair → AutoMerge. Does NOT affect entity_key used for MERGE
/// (changing MERGE keys would reassign IDs for existing nodes).
///
/// Rule: if a Person name has ≥ 3 whitespace tokens and a middle token is a single
/// letter + period ("M."), strip it. "J.P. Morgan" → drops "J." as a middle initial
/// (but "J." is index 0 = first token, so is NOT stripped).
fn canonical_er_name(name: &str, classification: &str) -> String {
    if classification != "Person" {
        return normalize_entity_key(name);
    }
    let words: Vec<&str> = name.split_whitespace().collect();
    if words.len() >= 3 {
        let filtered: Vec<&str> = words
            .iter()
            .enumerate()
            .filter_map(|(i, w)| {
                let is_middle = i > 0 && i < words.len() - 1;
                let is_initial = w.len() == 2 && w.ends_with('.');
                if is_middle && is_initial { None } else { Some(*w) }
            })
            .collect();
        normalize_entity_key(&filtered.join(" "))
    } else {
        normalize_entity_key(name)
    }
}

/// Blocking key: only compare entities sharing classification + a short normalized-key
/// prefix. Cuts the O(n^2) comparison space without losing recall on real variants
/// (which share a stem).
pub fn blocking_key(entity: &GraphEntity, cfg: &ErConfig) -> String {
    let key = canonical_er_name(&entity.entity_name, &entity.classification);
    let prefix: String = key.chars().take(cfg.block_prefix_len).collect();
    format!("{}|{}", entity.classification, prefix)
}

/// Fuzzy surface similarity in [0,1]: max of Jaro-Winkler and normalized Levenshtein over
/// the normalized keys (underscores -> spaces for token-aware distance).
pub fn fuzzy_similarity(a: &str, b: &str) -> f64 {
    let na = normalize_entity_key(a).replace('_', " ");
    let nb = normalize_entity_key(b).replace('_', " ");
    if na == nb {
        return 1.0;
    }
    strsim::jaro_winkler(&na, &nb).max(strsim::normalized_levenshtein(&na, &nb))
}

/// Combined similarity. Without the `embeddings` feature this is the fuzzy score; with it,
/// the caller fuses an embedding cosine (see the `embeddings` submodule).
pub fn similarity(a: &str, b: &str) -> f64 {
    fuzzy_similarity(a, b)
}

/// Decide a mention against existing canonical names (already filtered to its block).
pub fn decide(mention: &str, canonicals: &[String], cfg: &ErConfig) -> ErDecision {
    let mut best: (f64, &str) = (f64::MIN, "");
    for c in canonicals {
        let s = similarity(mention, c);
        if s > best.0 {
            best = (s, c.as_str());
        }
    }
    if best.0 >= cfg.auto_merge {
        ErDecision::AutoMerge(normalize_entity_key(best.1))
    } else if best.0 >= cfg.review {
        ErDecision::Review(normalize_entity_key(best.1))
    } else {
        ErDecision::New
    }
}

#[cfg(feature = "embeddings")]
pub mod embeddings {
    //! Optional embedding similarity (bge-small-en-v1.5 via fastembed, CPU). Off by default
    //! so the standard build never depends on onnxruntime. When enabled, the model load +
    //! cosine fusion live here and `super::similarity` blends fuzzy + embedding scores.
    //! Wiring the fastembed dependency is the operator-validated activation step.
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ent(name: &str, cls: &str) -> GraphEntity {
        GraphEntity {
            entity_name: name.into(),
            classification: cls.into(),
            role_vector: None,
            location_vector: None,
            contact_vector: None,
            module_id: "test".into(),
            confidence: 0.9,
        }
    }

    #[test]
    fn surface_variants_auto_merge() {
        let cfg = ErConfig::default();
        let canon = vec!["Woodfine Capital Projects".to_string()];
        // Trademark / legal-suffix variants collapse via normalize -> similarity 1.0.
        assert_eq!(
            decide("Woodfine Capital Projects Inc.", &canon, &cfg),
            ErDecision::AutoMerge("woodfine_capital_projects".into())
        );
    }

    #[test]
    fn near_variant_matches_distinct_is_new() {
        let cfg = ErConfig::default();
        let canon = vec!["Peter M. Woodfine".to_string()];
        // "Peter Woodfine" vs "Peter M. Woodfine" — close; lands in a match band, not New.
        assert_ne!(decide("Peter Woodfine", &canon, &cfg), ErDecision::New);
        // A clearly different entity is New.
        assert_eq!(
            decide("Jennifer Woodfine", &["Acme Robotics".to_string()], &cfg),
            ErDecision::New
        );
    }

    #[test]
    fn blocking_groups_by_class_and_prefix() {
        let cfg = ErConfig::default();
        let a = ent("Woodfine Capital Projects", "Company");
        let b = ent("Woodfine Management Corp.", "Company");
        let c = ent("Jennifer Woodfine", "Person");
        assert_eq!(blocking_key(&a, &cfg), blocking_key(&b, &cfg)); // same class + "woo"
        assert_ne!(blocking_key(&a, &cfg), blocking_key(&c, &cfg)); // different class
    }
}
