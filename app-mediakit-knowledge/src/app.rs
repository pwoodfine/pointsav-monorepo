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
use axum::extract::{Path, Query, Request, State};
use axum::http::{header, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::Router;

use crate::assets::StaticAssets;
use crate::config::Config;
use std::collections::HashMap;

use crate::content::{self, ContentIndex, Lang, MountSet};
use crate::discovery;
use crate::legal::{self, LegalTokens};
use crate::search::SearchIndex;
use crate::sitedata;
use crate::ui::{self, Tenant};
use maud::html;

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
    /// Per-category English document counts, computed once here rather than
    /// rescanning the full index on every request (`ContentIndex` is
    /// immutable after startup, so this never goes stale within a process
    /// lifetime). A prior version computed this twice per home-page request
    /// alone (once directly, once again inside `nav_cats`).
    pub category_counts: Arc<std::collections::BTreeMap<String, usize>>,
    pub tenant: Tenant,
    /// Canonical copyright/trademark copy, loaded from
    /// `factory-release-engineering/tokens/legal-tokens-{brand}.yaml`. Falls
    /// back to `LegalTokens::default()` if the file is absent or malformed.
    pub legal: Arc<LegalTokens>,
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
        // `primary()`, not `mounts.first()` — a latent bug for any multi-mount
        // config (e.g. a primary + read-only guide mount in a different order):
        // `first()` silently reads whichever mount happens to be listed first
        // in knowledge.toml, not necessarily the editable primary one.
        let important_info = mounts.primary().and_then(|m| {
            std::fs::read_to_string(m.path.join("important-information.md"))
                .ok()
                .map(|text| content::render(&content::parse(&text).body_md).html)
        });
        // Per-wiki category nav + redirects from the content repo root.
        let root = mounts.primary().map(|m| m.path.clone());
        let categories = root
            .as_ref()
            .map(|r| sitedata::load_categories(r))
            .unwrap_or_default();
        let redirects = root
            .as_ref()
            .map(|r| sitedata::load_redirects(r))
            .unwrap_or_default();
        // Canonical legal copy — falls back to today's known-correct hardcoded
        // values if the token file is absent/malformed (see legal.rs).
        let legal = legal::load_default(&config.site.brand).unwrap_or_default();
        let category_counts = index.category_counts();
        Self {
            config: Arc::new(config),
            mounts: Arc::new(mounts),
            index: Arc::new(index),
            search: Arc::new(search),
            important_info: Arc::new(important_info),
            categories: Arc::new(categories),
            redirects: Arc::new(redirects),
            category_counts: Arc::new(category_counts),
            tenant,
            legal: Arc::new(legal),
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
        .route("/favicon.ico", get(favicon_ico))
        .route("/static/syntax.css", get(syntax_css_handler))
        .route("/static/{*path}", get(static_asset))
        .fallback(fallback_404)
        .with_state(state)
        // Outermost: 301 any trailing-slash path to its canonical non-slash
        // form before routing — a real finding was /wiki/{slug} and
        // /wiki/{slug}/ both serving 200 (duplicate content, no signal to
        // crawlers which is authoritative) while /category/{slug}/ 404'd,
        // an inconsistency on top of the duplication.
        .layer(middleware::from_fn(redirect_trailing_slash))
}

/// 301 a trailing-slash path (except bare `/`) to its slash-stripped form,
/// preserving the query string.
async fn redirect_trailing_slash(req: Request, next: Next) -> Response {
    let path = req.uri().path();
    if path.len() > 1 && path.ends_with('/') {
        let mut target = path.trim_end_matches('/').to_string();
        if let Some(q) = req.uri().query() {
            target.push('?');
            target.push_str(q);
        }
        return moved_301(&target);
    }
    next.run(req).await
}

/// A styled 404 for any route that doesn't match — a reviewer never sees
/// axum's default blank 404 body (a real finding: unmatched top-level paths
/// returned a bare zero-byte response instead of the site's own 404 page).
async fn fallback_404(State(state): State<AppState>) -> Response {
    not_found(&state, "No such page.")
}

/// `/favicon.ico` — browsers and crawlers request this exact path by
/// convention regardless of the `<link rel="icon">` in `<head>`; redirect to
/// the real embedded SVG rather than let it 404.
async fn favicon_ico() -> Response {
    moved_301("/static/favicon.svg")
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
    let counts = &state.category_counts;
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
    let head = ui::doc_head("Not found", "", tenant, "", true);
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
                &state.legal,
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
    // The home page is the highest-value URL on the site — never leave its
    // description empty (a prior audit found this was the only page type
    // with no meta/og:description anywhere on any of the 3 wikis).
    let description = index_parsed
        .as_ref()
        .and_then(|p| p.frontmatter.short_description.clone())
        .unwrap_or_else(|| {
            format!(
                "{} — a record repository maintained by {}.",
                tenant.home_label(),
                tenant.issuer()
            )
        });

    // Category cards — categories.yaml order/names (via nav_cats), with counts.
    // Was computed twice per home-page request (here + again inside
    // nav_cats); both now read the one value memoized in AppState::build.
    let counts = &state.category_counts;
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
    let head = ui::doc_head(tenant.home_label(), &description, tenant, "/", false);
    Html(ui::page(tenant, "en", head, body, &nav, &[], "", state.important_info.as_deref(), &state.legal).into_string()).into_response()
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
    // Avoid "Articles in the The Buildings area." when a category's display
    // name already leads with an article (a real finding: 2 of 3 wikis had
    // this doubled verbatim in search snippets).
    let description = if label.split_whitespace().next().is_some_and(|w| w.eq_ignore_ascii_case("the")) {
        format!("Articles in {label}.")
    } else {
        format!("Articles in the {label} area.")
    };
    let trail = vec![("/".to_string(), tenant.home_label().to_string())];
    let body = html! { (ui::breadcrumb(&trail, &label)) (ui::category_index(&label, &docs)) };
    let path = format!("/category/{name}");
    let home = tenant.home_url();
    let home = home.trim_end_matches('/');
    let jsonld_trail = [
        (format!("{home}/"), tenant.home_label().to_string()),
        (format!("{home}{path}"), label.clone()),
    ];
    let head = ui::doc_head(&label, &description, tenant, &path, false);
    let head = html! { (head) (ui::breadcrumb_jsonld(&jsonld_trail)) };
    Html(ui::page(tenant, "en", head, body, &nav_cats(&state), &[], "", state.important_info.as_deref(), &state.legal).into_string()).into_response()
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
    // Unbounded, query-driven URL space — crawlable (has a canonical URL,
    // useful for a crawler to discover the search feature exists) but
    // shouldn't be indexed page-by-page-per-query.
    let head = ui::doc_head(&title, &desc, tenant, "/search", true);
    Html(ui::page(tenant, "en", head, body, &nav_cats(&state), &[], &q, state.important_info.as_deref(), &state.legal).into_string()).into_response()
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
            &format!("/history/{}", doc.slug),
            false,
        );
        return Html(ui::page(tenant, "en", head, body, &nav_cats(&state), &[], "", state.important_info.as_deref(), &state.legal).into_string())
            .into_response();
    }

    let revs = crate::history::file_history(repo_root, rel, 50);
    let body = ui::history_page(&doc.title, &doc.slug, tenant.issuer(), &revs);
    let head = ui::doc_head(
        &format!("{} — History", doc.title),
        &format!("Revision history of {}", doc.title),
        tenant,
        &format!("/history/{}", doc.slug),
        false,
    );
    Html(ui::page(tenant, "en", head, body, &nav_cats(&state), &[], "", state.important_info.as_deref(), &state.legal).into_string()).into_response()
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
        "/special/all-pages",
        false,
    );
    Html(ui::page(tenant, "en", head, body, &nav_cats(&state), &[], "", state.important_info.as_deref(), &state.legal).into_string()).into_response()
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
    let head = ui::doc_head("Recent changes", "Records most recently updated.", tenant, "/special/recent-changes", false);
    Html(ui::page(tenant, "en", head, body, &nav_cats(&state), &[], "", state.important_info.as_deref(), &state.legal).into_string()).into_response()
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

// `index` is already represented by `/` (see `serve_article`'s redirect) —
// never list it a second time on a discovery surface.
fn not_index(d: &&content::DocRef) -> bool {
    d.slug != "index"
}

async fn sitemap(State(state): State<AppState>) -> Response {
    let docs: Vec<_> = state.index.documents().filter(not_index).collect();
    let body = discovery::sitemap_xml(&site_base(&state), &docs);
    (
        [(header::CONTENT_TYPE, "application/xml; charset=utf-8")],
        body,
    )
        .into_response()
}

async fn feed_atom(State(state): State<AppState>) -> Response {
    let docs: Vec<_> = state.index.recent(20).into_iter().filter(|d| d.slug != "index").collect();
    let body = discovery::atom_feed(&site_base(&state), &state.config.site.title, &docs);
    (
        [(header::CONTENT_TYPE, "application/atom+xml; charset=utf-8")],
        body,
    )
        .into_response()
}

async fn llms(State(state): State<AppState>) -> Response {
    let docs: Vec<_> = state.index.documents().filter(not_index).collect();
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
    // `index` is the home page's own lede source (see `home()`) — serving it
    // a second time at /wiki/index was a real near-duplicate-content finding
    // (both listed in the sitemap). Redirect to the canonical `/` — there is
    // no separate Spanish home route, so both languages land on the one home
    // page (matches the engine's existing single-home-route reality).
    if slug == "index" {
        return moved_301("/");
    }
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
        let body = ui::article(&title, &doc.slug, None, Some(&short), Some(&date), None, &rendered.html);
        // Canonical points at the CURRENT version, not this historical snapshot —
        // an as-of view shouldn't compete with the live article for indexing.
        let head = ui::doc_head(&format!("{title} (as of {date})"), "", tenant, &format!("{prefix}/wiki/{}", doc.slug), false);
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
                &state.legal,
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
    let article_body = ui::article(
        &title,
        &doc.slug,
        parsed.frontmatter.last_edited.as_deref(),
        sha,
        None,
        alt_ref,
        &rendered.html,
    );
    // Breadcrumb — a real finding: no page anywhere had one. Home -> Category
    // (when the article has one) -> current article (not a link).
    let mut trail = vec![("/".to_string(), tenant.home_label().to_string())];
    if let Some(cat) = doc.category.as_deref().filter(|c| !c.is_empty() && *c != "root") {
        trail.push((format!("/category/{cat}"), category_label(&state, cat)));
    }
    let body = html! { (ui::breadcrumb(&trail, &title)) (article_body) };
    let path = format!("{prefix}/wiki/{}", doc.slug);
    let head = ui::doc_head(&title, &description, tenant, &path, false);
    // hreflang — only when a genuine translation counterpart exists.
    let home = tenant.home_url();
    let home = home.trim_end_matches('/');
    let head = match &alt_lang {
        Some((alt_path, _)) => {
            let alt_code = if lang == Lang::Es { "en" } else { "es" };
            html! {
                (head)
                (ui::hreflang_links((lang_code, &format!("{home}{path}")), (alt_code, &format!("{home}{alt_path}"))))
            }
        }
        None => head,
    };
    // TechArticle + BreadcrumbList structured data — real findings: only the
    // site-level WebSite entity existed anywhere (no page-level schema), and
    // no breadcrumb schema despite the visible trail above carrying the same
    // data. `jsonld_trail` mirrors `trail` but as absolute URLs and including
    // the current page (BreadcrumbList.ListItem requires `item` even for the
    // last entry; the visible breadcrumb deliberately omits a link there).
    let current_url = format!("{home}{path}");
    let mut jsonld_trail: Vec<(String, String)> = trail
        .iter()
        .map(|(href, label)| (format!("{home}{href}"), label.clone()))
        .collect();
    jsonld_trail.push((current_url.clone(), title.clone()));
    let head = html! {
        (head)
        (ui::article_jsonld(tenant, &title, &description, &current_url, parsed.frontmatter.last_edited.as_deref()))
        (ui::breadcrumb_jsonld(&jsonld_trail))
    };
    Html(ui::page(tenant, lang_code, head, body, &nav_cats(&state), &rendered.headings, "", state.important_info.as_deref(), &state.legal).into_string())
        .into_response()
}

/// Serve an embedded static asset by path.
async fn static_asset(headers: axum::http::HeaderMap, Path(path): Path<String>) -> Response {
    match StaticAssets::get(&path) {
        Some(file) => {
            // Content-addressed ETag (the embedded asset's own sha256). NOT
            // `immutable`/a year-long max-age — this engine doesn't
            // cache-bust URLs (no ?v= or content-hashed filenames), so a
            // browser told never to revalidate would keep serving stale CSS/
            // JS long after a real deploy. `max-age=3600` (matching the
            // existing precedent on /static/syntax.css) plus a working
            // If-None-Match/304 path is the safe version of the same fix.
            // A real finding: only syntax.css (generated separately, not
            // through this handler) carried any caching headers; every other
            // static asset — app.css, tokens.css, fonts.css, content.css,
            // app.js, icons — was re-downloaded in full on every navigation.
            let etag = format!("\"{}\"", hex_encode(&file.metadata.sha256_hash()));
            if headers
                .get(header::IF_NONE_MATCH)
                .and_then(|v| v.to_str().ok())
                .is_some_and(|v| v == etag)
            {
                return Response::builder()
                    .status(StatusCode::NOT_MODIFIED)
                    .header(header::ETAG, etag)
                    .header(header::CACHE_CONTROL, "public, max-age=3600")
                    .body(Body::empty())
                    .unwrap();
            }
            let mime = mime_guess::from_path(&path).first_or_octet_stream();
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime.as_ref())
                .header(header::CACHE_CONTROL, "public, max-age=3600")
                .header(header::ETAG, etag)
                .body(Body::from(file.data.into_owned()))
                .unwrap()
        }
        None => (StatusCode::NOT_FOUND, "asset not found").into_response(),
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}
