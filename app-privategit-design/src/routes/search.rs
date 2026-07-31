// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

use crate::{state::AppState, vault};
use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    response::IntoResponse,
};
use serde::Deserialize;
use std::path::Path;

#[derive(Deserialize)]
pub struct SearchQuery {
    q: Option<String>,
}

pub async fn token_search(
    Query(params): Query<SearchQuery>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let q = match params.q.as_deref().filter(|s| !s.is_empty()) {
        Some(q) => q.to_string(),
        None => {
            return (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "application/json")],
                "[]".to_string(),
            );
        }
    };

    let idx = state.index.read().await;
    let hits = idx.search(&q);

    let json = {
        let items: Vec<String> = hits
            .into_iter()
            .take(20)
            .map(|doc| {
                let snippet = snippet(&doc.body, &q);
                let url = doc_url(&doc.id, &state.vault);
                format!(
                    "{{\"id\":{},\"title\":{},\"snippet\":{},\"url\":{}}}",
                    json_str(&doc.id),
                    json_str(&doc.title),
                    json_str(&snippet),
                    json_str(&url)
                )
            })
            .collect();
        format!("[{}]", items.join(","))
    };

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        json,
    )
}

fn snippet(body: &str, query: &str) -> String {
    let q_lower = query.to_lowercase();
    let b_lower = body.to_lowercase();
    if let Some(pos) = b_lower.find(q_lower.split_whitespace().next().unwrap_or("")) {
        // `pos` is a valid byte offset into `b_lower`, which is a lowercased copy of
        // `body` — lowercasing can change a character's byte length (e.g. some Unicode
        // casing expansions), so `pos` is not guaranteed to land on a `body` char
        // boundary. Round outward to the nearest valid boundaries rather than slicing
        // blind (previously panicked on any multi-byte char straddling the window).
        let start = floor_char_boundary(body, pos.saturating_sub(40));
        let end = ceil_char_boundary(body, (pos + 120).min(body.len()));
        let s = &body[start..end];
        s.replace('\n', " ").trim().to_string()
    } else {
        body.chars()
            .take(120)
            .collect::<String>()
            .replace('\n', " ")
    }
}

fn floor_char_boundary(s: &str, mut i: usize) -> usize {
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

fn ceil_char_boundary(s: &str, mut i: usize) -> usize {
    while i < s.len() && !s.is_char_boundary(i) {
        i += 1;
    }
    i
}

/// Turns an indexed doc's absolute file path into its route. Nested sections index as
/// `<vault>/<section>/<slug>/<tab>.md` (3 components) → `/<section>/<slug>/<tab>`. Flat
/// sections (research/developing/designing/about) index as `<vault>/<section>/<slug>.md`
/// (2 components) → `/<section>/<slug>/<default-tab>`, since a flat slug always resolves
/// to its section's single implicit tab. Falls back to `#` if it doesn't parse cleanly.
fn doc_url(id: &str, vault_dir: &Path) -> String {
    let Ok(rel) = Path::new(id).strip_prefix(vault_dir) else {
        return "#".to_string();
    };
    let parts: Vec<&str> = rel
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect();
    match parts.len() {
        3 => {
            let tab = parts[2].strip_suffix(".md").unwrap_or(parts[2]);
            format!("/{}/{}/{}", parts[0], parts[1], tab)
        }
        2 if vault::is_flat_section(parts[0]) => {
            let slug = parts[1].strip_suffix(".md").unwrap_or(parts[1]);
            format!("/{}/{}/{}", parts[0], slug, vault::default_tab(parts[0]))
        }
        _ => "#".to_string(),
    }
}

fn json_str(s: &str) -> String {
    let escaped = s
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t");
    format!("\"{}\"", escaped)
}
