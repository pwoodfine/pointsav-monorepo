// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

use serde_json::Value;
use std::collections::HashMap;
use std::sync::OnceLock;

/// Canonical classification vocabulary — shared with `raw_entities_to_graph`.
/// Both ingest gate and DPO pre-save validator must use this constant so they
/// agree on what is acceptable and the training signal matches what lands in LadybugDB.
///
/// This is the compile-time fallback. The live vocabulary is the `label`
/// column of `ontology/entity_types.csv` (COA-driven entity type labels,
/// operator direction 2026-06-28) — see [`init_ontology_classifications`].
pub const ALLOWED_CLASSIFICATIONS: [&str; 5] =
    ["Person", "Company", "Project", "Account", "Location"];

static ONTOLOGY_CLASSIFICATIONS: OnceLock<Vec<String>> = OnceLock::new();

/// label -> parent_class (`None` if the label is a root class, i.e. its
/// `parent_class` CSV cell is empty). Populated alongside
/// `ONTOLOGY_CLASSIFICATIONS` by [`init_ontology_classifications`] — see
/// [`parent_class_of`] / [`is_subclass_of`].
static ONTOLOGY_PARENT_CLASSES: OnceLock<HashMap<String, Option<String>>> = OnceLock::new();

/// Load the classification vocabulary from `entity_types.csv`'s `label`
/// column (and its `parent_class` column, if present). Call once at startup,
/// alongside taxonomy loading. Additive only: if the CSV is missing,
/// unreadable, or empty, [`is_allowed_classification`] keeps using the
/// compile-time [`ALLOWED_CLASSIFICATIONS`] fallback — adding a new entity
/// type (or a subclass relationship) is then just a CSV edit, with no code
/// change required.
/// label -> parent_class, as parsed from `entity_types.csv`'s `parent_class`
/// column (see [`parse_entity_types_csv`]).
type ParentClassMap = HashMap<String, Option<String>>;

/// Pure CSV-parsing core of [`init_ontology_classifications`], separated out
/// so it's testable with inline synthetic CSV text — no filesystem, no
/// process-global state. Returns `None` if the CSV has no `label` column or
/// no non-empty label rows.
fn parse_entity_types_csv(content: &str) -> Option<(Vec<String>, ParentClassMap)> {
    let mut rdr = csv::Reader::from_reader(content.as_bytes());
    let headers = rdr.headers().ok()?.clone();
    let label_idx = headers.iter().position(|h| h == "label")?;
    let parent_idx = headers.iter().position(|h| h == "parent_class");
    let mut labels = Vec::new();
    let mut parents = HashMap::new();
    for rec in rdr.records().flatten() {
        if let Some(label) = rec.get(label_idx) {
            let label = label.trim();
            if !label.is_empty() {
                labels.push(label.to_string());
                let parent = parent_idx
                    .and_then(|i| rec.get(i))
                    .map(str::trim)
                    .filter(|p| !p.is_empty())
                    .map(str::to_string);
                parents.insert(label.to_string(), parent);
            }
        }
    }
    if labels.is_empty() {
        None
    } else {
        Some((labels, parents))
    }
}

pub fn init_ontology_classifications(ontology_dir: &str) {
    let path = std::path::Path::new(ontology_dir).join("entity_types.csv");
    let parsed = std::fs::read_to_string(&path)
        .ok()
        .and_then(|content| parse_entity_types_csv(&content));
    match parsed {
        Some((labels, parents)) => {
            println!(
                "[entity_filter] loaded {} classification(s) from {}",
                labels.len(),
                path.display()
            );
            let _ = ONTOLOGY_CLASSIFICATIONS.set(labels);
            let _ = ONTOLOGY_PARENT_CLASSES.set(parents);
        }
        None => {
            println!(
                "[entity_filter] {} not found or empty; using compile-time ALLOWED_CLASSIFICATIONS",
                path.display()
            );
        }
    }
}

/// Returns true if `classification` is in the active vocabulary: the
/// ontology CSV if [`init_ontology_classifications`] loaded one, else the
/// compile-time [`ALLOWED_CLASSIFICATIONS`] fallback.
pub fn is_allowed_classification(classification: &str) -> bool {
    match ONTOLOGY_CLASSIFICATIONS.get() {
        Some(labels) => labels.iter().any(|l| l == classification),
        None => ALLOWED_CLASSIFICATIONS.contains(&classification),
    }
}

/// Returns true if `child` is `ancestor` itself, or a (possibly indirect)
/// subclass of `ancestor` per `map`'s parent-class chain — e.g. if `Broker`'s
/// parent is `Person`, `is_subclass_of_in(map, "Broker", "Person")` is true.
/// Walks the chain with a depth cap to stay safe against a malformed CSV
/// introducing a parent-class cycle. Pure — no process-global state — so
/// it's directly testable with a synthetic map.
fn is_subclass_of_in(map: &HashMap<String, Option<String>>, child: &str, ancestor: &str) -> bool {
    if child == ancestor {
        return true;
    }
    const MAX_DEPTH: u8 = 16;
    let mut current = child;
    for _ in 0..MAX_DEPTH {
        match map.get(current) {
            Some(Some(parent)) if parent == ancestor => return true,
            Some(Some(parent)) => current = parent,
            _ => return false,
        }
    }
    false
}

/// Returns `class`'s immediate parent class per `entity_types.csv`'s
/// `parent_class` column, or `None` if `class` is unknown or is a root class
/// (empty `parent_class` cell). Requires [`init_ontology_classifications`] to
/// have been called; returns `None` unconditionally otherwise (no compile-time
/// fallback — the 5 built-in `ALLOWED_CLASSIFICATIONS` are all root classes).
///
/// Public introspection accessor — not yet called internally (`is_subclass_of`
/// reads the map directly), kept `pub` for callers that need the immediate
/// parent rather than a subsumption check. Same `#[allow(dead_code)]`
/// treatment as `query_context_transitive` had before it found its caller.
#[allow(dead_code)]
pub fn parent_class_of(class: &str) -> Option<String> {
    ONTOLOGY_PARENT_CLASSES.get()?.get(class)?.clone()
}

/// Returns true if `child` is `ancestor` itself, or a (possibly indirect)
/// subclass of `ancestor`, per the live ontology loaded by
/// [`init_ontology_classifications`]. Returns `child == ancestor` if the
/// ontology hasn't been loaded (no parent-class data available at all).
pub fn is_subclass_of(child: &str, ancestor: &str) -> bool {
    match ONTOLOGY_PARENT_CLASSES.get() {
        Some(map) => is_subclass_of_in(map, child, ancestor),
        None => child == ancestor,
    }
}

/// A closed relation-type rule loaded from `ontology/relation_types.csv`:
/// the canonical relation name plus which entity classes are valid as the
/// edge's subject/object (subsumption-aware — a `valid_subject_classes` entry
/// of `Person` also matches a `Broker`, via [`is_subclass_of`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationTypeRule {
    pub relation_type: String,
    pub valid_subject_classes: Vec<String>,
    pub valid_object_classes: Vec<String>,
}

/// normalized (lowercased) predicate synonym -> the rule it resolves to.
/// The canonical `relation_type` name itself is always included as a synonym
/// of itself. Populated by [`init_relation_ontology`].
static RELATION_ONTOLOGY: OnceLock<HashMap<String, RelationTypeRule>> = OnceLock::new();

fn split_pipe_list(s: &str) -> Vec<String> {
    s.split('|')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(str::to_string)
        .collect()
}

/// Pure CSV-parsing core of [`init_relation_ontology`] — testable with inline
/// synthetic CSV text, no filesystem or process-global state. Returns `None`
/// if the CSV is missing required columns or has no usable rows.
fn parse_relation_types_csv(content: &str) -> Option<HashMap<String, RelationTypeRule>> {
    let mut rdr = csv::Reader::from_reader(content.as_bytes());
    let headers = rdr.headers().ok()?.clone();
    let type_idx = headers.iter().position(|h| h == "relation_type")?;
    let subj_idx = headers.iter().position(|h| h == "valid_subject_classes")?;
    let obj_idx = headers.iter().position(|h| h == "valid_object_classes")?;
    let syn_idx = headers.iter().position(|h| h == "predicate_synonyms");
    let mut by_predicate = HashMap::new();
    for rec in rdr.records().flatten() {
        let Some(relation_type) = rec
            .get(type_idx)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
        else {
            continue;
        };
        let rule = RelationTypeRule {
            relation_type: relation_type.clone(),
            valid_subject_classes: split_pipe_list(rec.get(subj_idx).unwrap_or("")),
            valid_object_classes: split_pipe_list(rec.get(obj_idx).unwrap_or("")),
        };
        let mut predicates = vec![relation_type.to_lowercase()];
        if let Some(syn_idx) = syn_idx {
            predicates.extend(
                split_pipe_list(rec.get(syn_idx).unwrap_or(""))
                    .into_iter()
                    .map(|s| s.to_lowercase()),
            );
        }
        for predicate in predicates {
            by_predicate.insert(predicate, rule.clone());
        }
    }
    if by_predicate.is_empty() {
        None
    } else {
        Some(by_predicate)
    }
}

/// Load the closed relation-type vocabulary from `ontology/relation_types.csv`.
/// Call once at startup, alongside [`init_ontology_classifications`]. Additive
/// only: if the CSV is missing, unreadable, or empty, [`validate_relation_edge`]
/// allows every edge through unchanged (fail-open — a missing/malformed CSV
/// must not silently stop all relation writes).
pub fn init_relation_ontology(ontology_dir: &str) {
    let path = std::path::Path::new(ontology_dir).join("relation_types.csv");
    let parsed = std::fs::read_to_string(&path)
        .ok()
        .and_then(|content| parse_relation_types_csv(&content));
    match parsed {
        Some(rules) => {
            println!(
                "[entity_filter] loaded {} relation-type predicate mapping(s) from {}",
                rules.len(),
                path.display()
            );
            let _ = RELATION_ONTOLOGY.set(rules);
        }
        None => {
            println!(
                "[entity_filter] {} not found or empty; relation-edge validation disabled (fail-open)",
                path.display()
            );
        }
    }
}

/// Pure core of [`validate_relation_edge`] — takes the ontology map
/// explicitly rather than reading the process-global [`RELATION_ONTOLOGY`],
/// so it's directly testable with a synthetic map (same rationale as
/// [`is_subclass_of_in`]).
fn validate_relation_edge_in(
    ontology: &HashMap<String, RelationTypeRule>,
    predicate: &str,
    subject_class: &str,
    object_class: &str,
) -> bool {
    let Some(rule) = ontology.get(&predicate.trim().to_lowercase()) else {
        return true;
    };
    let subject_ok = rule
        .valid_subject_classes
        .iter()
        .any(|c| is_subclass_of(subject_class, c));
    let object_ok = rule
        .valid_object_classes
        .iter()
        .any(|c| is_subclass_of(object_class, c));
    subject_ok && object_ok
}

/// Returns true if an edge with this `predicate` (matched case-insensitively
/// against `relation_types.csv`'s canonical names and synonyms) is valid
/// between an entity classified `subject_class` and one classified
/// `object_class` — checking class membership via [`is_subclass_of`], so a
/// rule's `valid_subject_classes: ["Person"]` also accepts a `Broker` subject.
///
/// **Fail-open by design**: if [`init_relation_ontology`] never loaded a
/// vocabulary (CSV missing/empty) or `predicate` doesn't match any known
/// relation type, this returns `true` — an unrecognized predicate is not
/// necessarily wrong, just not yet in the closed vocabulary (this is a v1
/// starter list; see `ontology/relation_types.csv`'s header comment). Once a
/// predicate *is* in the vocabulary, its subject/object classes are enforced.
pub fn validate_relation_edge(predicate: &str, subject_class: &str, object_class: &str) -> bool {
    match RELATION_ONTOLOGY.get() {
        Some(ontology) => validate_relation_edge_in(ontology, predicate, subject_class, object_class),
        None => true,
    }
}

/// Returns true if `name` looks like a code or environment identifier rather
/// than a proper entity name. Used as a deterministic backstop in
/// raw_entities_to_graph() and clean_dpo_side() to reject noise regardless
/// of model compliance.
pub fn is_noise_entity_name(name: &str) -> bool {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return true;
    }
    const PLACEHOLDERS: [&str; 6] = ["not specified", "n/a", "unknown", "tbd", "none", "null"];
    let lower = trimmed.to_lowercase();
    if PLACEHOLDERS.contains(&lower.as_str()) {
        return true;
    }
    // Backtick-wrapped identifiers: `ghi_kwh_m2_yr`
    if trimmed.starts_with('`') && trimmed.ends_with('`') {
        return true;
    }
    // Environment variable references
    if trimmed.starts_with('$') {
        return true;
    }
    // Glob patterns
    if trimmed.contains('*') {
        return true;
    }
    // File paths (absolute, relative, or URL-like)
    if trimmed.contains('/') {
        return true;
    }
    // File-extension-suffixed names: create-yoyo-snapshot.sh, build.py, local-content.service,
    // lora-update.timer (systemd timer units)
    const PATH_SUFFIXES: [&str; 11] = [
        ".sh", ".py", ".rs", ".md", ".json", ".jsonl", ".conf", ".toml", ".yaml", ".service",
        ".timer",
    ];
    if PATH_SUFFIXES.iter().any(|s| lower.ends_with(s)) {
        return true;
    }
    // Math/code expressions with parentheses: ops(slm), log(employment_35km), func()
    if trimmed.contains('(') && trimmed.contains(')') {
        return true;
    }
    // Operational status phrases joined by " + ": "service-content rebuilt + deployed",
    // "Yo-Yo env IP update + Doorman restart" — these are event descriptions, not entities.
    if trimmed.contains(" + ") {
        return true;
    }
    // Date-slug identifiers: hyphenated names with an 8-consecutive-digit segment are
    // mailbox message IDs (command-20260520-stage6-rebase-required) or dated slugs.
    // Real entity names (proper nouns) do not embed YYYYMMDD date runs.
    {
        let mut consecutive_digits: u8 = 0;
        let mut max_run: u8 = 0;
        for c in trimmed.chars() {
            if c.is_ascii_digit() {
                consecutive_digits += 1;
                if consecutive_digits > max_run {
                    max_run = consecutive_digits;
                }
            } else {
                consecutive_digits = 0;
            }
        }
        if max_run >= 8 {
            return true;
        }
    }
    // Snake_case identifiers without spaces: env vars, code symbols, metric names
    if !trimmed.contains(' ') && trimmed.contains('_') {
        return true;
    }
    // Numeric-prefix: entities that start with a digit or decimal are counts,
    // measurements, or data fragments ("5.0 km of the cluster centroid", "3,904 of 14,332")
    if trimmed.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        return true;
    }
    // Sentence fragment starters (expanded from 4 → 14)
    const FRAGMENT_STARTERS: [&str; 14] = [
        "the ", "a ", "an ", "this ", "all ", "any ", "each ", "most ", "some ", "these ",
        "those ", "section ", "for ", "of ",
    ];
    if FRAGMENT_STARTERS.iter().any(|p| lower.starts_with(p)) {
        return true;
    }
    // Comma-joined or conjunction phrases: not a single entity
    if trimmed.contains(", ") || lower.contains(" and ") {
        return true;
    }
    // Abstract common nouns that are never named entities in this domain.
    // Only applied to single-word entries (multi-word proper nouns are fine).
    if !trimmed.contains(' ') {
        const ABSTRACT_NOUNS: [&str; 18] = [
            "framework",
            "model",
            "hypothesis",
            "hypotheses",
            "pipeline",
            "approach",
            "process",
            "mechanism",
            "algorithm",
            "methodology",
            "criterion",
            "criteria",
            "paradigm",
            "construct",
            "abstraction",
            "concept",
            "system",
            "architecture",
        ];
        if ABSTRACT_NOUNS.contains(&lower.as_str()) {
            return true;
        }
    }
    false
}

/// Returns true if `name` matches a conventional-commit prefix pattern:
/// `type(scope)` — e.g. ops(slm), feat(cache), fix(auth), chore(db).
/// These appear in git commit log text and must not be treated as entities.
pub fn is_commit_prefix(name: &str) -> bool {
    let t = name.trim();
    let Some(open) = t.find('(') else {
        return false;
    };
    if !t.ends_with(')') {
        return false;
    }
    let head = &t[..open];
    !head.is_empty()
        && head
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        && open + 1 < t.len() - 1
}

/// Filter noise entities from one side of a DPO pair before saving.
///
/// Applies the **same** filter chain as `raw_entities_to_graph`:
///   1. commit-prefix and noise-name rejection
///   2. word-count gate (>8 words)
///   3. `coerce_classification` — corrects misclassified entities in place
///   4. `ALLOWED_CLASSIFICATIONS` gate — rejects out-of-vocabulary types
///
/// When `coerce_classification` corrects a classification (e.g. Company→Location)
/// the corrected value is written into the returned JSON object so the DPO training
/// pair teaches the correct form rather than the raw (wrong) model output.
pub fn clean_dpo_side(side: &[Value]) -> Vec<Value> {
    side.iter()
        .filter_map(|e| {
            let name = e.get("entity_name").and_then(|v| v.as_str())?;
            let cls = e.get("classification").and_then(|v| v.as_str())?;
            if is_commit_prefix(name) || is_noise_entity_name(name) {
                return None;
            }
            if name.split_whitespace().count() > 8 {
                return None;
            }
            let coerced = coerce_classification(name, cls)?;
            if !is_allowed_classification(&coerced) {
                return None;
            }
            if coerced != cls {
                let mut patched = e.clone();
                if let Some(obj) = patched.as_object_mut() {
                    obj.insert("classification".to_string(), Value::String(coerced));
                }
                Some(patched)
            } else {
                Some(e.clone())
            }
        })
        .collect()
}

/// Well-known country names that OLMo commonly misclassifies as Company.
/// Caller must pass the entity name already lowercased.
pub fn is_known_place(name_lower: &str) -> bool {
    const COUNTRIES: [&str; 28] = [
        "portugal",
        "spain",
        "france",
        "germany",
        "italy",
        "netherlands",
        "belgium",
        "austria",
        "switzerland",
        "poland",
        "sweden",
        "norway",
        "denmark",
        "finland",
        "united kingdom",
        "united states",
        "canada",
        "australia",
        "mexico",
        "brazil",
        "argentina",
        "india",
        "china",
        "japan",
        "south korea",
        "singapore",
        "hong kong",
        "new zealand",
    ];
    COUNTRIES.contains(&name_lower)
}

/// Apply type-coherence rules to correct or reject a model-emitted classification.
///
/// Returns `Some(corrected_classification)` to accept (with optional correction),
/// or `None` to reject the entity entirely.
pub fn coerce_classification(entity_name: &str, classification: &str) -> Option<String> {
    let name_lower = entity_name.to_lowercase();
    // Country misclassified as Company → reclassify as Location.
    if classification == "Company" && is_known_place(&name_lower) {
        return Some("Location".to_string());
    }
    // File-path-like name as Project → reject (it's a path, not a project).
    if classification == "Project" && entity_name.contains('/') {
        return None;
    }
    // Multi-word lowercase phrase as Project → reject (operational noise like "outbox status",
    // "corpus payload key", "enrichment queue", "automation bot"). Real project names use
    // hyphens as word separators (service-vm-fleet), not spaces. A space in a Project name
    // with all-lowercase characters is a strong signal of a generic phrase, not a proper name.
    if classification == "Project"
        && entity_name.contains(' ')
        && entity_name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c == ' ')
    {
        return None;
    }
    // ALL_CAPS with underscores as Account → reject (it's a constant or env var).
    if classification == "Account"
        && entity_name.len() >= 2
        && entity_name
            .chars()
            .all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit())
        && entity_name.chars().any(|c| c.is_ascii_uppercase())
    {
        return None;
    }
    // Lowercase-only Account (spaces or hyphens only) → reject (abstract noise phrase).
    // Covers "outbox status" (space) and "service-content" (hyphen) cases.
    // Real account IDs contain digits, colons, or uppercase letters.
    if classification == "Account"
        && (entity_name.contains(' ') || entity_name.contains('-'))
        && entity_name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c == ' ' || c == '-')
    {
        return None;
    }
    // Single-word all-lowercase Account with no digits, colons, or at-sign → reject.
    // Real account identifiers (GCP projects, service accounts, contract IDs) always
    // contain at least one structural character. A bare lowercase word ("outbox",
    // "inbox", "queue") is a generic noun, not an account reference.
    if classification == "Account"
        && !entity_name.contains(' ')
        && !entity_name.contains('-')
        && !entity_name.contains(':')
        && !entity_name.contains('@')
        && !entity_name.contains('.')
        && entity_name.chars().all(|c| c.is_ascii_lowercase())
    {
        return None;
    }
    Some(classification.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_allowed_classification_falls_back_to_const_when_unset() {
        // No init_ontology_classifications call in this test — must use the
        // compile-time fallback regardless of whether another test in this
        // binary has already set the (process-global, set-once) ontology cache.
        for label in ALLOWED_CLASSIFICATIONS {
            assert!(is_allowed_classification(label));
        }
        assert!(!is_allowed_classification("Technology"));
    }

    #[test]
    fn ontology_classifications_load_from_real_csv() {
        // Verify the actual on-disk entity_types.csv parses and matches the
        // compile-time vocabulary (same 5 labels — additive migration, see
        // BRIEF-flow-build-plan.md §COA-driven entity type labels).
        // Path relative to the manifest directory (service-content/).
        let ontology_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/ontology");
        init_ontology_classifications(ontology_dir);
        for label in ALLOWED_CLASSIFICATIONS {
            assert!(
                is_allowed_classification(label),
                "{label} must be allowed after loading entity_types.csv"
            );
        }
        assert!(!is_allowed_classification("Technology"));
    }

    #[test]
    fn parse_entity_types_csv_extracts_parent_class_column() {
        let csv = "label,description_projects,parent_class\n\
                   Person,a human,\n\
                   Broker,a real-estate broker,Person\n";
        let (labels, parents) = parse_entity_types_csv(csv).expect("should parse");
        assert_eq!(labels, vec!["Person".to_string(), "Broker".to_string()]);
        assert_eq!(parents.get("Person"), Some(&None));
        assert_eq!(parents.get("Broker"), Some(&Some("Person".to_string())));
    }

    #[test]
    fn parse_entity_types_csv_tolerates_missing_parent_class_column() {
        // Older/minimal CSVs without a parent_class column at all must still parse —
        // every label is treated as a root class (no parent recorded).
        let csv = "label,description_projects\nPerson,a human\n";
        let (labels, parents) = parse_entity_types_csv(csv).expect("should parse");
        assert_eq!(labels, vec!["Person".to_string()]);
        assert_eq!(parents.get("Person"), Some(&None));
    }

    #[test]
    fn is_subclass_of_direct_and_reflexive() {
        let mut map = HashMap::new();
        map.insert("Person".to_string(), None);
        map.insert("Broker".to_string(), Some("Person".to_string()));
        assert!(is_subclass_of_in(&map, "Broker", "Person"));
        assert!(is_subclass_of_in(&map, "Person", "Person"));
        assert!(!is_subclass_of_in(&map, "Person", "Broker"));
    }

    #[test]
    fn is_subclass_of_transitive_chain() {
        // ResidentialBroker -> Broker -> Person: a two-hop subsumption chain.
        let mut map = HashMap::new();
        map.insert("Person".to_string(), None);
        map.insert("Broker".to_string(), Some("Person".to_string()));
        map.insert("ResidentialBroker".to_string(), Some("Broker".to_string()));
        assert!(is_subclass_of_in(&map, "ResidentialBroker", "Person"));
        assert!(is_subclass_of_in(&map, "ResidentialBroker", "Broker"));
        assert!(!is_subclass_of_in(&map, "Person", "ResidentialBroker"));
    }

    #[test]
    fn is_subclass_of_unrelated_classes_and_cycle_safety() {
        let mut map = HashMap::new();
        map.insert("Person".to_string(), None);
        map.insert("Company".to_string(), None);
        assert!(!is_subclass_of_in(&map, "Person", "Company"));
        // A malformed CSV introducing a cycle must not hang — MAX_DEPTH bounds the walk.
        let mut cyclic = HashMap::new();
        cyclic.insert("A".to_string(), Some("B".to_string()));
        cyclic.insert("B".to_string(), Some("A".to_string()));
        assert!(!is_subclass_of_in(&cyclic, "A", "Nonexistent"));
    }

    #[test]
    fn parse_relation_types_csv_basic_and_synonyms() {
        let csv = "relation_type,valid_subject_classes,valid_object_classes,predicate_synonyms\n\
                   employed_by,Person,Company,employed by|works for\n";
        let ontology = parse_relation_types_csv(csv).expect("should parse");
        // Canonical name and every synonym all resolve to the same rule.
        for key in ["employed_by", "employed by", "works for"] {
            let rule = ontology.get(key).unwrap_or_else(|| panic!("missing {key}"));
            assert_eq!(rule.relation_type, "employed_by");
            assert_eq!(rule.valid_subject_classes, vec!["Person".to_string()]);
            assert_eq!(rule.valid_object_classes, vec!["Company".to_string()]);
        }
        assert!(!ontology.contains_key("acquired"));
    }

    #[test]
    fn parse_relation_types_csv_missing_columns_returns_none() {
        assert!(parse_relation_types_csv("relation_type\nemployed_by\n").is_none());
    }

    #[test]
    fn validate_relation_edge_enforces_known_predicate_classes() {
        let mut ontology = HashMap::new();
        ontology.insert(
            "employed_by".to_string(),
            RelationTypeRule {
                relation_type: "employed_by".to_string(),
                valid_subject_classes: vec!["Person".to_string()],
                valid_object_classes: vec!["Company".to_string()],
            },
        );
        assert!(validate_relation_edge_in(
            &ontology,
            "employed_by",
            "Person",
            "Company"
        ));
        // Wrong subject class for a known predicate — rejected.
        assert!(!validate_relation_edge_in(
            &ontology,
            "employed_by",
            "Location",
            "Company"
        ));
        // Wrong object class for a known predicate — rejected.
        assert!(!validate_relation_edge_in(
            &ontology,
            "employed_by",
            "Person",
            "Location"
        ));
    }

    #[test]
    fn validate_relation_edge_fails_open_for_unknown_predicate() {
        // An empty/unrelated ontology must not block a predicate it has never heard of —
        // fail-open, since open-IE extraction produces free-form verb phrases and an
        // unrecognized one is "not yet in the vocabulary," not "known to be wrong."
        let ontology = HashMap::new();
        assert!(validate_relation_edge_in(
            &ontology,
            "invented",
            "Person",
            "Project"
        ));
    }

    #[test]
    fn noise_rejects_env_var() {
        assert!(is_noise_entity_name("$SLM_DATA_DIR"));
    }

    #[test]
    fn noise_rejects_snake_case() {
        assert!(is_noise_entity_name("ghi_kwh_m2_yr"));
        assert!(is_noise_entity_name("FOUNDRY_ARCHIVE_NAME"));
    }

    #[test]
    fn noise_rejects_shell_script() {
        assert!(is_noise_entity_name("create-yoyo-snapshot.sh"));
    }

    #[test]
    fn noise_rejects_call_expression() {
        assert!(is_noise_entity_name("log(employment_35km)"));
        assert!(is_noise_entity_name("ops(slm)"));
    }

    #[test]
    fn noise_rejects_placeholder() {
        assert!(is_noise_entity_name("not specified"));
        assert!(is_noise_entity_name("N/A"));
        assert!(is_noise_entity_name("unknown"));
    }

    #[test]
    fn noise_rejects_backtick() {
        assert!(is_noise_entity_name("`ghi_kwh_m2_yr`"));
    }

    #[test]
    fn noise_allows_proper_names() {
        assert!(!is_noise_entity_name("Peter Woodfine"));
        assert!(!is_noise_entity_name("PointSav Digital Systems"));
        assert!(!is_noise_entity_name("Woodfine Management Corp."));
        assert!(!is_noise_entity_name("Vancouver"));
        assert!(!is_noise_entity_name("service-content"));
    }

    #[test]
    fn noise_rejects_numeric_prefix() {
        assert!(is_noise_entity_name("5.0 km of the cluster centroid"));
        assert!(is_noise_entity_name("3,904 of 14,332"));
        assert!(is_noise_entity_name("100 meters"));
    }

    #[test]
    fn noise_rejects_extended_fragment_starters() {
        assert!(is_noise_entity_name("all formal verification proofs"));
        assert!(is_noise_entity_name("some of the clusters"));
        assert!(is_noise_entity_name("each tier definition"));
        assert!(is_noise_entity_name(
            "Section 7 states the falsification programme"
        ));
    }

    #[test]
    fn noise_rejects_abstract_nouns() {
        assert!(is_noise_entity_name("framework"));
        assert!(is_noise_entity_name("model"));
        assert!(is_noise_entity_name("hypothesis"));
        assert!(is_noise_entity_name("hypotheses"));
        assert!(is_noise_entity_name("pipeline"));
        assert!(is_noise_entity_name("architecture"));
    }

    #[test]
    fn noise_allows_valid_multi_word_with_common_words() {
        // "Model" in a proper noun context is fine — filter is single-word only
        assert!(!is_noise_entity_name("Claude Model Registry"));
        // Proper nouns that happen to contain abstract concepts remain valid
        assert!(!is_noise_entity_name("Foundry Pipeline"));
    }

    #[test]
    fn commit_prefix_positive() {
        assert!(is_commit_prefix("ops(slm)"));
        assert!(is_commit_prefix("feat(cache)"));
        assert!(is_commit_prefix("fix(auth)"));
        assert!(is_commit_prefix("chore(db)"));
    }

    #[test]
    fn commit_prefix_negative() {
        assert!(!is_commit_prefix("service-content"));
        assert!(!is_commit_prefix("Peter Woodfine"));
        assert!(!is_commit_prefix("ops()")); // empty scope
    }

    #[test]
    fn coerce_country_as_company_to_location() {
        let result = coerce_classification("Portugal", "Company");
        assert_eq!(result, Some("Location".to_string()));
    }

    #[test]
    fn coerce_path_as_project_rejected() {
        let result = coerce_classification("src/main.rs", "Project");
        assert_eq!(result, None);
    }

    #[test]
    fn coerce_caps_as_account_rejected() {
        let result = coerce_classification("SLM_TIER_A_FIRST", "Account");
        assert_eq!(result, None);
    }

    #[test]
    fn coerce_valid_person_passthrough() {
        let result = coerce_classification("Jennifer Woodfine", "Person");
        assert_eq!(result, Some("Person".to_string()));
    }

    #[test]
    fn clean_dpo_side_removes_commit_prefix() {
        let side = vec![
            serde_json::json!({"entity_name": "ops(slm)", "classification": "Project"}),
            serde_json::json!({"entity_name": "Jennifer Woodfine", "classification": "Person"}),
        ];
        let cleaned = clean_dpo_side(&side);
        assert_eq!(cleaned.len(), 1);
        assert_eq!(
            cleaned[0]["entity_name"].as_str().unwrap(),
            "Jennifer Woodfine"
        );
    }

    #[test]
    fn clean_dpo_side_rejects_overlong_phrase() {
        let side = vec![
            serde_json::json!({
                "entity_name": "the quick brown fox jumped over the lazy dog here",
                "classification": "Person"
            }),
            serde_json::json!({"entity_name": "Peter Woodfine", "classification": "Person"}),
        ];
        let cleaned = clean_dpo_side(&side);
        assert_eq!(cleaned.len(), 1);
        assert_eq!(
            cleaned[0]["entity_name"].as_str().unwrap(),
            "Peter Woodfine"
        );
    }

    #[test]
    fn clean_dpo_side_applies_coerce_classification() {
        let side =
            vec![serde_json::json!({"entity_name": "Portugal", "classification": "Company"})];
        let cleaned = clean_dpo_side(&side);
        assert_eq!(cleaned.len(), 1);
        assert_eq!(cleaned[0]["entity_name"].as_str().unwrap(), "Portugal");
        assert_eq!(cleaned[0]["classification"].as_str().unwrap(), "Location");
    }

    #[test]
    fn coerce_lowercase_spaced_project_phrase_rejected() {
        // "outbox status", "corpus payload key", "automation bot" — operational noise phrases
        assert_eq!(coerce_classification("outbox status", "Project"), None);
        assert_eq!(coerce_classification("corpus payload key", "Project"), None);
        assert_eq!(coerce_classification("enrichment queue", "Project"), None);
        assert_eq!(coerce_classification("automation bot", "Project"), None);
        // Valid project names (no spaces) must pass through unchanged
        assert_eq!(
            coerce_classification("service-vm-fleet", "Project"),
            Some("Project".to_string())
        );
        assert_eq!(
            coerce_classification("slm-doorman-server", "Project"),
            Some("Project".to_string())
        );
        assert_eq!(
            coerce_classification("Doorman", "Project"),
            Some("Project".to_string())
        );
    }

    #[test]
    fn noise_rejects_service_suffix() {
        // systemd unit names extracted as Project — filter via .service suffix
        assert!(is_noise_entity_name("local-content.service"));
        assert!(is_noise_entity_name("llama-server.service"));
        // systemd timer unit names — .timer suffix
        assert!(is_noise_entity_name("lora-update.timer"));
        assert!(is_noise_entity_name("nightly-build.timer"));
        // service- prefix project names (no .service/.timer suffix) must pass
        assert!(!is_noise_entity_name("service-vm-fleet"));
        assert!(!is_noise_entity_name("service-content"));
    }

    #[test]
    fn noise_rejects_operational_plus_phrases() {
        // " + " is an operational event joiner, not an entity name component
        assert!(is_noise_entity_name("service-content rebuilt + deployed"));
        assert!(is_noise_entity_name(
            "Yo-Yo env IP update + Doorman restart"
        ));
        assert!(is_noise_entity_name("stage6 + restart"));
    }

    #[test]
    fn noise_rejects_date_slug_ids() {
        // Mailbox message IDs and dated slugs contain 8-consecutive-digit YYYYMMDD runs
        assert!(is_noise_entity_name(
            "command-20260520-stage6-rebase-required"
        ));
        assert!(is_noise_entity_name(
            "project-totebox-20260622-stage6-d9-d8-p8-fixes"
        ));
        assert!(is_noise_entity_name(
            "project-intelligence-20260620-session26c-stage6-prompt-fix"
        ));
        // Real project names with short digit runs must pass
        assert!(!is_noise_entity_name("service-vm-fleet"));
        assert!(!is_noise_entity_name("app-privategit-source"));
        assert!(!is_noise_entity_name("moonshot-sel4-vmm"));
        // A project with a version number (short digit run) must pass
        assert!(!is_noise_entity_name("v0.3.1"));
    }

    #[test]
    fn coerce_single_word_lowercase_account_rejected() {
        // Bare lowercase single-word → not an account identifier
        assert_eq!(coerce_classification("outbox", "Account"), None);
        assert_eq!(coerce_classification("inbox", "Account"), None);
        assert_eq!(coerce_classification("queue", "Account"), None);
        // Lowercase with hyphens also rejected by existing rule (no structural chars)
        assert_eq!(coerce_classification("gcp-foundry-prod", "Account"), None);
        // Accounts with structural characters (@ or .) pass through
        assert_eq!(
            coerce_classification("jennifer@woodfine.com", "Account"),
            Some("Account".to_string())
        );
        // ALL_CAPS with digits pass through
        assert_eq!(
            coerce_classification("PROJECT-12345", "Account"),
            Some("Account".to_string())
        );
    }

    #[test]
    fn clean_dpo_side_rejects_invalid_classification() {
        let side = vec![
            serde_json::json!({"entity_name": "OpenSSL", "classification": "Technology"}),
            serde_json::json!({"entity_name": "Peter Woodfine", "classification": "Person"}),
        ];
        let cleaned = clean_dpo_side(&side);
        assert_eq!(cleaned.len(), 1);
        assert_eq!(
            cleaned[0]["entity_name"].as_str().unwrap(),
            "Peter Woodfine"
        );
    }
}
