// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

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
    "moonshot-index: active (in-memory inverted index)"
}
