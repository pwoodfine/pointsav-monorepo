// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

use crate::state::AppState;
use serde_json::Value;

use super::shell::esc;

struct Hit {
    kind: &'static str,
    title: String,
    url: String,
    snippet: String,
    score: i32,
    tiebreak: String,
}

/// Recursively walk a DTCG object collecting every node that carries a
/// `$value` field, regardless of nesting depth (same shape as
/// card.rs::collect_kp_leaves, duplicated rather than shared since search
/// wants every category file, not just key-plans, and the two callers'
/// filtering needs are different enough not to force a shared abstraction).
fn collect_entities<'a>(
    obj: &'a serde_json::Map<String, Value>,
    out: &mut Vec<(&'a str, &'a Value)>,
) {
    for (key, val) in obj {
        if key == "$description" {
            continue;
        }
        if val.get("$value").is_some() {
            out.push((key.as_str(), val));
        } else if let Some(child) = val.as_object() {
            collect_entities(child, out);
        }
    }
}

/// Case-insensitive multi-word match: every query token must appear as a
/// substring in at least one of the given fields (AND across tokens, OR
/// across fields per token). Returns a score if it matches, None if not.
fn score_match(tokens: &[String], fields: &[(&str, i32)]) -> Option<i32> {
    let mut total = 0;
    for tok in tokens {
        let mut found = false;
        for (field, weight) in fields {
            if field.to_lowercase().contains(tok) {
                total += weight;
                found = true;
            }
        }
        if !found {
            return None;
        }
    }
    Some(total)
}

/// Strip the most common Markdown syntax markers so search snippets read as
/// plain prose instead of leaking '**', '#', '`', etc.
fn strip_markdown(md: &str) -> String {
    md.lines()
        .map(|l| l.trim_start_matches('#').trim_start_matches('>').trim())
        .collect::<Vec<_>>()
        .join(" ")
        .replace("**", "")
        .replace('`', "")
        .replace("*", "")
}

/// Wrap the first case-insensitive occurrence of any query token in <mark>,
/// windowed to a reasonable snippet length around the match.
fn highlight_snippet(text: &str, tokens: &[String]) -> String {
    let lower = text.to_lowercase();
    let mut best: Option<usize> = None;
    for tok in tokens {
        if let Some(pos) = lower.find(tok.as_str()) {
            best = Some(best.map_or(pos, |b| b.min(pos)));
        }
    }
    let Some(pos) = best else {
        let clipped: String = text.chars().take(160).collect();
        return esc(&clipped);
    };
    let window = 90usize;
    let start = pos.saturating_sub(window);
    let end = (pos + window).min(text.len());
    // Snap to char boundaries.
    let start = (start..=pos)
        .find(|i| text.is_char_boundary(*i))
        .unwrap_or(0);
    let end = (end..text.len().min(end + 4))
        .find(|i| text.is_char_boundary(*i))
        .unwrap_or(text.len());
    let prefix = if start > 0 { "…" } else { "" };
    let suffix = if end < text.len() { "…" } else { "" };
    let slice = &text[start..end];
    let slice_lower = slice.to_lowercase();
    let mut out = String::new();
    let mut i = 0usize;
    while i < slice.len() {
        let mut matched_len = 0usize;
        for tok in tokens {
            if !tok.is_empty() && slice_lower[i..].starts_with(tok.as_str()) {
                matched_len = matched_len.max(tok.len());
            }
        }
        if matched_len > 0 && slice.is_char_boundary(i + matched_len) {
            out.push_str("<mark>");
            out.push_str(&esc(&slice[i..i + matched_len]));
            out.push_str("</mark>");
            i += matched_len;
        } else {
            let next = (i + 1..=slice.len())
                .find(|j| slice.is_char_boundary(*j))
                .unwrap_or(slice.len());
            out.push_str(&esc(&slice[i..next]));
            i = next;
        }
    }
    format!("{prefix}{out}{suffix}")
}

pub fn render_search_results(query: &str, state: &AppState) -> String {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return r#"<div class="bim-search-page">
  <header class="bim-cat-pagehead">
    <span class="bim-cat-kicker">Catalog search</span>
    <h1>Search</h1>
  </header>
  <p class="bim-empty">Enter a search term above — categories, entity slugs, IFC classes, and research articles are all searched.</p>
</div>"#
            .to_string();
    }

    let tokens: Vec<String> = trimmed
        .split_whitespace()
        .map(|t| t.to_lowercase())
        .collect();

    let mut hits: Vec<Hit> = Vec::new();

    // Categories
    for cat in state.categories.iter() {
        let fields = [
            (cat.display_name.as_str(), 100),
            (cat.card_desc.as_str(), 10),
            (cat.ifc_anchor.as_str(), 30),
        ];
        if let Some(score) = score_match(&tokens, &fields) {
            hits.push(Hit {
                kind: "Category",
                title: cat.display_name.clone(),
                url: format!("/tokens/{}", cat.slug),
                snippet: highlight_snippet(&cat.card_desc, &tokens),
                score,
                tiebreak: cat.slug.clone(),
            });
        }
    }

    // Entities within every category's token data
    for cat in state.categories.iter() {
        let Some(file_val) = state.tokens.get(&cat.slug) else {
            continue;
        };
        let Some(bim) = file_val.get("bim").and_then(|v| v.as_object()) else {
            continue;
        };
        let mut leaves = Vec::new();
        for (_cat_key, cat_val) in bim {
            if let Some(obj) = cat_val.as_object() {
                collect_entities(obj, &mut leaves);
            }
        }
        for (slug, entity) in leaves {
            let description = entity
                .get("$description")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let ifc_class = entity
                .get("$value")
                .and_then(|v| v.get("ifc_class"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let display_name = entity
                .get("$value")
                .and_then(|v| v.get("display_name"))
                .and_then(|v| v.as_str())
                .unwrap_or(slug);
            let fields = [
                (display_name, 80),
                (slug, 60),
                (ifc_class, 40),
                (description, 8),
            ];
            if let Some(score) = score_match(&tokens, &fields) {
                let snippet_source = if description.is_empty() {
                    ifc_class
                } else {
                    description
                };
                hits.push(Hit {
                    kind: "BIM Object",
                    title: display_name.to_string(),
                    url: format!("/tokens/{}", cat.slug),
                    snippet: highlight_snippet(snippet_source, &tokens),
                    score,
                    tiebreak: slug.to_string(),
                });
            }
        }
    }

    // Research articles (mirrors render_research_index's own disk-read
    // pattern — kept separate from the in-memory category/entity search
    // above rather than forcing both onto one abstraction).
    let research_dir = state.config.vault_dir.join("research");
    if let Ok(rd) = std::fs::read_dir(&research_dir) {
        let mut names: Vec<String> = rd
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("md"))
            .filter_map(|e| {
                e.path()
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string())
            })
            .collect();
        names.sort();
        for slug in &names {
            let Ok(raw) = std::fs::read_to_string(research_dir.join(format!("{slug}.md"))) else {
                continue;
            };
            let title = raw
                .lines()
                .find_map(|l| l.strip_prefix("# ").map(|t| t.trim().to_string()))
                .unwrap_or_else(|| slug.replace('-', " "));
            let body = strip_markdown(&raw);
            let fields = [(title.as_str(), 90), (body.as_str(), 6)];
            if let Some(score) = score_match(&tokens, &fields) {
                hits.push(Hit {
                    kind: "Research",
                    title: title.clone(),
                    url: format!("/research/{slug}"),
                    snippet: highlight_snippet(&body, &tokens),
                    score,
                    tiebreak: slug.clone(),
                });
            }
        }
    }

    hits.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| a.tiebreak.cmp(&b.tiebreak))
    });

    let count = hits.len();
    let mut results_html = String::new();
    for hit in &hits {
        results_html.push_str(&format!(
            r#"<a class="bim-search-result" href="{url}" data-path="{url}">
  <span class="bim-search-result__kind">{kind}</span>
  <span class="bim-search-result__title">{title}</span>
  <span class="bim-search-result__snippet">{snippet}</span>
</a>"#,
            url = esc(&hit.url),
            kind = esc(hit.kind),
            title = esc(&hit.title),
            snippet = hit.snippet,
        ));
    }
    if results_html.is_empty() {
        results_html = format!(
            r#"<p class="bim-empty">No results for &ldquo;{q}&rdquo;. Try a different term, or <a href="/tokens" data-path="/tokens">browse all categories</a>.</p>"#,
            q = esc(trimmed)
        );
    }

    format!(
        r#"<div class="bim-search-page">
  <header class="bim-cat-pagehead">
    <span class="bim-cat-kicker">Catalog search</span>
    <h1>Search results</h1>
    <p class="bim-cat-pagehead__lede">{count} {result_word} for &ldquo;{q}&rdquo;</p>
  </header>
  <div class="bim-search-results">{results_html}</div>
</div>"#,
        count = count,
        result_word = if count == 1 { "result" } else { "results" },
        q = esc(trimmed),
        results_html = results_html,
    )
}
