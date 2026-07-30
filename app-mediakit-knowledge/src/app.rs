// SPDX-License-Identifier: FSL-1.1-ALv2
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

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
use std::collections::HashMap;

use crate::content::{self, ContentIndex, Lang, MountSet};
use crate::discovery;
use crate::search::SearchIndex;
use crate::sitedata;
use crate::ui::{self, Tenant};

/// Shared, immutable-after-startup application state.
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub mounts: Arc<MountSet>,
    pub index: Arc<ContentIndex>,
    pub search: Arc<SearchIndex>,
    /// Rendered HTML of `important-information.md` from the content repo (counsel-
    /// owned via Git), for the Important Information band. `None` → tenant default.
    pub important_info: Arc<Option<String>>,
    /// Canonical category nav from the content repo's `categories.yaml` (id, name,
    /// order); empty → fall back to `knowledge.toml` categories + slug discovery.
    pub categories: Arc<Vec<sitedata::Category>>,
    /// `from → to` 301 redirects from the content repo's `redirects.yaml`.
    pub redirects: Arc<HashMap<String, String>>,
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
        // Load the counsel-owned Important Information text from the content repo.
        let important_info = mounts.mounts.first().and_then(|m| {
            std::fs::read_to_string(m.path.join("important-information.md"))
                .ok()
                .map(|text| content::render(&content::parse(&text).body_md).html)
        });
        // Per-wiki category nav + redirects from the content repo root.
        let root = mounts.mounts.first().map(|m| m.path.clone());
        let categories = root
            .as_ref()
            .map(|r| sitedata::load_categories(r))
            .unwrap_or_default();
        let redirects = root
            .as_ref()
            .map(|r| sitedata::load_redirects(r))
            .unwrap_or_default();
        Self {
            config: Arc::new(config),
            mounts: Arc::new(mounts),
            index: Arc::new(index),
            search: Arc::new(search),
            important_info: Arc::new(important_info),
            categories: Arc::new(categories),
            redirects: Arc::new(redirects),
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
        .route("/es/wiki/{*slug}", get(wiki_es))
        .route("/category/{name}", get(category_page))
        .route("/search", get(search_page))
        .route("/history/{*slug}", get(history_page))
        .route("/special/all-pages", get(special_all_pages))
        .route("/special/recent-changes", get(special_recent))
        .route("/sitemap.xml", get(sitemap))
        .route("/robots.txt", get(robots))
        .route("/feed.atom", get(feed_atom))
        .route("/llms.txt", get(llms))
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
    // Prefer the canonical categories.yaml (id → route, name → display, order),
    // showing only categories that currently have content.
    if !state.categories.is_empty() {
        return state
            .categories
            .iter()
            .filter(|c| counts.contains_key(&c.id))
            .map(|c| {
                let name = if c.name.is_empty() {
                    humanize(&c.id)
                } else {
                    c.name.clone()
                };
                (c.id.clone(), name)
            })
            .collect();
    }
    // Fallback: knowledge.toml categories, else discovered.
    let mut cats: Vec<(String, String)> = Vec::new();
    if state.config.site.categories.is_empty() {
        for slug in counts.keys() {
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

/// Display label for a category id — the categories.yaml `name` when present.
fn category_label(state: &AppState, id: &str) -> String {
    state
        .categories
        .iter()
        .find(|c| c.id == id)
        .map(|c| c.name.clone())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| humanize(id))
}

/// A 301 Moved Permanently (aliases + redirects.yaml) — the spec code for
/// content moves; axum's `Redirect::permanent` is 308, so build it directly.
fn moved_301(location: &str) -> Response {
    (
        StatusCode::MOVED_PERMANENTLY,
        [(header::LOCATION, location.to_string())],
    )
        .into_response()
}

/// A chrome-wrapped 404 — a reviewer never sees a bare error string.
fn not_found(state: &AppState, message: &str) -> Response {
    let tenant = state.tenant;
    let body = ui::simple_message("Not found", message);
    let head = ui::doc_head("Not found", "", tenant);
    (
        StatusCode::NOT_FOUND,
        Html(
            ui::page(
                tenant,
                "en",
                head,
                body,
                &nav_cats(state),
                &[],
                "",
                state.important_info.as_deref(),
            )
            .into_string(),
        ),
    )
        .into_response()
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

    // Category cards — categories.yaml order/names (via nav_cats), with counts.
    let counts = state.index.category_counts();
    let cats: Vec<(String, String, usize)> = nav_cats(&state)
        .into_iter()
        .map(|(id, name)| {
            let n = counts.get(&id).copied().unwrap_or(0);
            (id, name, n)
        })
        .collect();
    let total: usize = state.index.article_count();

    // How-to guides (content_type/category "how-to") — surfaced as their own
    // section, distinct from the topic areas. Documentation only: projects and
    // corporate are TOPIC-only, so they never present a Guides affordance.
    let guides: Vec<(String, String, String)> = if tenant.serves_guides() {
        state
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
            .collect()
    } else {
        Vec::new()
    };

    let nav: Vec<(String, String)> = cats
        .iter()
        .map(|(s, l, _)| (s.clone(), l.clone()))
        .collect();
    let body = ui::home_page(tenant, &lede, total, &cats, &guides);
    let head = ui::doc_head(tenant.home_label(), &description, tenant);
    Html(
        ui::page(
            tenant,
            "en",
            head,
            body,
            &nav,
            &[],
            "",
            state.important_info.as_deref(),
        )
        .into_string(),
    )
    .into_response()
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
        return not_found(&state, &format!("No such area: \u{201c}{name}\u{201d}."));
    }
    let label = category_label(&state, &name);
    let description = format!("Articles in the {label} area.");
    let body = ui::category_index(&label, &docs);
    let head = ui::doc_head(&label, &description, tenant);
    Html(
        ui::page(
            tenant,
            "en",
            head,
            body,
            &nav_cats(&state),
            &[],
            "",
            state.important_info.as_deref(),
        )
        .into_string(),
    )
    .into_response()
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
    let (title, desc) = if q.trim().is_empty() {
        ("Search".to_string(), String::new())
    } else {
        (
            format!("{} — Search", q.trim()),
            format!("Search results for \u{201c}{}\u{201d}", q.trim()),
        )
    };
    let head = ui::doc_head(&title, &desc, tenant);
    Html(
        ui::page(
            tenant,
            "en",
            head,
            body,
            &nav_cats(&state),
            &[],
            &q,
            state.important_info.as_deref(),
        )
        .into_string(),
    )
    .into_response()
}

#[derive(serde::Deserialize)]
struct HistoryQuery {
    rev: Option<String>,
}

/// Article revision history (the git log of the file) — and, with `?rev=<sha>`,
/// the diff that revision made to the file. Both are the History tab.
async fn history_page(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    Query(params): Query<HistoryQuery>,
) -> Response {
    let tenant = state.tenant;
    let slug = slug.trim_end_matches('/');
    let Some(doc) = state.index.resolve(slug, Lang::En) else {
        return not_found(
            &state,
            &format!("No record found for \u{201c}{slug}\u{201d}."),
        );
    };
    let repo_root = &state.mounts.mounts[doc.mount_index].path;
    let rel = doc.path.strip_prefix(repo_root).unwrap_or(&doc.path);

    // `?rev=<sha>` → the diff view for that revision.
    if let Some(rev) = params.rev.as_deref().filter(|s| !s.is_empty()) {
        let Some(diff) = crate::history::file_diff(repo_root, rel, rev) else {
            return not_found(&state, &format!("No such revision: {rev}."));
        };
        let body = ui::diff_page(&doc.title, &doc.slug, tenant.issuer(), &diff);
        let head = ui::doc_head(
            &format!("{} — Diff {}", doc.title, diff.short_sha),
            &format!("Changes to {} in revision {}", doc.title, diff.short_sha),
            tenant,
        );
        return Html(
            ui::page(
                tenant,
                "en",
                head,
                body,
                &nav_cats(&state),
                &[],
                "",
                state.important_info.as_deref(),
            )
            .into_string(),
        )
        .into_response();
    }

    let revs = crate::history::file_history(repo_root, rel, 50);
    let body = ui::history_page(&doc.title, &doc.slug, tenant.issuer(), &revs);
    let head = ui::doc_head(
        &format!("{} — History", doc.title),
        &format!("Revision history of {}", doc.title),
        tenant,
    );
    Html(
        ui::page(
            tenant,
            "en",
            head,
            body,
            &nav_cats(&state),
            &[],
            "",
            state.important_info.as_deref(),
        )
        .into_string(),
    )
    .into_response()
}

/// "Index of record" — every article A–Z (the auditor's completeness check).
async fn special_all_pages(State(state): State<AppState>) -> Response {
    let tenant = state.tenant;
    let mut items: Vec<(String, String, String)> = state
        .index
        .documents()
        .map(|d| {
            (
                d.slug.clone(),
                d.title.clone(),
                d.short_description.clone().unwrap_or_default(),
            )
        })
        .collect();
    items.sort_by_key(|a| a.1.to_lowercase());
    let body = ui::special_list("Index of record", "All records", &items);
    let head = ui::doc_head(
        "Index of record",
        "A–Z index of every record in the registry.",
        tenant,
    );
    Html(
        ui::page(
            tenant,
            "en",
            head,
            body,
            &nav_cats(&state),
            &[],
            "",
            state.important_info.as_deref(),
        )
        .into_string(),
    )
    .into_response()
}

/// "Recent changes" — records most recently updated (the site-wide delta view).
async fn special_recent(State(state): State<AppState>) -> Response {
    let tenant = state.tenant;
    let items: Vec<(String, String, String)> = state
        .index
        .recent(50)
        .into_iter()
        .map(|d| {
            let meta = d
                .last_edited
                .as_deref()
                .map(|dt| format!("Updated {dt}"))
                .unwrap_or_default();
            (d.slug.clone(), d.title.clone(), meta)
        })
        .collect();
    let body = ui::special_list("Recent changes", "Recent changes", &items);
    let head = ui::doc_head("Recent changes", "Records most recently updated.", tenant);
    Html(
        ui::page(
            tenant,
            "en",
            head,
            body,
            &nav_cats(&state),
            &[],
            "",
            state.important_info.as_deref(),
        )
        .into_string(),
    )
    .into_response()
}

// --- Discovery surfaces (robots / sitemap / feeds / llms.txt) ---------------

/// Canonical base URL (no trailing slash) for absolute links; "" if unconfigured.
fn site_base(state: &AppState) -> String {
    state
        .config
        .site
        .canonical_url
        .as_deref()
        .unwrap_or("")
        .trim_end_matches('/')
        .to_string()
}

async fn robots(State(state): State<AppState>) -> Response {
    let body = discovery::robots_txt(&site_base(&state));
    ([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], body).into_response()
}

async fn sitemap(State(state): State<AppState>) -> Response {
    let docs: Vec<_> = state.index.documents().collect();
    let body = discovery::sitemap_xml(&site_base(&state), &docs);
    (
        [(header::CONTENT_TYPE, "application/xml; charset=utf-8")],
        body,
    )
        .into_response()
}

async fn feed_atom(State(state): State<AppState>) -> Response {
    let docs = state.index.recent(20);
    let body = discovery::atom_feed(&site_base(&state), &state.config.site.title, &docs);
    (
        [(header::CONTENT_TYPE, "application/atom+xml; charset=utf-8")],
        body,
    )
        .into_response()
}

async fn llms(State(state): State<AppState>) -> Response {
    let docs: Vec<_> = state.index.documents().collect();
    let body = discovery::llms_txt(&site_base(&state), &state.config.site.title, &docs);
    ([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], body).into_response()
}

/// Liveness probe.
async fn healthz() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

/// Article view — the rendered body wrapped in the new chrome shell.
/// The full 2-column article layout (tabs above h1, TOC sidebar) arrives in P3;
/// P2 renders the body inside the continuous header/sitenotice/footer chrome.
async fn wiki_raw(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    Query(params): Query<HistoryQuery>,
) -> Response {
    serve_article(state, slug, params, Lang::En).await
}

/// Spanish article route (`/es/wiki/{slug}`) — same handler, `Lang::Es`.
async fn wiki_es(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    Query(params): Query<HistoryQuery>,
) -> Response {
    serve_article(state, slug, params, Lang::Es).await
}

/// Render an article in `lang` (falls back to English content when no `.es`
/// counterpart exists), with alias/redirect resolution and the as-of view.
async fn serve_article(
    state: AppState,
    slug: String,
    params: HistoryQuery,
    lang: Lang,
) -> Response {
    let slug = slug.trim_end_matches('/');
    let prefix = if lang == Lang::Es { "/es" } else { "" };
    let lang_code = if lang == Lang::Es { "es" } else { "en" };
    let Some(doc) = state.index.resolve(slug, lang) else {
        // Not a current slug — try an alias (301 to canonical), then redirects.yaml.
        if let Some(canonical) = state.index.resolve_alias(slug) {
            return moved_301(&format!("{prefix}/wiki/{canonical}"));
        }
        if let Some(to) = state.redirects.get(&format!("/{slug}")) {
            return moved_301(to);
        }
        return not_found(
            &state,
            &format!("No record found for \u{201c}{slug}\u{201d}."),
        );
    };
    let tenant = state.tenant;
    let repo_root = &state.mounts.mounts[doc.mount_index].path;
    let rel = doc.path.strip_prefix(repo_root).unwrap_or(&doc.path);

    // Point-in-time "as-of" view — render the file as it stood at ?rev=<sha>.
    if let Some(rev) = params.rev.as_deref().filter(|s| !s.is_empty()) {
        let Some((text, date)) = crate::history::file_at_rev(repo_root, rel, rev) else {
            return not_found(&state, &format!("No such revision: {rev}."));
        };
        let parsed = content::parse(&text);
        let rendered = content::render_doc(&parsed);
        let title = parsed
            .frontmatter
            .title
            .clone()
            .unwrap_or_else(|| doc.title.clone());
        let short = rev.chars().take(8).collect::<String>();
        let body = ui::article(
            &title,
            &doc.slug,
            None,
            Some(&short),
            Some(&date),
            None,
            &rendered.html,
        );
        let head = ui::doc_head(&format!("{title} (as of {date})"), "", tenant);
        return Html(
            ui::page(
                tenant,
                lang_code,
                head,
                body,
                &nav_cats(&state),
                &rendered.headings,
                "",
                state.important_info.as_deref(),
            )
            .into_string(),
        )
        .into_response();
    }

    // Current view.
    let parsed = match content::load(doc) {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("read error: {e}"),
            )
                .into_response()
        }
    };
    let rendered = content::render_doc(&parsed);
    let title = parsed
        .frontmatter
        .title
        .clone()
        .unwrap_or_else(|| doc.title.clone());
    let description = parsed
        .frontmatter
        .short_description
        .clone()
        .unwrap_or_default();
    // Provenance: the short hash of the file's most recent commit.
    let prov = crate::history::file_history(repo_root, rel, 1);
    let sha = prov.first().map(|r| r.short_sha.as_str());
    // Language toggle — only when a genuine counterpart exists.
    let alt_lang: Option<(String, &str)> = if lang == Lang::Es {
        Some((format!("/wiki/{}", doc.slug), "English"))
    } else if state
        .index
        .resolve(slug, Lang::Es)
        .map(|d| d.lang == Lang::Es)
        .unwrap_or(false)
    {
        Some((format!("/es/wiki/{}", doc.slug), "Espa\u{00f1}ol"))
    } else {
        None
    };
    let alt_ref = alt_lang.as_ref().map(|(u, l)| (u.as_str(), *l));
    let body = ui::article(
        &title,
        &doc.slug,
        parsed.frontmatter.last_edited.as_deref(),
        sha,
        None,
        alt_ref,
        &rendered.html,
    );
    let head = ui::doc_head(&title, &description, tenant);
    Html(
        ui::page(
            tenant,
            lang_code,
            head,
            body,
            &nav_cats(&state),
            &rendered.headings,
            "",
            state.important_info.as_deref(),
        )
        .into_string(),
    )
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
