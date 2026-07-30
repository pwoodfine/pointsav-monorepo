//! Phase 4 Steps 4.4+4.5 — redb-backed wikilink graph + blake3 content hashes.
//! Phase 7D — citations table for URL freshness tracking.
//!
//! Three tables in a single redb database at `<state_dir>/links.redb`:
//!
//! - `OUTLINKS`: composite key `"from_slug\x00to_slug"` → u8 sentinel.
//!   Supports two query patterns:
//!   - Outlinks for a slug: prefix scan `"slug\x00" ..`.
//!   - Backlinks to a slug: full scan, filter by `"\x00target"` suffix.
//!
//! - `HASHES`: composite key `"slug\x00revision_sha"` → 32-byte blake3 digest.
//!   Federation-seam baseline (Phase 7 lights up an efficient reverse index).
//!
//! - `CITATIONS`: key `cite_id` (e.g. `"slug:fn-1"`) → JSON blob
//!   `{"url":"...","title":"...","status":"unknown"}`. Phase 9 URL validator
//!   updates `status` to `"fresh"` or `"stale"` on a nightly sweep.

use crate::claim::Claim;
use crate::error::WikiError;
use redb::{Database, ReadableDatabase, ReadableTable, TableDefinition};
use regex::Regex;
use std::{path::Path, sync::Arc};

const OUTLINKS: TableDefinition<&str, u8> = TableDefinition::new("outlinks");
const HASHES: TableDefinition<&str, &[u8]> = TableDefinition::new("hashes");

// Phase 3.3 — claim-dependency graph.
const CLAIM_DEPS: TableDefinition<&str, u8> = TableDefinition::new("claim_deps");

// Phase 7D — citation URL freshness tracking.
const CITATIONS: TableDefinition<&str, &str> = TableDefinition::new("citations");

pub struct LinkGraph {
    db: Arc<Database>,
}

impl LinkGraph {
    /// Returns true if any hash entry exists for this slug (prefix scan on
    /// the composite `"slug\x00revision"` key, so the exact revision is ignored).
    pub fn exists(&self, slug: &str) -> bool {
        let Ok(rtx) = self.db.begin_read() else {
            return false;
        };
        let Ok(table) = rtx.open_table(HASHES) else {
            return false;
        };
        let prefix = format!("{}\x00", slug);
        let Ok(mut range) = table.range(prefix.as_str()..) else {
            return false;
        };
        match range.next() {
            Some(Ok((k, _))) => k.value().starts_with(prefix.as_str()),
            _ => false,
        }
    }

    /// Open an existing database or create a new one at `path`.
    pub fn open_or_create(path: &Path) -> Result<Self, WikiError> {
        let db = if path.exists() {
            Database::open(path)
        } else {
            Database::create(path)
        }
        .map_err(|e| WikiError::LinkGraph(e.to_string()))?;

        // Ensure both tables exist on first open.
        let wtx = db
            .begin_write()
            .map_err(|e| WikiError::LinkGraph(e.to_string()))?;
        let _ = wtx
            .open_table(OUTLINKS)
            .map_err(|e| WikiError::LinkGraph(e.to_string()))?;
        let _ = wtx
            .open_table(HASHES)
            .map_err(|e| WikiError::LinkGraph(e.to_string()))?;
        let _ = wtx
            .open_table(CLAIM_DEPS)
            .map_err(|e| WikiError::LinkGraph(e.to_string()))?;
        let _ = wtx
            .open_table(CITATIONS)
            .map_err(|e| WikiError::LinkGraph(e.to_string()))?;
        wtx.commit()
            .map_err(|e| WikiError::LinkGraph(e.to_string()))?;

        Ok(Self { db: Arc::new(db) })
    }

    /// Create a temporary database for use in tests. Each call returns a
    /// distinct database so parallel tests do not conflict.
    pub fn for_testing() -> Arc<Self> {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("wiki-links-{}-{}.redb", std::process::id(), n));
        Arc::new(Self::open_or_create(&path).expect("link graph test init failed"))
    }

    /// Rebuild all wikilink edges for `slug`. Deletes existing outlinks, then
    /// inserts one row per `[[target]]` found in `body`.
    pub fn rebuild_for_slug(&self, slug: &str, body: &str) -> Result<(), WikiError> {
        let targets = parse_wikilinks(body);
        let prefix = format!("{}\x00", slug);

        let wtx = self
            .db
            .begin_write()
            .map_err(|e| WikiError::LinkGraph(e.to_string()))?;
        {
            let mut table = wtx
                .open_table(OUTLINKS)
                .map_err(|e| WikiError::LinkGraph(e.to_string()))?;

            // Collect existing keys for this slug (avoids borrow-while-mutate).
            let to_remove: Vec<String> = table
                .iter()
                .map_err(|e| WikiError::LinkGraph(e.to_string()))?
                .filter_map(|r| r.ok())
                .filter_map(|(k, _)| {
                    let key = k.value();
                    if key.starts_with(prefix.as_str()) {
                        Some(key.to_owned())
                    } else {
                        None
                    }
                })
                .collect();

            for key in &to_remove {
                table
                    .remove(key.as_str())
                    .map_err(|e| WikiError::LinkGraph(e.to_string()))?;
            }

            for target in &targets {
                let key = format!("{}\x00{}", slug, target);
                table
                    .insert(key.as_str(), 0u8)
                    .map_err(|e| WikiError::LinkGraph(e.to_string()))?;
            }
        }
        wtx.commit()
            .map_err(|e| WikiError::LinkGraph(e.to_string()))?;
        Ok(())
    }

    /// Returns the slugs of all articles that this slug links *to* (forward links).
    ///
    /// Prefix scan on the composite key `"slug\x00"` — O(outlinks-for-slug).
    pub fn forward_links(&self, slug: &str) -> Result<Vec<String>, WikiError> {
        let prefix = format!("{}\x00", slug);

        let rtx = self
            .db
            .begin_read()
            .map_err(|e| WikiError::LinkGraph(e.to_string()))?;
        let table = rtx
            .open_table(OUTLINKS)
            .map_err(|e| WikiError::LinkGraph(e.to_string()))?;

        let results = table
            .range(prefix.as_str()..)
            .map_err(|e| WikiError::LinkGraph(e.to_string()))?
            .filter_map(|r| r.ok())
            .take_while(|(k, _)| k.value().starts_with(prefix.as_str()))
            .filter_map(|(k, _)| {
                let key = k.value();
                key.find('\x00').map(|pos| key[pos + 1..].to_owned())
            })
            .collect();

        Ok(results)
    }

    /// Returns the slugs of all articles that contain a wikilink to `target`.
    ///
    /// Full-scan O(n) — corpus is small. Phase 7 adds a reverse-index table.
    pub fn backlinks(&self, target: &str) -> Result<Vec<String>, WikiError> {
        let suffix = format!("\x00{}", target);

        let rtx = self
            .db
            .begin_read()
            .map_err(|e| WikiError::LinkGraph(e.to_string()))?;
        let table = rtx
            .open_table(OUTLINKS)
            .map_err(|e| WikiError::LinkGraph(e.to_string()))?;

        let results = table
            .iter()
            .map_err(|e| WikiError::LinkGraph(e.to_string()))?
            .filter_map(|r| r.ok())
            .filter_map(|(k, _)| {
                let key = k.value();
                if key.ends_with(suffix.as_str()) {
                    key.find('\x00').map(|pos| key[..pos].to_owned())
                } else {
                    None
                }
            })
            .collect();

        Ok(results)
    }

    /// Store the blake3 hash of `body` keyed by `(slug, revision_sha)`.
    pub fn record_hash(
        &self,
        slug: &str,
        revision_sha: &str,
        body: &[u8],
    ) -> Result<(), WikiError> {
        let hash = blake3::hash(body);
        let hash_bytes: &[u8] = hash.as_bytes();
        let key = format!("{}\x00{}", slug, revision_sha);

        let wtx = self
            .db
            .begin_write()
            .map_err(|e| WikiError::LinkGraph(e.to_string()))?;
        {
            let mut table = wtx
                .open_table(HASHES)
                .map_err(|e| WikiError::LinkGraph(e.to_string()))?;
            table
                .insert(key.as_str(), hash_bytes)
                .map_err(|e| WikiError::LinkGraph(e.to_string()))?;
        }
        wtx.commit()
            .map_err(|e| WikiError::LinkGraph(e.to_string()))?;
        Ok(())
    }

    /// Look up a `(slug, revision_sha)` pair by its blake3 hash.
    ///
    /// Linear scan — Phase 7 lights up an efficient reverse index path.
    pub fn lookup_by_hash(&self, hash: &[u8; 32]) -> Result<Option<(String, String)>, WikiError> {
        let rtx = self
            .db
            .begin_read()
            .map_err(|e| WikiError::LinkGraph(e.to_string()))?;
        let table = rtx
            .open_table(HASHES)
            .map_err(|e| WikiError::LinkGraph(e.to_string()))?;

        let result = table
            .iter()
            .map_err(|e| WikiError::LinkGraph(e.to_string()))?
            .filter_map(|r| r.ok())
            .find_map(|(k, v)| {
                if v.value() == hash.as_slice() {
                    let key = k.value();
                    key.find('\x00')
                        .map(|pos| (key[..pos].to_owned(), key[pos + 1..].to_owned()))
                } else {
                    None
                }
            });

        Ok(result)
    }

    /// Rebuild the claim-dependency edges for `slug`. Deletes every edge
    /// originating from this slug's claims, then inserts one edge per
    /// `depends_on` reference. A bare reference (same-file) is namespaced
    /// with `slug`; a reference already containing `:` is global.
    pub fn rebuild_claims_for_slug(&self, slug: &str, claims: &[Claim]) -> Result<(), WikiError> {
        let from_prefix = format!("{}:", slug);

        let wtx = self
            .db
            .begin_write()
            .map_err(|e| WikiError::LinkGraph(e.to_string()))?;
        {
            let mut table = wtx
                .open_table(CLAIM_DEPS)
                .map_err(|e| WikiError::LinkGraph(e.to_string()))?;

            // Remove existing edges whose `from` claim belongs to this slug.
            let to_remove: Vec<String> = table
                .iter()
                .map_err(|e| WikiError::LinkGraph(e.to_string()))?
                .filter_map(|r| r.ok())
                .filter_map(|(k, _)| {
                    let key = k.value();
                    if key.starts_with(from_prefix.as_str()) {
                        Some(key.to_owned())
                    } else {
                        None
                    }
                })
                .collect();
            for key in &to_remove {
                table
                    .remove(key.as_str())
                    .map_err(|e| WikiError::LinkGraph(e.to_string()))?;
            }

            for claim in claims {
                for dep in &claim.depends_on {
                    let to = normalise_claim_ref(dep, slug);
                    let key = format!("{}\x00{}", claim.id, to);
                    table
                        .insert(key.as_str(), 0u8)
                        .map_err(|e| WikiError::LinkGraph(e.to_string()))?;
                }
            }
        }
        wtx.commit()
            .map_err(|e| WikiError::LinkGraph(e.to_string()))?;
        Ok(())
    }

    /// The claims that `claim_id` declares a `depends_on` edge to.
    pub fn claim_depends_on(&self, claim_id: &str) -> Result<Vec<String>, WikiError> {
        let prefix = format!("{}\x00", claim_id);

        let rtx = self
            .db
            .begin_read()
            .map_err(|e| WikiError::LinkGraph(e.to_string()))?;
        let table = rtx
            .open_table(CLAIM_DEPS)
            .map_err(|e| WikiError::LinkGraph(e.to_string()))?;

        let results = table
            .iter()
            .map_err(|e| WikiError::LinkGraph(e.to_string()))?
            .filter_map(|r| r.ok())
            .filter_map(|(k, _)| {
                let key = k.value();
                if key.starts_with(prefix.as_str()) {
                    key.find('\x00').map(|pos| key[pos + 1..].to_owned())
                } else {
                    None
                }
            })
            .collect();

        Ok(results)
    }

    /// Record a citation for `cite_id` (e.g. `"slug:fn-1"`). The status
    /// defaults to `"unknown"`. Phase 9 URL validator overwrites status.
    pub fn record_citation(&self, cite_id: &str, url: &str, title: &str) -> Result<(), WikiError> {
        let blob = format!(
            r#"{{"url":{},"title":{},"status":"unknown"}}"#,
            serde_json::to_string(url).unwrap_or_default(),
            serde_json::to_string(title).unwrap_or_default(),
        );
        let wtx = self
            .db
            .begin_write()
            .map_err(|e| WikiError::LinkGraph(e.to_string()))?;
        {
            let mut table = wtx
                .open_table(CITATIONS)
                .map_err(|e| WikiError::LinkGraph(e.to_string()))?;
            table
                .insert(cite_id, blob.as_str())
                .map_err(|e| WikiError::LinkGraph(e.to_string()))?;
        }
        wtx.commit()
            .map_err(|e| WikiError::LinkGraph(e.to_string()))?;
        Ok(())
    }

    /// Look up the JSON blob for `cite_id`. Returns `None` if not recorded.
    pub fn lookup_citation(&self, cite_id: &str) -> Result<Option<String>, WikiError> {
        let rtx = self
            .db
            .begin_read()
            .map_err(|e| WikiError::LinkGraph(e.to_string()))?;
        let table = rtx
            .open_table(CITATIONS)
            .map_err(|e| WikiError::LinkGraph(e.to_string()))?;
        let result = table
            .get(cite_id)
            .map_err(|e| WikiError::LinkGraph(e.to_string()))?
            .map(|v| v.value().to_owned());
        Ok(result)
    }

    /// Return just the status field for `cite_id`: `"fresh"`, `"stale"`, or `"unknown"`.
    pub fn citation_status(&self, cite_id: &str) -> &'static str {
        let Ok(Some(blob)) = self.lookup_citation(cite_id) else {
            return "unknown";
        };
        if blob.contains(r#""status":"fresh""#) {
            "fresh"
        } else if blob.contains(r#""status":"stale""#) {
            "stale"
        } else {
            "unknown"
        }
    }

    /// Phase 11 — return all citation records for a given slug as JSON-blob strings.
    /// The `asof` parameter is reserved for future date-filtering (currently ignored).
    pub fn citations_for_slug(&self, slug: &str, _asof: Option<&str>) -> Vec<(String, String)> {
        let Ok(rtx) = self.db.begin_read() else {
            return vec![];
        };
        let Ok(table) = rtx.open_table(CITATIONS) else {
            return vec![];
        };
        let prefix = format!("{}:", slug);
        let Ok(range) = table.range(prefix.as_str()..) else {
            return vec![];
        };
        range
            .filter_map(|r| r.ok())
            .take_while(|(k, _)| k.value().starts_with(prefix.as_str()))
            .map(|(k, v)| (k.value().to_owned(), v.value().to_owned()))
            .collect()
    }

    /// The claims that declare a `depends_on` edge **to** `claim_id` — the
    /// reverse direction, `cited_by` in the claim graph. Full-scan O(n).
    pub fn claim_dependents(&self, claim_id: &str) -> Result<Vec<String>, WikiError> {
        let suffix = format!("\x00{}", claim_id);

        let rtx = self
            .db
            .begin_read()
            .map_err(|e| WikiError::LinkGraph(e.to_string()))?;
        let table = rtx
            .open_table(CLAIM_DEPS)
            .map_err(|e| WikiError::LinkGraph(e.to_string()))?;

        let results = table
            .iter()
            .map_err(|e| WikiError::LinkGraph(e.to_string()))?
            .filter_map(|r| r.ok())
            .filter_map(|(k, _)| {
                let key = k.value();
                if key.ends_with(suffix.as_str()) {
                    key.find('\x00').map(|pos| key[..pos].to_owned())
                } else {
                    None
                }
            })
            .collect();

        Ok(results)
    }
}

/// Namespace a `depends_on` reference to its global form (convention §7).
/// A reference already containing `:` is global; a bare one is same-file.
fn normalise_claim_ref(dep: &str, slug: &str) -> String {
    if dep.contains(':') {
        dep.to_string()
    } else {
        format!("{}:{}", slug, dep)
    }
}

/// Parse `[[target]]` wikilinks from Markdown body. Returns the slug form of
/// each target: lowercased, spaces replaced with hyphens, anchor/alias stripped.
pub(crate) fn parse_wikilinks(body: &str) -> Vec<String> {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"\[\[([^\]|#\[]+)").unwrap());
    // Strip code first so `[[wikilink]]` shown as *example syntax* in fenced or
    // inline code is not treated as a real link (also keeps such examples out of
    // the live link graph).
    let cleaned = strip_code_spans(body);
    re.captures_iter(&cleaned)
        .map(|cap| cap[1].trim().to_lowercase().replace(' ', "-"))
        .filter(|s| !s.is_empty())
        .collect()
}

/// Remove fenced code blocks (lines toggled by a ``` ``` ``` or `~~~` fence) and
/// inline code spans (`` `…` ``) from `body`, so wikilink parsing ignores
/// example syntax. Fence lines and inline-code contents are dropped; everything
/// else is preserved line-for-line.
fn strip_code_spans(body: &str) -> String {
    let mut out = String::with_capacity(body.len());
    let mut in_fence = false;
    for line in body.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            continue; // drop the fence line itself
        }
        if in_fence {
            continue; // drop fenced content
        }
        // Drop inline `…` spans on this line.
        let mut in_code = false;
        for ch in line.chars() {
            if ch == '`' {
                in_code = !in_code;
                continue;
            }
            if !in_code {
                out.push(ch);
            }
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn graph() -> Arc<LinkGraph> {
        LinkGraph::for_testing()
    }

    #[test]
    fn parse_simple_wikilink() {
        let links = parse_wikilinks("See [[foo-bar]] and [[baz]].");
        assert_eq!(links, vec!["foo-bar", "baz"]);
    }

    #[test]
    fn parse_wikilink_with_alias() {
        let links = parse_wikilinks("See [[foo-bar|display text]].");
        assert_eq!(links, vec!["foo-bar"]);
    }

    #[test]
    fn parse_wikilink_with_anchor() {
        let links = parse_wikilinks("See [[foo-bar#section]].");
        assert_eq!(links, vec!["foo-bar"]);
    }

    #[test]
    fn ignores_wikilinks_in_inline_code() {
        // The `[[example]]` is shown as inline-code syntax; only the real link counts.
        let links = parse_wikilinks("Write `[[example]]` to link to [[real-page]].");
        assert_eq!(links, vec!["real-page"]);
    }

    #[test]
    fn ignores_wikilinks_in_fenced_code() {
        let body = "Intro [[real-one]].\n\n```\nuse [[wikilink]] like [[slug]]\n```\n\nOutro [[real-two]].\n";
        let links = parse_wikilinks(body);
        assert_eq!(links, vec!["real-one", "real-two"]);
    }

    #[test]
    fn backlinks_empty_when_no_links() {
        let g = graph();
        let bl = g.backlinks("target").unwrap();
        assert!(bl.is_empty());
    }

    #[test]
    fn backlinks_found_after_rebuild() {
        let g = graph();
        g.rebuild_for_slug("article-a", "See [[target]].").unwrap();
        let bl = g.backlinks("target").unwrap();
        assert_eq!(bl, vec!["article-a"]);
    }

    #[test]
    fn backlinks_cleared_after_rebuild_removes_link() {
        let g = graph();
        g.rebuild_for_slug("article-a", "See [[target]].").unwrap();
        g.rebuild_for_slug("article-a", "No links here.").unwrap();
        let bl = g.backlinks("target").unwrap();
        assert!(bl.is_empty());
    }

    #[test]
    fn hash_round_trip() {
        let g = graph();
        let body = b"Hello, world!";
        g.record_hash("my-slug", "abc123", body).unwrap();
        let expected = blake3::hash(body);
        let result = g.lookup_by_hash(expected.as_bytes()).unwrap();
        assert_eq!(result, Some(("my-slug".to_owned(), "abc123".to_owned())));
    }

    #[test]
    fn claim_deps_round_trip() {
        let g = graph();
        let ex = crate::claim::extract_claims(
            "<!--claim id=a confidence=established cites=[x] depends_on=[b]-->t<!--/claim-->",
            "topic-one",
        );
        g.rebuild_claims_for_slug("topic-one", &ex.claims).unwrap();
        assert_eq!(
            g.claim_depends_on("topic-one:a").unwrap(),
            vec!["topic-one:b"]
        );
        // The reverse direction — `cited_by`.
        assert_eq!(
            g.claim_dependents("topic-one:b").unwrap(),
            vec!["topic-one:a"]
        );
    }

    #[test]
    fn claim_deps_global_ref_is_preserved() {
        let g = graph();
        let ex = crate::claim::extract_claims(
            "<!--claim id=a confidence=established cites=[x] \
             depends_on=[other-topic:c]-->t<!--/claim-->",
            "topic-one",
        );
        g.rebuild_claims_for_slug("topic-one", &ex.claims).unwrap();
        assert_eq!(
            g.claim_depends_on("topic-one:a").unwrap(),
            vec!["other-topic:c"]
        );
    }

    #[test]
    fn claim_deps_cleared_when_rebuilt_without_the_edge() {
        let g = graph();
        let with_dep = crate::claim::extract_claims(
            "<!--claim id=a confidence=established cites=[x] depends_on=[b]-->t<!--/claim-->",
            "t",
        );
        g.rebuild_claims_for_slug("t", &with_dep.claims).unwrap();
        let without_dep = crate::claim::extract_claims(
            "<!--claim id=a confidence=structural cites=[]-->t<!--/claim-->",
            "t",
        );
        g.rebuild_claims_for_slug("t", &without_dep.claims).unwrap();
        assert!(g.claim_depends_on("t:a").unwrap().is_empty());
        assert!(g.claim_dependents("t:b").unwrap().is_empty());
    }
}
