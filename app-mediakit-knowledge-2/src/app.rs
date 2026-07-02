//! Application state and axum router assembly.
//!
//! All routes are declared in one place (`router`) so the URL surface is
//! legible at a glance. Handlers are added phase by phase; through P1 this
//! wires the liveness probe, static assets, and a raw (chrome-less) article
//! view so the content pipeline can be exercised end to end.

use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::Router;

use crate::assets::StaticAssets;
use crate::config::Config;
use crate::content::{self, ContentIndex, Lang, MountSet};
use crate::search::SearchIndex;
use crate::ui::{self, Tenant};

/// Shared, immutable-after-startup application state.
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub mounts: Arc<MountSet>,
    pub index: Arc<ContentIndex>,
    pub search: Arc<SearchIndex>,
    pub tenant: Tenant,
}

impl AppState {
    /// Build state from parsed config, walking the mounts to build the index.
    pub fn build(config: Config) -> Self {
        let mounts = MountSet::from_config(&config);
        let index = ContentIndex::build(&mounts);
        let tenant = Tenant::from_instance(config.site.instance.as_deref());
        tracing::info!("indexed {} article(s)", index.article_count());
        let search = SearchIndex::build(&index).expect("build search index");
        Self {
            config: Arc::new(config),
            mounts: Arc::new(mounts),
            index: Arc::new(index),
            search: Arc::new(search),
            tenant,
        }
    }
}

/// Build the router. The route map is the engine's public contract.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(home))
        .route("/healthz", get(healthz))
        .route("/health", get(healthz))
        .route("/wiki/{*slug}", get(wiki_raw))
        .route("/category/{name}", get(category_page))
        .route("/search", get(search_page))
        .route("/static/syntax.css", get(syntax_css_handler))
        .route("/static/{*path}", get(static_asset))
        .with_state(state)
}

/// Generated syntax-highlight stylesheet (light + dark themes).
async fn syntax_css_handler() -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/css; charset=utf-8")
        .header(header::CACHE_CONTROL, "public, max-age=3600")
        .body(Body::from(content::syntax_css()))
        .unwrap()
}

/// Turn a category slug into a human label: "design-system" → "Design System".
fn humanize(slug: &str) -> String {
    slug.split(['-', '_'])
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Ordered `(slug, label)` category list for the sidebar nav — the configured
/// order when set, else discovered categories. Mirrors the home-grid ordering.
fn nav_cats(state: &AppState) -> Vec<(String, String)> {
    let counts = state.index.category_counts();
    let mut cats: Vec<(String, String)> = Vec::new();
    if state.config.site.categories.is_empty() {
        for (slug, _) in &counts {
            cats.push((slug.clone(), humanize(slug)));
        }
    } else {
        for slug in &state.config.site.categories {
            if counts.contains_key(slug) {
                cats.push((slug.clone(), humanize(slug)));
            }
        }
    }
    cats
}

/// Home page (Main Page) — index lede + a "Browse by area" category grid.
async fn home(State(state): State<AppState>) -> Response {
    let tenant = state.tenant;

    // Lede + description from index.md, if present.
    let index_parsed = state
        .index
        .resolve("index", Lang::En)
        .and_then(|doc| content::load(doc).ok());
    let lede = index_parsed
        .as_ref()
        .map(|p| content::render(&p.body_md).html)
        .unwrap_or_default();
    let description = index_parsed
        .as_ref()
        .and_then(|p| p.frontmatter.short_description.clone())
        .unwrap_or_default();

    // Category cards: prefer the configured order, fall back to discovered.
    let counts = state.index.category_counts();
    let mut cats: Vec<(String, String, usize)> = Vec::new();
    if state.config.site.categories.is_empty() {
        for (slug, n) in &counts {
            cats.push((slug.clone(), humanize(slug), *n));
        }
    } else {
        for slug in &state.config.site.categories {
            if let Some(n) = counts.get(slug) {
                cats.push((slug.clone(), humanize(slug), *n));
            }
        }
    }
    let total: usize = state.index.article_count();

    // How-to guides (content_type/category "how-to") — surfaced as their own
    // section, distinct from the topic areas. Show a sample + a browse-all link.
    let guides: Vec<(String, String, String)> = state
        .index
        .in_category("how-to")
        .into_iter()
        .map(|d| {
            (
                d.slug.clone(),
                d.title.clone(),
                d.short_description.clone().unwrap_or_default(),
            )
        })
        .collect();

    let nav: Vec<(String, String)> = cats.iter().map(|(s, l, _)| (s.clone(), l.clone())).collect();
    let body = ui::home_page(tenant, &lede, total, &cats, &guides);
    let head = ui::doc_head(tenant.home_label(), &description, tenant);
    Html(ui::page(tenant, "en", head, body, &nav, &[], "").into_string()).into_response()
}

/// Category listing page — every article in one category.
async fn category_page(State(state): State<AppState>, Path(name): Path<String>) -> Response {
    let tenant = state.tenant;
    let docs: Vec<(String, String, String)> = state
        .index
        .in_category(&name)
        .into_iter()
        .map(|d| {
            (
                d.slug.clone(),
                d.title.clone(),
                d.short_description.clone().unwrap_or_default(),
            )
        })
        .collect();
    if docs.is_empty() {
        return (StatusCode::NOT_FOUND, format!("no such category: {name}")).into_response();
    }
    let label = humanize(&name);
    let description = format!("Articles in the {label} area.");
    let body = ui::category_index(&label, &docs);
    let head = ui::doc_head(&label, &description, tenant);
    Html(ui::page(tenant, "en", head, body, &nav_cats(&state), &[], "").into_string()).into_response()
}

#[derive(serde::Deserialize)]
struct SearchQuery {
    q: Option<String>,
}

/// Full-text search results. The query hits title + body (tantivy); each result
/// carries the same title + description shown on category/guide cards.
async fn search_page(State(state): State<AppState>, Query(params): Query<SearchQuery>) -> Response {
    let tenant = state.tenant;
    let q = params.q.unwrap_or_default();
    let results: Vec<(String, String, String)> = state
        .search
        .query(&q, 30)
        .into_iter()
        .filter_map(|slug| {
            state.index.resolve(&slug, Lang::En).map(|d| {
                (
                    d.slug.clone(),
                    d.title.clone(),
                    d.short_description.clone().unwrap_or_default(),
                )
            })
        })
        .collect();
    let body = ui::search_results(&q, &results);
    let desc = if q.trim().is_empty() {
        String::new()
    } else {
        format!("Search results for \u{201c}{}\u{201d}", q.trim())
    };
    let head = ui::doc_head("Search", &desc, tenant);
    Html(ui::page(tenant, "en", head, body, &nav_cats(&state), &[], &q).into_string()).into_response()
}

/// Liveness probe.
async fn healthz() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

/// Article view — the rendered body wrapped in the new chrome shell.
/// The full 2-column article layout (tabs above h1, TOC sidebar) arrives in P3;
/// P2 renders the body inside the continuous header/sitenotice/footer chrome.
async fn wiki_raw(State(state): State<AppState>, Path(slug): Path<String>) -> Response {
    let slug = slug.trim_end_matches('/');
    let Some(doc) = state.index.resolve(slug, Lang::En) else {
        return (StatusCode::NOT_FOUND, format!("no article: {slug}")).into_response();
    };
    let parsed = match content::load(doc) {
        Ok(p) => p,
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, format!("read error: {e}")).into_response()
        }
    };
    let rendered = content::render(&parsed.body_md);
    let title = parsed
        .frontmatter
        .title
        .clone()
        .unwrap_or_else(|| doc.title.clone());
    let description = parsed.frontmatter.short_description.clone().unwrap_or_default();
    let tenant = state.tenant;
    let body = ui::article(&title, parsed.frontmatter.last_edited.as_deref(), &rendered.html);
    let head = ui::doc_head(&title, &description, tenant);
    Html(ui::page(tenant, "en", head, body, &nav_cats(&state), &rendered.headings, "").into_string())
        .into_response()
}

/// Serve an embedded static asset by path.
async fn static_asset(Path(path): Path<String>) -> Response {
    match StaticAssets::get(&path) {
        Some(file) => {
            let mime = mime_guess::from_path(&path).first_or_octet_stream();
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime.as_ref())
                .body(Body::from(file.data.into_owned()))
                .unwrap()
        }
        None => (StatusCode::NOT_FOUND, "asset not found").into_response(),
    }
}
