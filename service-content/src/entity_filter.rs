use serde_json::Value;

/// Canonical classification vocabulary — shared with `raw_entities_to_graph`.
/// Both ingest gate and DPO pre-save validator must use this constant so they
/// agree on what is acceptable and the training signal matches what lands in LadybugDB.
pub const ALLOWED_CLASSIFICATIONS: [&str; 5] =
    ["Person", "Company", "Project", "Account", "Location"];

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
    // File-extension-suffixed names: create-yoyo-snapshot.sh, build.py, local-content.service
    const PATH_SUFFIXES: [&str; 10] = [
        ".sh", ".py", ".rs", ".md", ".json", ".jsonl", ".conf", ".toml", ".yaml", ".service",
    ];
    if PATH_SUFFIXES.iter().any(|s| lower.ends_with(s)) {
        return true;
    }
    // Math/code expressions with parentheses: ops(slm), log(employment_35km), func()
    if trimmed.contains('(') && trimmed.contains(')') {
        return true;
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
            if !ALLOWED_CLASSIFICATIONS.contains(&coerced.as_str()) {
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
        // service- prefix project names (no .service suffix) must pass
        assert!(!is_noise_entity_name("service-vm-fleet"));
        assert!(!is_noise_entity_name("service-content"));
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
