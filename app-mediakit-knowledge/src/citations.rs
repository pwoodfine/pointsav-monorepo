//! Phase 2 Step 5 — citation registry loader and HTTP handler.
//!
//! Parses `~/Foundry/citations.yaml` (or the path supplied via `--citations-yaml`)
//! into a `Vec<CitationEntry>` and exposes the result via `GET /api/citations`.
//!
//! The YAML is read fresh on every request in Phase 2 for simplicity. Caching
//! (a single Arc<RwLock<Vec<CitationEntry>>> refreshed on a timed interval)
//! is a Phase 4 optimisation once the registry is confirmed stable.
//!
//! YAML structure observed in `~/Foundry/citations.yaml`:
//!   The file has a top-level `citations:` map whose values are records keyed
//!   by citation ID.  Example:
//!
//!   ```yaml
//!   citations:
//!     ni-51-102:
//!       type: regulatory-instrument
//!       title: "National Instrument 51-102 ..."
//!       url: "https://..."
//!       jurisdiction: ca-bcsc
//!       last_verified: 2026-04-26
//!       evidence_class: regulatory-primary
//!   ```
//!
//!   This module flattens each map entry into a `CitationEntry` by promoting
//!   the map key to the `id` field.

use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::Arc;

use crate::claim::Claim;
use crate::error::WikiError;
use crate::server::AppState;

/// A single entry from the citation registry.
///
/// Fields match the schema documented in `~/Foundry/citations.yaml`.  All
/// fields beyond `id` and `title` are optional so that missing or future
/// fields do not fail deserialization. The `extra` catch-all captures any
/// fields not explicitly listed — forward-compatible as the registry schema
/// evolves.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CitationEntry {
    /// Citation identifier — the YAML map key, e.g. `ni-51-102`.
    pub id: String,
    /// Authoritative title of the cited work.
    pub title: String,
    /// Stable URL for the cited work.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Publisher or issuing body.  Not always present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publisher: Option<String>,
    /// Regulatory or standards jurisdiction (`ca-bcsc`, `eu`, `ietf`, …).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jurisdiction: Option<String>,
    /// Registry type string (`regulatory-instrument`, `research-paper`, …).
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub entry_type: Option<String>,
    /// Evidence classification (`regulatory-primary`, `research-primary`, …).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_class: Option<String>,
    /// Date of most recent content-hash verification (ISO 8601 string).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_verified: Option<String>,
    /// Other known aliases for this citation — used by squiggle hygiene.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
    /// Forward-compatible catch-all for any field not listed above.
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_yaml::Value>,
}

/// Intermediate deserialization target for the raw YAML structure.
///
/// The file has a single top-level key `citations:` whose value is a map
/// from citation-id strings to per-citation records.  The records themselves
/// do not include their own `id` field; that is the map key.
#[derive(Debug, Deserialize)]
struct CitationFile {
    citations: BTreeMap<String, RawCitationRecord>,
}

/// A single citation record as it appears under the `citations:` key — no
/// `id` field yet; the id is the map key.
#[derive(Debug, Deserialize)]
struct RawCitationRecord {
    #[serde(default)]
    title: String,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    publisher: Option<String>,
    #[serde(default)]
    jurisdiction: Option<String>,
    #[serde(rename = "type", default)]
    entry_type: Option<String>,
    #[serde(default)]
    evidence_class: Option<String>,
    #[serde(default)]
    last_verified: Option<serde_yaml::Value>, // may be a date or a string
    #[serde(default)]
    aliases: Vec<String>,
    #[serde(flatten)]
    extra: BTreeMap<String, serde_yaml::Value>,
}

/// Load and parse a citation registry YAML file.
///
/// Returns a `Vec<CitationEntry>` sorted by ID for deterministic output.
/// On any I/O or parse error, returns `WikiError::CitationLoadFailed`.
pub async fn load_registry(path: &Path) -> Result<Vec<CitationEntry>, WikiError> {
    let raw = tokio::fs::read_to_string(path).await.map_err(|e| {
        WikiError::CitationLoadFailed(format!("cannot read {}: {e}", path.display()))
    })?;

    // The workspace `citations.yaml` opens with an optional YAML frontmatter
    // block (workspace metadata: schema, last_updated, maintainer, etc.)
    // before the actual `citations:` document. Strip it if present so the
    // parser sees only the citation records. Files without frontmatter pass
    // through untouched.
    let body: &str = if let Some(rest) = raw.strip_prefix("---\n") {
        if let Some(end) = rest.find("\n---\n") {
            &rest[end + "\n---\n".len()..]
        } else {
            raw.as_str()
        }
    } else {
        raw.as_str()
    };

    let file: CitationFile = serde_yaml::from_str(body).map_err(|e| {
        WikiError::CitationLoadFailed(format!("cannot parse {}: {e}", path.display()))
    })?;

    let mut entries: Vec<CitationEntry> = file
        .citations
        .into_iter()
        .map(|(id, rec)| {
            // Normalise `last_verified`: the YAML may emit a date node (not a
            // plain string) for YYYY-MM-DD values.  Serialize it back to a
            // string for the JSON response so the client sees a consistent type.
            let last_verified = rec.last_verified.map(|v| match &v {
                serde_yaml::Value::String(s) => s.clone(),
                other => serde_yaml::to_string(other)
                    .unwrap_or_default()
                    .trim()
                    .to_string(),
            });

            CitationEntry {
                id,
                title: rec.title,
                url: rec.url,
                publisher: rec.publisher,
                jurisdiction: rec.jurisdiction,
                entry_type: rec.entry_type,
                evidence_class: rec.evidence_class,
                last_verified,
                aliases: rec.aliases,
                extra: rec.extra,
            }
        })
        .collect();

    entries.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(entries)
}

/// HTTP handler for `GET /api/citations`.
///
/// Reads the registry fresh on every call in Phase 2.  If the file is absent
/// or unparseable, returns a 500 with the error message in the body (the
/// server started without one error stopping citation autocomplete; all other
/// surfaces continue working normally).
pub async fn get_citations(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<CitationEntry>>, WikiError> {
    let entries = load_registry(&state.citations_yaml).await?;
    Ok(Json(entries))
}

// ─── Phase 3.2 — per-claim citation resolution ──────────────────────────────
//
// The article-level resolver above (`load_registry`) is extended here to the
// per-claim grain: a claim (`claim.rs`) carries a `cites` set of registry IDs,
// and each is resolved against the registry. Unresolved IDs are a linter error
// (convention §9) and an amber/red signal for the citation ribbon.

/// One citation ID that resolved against the registry.
#[derive(Debug, Clone, Serialize)]
pub struct ResolvedCite {
    pub id: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

/// The outcome of resolving one claim's `cites` set — convention §6, Plan §3.2.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ClaimCitations {
    /// Cites that matched a registry entry (by `id` or alias).
    pub resolved: Vec<ResolvedCite>,
    /// Cites with no matching registry entry.
    pub unresolved: Vec<String>,
}

impl ClaimCitations {
    /// True when every cite resolved. An empty `cites` set is trivially
    /// fully resolved — valid for `structural` claims (convention §4.3).
    pub fn all_resolved(&self) -> bool {
        self.unresolved.is_empty()
    }
}

/// Look up one citation ID in the registry, matching the entry `id` or any
/// of its declared `aliases`.
pub fn resolve<'a>(registry: &'a [CitationEntry], cite_id: &str) -> Option<&'a CitationEntry> {
    registry
        .iter()
        .find(|e| e.id == cite_id || e.aliases.iter().any(|a| a == cite_id))
}

/// Resolve every ID in a claim's `cites` set against the registry.
pub fn resolve_claim_cites(claim: &Claim, registry: &[CitationEntry]) -> ClaimCitations {
    let mut out = ClaimCitations::default();
    for cite_id in &claim.cites {
        match resolve(registry, cite_id) {
            Some(entry) => out.resolved.push(ResolvedCite {
                id: entry.id.clone(),
                title: entry.title.clone(),
                url: entry.url.clone(),
            }),
            None => out.unresolved.push(cite_id.clone()),
        }
    }
    out
}

// ─── Phase 2 — CitationRegistry: HashMap-backed lookup with hover cards ────────

/// An in-memory citation registry indexed for O(1) lookup by citation ID or
/// alias. Built from a loaded `Vec<CitationEntry>` produced by `load_registry`.
///
/// The `CitationRegistry` is the authoritative runtime structure for the Phase 2
/// render pipeline. Route handlers that need per-article citation resolution
/// should call `CitationRegistry::load()` at startup and store the result in
/// `AppState`, or load it lazily per request from the YAML path.
#[derive(Debug, Default, Clone)]
pub struct CitationRegistry {
    /// All entries ordered by ID (for deterministic serialisation).
    pub entries: Vec<CitationEntry>,
    /// Fast lookup: id → index into `entries`. Aliases are also indexed here.
    index: HashMap<String, usize>,
}

impl CitationRegistry {
    /// Load and parse a citation YAML file, building the registry.
    ///
    /// This is a synchronous wrapper around `load_registry` for call sites that
    /// do not have an async context (e.g. startup code or tests). Returns an
    /// error when the file is absent or unparseable.
    pub fn load(path: &Path) -> Result<Self, crate::error::WikiError> {
        let raw = std::fs::read_to_string(path).map_err(|e| {
            crate::error::WikiError::CitationLoadFailed(format!(
                "cannot read {}: {e}",
                path.display()
            ))
        })?;

        // Strip optional YAML frontmatter block (same logic as the async loader).
        let body: &str = if let Some(rest) = raw.strip_prefix("---\n") {
            if let Some(end) = rest.find("\n---\n") {
                &rest[end + "\n---\n".len()..]
            } else {
                raw.as_str()
            }
        } else {
            raw.as_str()
        };

        let file: CitationFile = serde_yaml::from_str(body).map_err(|e| {
            crate::error::WikiError::CitationLoadFailed(format!(
                "cannot parse {}: {e}",
                path.display()
            ))
        })?;

        let mut entries: Vec<CitationEntry> = file
            .citations
            .into_iter()
            .map(|(id, rec)| {
                let last_verified = rec.last_verified.map(|v| match &v {
                    serde_yaml::Value::String(s) => s.clone(),
                    other => serde_yaml::to_string(other)
                        .unwrap_or_default()
                        .trim()
                        .to_string(),
                });
                CitationEntry {
                    id,
                    title: rec.title,
                    url: rec.url,
                    publisher: rec.publisher,
                    jurisdiction: rec.jurisdiction,
                    entry_type: rec.entry_type,
                    evidence_class: rec.evidence_class,
                    last_verified,
                    aliases: rec.aliases,
                    extra: rec.extra,
                }
            })
            .collect();
        entries.sort_by(|a, b| a.id.cmp(&b.id));

        // Build index: both primary id and aliases map to the entry position.
        let mut index = HashMap::new();
        for (i, e) in entries.iter().enumerate() {
            index.insert(e.id.clone(), i);
            for alias in &e.aliases {
                index.entry(alias.clone()).or_insert(i);
            }
        }

        Ok(CitationRegistry { entries, index })
    }

    /// Look up a citation by ID or alias.
    pub fn get(&self, id: &str) -> Option<&CitationEntry> {
        self.index.get(id).and_then(|&i| self.entries.get(i))
    }

    /// Return an HTML snippet for a citation hover card.
    ///
    /// The card is suitable for injection into a `<span data-citation-id="…">`
    /// tooltip via JavaScript. Returns `None` when the ID is not in the registry.
    ///
    /// Card structure:
    /// ```html
    /// <div class="citation-hover-card">
    ///   <div class="chc-title"><a href="…">Title</a></div>
    ///   <div class="chc-meta">Publisher · jurisdiction · last_verified</div>
    /// </div>
    /// ```
    pub fn hover_card_html(&self, id: &str) -> Option<String> {
        let e = self.get(id)?;
        let title_link = if let Some(ref url) = e.url {
            format!(
                r#"<a href="{}" target="_blank" rel="noopener">{}</a>"#,
                escape_html(url),
                escape_html(&e.title)
            )
        } else {
            escape_html(&e.title)
        };
        let mut meta_parts = Vec::new();
        if let Some(ref p) = e.publisher {
            meta_parts.push(escape_html(p));
        }
        if let Some(ref j) = e.jurisdiction {
            meta_parts.push(escape_html(j));
        }
        if let Some(ref lv) = e.last_verified {
            meta_parts.push(format!("verified {}", escape_html(lv)));
        }
        let meta = if meta_parts.is_empty() {
            String::new()
        } else {
            format!(r#"<div class="chc-meta">{}</div>"#, meta_parts.join(" · "))
        };
        Some(format!(
            r#"<div class="citation-hover-card"><div class="chc-title">{title_link}</div>{meta}</div>"#,
        ))
    }
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal registry entry for resolution tests.
    fn entry(id: &str, aliases: &[&str]) -> CitationEntry {
        CitationEntry {
            id: id.to_string(),
            title: format!("Title of {id}"),
            url: Some(format!("https://example.com/{id}")),
            publisher: None,
            jurisdiction: None,
            entry_type: None,
            evidence_class: None,
            last_verified: None,
            aliases: aliases.iter().map(|s| s.to_string()).collect(),
            extra: BTreeMap::new(),
        }
    }

    #[test]
    fn resolve_matches_by_id() {
        let reg = vec![entry("ni-51-102", &[])];
        assert!(resolve(&reg, "ni-51-102").is_some());
        assert!(resolve(&reg, "no-such-id").is_none());
    }

    #[test]
    fn resolve_matches_by_alias() {
        let reg = vec![entry("ni-51-102", &["ni51102"])];
        assert_eq!(
            resolve(&reg, "ni51102").map(|e| e.id.as_str()),
            Some("ni-51-102")
        );
    }

    #[test]
    fn resolve_claim_cites_splits_resolved_and_unresolved() {
        let reg = vec![entry("git-scm", &[]), entry("rfc-9162", &[])];
        let ex = crate::claim::extract_claims(
            "<!--claim id=c confidence=established \
             cites=[git-scm,missing-id,rfc-9162]-->x<!--/claim-->",
            "topic",
        );
        let res = resolve_claim_cites(&ex.claims[0], &reg);
        assert_eq!(res.resolved.len(), 2);
        assert_eq!(res.unresolved, vec!["missing-id"]);
        assert!(!res.all_resolved());
    }

    #[test]
    fn structural_claim_with_no_cites_is_fully_resolved() {
        let ex = crate::claim::extract_claims(
            "<!--claim id=c confidence=structural cites=[]-->x<!--/claim-->",
            "topic",
        );
        let res = resolve_claim_cites(&ex.claims[0], &[]);
        assert!(res.all_resolved());
        assert!(res.resolved.is_empty());
    }

    #[tokio::test]
    async fn parses_minimal_yaml() {
        let yaml = r#"
citations:
  test-id:
    type: regulatory-instrument
    title: "Test Entry"
    url: "https://example.com"
    jurisdiction: test-jurisdiction
    evidence_class: regulatory-primary
"#;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("citations.yaml");
        tokio::fs::write(&path, yaml).await.unwrap();
        let entries = load_registry(&path).await.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "test-id");
        assert_eq!(entries[0].title, "Test Entry");
        assert_eq!(entries[0].url.as_deref(), Some("https://example.com"));
        assert_eq!(
            entries[0].jurisdiction.as_deref(),
            Some("test-jurisdiction")
        );
    }

    #[tokio::test]
    async fn returns_error_on_missing_file() {
        let result = load_registry(std::path::Path::new("/nonexistent/citations.yaml")).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, WikiError::CitationLoadFailed(_)));
    }

    #[tokio::test]
    async fn entries_are_sorted_by_id() {
        let yaml = r#"
citations:
  zzz-last:
    title: "Last"
  aaa-first:
    title: "First"
  mmm-middle:
    title: "Middle"
"#;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("citations.yaml");
        tokio::fs::write(&path, yaml).await.unwrap();
        let entries = load_registry(&path).await.unwrap();
        assert_eq!(entries[0].id, "aaa-first");
        assert_eq!(entries[1].id, "mmm-middle");
        assert_eq!(entries[2].id, "zzz-last");
    }
}
