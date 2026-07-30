//! moonshot-index — sovereign local search index.
//!
//! Two cooperating layers, per `BRIEF-workplace-comprehensive-search`:
//!
//! - [`TrigramIndex`] — the substring **correctness floor**. GUARANTEE: if a
//!   case-insensitive query of ≥3 bytes occurs as a substring of a document's
//!   filename OR content, [`TrigramIndex::search`] returns that document.
//!   A token/word index *cannot* make this promise — which is exactly why
//!   Spotlight / Microsoft 365 / EasyFind silently miss files whose name or body
//!   contains the query. Queries <3 bytes fail OPEN to a full scan, so a short
//!   query never produces a false "not found". This is the anti-Spotlight core.
//!
//! - [`InvertedIndex`] — token/word index; the future BM25-style *ranked* layer
//!   that orders the correctness floor's matches. Kept as-is for now.
//!
//! v1 is pure-`std`, zero dependencies (sovereign, offline). A vendored Tantivy
//! ranked layer and `gix`-based git-history awareness are later phases; see the BRIEF.

use std::collections::{HashMap, HashSet};

pub struct Document {
    pub id: String,
    pub title: String,
    pub body: String,
}

/// In-memory inverted index for token/component search.
/// Sovereign replacement for tantivy.
pub struct InvertedIndex {
    index: HashMap<String, Vec<String>>,
    docs: HashMap<String, Document>,
}

const STOP_WORDS: &[&str] = &[
    "a", "an", "and", "are", "as", "at", "be", "been", "being", "by", "for", "from", "in", "is",
    "it", "its", "of", "on", "or", "the", "this", "that", "these", "those", "to", "was", "were",
    "with",
];

fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| s.len() > 1 && !STOP_WORDS.contains(s))
        .map(|s| s.to_string())
        .collect()
}

impl InvertedIndex {
    pub fn new() -> Self {
        InvertedIndex {
            index: HashMap::new(),
            docs: HashMap::new(),
        }
    }

    pub fn insert(&mut self, doc: Document) {
        self.remove(&doc.id);
        let id = doc.id.clone();
        let terms: Vec<String> = tokenize(&doc.title)
            .into_iter()
            .chain(tokenize(&doc.body))
            .collect();
        for term in terms {
            self.index.entry(term).or_default().push(id.clone());
        }
        self.docs.insert(id, doc);
    }

    pub fn remove(&mut self, id: &str) {
        if self.docs.remove(id).is_some() {
            self.index.retain(|_, ids| {
                ids.retain(|i| i != id);
                !ids.is_empty()
            });
        }
    }

    /// AND-match: all query terms must appear; results ranked by hit count.
    pub fn search(&self, query: &str) -> Vec<&Document> {
        let terms: HashSet<String> = tokenize(query).into_iter().collect();
        if terms.is_empty() {
            return Vec::new();
        }
        let term_count = terms.len();
        let mut hits: HashMap<&str, usize> = HashMap::new();
        for term in &terms {
            if let Some(ids) = self.index.get(term) {
                for id in ids {
                    if self.docs.contains_key(id.as_str()) {
                        *hits.entry(id.as_str()).or_default() += 1;
                    }
                }
            }
        }
        let mut ranked: Vec<(&str, usize)> = hits
            .into_iter()
            .filter(|(_, count)| *count >= term_count)
            .collect();
        ranked.sort_by_key(|b| std::cmp::Reverse(b.1));
        ranked
            .into_iter()
            .filter_map(|(id, _)| self.docs.get(id))
            .collect()
    }

    pub fn len(&self) -> usize {
        self.docs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.docs.is_empty()
    }
}

impl Default for InvertedIndex {
    fn default() -> Self {
        Self::new()
    }
}

pub fn system_status() -> &'static str {
    "moonshot-index: active (trigram substring floor + inverted index)"
}

// ─────────────────────────────────────────────────────────────────────────────
// Trigram substring index — the correctness floor (anti-Spotlight guarantee)
// ─────────────────────────────────────────────────────────────────────────────

/// Where a query matched within a document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchKind {
    /// Matched in the filename / path only.
    Filename,
    /// Matched in the file contents only.
    Content,
    /// Matched in both the filename and the contents.
    Both,
}

/// One search result from [`TrigramIndex::search`].
#[derive(Debug, Clone)]
pub struct SearchHit {
    /// Caller-supplied document id (e.g. a repo-relative path).
    pub id: String,
    /// The document's filename / path, as supplied (original case).
    pub name: String,
    /// Where the query matched.
    pub kind: MatchKind,
    /// A short excerpt around the first content match (lowercased in v1; empty
    /// for filename-only hits).
    pub snippet: String,
    /// Non-overlapping occurrence count across name + content.
    pub occurrences: usize,
}

/// Outcome of an [`TrigramIndex::index_dir`] pass.
#[derive(Debug, Default, Clone, Copy)]
pub struct IndexStats {
    /// Files indexed (filename always; content unless skipped).
    pub files: usize,
    /// Files whose body was skipped (over the size cap or unreadable) — their
    /// filename is still indexed and searchable.
    pub content_skipped: usize,
}

struct TriDoc {
    id: String,
    name: String,
    name_lc: String,
    /// Empty when the body exceeded the size cap and was skipped — the filename is
    /// still fully indexed and searchable (anti-Spotlight rule: never drop a name).
    content_lc: String,
}

/// Default per-file content cap (bytes). Files larger than this still have their
/// filename indexed; only their body is skipped. 5 MiB.
pub const DEFAULT_MAX_CONTENT_BYTES: usize = 5 * 1024 * 1024;

/// Substring correctness floor (Russ Cox / Zoekt trigram model). See module docs
/// for the guarantee.
pub struct TrigramIndex {
    docs: Vec<TriDoc>,
    /// trigram -> ascending, deduped doc indices.
    postings: HashMap<[u8; 3], Vec<u32>>,
    max_content_bytes: usize,
}

impl TrigramIndex {
    pub fn new() -> Self {
        Self::with_max_content_bytes(DEFAULT_MAX_CONTENT_BYTES)
    }

    pub fn with_max_content_bytes(max_content_bytes: usize) -> Self {
        TrigramIndex {
            docs: Vec::new(),
            postings: HashMap::new(),
            max_content_bytes,
        }
    }

    pub fn len(&self) -> usize {
        self.docs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.docs.is_empty()
    }

    /// Index one document. `name` is the filename/path (always fully indexed);
    /// `content` is the body (indexed unless it exceeds the size cap).
    pub fn add_document(&mut self, id: impl Into<String>, name: impl Into<String>, content: &str) {
        let id = id.into();
        let name = name.into();
        let name_lc = name.to_lowercase();
        let within_cap = content.len() <= self.max_content_bytes;
        let content_lc = if within_cap {
            content.to_lowercase()
        } else {
            String::new()
        };

        let doc_idx = self.docs.len() as u32;

        // Collect the doc's distinct trigrams (from name, and content if indexed),
        // then append this doc index to each — postings stay ascending & deduped
        // because doc indices are assigned monotonically.
        let mut tris: HashSet<[u8; 3]> = HashSet::new();
        each_trigram(&name_lc, |t| {
            tris.insert(t);
        });
        if within_cap {
            each_trigram(&content_lc, |t| {
                tris.insert(t);
            });
        }
        for t in tris {
            self.postings.entry(t).or_default().push(doc_idx);
        }

        self.docs.push(TriDoc {
            id,
            name,
            name_lc,
            content_lc,
        });
    }

    /// Recursively index every regular file under `root`. The document id and name
    /// are the path *relative to `root`*, so both filenames and directory paths are
    /// searchable. File bytes are read as text via lossy UTF-8 (never re-guessing
    /// type — the anti-Spotlight rule); files over the size cap or unreadable have
    /// only their name indexed. Symlinks are skipped (avoids cycles). Directories
    /// named `.git` are skipped (object DB is handled by the future git layer).
    pub fn index_dir(&mut self, root: impl AsRef<std::path::Path>) -> std::io::Result<IndexStats> {
        let root = root.as_ref();
        let mut stats = IndexStats::default();
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            let entries = match std::fs::read_dir(&dir) {
                Ok(e) => e,
                Err(_) => continue, // unreadable dir — skip, never abort the whole pass
            };
            for entry in entries.flatten() {
                let path = entry.path();
                let ft = match entry.file_type() {
                    Ok(ft) => ft,
                    Err(_) => continue,
                };
                if ft.is_symlink() {
                    continue;
                }
                if ft.is_dir() {
                    if path.file_name().and_then(|n| n.to_str()) == Some(".git") {
                        continue;
                    }
                    stack.push(path);
                } else if ft.is_file() {
                    let rel = path
                        .strip_prefix(root)
                        .unwrap_or(&path)
                        .to_string_lossy()
                        .into_owned();
                    let over_cap = entry
                        .metadata()
                        .map(|m| m.len() as usize > self.max_content_bytes)
                        .unwrap_or(true);
                    let (content, skipped) = if over_cap {
                        (String::new(), true)
                    } else {
                        match std::fs::read(&path) {
                            Ok(bytes) => (String::from_utf8_lossy(&bytes).into_owned(), false),
                            Err(_) => (String::new(), true),
                        }
                    };
                    self.add_document(rel.clone(), rel, &content);
                    stats.files += 1;
                    if skipped {
                        stats.content_skipped += 1;
                    }
                }
            }
        }
        Ok(stats)
    }

    /// Search for `query` as a case-insensitive substring. Honours the guarantee
    /// in the module docs. Results: filename/both matches before content-only,
    /// then by occurrence count, then by name.
    pub fn search(&self, query: &str) -> Vec<SearchHit> {
        let q = query.to_lowercase();
        if q.is_empty() {
            return Vec::new();
        }

        // Candidate generation. For <3 bytes there are no trigrams, so fail OPEN
        // to a full scan rather than risk a false "not found".
        let candidates: Vec<u32> = if q.len() < 3 {
            (0..self.docs.len() as u32).collect()
        } else {
            self.candidates_for(&q)
        };

        let mut hits: Vec<SearchHit> = Vec::new();
        for idx in candidates {
            let doc = &self.docs[idx as usize];
            // Verify — the trigram filter is a superset; confirm the real substring.
            let in_name = doc.name_lc.contains(&q);
            let in_content = doc.content_lc.contains(&q);
            if !in_name && !in_content {
                continue;
            }
            let kind = match (in_name, in_content) {
                (true, true) => MatchKind::Both,
                (true, false) => MatchKind::Filename,
                (false, true) => MatchKind::Content,
                (false, false) => unreachable!(),
            };
            let occurrences =
                count_occurrences(&doc.name_lc, &q) + count_occurrences(&doc.content_lc, &q);
            let snippet = if in_content {
                make_snippet(&doc.content_lc, &q)
            } else {
                String::new()
            };
            hits.push(SearchHit {
                id: doc.id.clone(),
                name: doc.name.clone(),
                kind,
                snippet,
                occurrences,
            });
        }

        hits.sort_by(|a, b| {
            kind_rank(a.kind)
                .cmp(&kind_rank(b.kind))
                .then(b.occurrences.cmp(&a.occurrences))
                .then(a.name.cmp(&b.name))
        });
        hits
    }

    /// Intersect the posting lists of the query's trigrams. A superset of the
    /// documents that actually contain `q` (the caller verifies).
    fn candidates_for(&self, q: &str) -> Vec<u32> {
        let mut qtris: Vec<[u8; 3]> = Vec::new();
        {
            let mut seen: HashSet<[u8; 3]> = HashSet::new();
            each_trigram(q, |t| {
                if seen.insert(t) {
                    qtris.push(t);
                }
            });
        }

        let mut lists: Vec<&Vec<u32>> = Vec::with_capacity(qtris.len());
        for t in &qtris {
            match self.postings.get(t) {
                Some(l) => lists.push(l),
                // A required trigram is absent from the whole corpus → nothing can
                // contain q. (Guarantee preserved: if some doc contained q, this
                // trigram would be present.)
                None => return Vec::new(),
            }
        }
        // Intersect from the shortest list outward.
        lists.sort_by_key(|l| l.len());
        let mut acc: Vec<u32> = lists[0].clone();
        for l in &lists[1..] {
            acc = intersect_sorted(&acc, l);
            if acc.is_empty() {
                break;
            }
        }
        acc
    }
}

impl Default for TrigramIndex {
    fn default() -> Self {
        Self::new()
    }
}

fn kind_rank(k: MatchKind) -> u8 {
    match k {
        MatchKind::Both => 0,
        MatchKind::Filename => 1,
        MatchKind::Content => 2,
    }
}

/// Call `f` with every byte-trigram of `s` (over its UTF-8 bytes). Consistent with
/// the byte-substring check used at verify time, so the guarantee holds for any
/// text, not just ASCII.
fn each_trigram(s: &str, mut f: impl FnMut([u8; 3])) {
    let b = s.as_bytes();
    if b.len() < 3 {
        return;
    }
    for w in b.windows(3) {
        f([w[0], w[1], w[2]]);
    }
}

/// Two-pointer intersection of two ascending, deduped slices.
fn intersect_sorted(a: &[u32], b: &[u32]) -> Vec<u32> {
    let mut out = Vec::new();
    let (mut i, mut j) = (0usize, 0usize);
    while i < a.len() && j < b.len() {
        match a[i].cmp(&b[j]) {
            std::cmp::Ordering::Equal => {
                out.push(a[i]);
                i += 1;
                j += 1;
            }
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
        }
    }
    out
}

/// Count non-overlapping occurrences of `needle` in `hay` (both already lowercased).
fn count_occurrences(hay: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    let mut n = 0;
    let mut start = 0;
    while let Some(pos) = hay[start..].find(needle) {
        n += 1;
        start += pos + needle.len();
    }
    n
}

/// A short excerpt around the first occurrence of `needle` in `hay` (lowercased).
fn make_snippet(hay: &str, needle: &str) -> String {
    const PAD: usize = 32;
    let Some(pos) = hay.find(needle) else {
        return String::new();
    };
    // Clamp to char boundaries so slicing never panics.
    let start = floor_char_boundary(hay, pos.saturating_sub(PAD));
    let end = ceil_char_boundary(hay, (pos + needle.len() + PAD).min(hay.len()));
    let mut s = String::new();
    if start > 0 {
        s.push('…');
    }
    s.push_str(&hay[start..end]);
    if end < hay.len() {
        s.push('…');
    }
    s
}

fn floor_char_boundary(s: &str, mut i: usize) -> usize {
    if i >= s.len() {
        return s.len();
    }
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

fn ceil_char_boundary(s: &str, mut i: usize) -> usize {
    if i >= s.len() {
        return s.len();
    }
    while i < s.len() && !s.is_char_boundary(i) {
        i += 1;
    }
    i
}

// ─────────────────────────────────────────────────────────────────────────────
// Ranked search engine — trigram correctness floor UNIFIED with BM25 ranking
// ─────────────────────────────────────────────────────────────────────────────

/// Hybrid search per the research: the [`TrigramIndex`] substring floor (recall
/// guarantee) unified with a BM25 token ranker (relevance). Multi-word queries are
/// ranked by BM25; every substring/trigram hit for the whole query is ALSO returned
/// and never dropped for the sake of ranking. Filename-term matches are boosted.
/// Pure `std`, zero dependencies — the vendored Tantivy layer is a later phase.
pub struct SearchEngine {
    trigram: TrigramIndex,
    docs: Vec<EngineDoc>,
    id_to_idx: HashMap<String, u32>,
    postings: HashMap<String, Vec<(u32, u32)>>, // term -> [(doc_idx, term_freq)]
    df: HashMap<String, u32>,                   // document frequency per term
    total_len: u64,
    k1: f64,
    b: f64,
    name_boost: f64,
}

struct EngineDoc {
    id: String,
    name: String,
    len: u32, // token count (name + content), for BM25 length normalisation
    name_terms: HashSet<String>,
}

impl SearchEngine {
    pub fn new() -> Self {
        SearchEngine {
            trigram: TrigramIndex::new(),
            docs: Vec::new(),
            id_to_idx: HashMap::new(),
            postings: HashMap::new(),
            df: HashMap::new(),
            total_len: 0,
            k1: 1.2,
            b: 0.75,
            name_boost: 1.0,
        }
    }

    pub fn len(&self) -> usize {
        self.docs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.docs.is_empty()
    }

    /// Index one document into both the substring floor and the BM25 ranker.
    pub fn add_document(&mut self, id: impl Into<String>, name: impl Into<String>, content: &str) {
        let id = id.into();
        let name = name.into();
        let idx = self.docs.len() as u32;

        // Substring floor (owns its own guarantee).
        self.trigram.add_document(id.clone(), name.clone(), content);

        // BM25 token stats.
        let name_tokens = tokenize(&name);
        let content_tokens = tokenize(content);
        let name_terms: HashSet<String> = name_tokens.iter().cloned().collect();
        let mut tf: HashMap<String, u32> = HashMap::new();
        for t in name_tokens.iter().chain(content_tokens.iter()) {
            *tf.entry(t.clone()).or_default() += 1;
        }
        let len = (name_tokens.len() + content_tokens.len()) as u32;
        for (term, f) in tf {
            self.postings
                .entry(term.clone())
                .or_default()
                .push((idx, f));
            *self.df.entry(term).or_default() += 1;
        }
        self.total_len += len as u64;
        self.id_to_idx.insert(id.clone(), idx);
        self.docs.push(EngineDoc {
            id,
            name,
            len,
            name_terms,
        });
    }

    /// Recursively index a directory (delegates to the trigram floor for reading, then
    /// mirrors each file into the BM25 ranker). See [`TrigramIndex::index_dir`].
    pub fn index_dir(&mut self, root: impl AsRef<std::path::Path>) -> std::io::Result<IndexStats> {
        // Re-walk here so both indexes see identical docs with identical ids.
        let root = root.as_ref();
        let mut stats = IndexStats::default();
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            let entries = match std::fs::read_dir(&dir) {
                Ok(e) => e,
                Err(_) => continue,
            };
            for entry in entries.flatten() {
                let path = entry.path();
                let ft = match entry.file_type() {
                    Ok(ft) => ft,
                    Err(_) => continue,
                };
                if ft.is_symlink() {
                    continue;
                }
                if ft.is_dir() {
                    if path.file_name().and_then(|n| n.to_str()) == Some(".git") {
                        continue;
                    }
                    stack.push(path);
                } else if ft.is_file() {
                    let rel = path
                        .strip_prefix(root)
                        .unwrap_or(&path)
                        .to_string_lossy()
                        .into_owned();
                    let over_cap = entry
                        .metadata()
                        .map(|m| m.len() > 5 * 1024 * 1024)
                        .unwrap_or(true);
                    let (content, skipped) = if over_cap {
                        (String::new(), true)
                    } else {
                        match std::fs::read(&path) {
                            Ok(bytes) => (String::from_utf8_lossy(&bytes).into_owned(), false),
                            Err(_) => (String::new(), true),
                        }
                    };
                    self.add_document(rel.clone(), rel, &content);
                    stats.files += 1;
                    if skipped {
                        stats.content_skipped += 1;
                    }
                }
            }
        }
        Ok(stats)
    }

    /// Ranked search. Multi-word queries rank by BM25 + filename boost; every substring
    /// hit for the whole query is included regardless of BM25 (the no-silent-miss
    /// guarantee survives ranking).
    pub fn query(&self, q: &str) -> Vec<SearchHit> {
        if q.trim().is_empty() {
            return Vec::new();
        }
        let n = self.docs.len().max(1) as f64;
        let avgdl = (self.total_len as f64 / n).max(1.0);

        // BM25 accumulation over query terms.
        let mut scores: HashMap<u32, f64> = HashMap::new();
        for term in tokenize(q) {
            let Some(plist) = self.postings.get(&term) else {
                continue;
            };
            let df = *self.df.get(&term).unwrap_or(&1) as f64;
            let idf = (((n - df + 0.5) / (df + 0.5)) + 1.0).ln();
            for (doc, tf) in plist {
                let tf = *tf as f64;
                let dl = self.docs[*doc as usize].len as f64;
                let denom = tf + self.k1 * (1.0 - self.b + self.b * dl / avgdl);
                let mut s = idf * (tf * (self.k1 + 1.0)) / denom;
                if self.docs[*doc as usize].name_terms.contains(&term) {
                    s += self.name_boost * idf; // filename hits matter more
                }
                *scores.entry(*doc).or_default() += s;
            }
        }

        // Substring floor for the whole query — these MUST all appear.
        let sub_hits = self.trigram.search(q);
        let mut sub_by_idx: HashMap<u32, SearchHit> = HashMap::new();
        for h in sub_hits {
            if let Some(&idx) = self.id_to_idx.get(&h.id) {
                sub_by_idx.insert(idx, h);
            }
        }

        // Merge: union of BM25-scored and substring-hit docs.
        let mut idxs: HashSet<u32> = scores.keys().copied().collect();
        idxs.extend(sub_by_idx.keys().copied());

        // A small floor so a substring-only hit (BM25 = 0, e.g. a mid-word match) still
        // outranks nothing and is never dropped.
        const SUBSTRING_FLOOR: f64 = 0.01;

        let mut ranked: Vec<(f64, SearchHit)> = Vec::with_capacity(idxs.len());
        for idx in idxs {
            let bm25 = scores.get(&idx).copied().unwrap_or(0.0);
            let (hit, score) = if let Some(sh) = sub_by_idx.get(&idx) {
                // Real substring hit — keep its precise kind/snippet.
                let score = bm25 + SUBSTRING_FLOOR + kind_score_bonus(sh.kind);
                (sh.clone(), score)
            } else {
                // BM25 token match only (terms present but not as one contiguous substring).
                let doc = &self.docs[idx as usize];
                let kind = if tokenize(q).iter().any(|t| doc.name_terms.contains(t)) {
                    MatchKind::Filename
                } else {
                    MatchKind::Content
                };
                let hit = SearchHit {
                    id: doc.id.clone(),
                    name: doc.name.clone(),
                    kind,
                    snippet: String::new(),
                    occurrences: 0,
                };
                (hit, bm25 + kind_score_bonus(kind))
            };
            ranked.push((score, hit));
        }

        ranked.sort_by(|a, b| {
            b.0.partial_cmp(&a.0)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.1.name.cmp(&b.1.name))
        });
        ranked.into_iter().map(|(_, h)| h).collect()
    }
}

impl Default for SearchEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// A small ranking nudge so filename/both matches edge out content-only at equal relevance.
fn kind_score_bonus(k: MatchKind) -> f64 {
    match k {
        MatchKind::Both => 0.5,
        MatchKind::Filename => 0.4,
        MatchKind::Content => 0.0,
    }
}

#[cfg(test)]
mod engine_tests {
    use super::*;

    fn ids(hits: &[SearchHit]) -> Vec<&str> {
        hits.iter().map(|h| h.id.as_str()).collect()
    }

    #[test]
    fn multi_term_ranks_more_relevant_higher() {
        let mut e = SearchEngine::new();
        e.add_document("a", "a.md", "revenue revenue revenue quarterly revenue");
        e.add_document("b", "b.md", "revenue mentioned once, mostly about weather");
        e.add_document("c", "c.md", "nothing relevant here at all");
        let hits = e.query("quarterly revenue");
        // 'a' (both terms, high tf) must outrank 'b' (one term, low tf); 'c' absent.
        assert_eq!(hits[0].id, "a");
        assert!(ids(&hits).contains(&"b"));
        assert!(!ids(&hits).contains(&"c"));
    }

    #[test]
    fn substring_only_hit_survives_ranking() {
        // 'arfoob' is a mid-word substring BM25's tokenizer can't see — the trigram floor
        // must still surface it through the engine (the guarantee survives ranking).
        let mut e = SearchEngine::new();
        e.add_document("x", "x.md", "barfoobaz");
        e.add_document("y", "y.md", "unrelated content about revenue");
        let hits = e.query("arfoob");
        assert!(
            ids(&hits).contains(&"x"),
            "mid-word substring must survive the ranker"
        );
    }

    #[test]
    fn filename_term_is_boosted_over_content_only() {
        let mut e = SearchEngine::new();
        e.add_document("in-name", "invoice.md", "some body text");
        e.add_document(
            "in-body",
            "notes.md",
            "this mentions invoice in the body once",
        );
        let hits = e.query("invoice");
        assert_eq!(hits[0].id, "in-name", "filename match should rank first");
    }

    #[test]
    fn empty_query_returns_nothing() {
        let mut e = SearchEngine::new();
        e.add_document("a", "a.md", "hello");
        assert!(e.query("").is_empty());
        assert!(e.query("   ").is_empty());
    }
}

#[cfg(test)]
mod trigram_tests {
    use super::*;

    fn idx(docs: &[(&str, &str, &str)]) -> TrigramIndex {
        let mut t = TrigramIndex::new();
        for (id, name, content) in docs {
            t.add_document(*id, *name, content);
        }
        t
    }

    fn ids(hits: &[SearchHit]) -> Vec<&str> {
        hits.iter().map(|h| h.id.as_str()).collect()
    }

    #[test]
    fn filename_substring_is_always_found() {
        // The exact anti-Spotlight case: the term is in the FILENAME.
        let t = idx(&[("1", "report_foo_v2.md", "unrelated body text")]);
        let hits = t.search("foo");
        assert_eq!(ids(&hits), vec!["1"]);
        assert_eq!(hits[0].kind, MatchKind::Filename);
    }

    #[test]
    fn content_substring_is_found() {
        let t = idx(&[("1", "notes.md", "the quarterly revenue rose")]);
        let hits = t.search("revenue");
        assert_eq!(ids(&hits), vec!["1"]);
        assert_eq!(hits[0].kind, MatchKind::Content);
        assert!(hits[0].snippet.contains("revenue"));
    }

    #[test]
    fn match_in_both_is_reported_as_both() {
        let t = idx(&[("1", "revenue.md", "revenue figures")]);
        assert_eq!(t.search("revenue")[0].kind, MatchKind::Both);
    }

    #[test]
    fn case_insensitive() {
        let t = idx(&[("1", "Notes.md", "Quarterly REVENUE")]);
        assert_eq!(ids(&t.search("revenue")), vec!["1"]);
        assert_eq!(ids(&t.search("REVENUE")), vec!["1"]);
        assert_eq!(ids(&t.search("ReVeNuE")), vec!["1"]);
    }

    #[test]
    fn midword_substring_the_tokenizer_killer() {
        // A token index splits on boundaries and would MISS this. Trigram must not.
        let t = idx(&[("1", "x.md", "barfoobaz")]);
        assert_eq!(ids(&t.search("arfoob")), vec!["1"]);
        assert_eq!(ids(&t.search("foo")), vec!["1"]);
    }

    #[test]
    fn short_query_fails_open_and_still_finds() {
        // <3 bytes → no trigrams → full-scan fallback, never a false miss.
        let t = idx(&[("1", "foo.md", ""), ("2", "bar.md", "")]);
        assert_eq!(ids(&t.search("fo")), vec!["1"]);
        assert_eq!(ids(&t.search("b")), vec!["2"]);
    }

    #[test]
    fn absent_string_returns_empty_but_only_when_truly_absent() {
        let t = idx(&[("1", "foo.md", "hello world")]);
        assert!(t.search("zzz").is_empty());
        assert!(t.search("worlds").is_empty()); // superstring not present
    }

    #[test]
    fn oversized_content_still_indexes_filename() {
        let mut t = TrigramIndex::with_max_content_bytes(8);
        t.add_document(
            "1",
            "budget_forecast.md",
            "this body is definitely longer than eight bytes",
        );
        // Content was skipped (over cap) but the filename must still be findable.
        assert_eq!(ids(&t.search("forecast")), vec!["1"]);
        // And the skipped body is NOT falsely matched.
        assert!(t.search("definitely").is_empty());
    }

    #[test]
    fn ranking_puts_filename_and_both_ahead_of_content_only() {
        let t = idx(&[
            ("content-only", "a.md", "mentions invoice here"),
            ("in-name", "invoice.md", "nothing relevant"),
            ("both", "invoice_list.md", "another invoice line"),
        ]);
        let hits = t.search("invoice");
        let order = ids(&hits);
        // "both" (kind Both) first, then "in-name" (Filename), then content-only.
        assert_eq!(order, vec!["both", "in-name", "content-only"]);
    }

    #[test]
    fn index_dir_finds_real_files_by_name_and_content() {
        // Index this crate's own src/ — a real directory, no fixtures needed.
        let mut t = TrigramIndex::new();
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/src");
        let stats = t.index_dir(root).unwrap();
        assert!(stats.files >= 1, "should index at least lib.rs");
        // Content search: "TrigramIndex" appears in lib.rs.
        let by_content = t.search("TrigramIndex");
        assert!(by_content.iter().any(|h| h.name.contains("lib.rs")));
        // Filename search: "lib.rs" is itself findable (the anti-Spotlight case).
        assert!(t.search("lib.rs").iter().any(|h| h.name.contains("lib.rs")));
    }

    #[test]
    fn the_guarantee_property_every_substring_is_found() {
        // Property: for a corpus, EVERY >=3-byte substring of any doc's name or
        // content must be returned by a search for it. This is the formal
        // no-silent-miss guarantee expressed as a test.
        let corpus = [
            (
                "1",
                "Proforma_Bencal_SPV1.json",
                "rent roll and cap rate assumptions",
            ),
            (
                "2",
                "letter-of-intent.md",
                "the parties hereby agree to the following terms",
            ),
            ("3", "réunion_notes.md", "café budget für Q3"), // non-ASCII on purpose
        ];
        let t = idx(&corpus);
        for (id, name, content) in corpus.iter() {
            for field in [*name, *content] {
                let bytes = field.as_bytes();
                if bytes.len() < 3 {
                    continue;
                }
                // sample every 3-byte window as a substring query
                for w in bytes.windows(3) {
                    // Only test windows that are valid UTF-8 substrings (char-aligned)
                    if let Ok(sub) = std::str::from_utf8(w) {
                        let found = t.search(sub).iter().any(|h| h.id == *id);
                        assert!(
                            found,
                            "GUARANTEE VIOLATED: substring {:?} of doc {} not found",
                            sub, id
                        );
                    }
                }
            }
        }
    }
}
