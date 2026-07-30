//! Search routes.
//!
//! Phase 4 implementation. Provides the full GET /search results page
//! (hard requirement §17.6) plus the JSON API endpoints.
//!
//! Routes owned by this module:
//! - `GET /search?q={query}`        — full HTML search results page (HARD REQUIREMENT)
//! - `GET /api/search?q={query}`    — JSON search API
//! - `GET /api/complete?q={query}`  — title autocomplete
//!
//! NOTE: These handlers are currently wired through `server::router()` via the
//! delegation in `routes/mod.rs`. The implementations here are available for
//! standalone use and future direct wiring as Phase 4 migration progresses.

use axum::{
    extract::{Query, State},
    response::{Html, IntoResponse},
    Json,
};
use maud::{html, Markup, DOCTYPE};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::search::{search as run_search, SearchHit};
use crate::server::AppState;

/// Query parameters for `GET /search` and `GET /api/search`.
#[derive(Deserialize, Default)]
pub struct SearchParams {
    #[serde(default)]
    pub q: String,
}

/// `GET /search?q={query}` — full HTML search results page.
///
/// Hard requirement from §17.6. Returns article cards (title, status badge,
/// category chip, lede) rather than plain JSON. Three sections:
/// 1. Exact title match if any
/// 2. BM25 results as article cards
/// 3. Zero-result state: category browse links + "try shorter query" prompt
///
/// This is the canonical implementation used by the live sites.
pub async fn search_page(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchParams>,
) -> impl IntoResponse {
    let query = params.q.trim().to_string();

    let hits: Vec<SearchHit> = if query.is_empty() {
        Vec::new()
    } else {
        run_search(&state.search, &query, 25).unwrap_or_default()
    };

    let title = if query.is_empty() {
        "Search".to_string()
    } else {
        format!("Search: {query}")
    };

    let page = render_search_page(&query, &hits, &title, &state.site_title);
    Html(page.into_string())
}

/// `GET /api/search?q={query}` — JSON search API.
///
/// Returns `{ results: [ {slug, title, lede, score} ] }`.
pub async fn search_api(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchParams>,
) -> Json<Value> {
    let query = params.q.trim().to_string();
    if query.is_empty() {
        return Json(json!({ "results": [] }));
    }
    let hits = run_search(&state.search, &query, 25).unwrap_or_default();
    let results: Vec<Value> = hits
        .into_iter()
        .map(|h| {
            json!({
                "slug": h.slug,
                "title": h.title,
                "lede": h.snippet,
                "score": h.score,
            })
        })
        .collect();
    Json(json!({ "results": results }))
}

/// `GET /api/complete?q={prefix}` — title autocomplete endpoint.
///
/// Returns a JSON array of title strings matching the prefix query.
/// Capped at 10 results. Used by the Cmd+K command palette and search box.
pub async fn complete_api(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchParams>,
) -> Json<Value> {
    let q = params.q.trim().to_lowercase();
    if q.is_empty() {
        return Json(json!([]));
    }
    // Use BM25 search and return title strings only.
    let hits = run_search(&state.search, &q, 10).unwrap_or_default();
    let titles: Vec<Value> = hits
        .into_iter()
        .map(|h| json!({ "title": h.title, "slug": h.slug }))
        .collect();
    Json(json!(titles))
}

/// `GET /api/citations` — citation data for hover cards.
///
/// Delegated to the citations module. Handler provided here for route
/// assembly completeness; wired in server::router() via crate::citations.
///
/// This stub is used when routes/mod.rs wires citations independently.
pub async fn citations_api(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    // Delegate to the existing citations handler.
    crate::citations::get_citations(State(state)).await
}

// ─── Internal rendering helpers ─────────────────────────────────────────────

/// Render the full search results HTML page.
///
/// Three sections per §17.6 hard requirement:
/// 1. Exact title match (if any hit's title matches the query exactly)
/// 2. BM25 result cards (title, category chip from slug prefix, lede snippet)
/// 3. Zero-result state with category browse links + "try shorter query" prompt
fn render_search_page(query: &str, hits: &[SearchHit], title: &str, site_title: &str) -> Markup {
    // Detect exact title match.
    let exact_match = hits
        .iter()
        .find(|h| h.title.to_lowercase() == query.to_lowercase());

    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover";
                title { (title) " — " (site_title) }
                link rel="stylesheet" href="/static/tokens.css";
                link rel="stylesheet" href="/static/style.css";
                link rel="preload" as="font" type="font/woff2" crossorigin href="/static/fonts/inter-latin-regular.woff2";
                link rel="preload" as="font" type="font/woff2" crossorigin href="/static/fonts/source-serif-4-latin-regular.woff2";
                script src="/static/wiki.js" defer {};
            }
            body {
                header.site-header {
                    a.site-logo href="/" { (site_title) }
                    nav.site-nav {
                        form.search-form-header action="/search" method="get" {
                            input
                                type="search"
                                name="q"
                                value=(query)
                                placeholder="Search…"
                                autocomplete="off";
                            button type="submit" { "Search" }
                        }
                    }
                }
                main.search-page {
                    h1 { "Search" }

                    // Search form
                    form.search-form action="/search" method="get" {
                        input
                            type="search"
                            name="q"
                            value=(query)
                            placeholder="Search TOPICs and GUIDEs"
                            autocomplete="off"
                            autofocus[query.is_empty()];
                        button type="submit" { "Search" }
                    }

                    @if !query.is_empty() {
                        @if hits.is_empty() {
                            // Section 3: zero-result state
                            div.search-zero-results {
                                p.search-empty {
                                    "No results for "
                                    em { (query) }
                                    ". "
                                    "Try a shorter query or browse by category."
                                }
                                nav.category-browse {
                                    h2 { "Browse by category" }
                                    ul {
                                        li { a href="/category/architecture" { "Architecture" } }
                                        li { a href="/category/substrate" { "Substrate & Systems" } }
                                        li { a href="/category/services" { "Services & Applications" } }
                                        li { a href="/category/infrastructure" { "Infrastructure" } }
                                        li { a href="/category/reference" { "Reference" } }
                                        li { a href="/category/archetypes" { "Archetypes" } }
                                    }
                                }
                            }
                        } @else {
                            // Section 1: exact title match
                            @if let Some(exact) = exact_match {
                                div.search-exact-match {
                                    h2.search-section-label { "Best match" }
                                    (render_article_card(exact, true))
                                }
                            }

                            // Section 2: BM25 results as article cards
                            div.search-results-section {
                                p.search-summary {
                                    strong { (hits.len()) }
                                    " result" @if hits.len() != 1 { "s" }
                                    " for "
                                    em { (query) }
                                }
                                ol.search-results {
                                    @for hit in hits {
                                        li.search-hit {
                                            (render_article_card(hit, false))
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                footer.site-footer {
                    p {
                        "© 2026 Woodfine Capital Projects Inc. All rights reserved."
                    }
                }
            }
        }
    }
}

/// Render a single article result card.
///
/// Shows: title link, category chip (derived from slug prefix), lede snippet,
/// and status badge when slug contains a recognisable status prefix.
fn render_article_card(hit: &SearchHit, featured: bool) -> Markup {
    // Derive category chip from slug prefix (e.g. "architecture/foo" → "architecture").
    let category = if let Some(slash_pos) = hit.slug.find('/') {
        &hit.slug[..slash_pos]
    } else {
        ""
    };

    html! {
        article.article-card class=(if featured { "article-card--featured" } else { "" }) {
            div.article-card-header {
                a.article-card-title href={ "/wiki/" (hit.slug) } {
                    (hit.title)
                }
                @if !category.is_empty() {
                    span.category-chip { (category) }
                }
            }
            @if !hit.snippet.is_empty() {
                p.article-card-lede { (hit.snippet) }
            }
            span.article-card-slug { (hit.slug) }
        }
    }
}
