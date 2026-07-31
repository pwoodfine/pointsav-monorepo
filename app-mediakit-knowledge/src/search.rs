// SPDX-License-Identifier: FSL-1.1-ALv2
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

//! Full-text search over the content corpus (tantivy 0.24, in-RAM).
//!
//! The index is built once at startup from the English documents and lives for
//! the process lifetime. Queries run against `title` + `body`; only the slug is
//! retrieved — titles and descriptions come back from `ContentIndex`, so the
//! result cards match the category/guide listings exactly.

use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{Field, Schema, Value, STORED, STRING, TEXT};
use tantivy::{doc, Index, IndexReader, TantivyDocument};

use crate::content::{self, ContentIndex};

struct Fields {
    slug: Field,
    title: Field,
    body: Field,
}

/// In-memory full-text index, rebuilt at startup from the content corpus.
pub struct SearchIndex {
    index: Index,
    reader: IndexReader,
    fields: Fields,
}

impl SearchIndex {
    /// Build the index from every English document (bodies loaded from disk).
    pub fn build(content: &ContentIndex) -> tantivy::Result<Self> {
        let mut sb = Schema::builder();
        let slug = sb.add_text_field("slug", STRING | STORED); // exact key, retrieved
        let title = sb.add_text_field("title", TEXT); // tokenised, searched
        let body = sb.add_text_field("body", TEXT); // tokenised, searched
        let schema = sb.build();
        let fields = Fields { slug, title, body };

        let index = Index::create_in_ram(schema);
        let mut writer = index.writer(15_000_000)?;
        for d in content.documents() {
            let body_text = content::load(d).map(|p| p.body_md).unwrap_or_default();
            writer.add_document(doc!(
                slug => d.slug.clone(),
                title => d.title.clone(),
                body => body_text,
            ))?;
        }
        writer.commit()?;
        let reader = index.reader()?;
        Ok(Self {
            index,
            reader,
            fields,
        })
    }

    /// Run a query against title + body; returns up to `limit` slugs, ranked.
    pub fn query(&self, q: &str, limit: usize) -> Vec<String> {
        let q = q.trim();
        if q.is_empty() {
            return Vec::new();
        }
        let searcher = self.reader.searcher();
        let parser = QueryParser::for_index(&self.index, vec![self.fields.title, self.fields.body]);
        let Ok(query) = parser.parse_query(q) else {
            return Vec::new();
        };
        let Ok(top) = searcher.search(&query, &TopDocs::with_limit(limit)) else {
            return Vec::new();
        };
        let mut slugs = Vec::with_capacity(top.len());
        for (_score, addr) in top {
            let doc: TantivyDocument = match searcher.doc(addr) {
                Ok(d) => d,
                Err(_) => continue,
            };
            if let Some(s) = doc.get_first(self.fields.slug).and_then(|v| v.as_str()) {
                slugs.push(s.to_string());
            }
        }
        slugs
    }
}
