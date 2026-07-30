//! Phase 3 Step 3.3 — Atom + JSON Feed 1.1 syndication endpoints.
//!
//! Routes:
//!   GET /feed.atom  — RFC 4287 Atom feed of the 25 most-recently-modified TOPICs.
//!   GET /feed.json  — JSON Feed 1.1 equivalent (hand-rolled; no extra crate).
//!
//! Both feeds filter bilingual sibling files (`*.es.md`) so each TOPIC appears
//! once. Sort order is file mtime descending; mtime is the kernel-reported
//! modification time of the on-disk file. Phase 4 upgrades this to `git log`
//! timestamps once the content directory is a Git repo.
//!
//! `collect_recent_items` is pure / testable — it takes a `&Path` and returns a
//! sorted, capped `Vec<FeedItem>`. The HTTP handlers call it and then delegate
//! to `render_atom` / `render_json_feed`.

use axum::{
    extract::State,
    http::{header, HeaderValue},
    response::{IntoResponse, Json, Response},
};
use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use std::path::Path;
use std::sync::Arc;

use crate::error::WikiError;
use crate::render::parse_page;
use crate::server::AppState;

/// A single feed entry derived from a TOPIC file on disk.
#[derive(Debug, Clone)]
pub struct FeedItem {
    pub slug: String,
    pub title: String,
    /// File mtime. Phase 4 replaces this with the most-recent Git commit
    /// timestamp for the slug once the content directory is a tracked repo.
    pub updated: DateTime<Utc>,
    /// First-paragraph text snippet, capped at ~180 characters.
    pub summary: String,
}

/// Walk `content_dir` recursively (one level of category subdirectories),
/// parse frontmatter via `crate::render::parse_page`, filter out `*.es.md`
/// bilingual siblings and `index` / `_`-prefixed files, sort by file mtime
/// descending, and return the top `limit` items.
///
/// Subdirectory TOPICs get path-qualified slugs (`<category>/<stem>`) so the
/// feed links resolve correctly against the `/wiki/{*slug}` wildcard route.
///
/// IO errors on individual files are silently skipped (consistent with how
/// the index page handles unreadable entries). A parse error on a TOPIC file
/// also causes that file to be skipped — callers see only clean items.
pub async fn collect_recent_items(
    content_dir: &Path,
    limit: usize,
) -> Result<Vec<FeedItem>, WikiError> {
    // Phase 1: collect (path, slug) pairs via a two-level walk.
    let mut candidates: Vec<(std::path::PathBuf, String)> = Vec::new();
    let mut top_entries = tokio::fs::read_dir(content_dir).await?;

    while let Some(entry) = top_entries.next_entry().await? {
        let file_type = match entry.file_type().await {
            Ok(ft) => ft,
            Err(_) => continue,
        };
        let name = entry.file_name();
        let name_str = name.to_string_lossy().to_string();

        if file_type.is_dir() {
            // Descend one level into a category subdirectory.
            let subdir_name = name_str;
            let mut sub_entries = match tokio::fs::read_dir(entry.path()).await {
                Ok(e) => e,
                Err(_) => continue,
            };
            while let Some(sub) = sub_entries.next_entry().await? {
                let sub_name = sub.file_name();
                let sub_str = sub_name.to_string_lossy().to_string();
                let stem = match sub_str.strip_suffix(".md") {
                    Some(s) => s.to_string(),
                    None => continue,
                };
                if stem.ends_with(".es")
                    || stem == "index"
                    || stem == "_index"
                    || stem.starts_with('_')
                {
                    continue;
                }
                candidates.push((sub.path(), format!("{subdir_name}/{stem}")));
            }
        } else {
            // Root-level file.
            let stem = match name_str.strip_suffix(".md") {
                Some(s) => s.to_string(),
                None => continue,
            };
            if stem.ends_with(".es") || stem == "index" || stem == "_index" || stem.starts_with('_')
            {
                continue;
            }
            candidates.push((entry.path(), stem));
        }
    }

    // Phase 2: stat + parse each candidate.
    let mut items: Vec<FeedItem> = Vec::new();
    for (path, slug) in candidates {
        let meta = match tokio::fs::metadata(&path).await {
            Ok(m) => m,
            Err(_) => continue,
        };
        let mtime: DateTime<Utc> = match meta.modified() {
            Ok(t) => DateTime::<Utc>::from(t),
            Err(_) => DateTime::<Utc>::from(std::time::UNIX_EPOCH),
        };

        let text = match tokio::fs::read_to_string(&path).await {
            Ok(t) => t,
            Err(_) => continue,
        };
        let parsed = match parse_page(&text) {
            Ok(p) => p,
            Err(_) => continue,
        };

        let title = parsed
            .frontmatter
            .title
            .clone()
            .unwrap_or_else(|| slug.clone());
        let summary = first_paragraph_snippet(&parsed.body_md, 180);

        items.push(FeedItem {
            slug,
            title,
            updated: mtime,
            summary,
        });
    }

    // Sort by mtime descending (most-recently modified first).
    items.sort_by_key(|i| std::cmp::Reverse(i.updated));
    items.truncate(limit);

    Ok(items)
}

/// Extract a plain-text snippet from the first non-empty paragraph in the
/// Markdown body. Strips Markdown syntax crudely (no full parse — just skip
/// headings, list bullets, HR, and backtick/emphasis characters). Caps at
/// `max_chars` characters. Returns an empty string when the body is empty.
pub fn first_paragraph_snippet(body_md: &str, max_chars: usize) -> String {
    let para = body_md
        .lines()
        .find(|l| {
            let trimmed = l.trim();
            !trimmed.is_empty()
                && !trimmed.starts_with('#')
                && !trimmed.starts_with("---")
                && !trimmed.starts_with("===")
        })
        .unwrap_or("");

    // Strip wikilinks: [[slug|display]] → display; [[slug]] → slug.
    let mut delinked = String::with_capacity(para.len());
    let mut rest = para;
    while let Some(pos) = rest.find("[[") {
        delinked.push_str(&rest[..pos]);
        match rest[pos..].find("]]") {
            Some(end) => {
                let inner = &rest[pos + 2..pos + end];
                let display = inner.find('|').map_or(inner, |p| &inner[p + 1..]);
                delinked.push_str(display);
                rest = &rest[pos + end + 2..];
            }
            None => {
                delinked.push_str(&rest[pos..]);
                rest = "";
            }
        }
    }
    delinked.push_str(rest);
    // Strip remaining inline markers: leading list/quote chars + emphasis + backticks.
    let clean: String = delinked
        .trim_start_matches(['-', '*', '+', '>', ' '])
        .chars()
        .filter(|&c| c != '`' && c != '*' && c != '_')
        .collect();

    let clean = clean.trim();
    // Count Unicode characters (not bytes) to avoid slicing mid-codepoint on
    // multi-byte sequences such as em-dashes (—, 3 bytes each).
    let char_count = clean.chars().count();
    if char_count <= max_chars {
        clean.to_string()
    } else {
        // Collect up to max_chars chars, then walk back to a word boundary.
        let truncated: String = clean.chars().take(max_chars).collect();
        if let Some(last_space) = truncated.rfind(' ') {
            format!("{}…", &truncated[..last_space])
        } else {
            format!("{truncated}…")
        }
    }
}

/// Extract the URL of the first image found in the Markdown body.
/// Returns None if no images are found.
pub fn first_image_url(body_md: &str) -> Option<String> {
    let re_img = regex::Regex::new(r"!\[.*?\]\((.*?)\)").unwrap();
    re_img.captures(body_md).map(|caps| caps[1].to_string())
}

/// Render an RFC 4287 Atom feed XML string from a slice of `FeedItem`s.
///
/// Uses `atom_syndication` 0.12. `FixedDateTime` is
/// `chrono::DateTime<chrono::FixedOffset>`; the `DateTime<Utc>: Into<FixedDateTime>`
/// impl in chrono converts cleanly. Feed-level metadata is static; per-entry
/// `<link>` URLs are relative (`/wiki/<slug>`) — aggregators resolve against
/// their configured base URL.
pub fn render_atom(items: &[FeedItem], site_title: &str) -> String {
    use atom_syndication::{Entry, Feed, FixedDateTime, Generator, Link, Person, Text};

    // Feed-level `<updated>` is the most-recent entry mtime, or epoch if empty.
    let epoch: FixedDateTime = DateTime::<Utc>::from(std::time::UNIX_EPOCH).into();
    let feed_updated: FixedDateTime = items
        .first()
        .map(|i| -> FixedDateTime { i.updated.into() })
        .unwrap_or(epoch);

    let entries: Vec<Entry> = items
        .iter()
        .map(|item| {
            let link = Link {
                href: format!("/wiki/{}", item.slug),
                rel: "alternate".to_string(),
                ..Default::default()
            };
            Entry {
                id: format!("urn:pointsav:knowledge:topic:{}", item.slug),
                title: Text::plain(item.title.clone()),
                updated: item.updated.into(),
                links: vec![link],
                summary: if !item.summary.is_empty() {
                    Some(Text::plain(item.summary.clone()))
                } else {
                    None
                },
                ..Entry::default()
            }
        })
        .collect();

    let feed = Feed {
        id: "urn:pointsav:knowledge:feed".to_string(),
        title: Text::plain(site_title),
        updated: feed_updated,
        authors: vec![Person {
            name: site_title.to_string(),
            ..Default::default()
        }],
        generator: Some(Generator {
            value: "app-mediakit-knowledge".to_string(),
            ..Default::default()
        }),
        entries,
        ..Feed::default()
    };

    feed.to_string()
}

/// Render a JSON Feed 1.1 (https://www.jsonfeed.org/version/1.1/) value.
///
/// Hand-rolled via `serde_json::json!` — no extra crate required.
/// `home_page_url` is `/` (relative) so the feed is host-agnostic; consuming
/// aggregators resolve it against the configured base URL.
pub fn render_json_feed(items: &[FeedItem], site_title: &str) -> Value {
    let feed_items: Vec<Value> = items
        .iter()
        .map(|item| {
            json!({
                "id": format!("urn:pointsav:knowledge:topic:{}", item.slug),
                "title": item.title,
                "url": format!("/wiki/{}", item.slug),
                "date_modified": item.updated.to_rfc3339(),
                "summary": item.summary,
            })
        })
        .collect();

    json!({
        "version": "https://jsonfeed.org/version/1.1",
        "title": site_title,
        "home_page_url": "/",
        "feed_url": "/feed.json",
        "items": feed_items,
    })
}

/// `GET /feed.atom` — Atom feed handler.
///
/// Collects the 25 most-recently-modified TOPICs and renders an RFC 4287
/// Atom document. Content-Type: `application/atom+xml; charset=utf-8`.
pub async fn get_atom(State(state): State<Arc<AppState>>) -> Result<Response, WikiError> {
    let items = collect_recent_items(state.primary_path(), 25).await?;
    let xml = render_atom(&items, &state.site_title);
    let mut resp = xml.into_response();
    resp.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/atom+xml; charset=utf-8"),
    );
    Ok(resp)
}

/// `GET /feed.json` — JSON Feed 1.1 handler.
///
/// Collects the 25 most-recently-modified TOPICs and returns a JSON Feed 1.1
/// document. Content-Type is set automatically by axum's `Json` extractor
/// (`application/json`).
pub async fn get_json_feed(State(state): State<Arc<AppState>>) -> Result<Json<Value>, WikiError> {
    let items = collect_recent_items(state.primary_path(), 25).await?;
    let value = render_json_feed(&items, &state.site_title);
    Ok(Json(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_paragraph_snippet_returns_first_non_heading_line() {
        let body = "\n\n## Heading\n\nThis is the first paragraph.\n\nSecond paragraph.\n";
        let s = first_paragraph_snippet(body, 180);
        assert!(
            s.contains("first paragraph"),
            "snippet should contain paragraph text: {s}"
        );
    }

    #[test]
    fn first_paragraph_snippet_truncates_at_word_boundary() {
        // 200 copies of "A " = 400 chars, well above the 50-char limit.
        let body = "A ".repeat(200);
        let s = first_paragraph_snippet(&body, 50);
        // Should be truncated (≤ 54 chars including the ellipsis + some slack).
        // Use char count — the fix uses .chars().count() not .len() (bytes).
        assert!(
            s.chars().count() <= 55,
            "snippet should be ≤50 chars: char_count={}",
            s.chars().count()
        );
    }

    #[test]
    fn first_paragraph_snippet_handles_multibyte_chars() {
        // em-dash is 3 bytes but 1 char; slicing at byte 180 on a string of
        // em-dashes would previously panic. This test confirms safety.
        let body = "—".repeat(200);
        let s = first_paragraph_snippet(&body, 180);
        // Must not panic; char count must be ≤ 180 + ellipsis (1 char).
        assert!(
            s.chars().count() <= 181,
            "snippet char_count out of range: {}",
            s.chars().count()
        );
    }

    #[test]
    fn render_atom_produces_xml_with_feed_tag() {
        let items = vec![FeedItem {
            slug: "topic-test".to_string(),
            title: "Test Topic".to_string(),
            updated: Utc::now(),
            summary: "A short summary.".to_string(),
        }];
        let xml = render_atom(&items, "Test Wiki");
        assert!(
            xml.contains("<feed"),
            "Atom output should contain <feed: {xml}"
        );
        assert!(
            xml.contains("topic-test"),
            "Atom output should contain slug: {xml}"
        );
        assert!(
            xml.contains("Test Wiki"),
            "Atom output should contain feed title: {xml}"
        );
    }

    #[test]
    fn render_json_feed_has_required_fields() {
        let items = vec![FeedItem {
            slug: "topic-foo".to_string(),
            title: "Foo".to_string(),
            updated: Utc::now(),
            summary: "Foo summary.".to_string(),
        }];
        let value = render_json_feed(&items, "Test Wiki");
        assert_eq!(
            value["version"].as_str().unwrap(),
            "https://jsonfeed.org/version/1.1"
        );
        assert_eq!(value["feed_url"].as_str().unwrap(), "/feed.json");
        assert_eq!(value["home_page_url"].as_str().unwrap(), "/");
        let items_arr = value["items"].as_array().unwrap();
        assert_eq!(items_arr.len(), 1);
        assert_eq!(items_arr[0]["title"].as_str().unwrap(), "Foo");
    }

    #[test]
    fn render_atom_empty_feed_is_valid_xml() {
        let xml = render_atom(&[], "Test Wiki");
        assert!(
            xml.contains("<feed"),
            "empty Atom feed should still be valid XML: {xml}"
        );
    }
}
