//! HTTP server and route handlers.
//!
//! Phase 1.1 additions (all additive — no existing routes or responses changed):
//! - `wiki_chrome()` — full article-page shell with Wikipedia muscle-memory chrome
//! - Article / Talk tabs (top-left)
//! - Read / Edit / View history tabs (top-right; Edit + View-history are href="#" placeholders)
//! - IVC masthead band placeholder ("verification not yet available — Phase 7")
//! - Collapsible left-rail TOC (pure CSS + minimal JS; JS loaded from /static/wiki.js)
//! - Language-switcher button (populated from frontmatter `translations:`)
//! - Hatnote (italic, indented; rendered when frontmatter has a `hatnote:` field)
//! - "From PointSav Knowledge" tagline below the page title
//! - Reader density toggle (Off / Exceptions only / All; persisted to localStorage)
//! - Per-section [edit] pencils (injected by render::inject_edit_pencils)
//! - Footer convention (categories → license → about/contact links)
//! - The existing `chrome()` function is retained for the index page.
//!
//! Iteration-2 additions (all additive — no existing behaviour changed):
//! - Recursive content-directory walk: `collect_topic_files()` descends into
//!   category subdirectories (`architecture/`, `services/`, etc.) so that all
//!   130+ TOPICs are visible to the bucketing, featured-pin, and slug-resolution
//!   logic. Slugs for subdirectory TOPICs take the form `<category>/<stem>`.
//! - `short_description` subtitle: rendered as `<p class="article__lede">
//!   <em>…</em></p>` immediately below the article H1.
//! - Leapfrog 2030 facts panel: reads `leapfrog-facts.yaml` from `content_dir`;
//!   renders a "Leapfrog 2030" bullet panel on the home page right column.
//! - Breadcrumb navigation: `category:` frontmatter → "Documentation > Category > Title"
//!   breadcrumb rendered above the article TOC rail.
//! - Language toggle auto-detection (Item 11): `wiki_page()` checks for a `.es.md`
//!   sibling on disk and auto-injects the EN↔ES toggle without requiring explicit
//!   `translations:` frontmatter. EN articles get an "es" link; ES articles (`*.es`)
//!   get an "en" link back to the base slug. Explicit `translations:` frontmatter
//!   takes precedence over auto-detection when present and non-empty.

use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use maud::{html, Markup, PreEscaped, DOCTYPE};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;
use std::path::{Path as FsPath, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::fs;

use crate::assets::StaticAsset;
use crate::chrome::sovereign::{
    sovereign_footer, sovereign_mobile_nav_drawer, sovereign_nav, sovereign_page,
    sovereign_secondary_nav, Tenant,
};
use crate::error::WikiError;
use crate::jsonld::jsonld_for_topic;
use crate::render::{
    extract_headings, inject_edit_pencils, parse_page, render_html_raw, Frontmatter, TranslationMap,
};
use crate::search::{search as run_search, SearchIndex};

// ── Read-only chrome placeholders (auth removed — git-only workflow) ────────
//
// Auth, sessions, edit review, and SQLite were removed when the wiki moved to a
// git-only contribution workflow. The chrome layer still threads an optional
// "current user" and a pending-edit count through every handler signature; to
// avoid churning ~40 handlers, those names survive as inert placeholders:
//
//   * `User`           — never constructed; kept so handler signatures and the
//                        nav widget compile. Always treated as anonymous.
//   * `CurrentUser`    — an axum extractor that always yields `None`.
//   * `pending_count_for` — always returns 0 (no review queue).
//   * `validate_slug`  — conservative slug check (relocated from the old edit
//                        module); used by the raw-markdown and history handlers.

/// Inert user placeholder. Never constructed now that auth is removed; present
/// only so handler signatures and the anonymous nav widget continue to compile.
#[allow(dead_code)]
pub struct User {
    pub username: String,
}

impl User {
    #[allow(dead_code)]
    pub fn is_admin(&self) -> bool {
        false
    }
}

/// Request extractor that previously resolved the session cookie to a user.
/// With auth removed it always yields `None` — every request is anonymous.
pub struct CurrentUser(pub Option<User>);

impl axum::extract::FromRequestParts<Arc<AppState>> for CurrentUser {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        _parts: &mut axum::http::request::Parts,
        _state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        Ok(CurrentUser(None))
    }
}

/// Pending-edit count for the nav badge. No review queue exists in the git-only
/// workflow, so this is always zero.
async fn pending_count_for(_state: &AppState, _user: Option<&User>) -> i64 {
    0
}

/// Validate a slug. Allowed: lowercase ASCII letters, digits, dots, hyphens,
/// underscores, and `/` (for category-scoped slugs). Rejects empty, leading
/// dot, `..` sequence, and anything else. Relocated from the removed edit module.
pub fn validate_slug(slug: &str) -> Result<(), WikiError> {
    if slug.is_empty() {
        return Err(WikiError::SlugInvalid("empty".into()));
    }
    if slug.starts_with('.') {
        return Err(WikiError::SlugInvalid(slug.into()));
    }
    if slug.contains("..") {
        return Err(WikiError::SlugInvalid(slug.into()));
    }
    for c in slug.chars() {
        match c {
            'a'..='z' | '0'..='9' | '.' | '-' | '_' | '/' => {}
            _ => return Err(WikiError::SlugInvalid(slug.into())),
        }
    }
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Locale {
    #[default]
    En,
    Es,
}

impl Locale {
    fn lang_attr(self) -> &'static str {
        match self {
            Locale::En => "en",
            Locale::Es => "es",
        }
    }
    fn suffix(self) -> &'static str {
        match self {
            Locale::En => "",
            Locale::Es => ".es",
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    /// Phase 0: declarative mount set. Index 0 is always the primary (editable)
    /// content mount; subsequent entries are read-only guide/fleet mounts.
    /// Replaces the former `content_dir`, `guide_dir`, `guide_dir_2` flat fields.
    pub mounts: Vec<crate::mounts::Mount>,
    /// Path to the workspace citation registry YAML file.
    /// Defaults to `/srv/foundry/citations.yaml`; overridable via
    /// `--citations-yaml` / `WIKI_CITATIONS_YAML`.
    pub citations_yaml: PathBuf,
    /// Display name for this wiki instance, shown in the browser tab, site
    /// header, and home-page H1 fallback. Set via `--site-title` /
    /// `WIKI_SITE_TITLE`. Default: `"PointSav Documentation Wiki"`.
    pub site_title: String,
    /// Phase 3 Step 3.2: tantivy full-text search index. Built on
    /// startup from a tree walk of `content_dir`; reindexed on every
    /// successful edit / create. Clone-cheap (Arc-wrapped internals).
    pub search: Arc<SearchIndex>,
    /// Phase 4 Step 4.1: git2 repository for content versioning. Mutex-wrapped
    /// because Repository is not thread-safe for mutating operations.
    pub git: Arc<Mutex<git2::Repository>>,
    /// Phase 4 Step 4.7: tenant name for the read-only git remote.
    pub git_tenant: String,
    /// Phase 4 Step 4.6: when true, mount `POST /mcp` and expose the
    /// MCP JSON-RPC 2.0 endpoint. Default off — the route is absent
    /// when this flag is not set.
    pub mcp_enabled: bool,
    /// Phase 10: Leapfrog 2030 glossary auto-linker.
    pub glossary: Arc<crate::glossary::Glossary>,
    /// Phase 4 Steps 4.4+4.5: redb-backed wikilink graph and blake3 hashes.
    /// Always present; database file at `<state_dir>/links.redb`.
    pub links: Arc<crate::links::LinkGraph>,
    /// Optional brand theme selector. When set to `"woodfine"`, BCSC
    /// forward-looking-statement disclaimer appears in all page footers.
    /// Set via `--brand-theme` / `WIKI_BRAND_THEME`.
    pub brand_theme: Option<String>,
    /// Brand instance selector for the hybrid UI parameterisation
    /// (`html[data-instance]`). Allowed: `documentation`, `projects`,
    /// `corporate`. Set via `WIKI_BRAND_INSTANCE`; default
    /// `"documentation"`.
    pub brand_instance: String,
    /// Phase 0: content-type blueprint registry. Built-ins (topic, guide) plus
    /// any customer `blueprints/*.yaml` files loaded from the primary mount.
    /// Wires blueprints.rs into the render pipeline at dispatch time.
    pub blueprints: crate::blueprints::Registry,
    /// Phase 7: peer wiki instances for federated `knowledge/search`.
    /// Empty by default; populated from `[[peer]]` entries in knowledge.toml.
    pub peers: Vec<crate::config::PeerConfig>,
    /// Canonical base URL (no trailing slash) used in sitemap.xml `<loc>` values.
    /// Populated from `knowledge.toml [site] canonical_url`. `None` → relative URLs.
    pub canonical_url: Option<String>,
    /// Phase 7: ActivityPub outbox URL for `on_article_saved()` emission.
    /// Populated from `knowledge.toml [federation] outbox_url`. `None` → disabled.
    pub activitypub_outbox_url: Option<String>,
    /// Per-instance "New here? Start with these" chips from `[[start_here]]` in knowledge.toml.
    /// Empty → engine renders four hardcoded PointSav documentation chips (backward-compat default).
    pub start_here: Vec<crate::config::StartHereEntry>,
}

impl AppState {
    /// Primary content directory — mount 0's path.
    pub fn primary_path(&self) -> &std::path::Path {
        &self.mounts[0].path
    }

    /// Non-primary mount paths checked alongside the primary when resolving
    /// wikilinks (TOPIC↔GUIDE cross-mount resolution).
    pub fn link_roots(&self) -> Vec<&std::path::Path> {
        crate::mounts::link_roots(&self.mounts)
    }

    /// Two-slot array of optional guide paths, compatible with legacy callers
    /// that take `(Option<&Path>, Option<&Path>)` or `&[Option<&Path>]`.
    pub fn guide_dirs_arr(&self) -> [Option<&std::path::Path>; 2] {
        let roots = crate::mounts::link_roots(&self.mounts);
        [roots.first().copied(), roots.get(1).copied()]
    }
}

pub fn router(state: AppState) -> Router {
    let mcp_enabled = state.mcp_enabled;
    let mut r = Router::new()
        .route("/", get(index))
        // Static pages (Disclaimer, Contact, etc.) served on-domain with wiki chrome
        .route("/page/{slug}", get(page_handler))
        // Wildcard capture allows category-scoped slugs: `/wiki/architecture/compounding-substrate`
        .route("/wiki/{*slug}", get(wiki_page))
        .route("/es/", get(home_es))
        .route("/es/wiki/{*slug}", get(wiki_page_es))
        .route("/static/{*path}", get(static_asset))
        .route("/images/{*path}", get(serve_content_image))
        .route("/healthz", get(healthz))
        .route("/health", get(healthz))
        // Phase 2 Step 5 — citation registry for autocomplete
        .route("/api/citations", get(crate::citations::get_citations))
        // D2: search autocomplete
        .route("/api/complete", get(search_complete))
        // Leapfrog: Page Preview hover endpoint
        .route("/api/preview/{*slug}", get(preview_api))
        // Doorman AI-assist endpoints — reserved, not implemented in this build.
        // Return 501 (not 404) with a JSON envelope so clients can surface a
        // clear "not available" message. See tests/doorman_test.rs.
        .route("/api/doorman/complete", post(doorman_complete_stub))
        .route("/api/doorman/instruct", post(doorman_instruct_stub))
        // Wave 5B — category listing pages
        .route("/category/{name}", get(category_page))
        // Phase 3 Step 3.2 — full-text search HTML page over the tantivy index
        .route("/search", get(search_page))
        // Sprint M — cross-instance federated search HTML page
        .route("/search/all", get(search_all_page))
        .route("/wanted", get(wanted_page))
        .route("/random", get(random_page))
        // Phase 4 Step 4.6 — MCP route mounted conditionally; see mcp_enabled guard below
        // Phase 3 Step 3.3 — Atom + JSON Feed syndication
        .route("/feed.atom", get(crate::feeds::get_atom))
        .route("/feed.json", get(crate::feeds::get_json_feed))
        // Phase 3 Step 3.4 — crawler discovery + raw Markdown source
        .route("/sitemap.xml", get(sitemap_xml))
        .route("/robots.txt", get(robots_txt))
        .route("/llms.txt", get(llms_txt))
        // Phase 4 Step 4.2 — history and blame
        .route("/history/{*slug}", get(history_page))
        .route("/blame/{*slug}", get(blame_page))
        .route("/diff/{*slug}", get(diff_page))
        // Sprint C — special pages
        .route("/special/recent-changes", get(recent_changes_page))
        .route("/special/all-pages", get(all_pages_page))
        .route("/special/statistics", get(statistics_page))
        // Sprint C4 — Talk namespace
        .route("/talk/{*slug}", get(talk_page).post(talk_post))
        // Phase 4 Step 4.7 — read-only git remote (smart-HTTP)
        .route(
            "/git-server/{tenant}/info/refs",
            get(crate::git_protocol::info_refs),
        )
        .route(
            "/git-server/{tenant}/git-upload-pack",
            post(crate::git_protocol::upload_pack),
        )
        // axum 0.8 doesn't allow a literal `.md` suffix after a dynamic
        // segment, so the route captures `{slug}` as a single segment and
        // the handler strips an optional trailing `.md` for the
        // git-clone-style UX (`/git/topic-foo.md` or `/git/topic-foo`).
        .route("/git/{*slug}", get(git_markdown))
        // L25: editor route stub — loads CodeMirror only on /edit/* (route-gated bundle)
        .route("/edit/{*slug}", get(edit_page))
        // Wikipedia-parity special pages
        .route("/special/whatlinkshere/{*slug}", get(what_links_here))
        .route("/special/pageinfo/{*slug}", get(page_info))
        .route("/special/cite/{*slug}", get(cite_page))
        .route("/special/categories", get(categories_index_page))
        .route("/special/hash-lookup/{hash}", get(hash_lookup_page))
        .route("/special/specialpages", get(special_pages_index))
        .route("/special/wanted", get(wanted_page))
        // Phase 4 Step 4.8 — OpenAPI 3.1 specification
        .route("/openapi.yaml", get(openapi_yaml))
        .route("/openapi.json", get(openapi_json))
        // P2: same-origin page-view beacon (no cookies, no third-party network)
        .route("/_beacon", post(beacon_handler));
    // Phase 4 Step 4.6 — MCP JSON-RPC 2.0 endpoint; only mounted when
    // --enable-mcp is set (default off).
    if mcp_enabled {
        r = r.route("/mcp", post(crate::mcp::handler));
    }
    r.with_state(Arc::new(state))
}

/// P2: same-origin page-view beacon. Accepts a JSON body `{"u": "/path", "t": <ms>}` from
/// `navigator.sendBeacon` and returns 204 immediately. No cookies. No third-party network.
async fn beacon_handler() -> impl axum::response::IntoResponse {
    axum::http::StatusCode::NO_CONTENT
}

/// Doorman AI-assist stubs (Phase 4 reserved surface).
///
/// `/api/doorman/complete` and `/api/doorman/instruct` are reserved endpoints
/// that are not implemented in this build. They return 501 Not Implemented with
/// a JSON envelope (`phase`, `reason`) so a client can surface a clear
/// "not available" message rather than a bare 404.
async fn doorman_complete_stub() -> impl axum::response::IntoResponse {
    doorman_not_implemented("Doorman completion assist is not implemented in this build")
}

async fn doorman_instruct_stub() -> impl axum::response::IntoResponse {
    doorman_not_implemented("Doorman instruction assist is not implemented in this build")
}

/// Derive a content-type display label from the leading slug prefix.
fn content_type_label(slug: &str) -> &'static str {
    match slug.split('/').next().unwrap_or("") {
        "how-to" | "guides" => "Guide",
        "reference" => "Reference",
        "patterns" | "applications" | "substrate" | "design-system" | "services" | "governance"
        | "infrastructure" | "architecture" => "Topic",
        _ => "",
    }
}

fn doorman_not_implemented(reason: &str) -> impl axum::response::IntoResponse {
    (
        axum::http::StatusCode::NOT_IMPLEMENTED,
        axum::Json(serde_json::json!({ "phase": 4, "reason": reason })),
    )
}

/// D2: `GET /api/complete?q={prefix}` — title autocomplete for search box.
///
/// Returns a JSON array of `{title, slug}` objects whose titles or slugs
/// contain the query string (case-insensitive). Capped at 10 results.
async fn search_complete(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchQueryParams>,
) -> Json<serde_json::Value> {
    let q = params.q.to_lowercase();
    if q.is_empty() {
        return Json(json!([]));
    }
    let topic_files = collect_all_topic_files(state.primary_path(), &state.guide_dirs_arr())
        .await
        .unwrap_or_default();

    let mut hits = Vec::new();
    for tf in &topic_files {
        if hits.len() >= 10 {
            break;
        }
        let (title, lede) = if let Ok(text) = fs::read_to_string(&tf.path).await {
            if let Ok(p) = crate::render::parse_page(&text) {
                let t = p.frontmatter.title.unwrap_or_else(|| tf.slug.clone());
                let l = p
                    .frontmatter
                    .short_description
                    .unwrap_or_else(|| crate::feeds::first_paragraph_snippet(&p.body_md, 120));
                (t, l)
            } else {
                (tf.slug.clone(), String::new())
            }
        } else {
            (tf.slug.clone(), String::new())
        };
        if title.to_lowercase().contains(&q) || tf.slug.to_lowercase().contains(&q) {
            hits.push(json!({"title": title, "slug": tf.slug, "lede": lede}));
        }
    }
    Json(json!(hits))
}

async fn preview_api(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
) -> Result<Json<serde_json::Value>, WikiError> {
    if slug.contains("..") || slug.is_empty() {
        return Err(WikiError::NotFound(slug));
    }

    let path = state.primary_path().join(format!("{slug}.md"));
    let text = match fs::read_to_string(&path).await {
        Ok(t) => t,
        Err(_) => return Err(WikiError::NotFound(slug)),
    };

    let parsed = parse_page(&text)?;
    let title = parsed
        .frontmatter
        .title
        .unwrap_or_else(|| slug.rsplit('/').next().unwrap_or(&slug).replace('-', " "));
    let snippet = crate::feeds::first_paragraph_snippet(&parsed.body_md, 300);
    let image_url = crate::feeds::first_image_url(&parsed.body_md);

    Ok(Json(json!({
        "title": title,
        "snippet": snippet,
        "image_url": image_url,
        "slug": slug
    })))
}

async fn healthz() -> &'static str {
    "ok"
}

/// `GET /openapi.yaml` — Phase 4 Step 4.8.
///
/// Serves the hand-authored OpenAPI 3.1 specification embedded at compile
/// time via `include_str!`. Always current — no runtime I/O.
async fn openapi_yaml() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/yaml")],
        include_str!("../../openapi.yaml"),
    )
}

/// `GET /openapi.json` — compatibility alias; 301-redirects to `/openapi.yaml`.
///
/// Allows tool discovery via the conventional `.json` path without duplicating
/// the spec or misrepresenting the content type.
async fn openapi_json() -> impl IntoResponse {
    axum::response::Redirect::permanent("/openapi.yaml")
}

#[derive(Deserialize)]
struct IndexQueryParams {
    /// Present as `?noredirect=1` (or any value) to suppress Accept-Language → /es/ redirect.
    noredirect: Option<String>,
}

#[derive(Deserialize)]
struct SearchQueryParams {
    #[serde(default)]
    q: String,
    /// Filter results to a single category slug (matches the slug prefix, e.g. `architecture`).
    category: Option<String>,
    /// Filter results to a single status value (e.g. `active`, `stub`, `pre-build`).
    status: Option<String>,
}

#[derive(Deserialize)]
struct WikiPageQuery {
    redirectedfrom: Option<String>,
    #[serde(default)]
    printable: bool,
    /// Past-revision view: a git SHA prefix, ref name, or `SHA~` parent
    /// shorthand. When present, reads from git history instead of disk
    /// (content_dir only). Blame enrichment is skipped for past revisions.
    asof: Option<String>,
}

/// `GET /wanted` — "Wanted articles" page.
///
/// Walks every .md file in content_dir, renders each, and collects all
/// anchors tagged with `class="wiki-redlink"`. Returns a table sorted by
/// inbound-link count (most-wanted first), matching Wikipedia's Special:WantedPages.
/// `GET /random` — redirect to a randomly chosen article.
///
/// Collects all topic slugs, picks one using a time-seeded index (no external
/// crate needed), and issues a 302 redirect to `/wiki/<slug>`. Returns 404
/// when the content directory is empty.
async fn random_page(State(state): State<Arc<AppState>>) -> Result<Response, WikiError> {
    let topic_files =
        collect_all_topic_files(state.primary_path(), &state.guide_dirs_arr()).await?;
    let slugs: Vec<String> = topic_files.into_iter().map(|tf| tf.slug).collect();
    if slugs.is_empty() {
        return Err(WikiError::NotFound("random".into()));
    }
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos() as usize;
    let slug = &slugs[nanos % slugs.len()];
    Ok(Response::builder()
        .status(StatusCode::FOUND)
        .header(header::LOCATION, format!("/wiki/{slug}"))
        .body(axum::body::Body::empty())
        .unwrap())
}

async fn wanted_page(
    State(state): State<Arc<AppState>>,
    CurrentUser(maybe_user): CurrentUser,
) -> Result<Markup, WikiError> {
    let pending_count = pending_count_for(&state, maybe_user.as_ref()).await;
    let re = Regex::new(r#"href="/wiki/([^"]+)"[^>]*class="wiki-redlink""#).expect("static regex");

    // Walk all topic files and collect redlinks.
    let topic_files =
        collect_all_topic_files(state.primary_path(), &state.guide_dirs_arr()).await?;

    let mut wanted: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for tf in &topic_files {
        if let Ok(text) = fs::read_to_string(&tf.path).await {
            let html =
                crate::render::render_html_raw(&text, state.primary_path(), &state.link_roots());
            for cap in re.captures_iter(&html) {
                let missing = cap[1].to_string();
                wanted.entry(missing).or_default().push(tf.slug.clone());
            }
        }
    }

    // Sort by inbound count descending.
    let mut rows: Vec<(String, Vec<String>)> = wanted.into_iter().collect();
    rows.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.cmp(&b.0)));

    Ok(chrome(
        &format!("Wanted articles — {}", state.site_title),
        html! {
            h1 { "Wanted articles" }
            p.wiki-wanted-intro {
                "Articles linked from other pages that do not yet exist. "
                "Most-linked first."
            }
            @if rows.is_empty() {
                p { em { "No wanted articles — all wikilinks resolve." } }
            } @else {
                table.wiki-wanted-table {
                    thead {
                        tr {
                            th { "Missing article" }
                            th { "Links in" }
                            th { "Linked from" }
                        }
                    }
                    tbody {
                        @for (slug, sources) in &rows {
                            tr {
                                td { (slug) }
                                td.wanted-count { (sources.len()) }
                                td.wanted-sources {
                                    @for (i, src) in sources.iter().enumerate() {
                                        @if i > 0 { ", " }
                                        a href={ "/wiki/" (src) } { (src) }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        },
        &state.site_title,
        maybe_user.as_ref(),
        pending_count,
    ))
}

/// Phase 3 Step 3.2 — `GET /search?q=...` HTML results page.
///
/// Empty query → empty results + the search form. Renders within the
/// existing `chrome()` shell for layout consistency with the index page.
/// Phase 3.x may upgrade to autocomplete + image previews per UX-DESIGN.md
/// §1 item 13; Phase 3.2 ships the basic form + ranked result list.
async fn search_page(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchQueryParams>,
    CurrentUser(maybe_user): CurrentUser,
) -> Result<Markup, WikiError> {
    let query = params.q.trim().to_string();
    let filter_category = params
        .category
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let filter_status = params
        .status
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());

    let raw_hits = if query.is_empty() {
        Vec::new()
    } else {
        run_search(&state.search, &query, 50)?
    };

    // Post-search filters: category by slug prefix, status by frontmatter read.
    let mut hits = Vec::new();
    for hit in raw_hits {
        if hits.len() >= 25 {
            break;
        }
        // Category filter: slug prefix match (e.g. "architecture/slug").
        if let Some(cat) = filter_category {
            let prefix = format!("{cat}/");
            if !hit.slug.starts_with(&prefix) {
                continue;
            }
        }
        // Status filter: read frontmatter (only for filtered result set, ≤50 hits).
        if let Some(wanted_status) = filter_status {
            let path = state.primary_path().join(format!("{}.md", hit.slug));
            let ok = if let Ok(text) = fs::read_to_string(&path).await {
                if let Ok((fm, _)) = crate::walker::parse_frontmatter(&text) {
                    fm.status.as_deref() == Some(wanted_status)
                } else {
                    false
                }
            } else {
                false
            };
            if !ok {
                continue;
            }
        }
        hits.push(hit);
    }

    let pending_count = pending_count_for(&state, maybe_user.as_ref()).await;
    Ok(chrome(
        if query.is_empty() {
            "Search".to_string()
        } else {
            format!("Search: {query}")
        }
        .as_str(),
        html! {
            h1 { "Search" }
            form.search-form action="/search" method="get" {
                input
                    type="search"
                    name="q"
                    value=(query)
                    placeholder="Search TOPICs"
                    autocomplete="off"
                    autofocus?[query.is_empty()];
                input type="hidden" name="category" value=(filter_category.unwrap_or(""));
                input type="hidden" name="status" value=(filter_status.unwrap_or(""));
                button type="submit" { "Search" }
            }
            @if !query.is_empty() {
                @if hits.is_empty() {
                    p.search-empty {
                        "No results for "
                        em { (query) }
                        "."
                        @if filter_category.is_some() || filter_status.is_some() {
                            " (filters active — "
                            a href={ "/search?q=" (query) } { "clear filters" }
                            ")"
                        }
                    }
                } @else {
                    p.search-summary {
                        (hits.len())
                        " result" @if hits.len() != 1 { "s" }
                        " for "
                        em { (query) }
                        @if let Some(_cat) = filter_category {
                            " in category "" (_cat) """
                        }
                        @if let Some(_st) = filter_status {
                            " with status "" (_st) """
                        }
                        "."
                    }
                    ol.search-results {
                        @for hit in &hits {
                            li.search-hit {
                                div.search-hit-header {
                                    a.search-hit-title href={ "/wiki/" (hit.slug) } {
                                        (hit.title)
                                    }
                                    @let ct = content_type_label(&hit.slug);
                                    @if !ct.is_empty() {
                                        span.search-hit-type { (ct) }
                                    }
                                }
                                span.search-hit-slug { (hit.slug) }
                                @if !hit.snippet.is_empty() {
                                    p.search-hit-snippet { (hit.snippet) }
                                }
                            }
                        }
                    }
                }
            }
            @if !state.peers.is_empty() && !query.is_empty() {
                p.search-all-link {
                    a href={ "/search/all?q=" (query) } {
                        "Search all sites →"
                    }
                }
            }
        },
        &state.site_title,
        maybe_user.as_ref(),
        pending_count,
    ))
}

/// Sprint M — `GET /search/all?q=...` cross-instance federated search page.
///
/// Fans out to all peer instances via `federation_search()` (reuses the same
/// logic as `POST /mcp` `search` method). When no peers are configured, falls
/// back gracefully to the local-only results with a note. Only available when
/// the binary is configured with `[[peer]]` entries in knowledge.toml, though
/// the route is always mounted so the URL is stable.
async fn search_all_page(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchQueryParams>,
    CurrentUser(maybe_user): CurrentUser,
) -> Result<Markup, WikiError> {
    let query = params.q.trim().to_string();

    let results: Vec<serde_json::Value> = if query.is_empty() {
        Vec::new()
    } else {
        let fed_params = serde_json::json!({ "query": query, "limit": 30 });
        match crate::mcp::federation_search(&state, &fed_params).await {
            Ok(v) => v["results"].as_array().cloned().unwrap_or_default(),
            Err((_, msg)) => {
                tracing::warn!(err = %msg, "search/all: federation_search error");
                Vec::new()
            }
        }
    };

    let pending_count = pending_count_for(&state, maybe_user.as_ref()).await;
    Ok(chrome(
        if query.is_empty() {
            "Search all sites".to_string()
        } else {
            format!("Search all: {query}")
        }
        .as_str(),
        html! {
            h1 { "Search all sites" }
            @if state.peers.is_empty() {
                p.search-note {
                    "This instance has no peer sites configured. "
                    a href="/search" { "Search this site →" }
                }
            }
            form.search-form action="/search/all" method="get" {
                input
                    type="search"
                    name="q"
                    value=(query)
                    placeholder="Search across all sites"
                    autocomplete="off"
                    autofocus?[query.is_empty()];
                button type="submit" { "Search" }
            }
            @if !query.is_empty() {
                @if results.is_empty() {
                    p.search-empty {
                        "No results for "
                        em { (query) }
                        " across all sites."
                    }
                } @else {
                    p.search-summary {
                        (results.len())
                        " result" @if results.len() != 1 { "s" }
                        " for "
                        em { (query) }
                        " across all sites."
                    }
                    ol.search-results {
                        @for hit in &results {
                            @let slug  = hit["slug"].as_str().unwrap_or("");
                            @let title = hit["title"].as_str().unwrap_or(slug);
                            @let lede  = hit["lede"].as_str().unwrap_or("");
                            @let inst  = hit["instance"].as_str().unwrap_or("");
                            li.search-hit {
                                a.search-hit-title href={ "/wiki/" (slug) } { (title) }
                                span.search-hit-slug { (slug) }
                                @if !inst.is_empty() {
                                    span.instance-badge { (inst) }
                                }
                                @if !lede.is_empty() {
                                    p.search-hit-snippet { (lede) }
                                }
                            }
                        }
                    }
                }
            }
        },
        &state.site_title,
        maybe_user.as_ref(),
        pending_count,
    ))
}

// ─── Home-page data types ───────────────────────────────────────────────────

/// Summary of a single TOPIC used in the home-page category panels and
/// recent-additions feed.
#[derive(Debug, Clone)]
pub struct TopicSummary {
    /// Slug (filename without `.md`; category-scoped for subdirectory files).
    pub slug: String,
    /// Title from frontmatter, or the slug when absent.
    pub title: String,
    /// `last_edited:` frontmatter value; may be None if not set.
    pub last_edited: Option<String>,
    /// `short_description` from frontmatter; may be None if not set.
    pub short_description: Option<String>,
    /// `hero_image` from frontmatter; may be None if not set.
    pub hero_image: Option<String>,
    /// `status` from frontmatter: `stable | pre-build | draft | stub`.
    pub status: Option<String>,
    /// First non-blank, non-heading line of the body Markdown.
    pub lede_first_line: String,
    /// Absolute path to the source file on disk (used for git fallback).
    pub file_path: PathBuf,
}

/// Wikipedia-style stats banner shown immediately under the welcome lede.
/// Renders as: "N articles across N categories — last updated YYYY-MM-DD."
/// Per `content-wiki-documentation/index.md` ENGINE comment + iteration-2
/// home-page leapfrog primitive (Wikipedia welcome-banner pattern preserved).
#[derive(Debug, Clone)]
pub struct HomeStats {
    pub article_count: usize,
    pub category_count: usize,
    pub last_updated: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
struct FeaturedTopicPin {
    slug: String,
    since: Option<String>,
    note: Option<String>,
}

struct FeaturedArticle {
    title: String,
    slug: String,
    snippet: String,
    hero_image: Option<String>,
}

#[derive(Deserialize)]
#[allow(dead_code)] // Validation-only struct: Deserialize enforces shape; fields unread by design.
struct LeapfrogFacts {
    facts: Vec<LeapfrogFact>,
}

#[derive(Deserialize)]
#[allow(dead_code)] // Validation-only struct: Deserialize enforces shape; fields unread by design.
struct LeapfrogFact {
    text: String,
    link_slug: Option<String>,
}

#[derive(Deserialize)]
#[allow(dead_code)] // Validation-only struct: Deserialize enforces shape; fields unread by design.
struct ReferenceInvariants {
    heading: String,
    items: Vec<ReferenceInvariant>,
}

#[derive(Deserialize)]
#[allow(dead_code)] // Validation-only struct: Deserialize enforces shape; fields unread by design.
struct ReferenceInvariant {
    label: Option<String>,
    text: String,
    link_slug: Option<String>,
}

/// Category buckets: `BTreeMap<category_name, Vec<TopicSummary>>`.
pub type CategoryBuckets = BTreeMap<String, Vec<TopicSummary>>;

/// Ratified internal article-classification category set in canonical order.
/// Per naming-convention.md §4 (ratified 2026-05-09). Order is immutable.
/// These are the article frontmatter `category:` values — not the home page
/// navigation tiles (see HOMEPAGE_CATEGORIES for the slide IA layer).
const RATIFIED_CATEGORIES: &[&str] = &[
    "architecture",
    "substrate",
    "patterns",
    "systems",
    "services",
    "applications",
    "governance",
    "infrastructure",
    "reference",
    "design-system",
];

/// Homepage browse grid: 7-category slide IA (project-orgcharts JW4, approved 2026-06-14).
/// BRIEF-design-system-slides.md: "Same IA governs both the slide deck and wiki documentation."
///
/// Each entry: (display_name, primary_slug, description, all_slugs).
/// `primary_slug` is the /category/<slug> URL — maps to the most representative
/// internal category so the linked page has real content.
/// `description` is the JW4 approved one-sentence description for the tile.
/// `all_slugs` are the internal article categories summed for the article count.
///
/// Mapping is exclusive (no internal category repeated) pending operator confirmation
/// (see BRIEF-knowledge-platform-master.md §8.4 open decision #1).
/// Each entry: (display_name, primary_slug, description, accent_color, all_slugs).
/// `accent_color` is a CSS color value used as `--cat-accent` on the tile border-top.
const HOMEPAGE_CATEGORIES: &[(&str, &str, &str, &str, &[&str])] = &[
    (
        "Developer Platform",
        "reference",
        "Who we are, how you join, and the house style for everything running on the platform.",
        "#7c3aed",
        &["design-system", "reference", "governance"],
    ),
    (
        "Operator Workspace",
        "applications",
        "The Console OS surfaces operators work in every day.",
        "#0d9488",
        &["applications"],
    ),
    (
        "System of Record",
        "systems",
        "Toteboxes, archives, and the services that keep the records.",
        "#164679",
        &["systems"],
    ),
    (
        "Integration & Data Portability",
        "services",
        "",
        "#b45309",
        &["services", "patterns"],
    ),
    (
        "Machine-Based Authorization",
        "infrastructure",
        "Pairing as permission across the private network — authorization by device, not by role.",
        "#4f46e5",
        &["infrastructure"],
    ),
    (
        "Multi-Entity Consolidation",
        "architecture",
        "Aggregating fleets of archives and scaling across user tiers and composition.",
        "#c7a961",
        &["architecture"],
    ),
    (
        "Platform Foundation",
        "substrate",
        "Where the platform runs — on-prem, leased, public cloud, hybrid — and the GIS engine beneath it.",
        "#166534",
        &["substrate"],
    ),
];

// ─── Home-page helpers ──────────────────────────────────────────────────────

/// A single topic file discovered during a recursive walk of `content_dir`.
///
/// `slug` is the routing slug used in `/wiki/<slug>` URLs. For files
/// directly in `content_dir` the slug equals the filename stem (e.g.,
/// `topic-hello`). For files in a subdirectory the slug is
/// `<subdir>/<stem>` (e.g., `architecture/compounding-substrate`).
pub struct TopicFile {
    pub slug: String,
    pub path: PathBuf,
}

/// Repo-management files that are not wiki content. Filtered out at the
/// root level of any content directory so they never appear in article
/// listings or the "All articles" catch-all section.
const SYSTEM_FILE_STEMS: &[&str] = &[
    "README",
    "CHANGELOG",
    "MANIFEST",
    "CLAUDE",
    "AGENT",
    "NEXT",
    "NOTAM",
    "TRADEMARK",
    "CODE_OF_CONDUCT",
    "BUDGET",
    "DOCTRINE",
    "LICENSE",
    "CONTRIBUTING",
    "SECURITY",
];

/// Recursively collect all TOPIC `.md` files under `content_dir`.
///
/// Skips:
/// - `*.es.md` bilingual siblings
/// - `index.md` and `_index.md` at any level
/// - Files whose stem starts with `_`
/// - Repo-management files listed in `SYSTEM_FILE_STEMS`
/// - Non-`.md` files
///
/// Descends one level into subdirectories (category folders). Does not
/// recurse further — the content tree is `<content_dir>/<category>/<slug>.md`.
pub async fn collect_topic_files(content_dir: &FsPath) -> std::io::Result<Vec<TopicFile>> {
    let mut out = Vec::new();
    let mut entries = fs::read_dir(content_dir).await?;

    while let Some(entry) = entries.next_entry().await? {
        let file_type = entry.file_type().await?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy().to_string();

        if file_type.is_dir() {
            // Skip hidden directories (.git, .github, etc.).
            if name_str.starts_with('.') {
                continue;
            }
            // Descend into category subdirectory.
            let subdir_name = name_str.clone();
            let mut sub_entries = match fs::read_dir(entry.path()).await {
                Ok(e) => e,
                Err(_) => continue,
            };
            while let Some(sub_entry) = sub_entries.next_entry().await? {
                let sub_name = sub_entry.file_name();
                let sub_str = sub_name.to_string_lossy().to_string();
                let stem = match sub_str.strip_suffix(".md") {
                    Some(s) => s.to_string(),
                    None => continue,
                };
                if stem.ends_with(".es")
                    || stem == "index"
                    || stem == "_index"
                    || stem.starts_with('_')
                    || SYSTEM_FILE_STEMS.contains(&stem.as_str())
                {
                    continue;
                }
                out.push(TopicFile {
                    slug: format!("{subdir_name}/{stem}"),
                    path: sub_entry.path(),
                });
            }
        } else {
            // File at root level.
            let stem = match name_str.strip_suffix(".md") {
                Some(s) => s.to_string(),
                None => continue,
            };
            if stem.ends_with(".es")
                || stem == "index"
                || stem == "_index"
                || stem.starts_with('_')
                || SYSTEM_FILE_STEMS.contains(&stem.as_str())
            {
                continue;
            }
            out.push(TopicFile {
                slug: stem,
                path: entry.path(),
            });
        }
    }

    Ok(out)
}

/// Collect topic files from `content_dir` and zero or more `guide_dirs`.
/// Slugs are unique within each dir; guide slugs are prefixed by their subdir
/// name so they don't collide with content slugs.
pub async fn collect_all_topic_files(
    content_dir: &FsPath,
    guide_dirs: &[Option<&FsPath>],
) -> std::io::Result<Vec<TopicFile>> {
    let mut files = collect_topic_files(content_dir).await?;
    for gd in guide_dirs.iter().flatten() {
        if gd.is_dir() {
            if let Ok(guide_files) = collect_topic_files(gd).await {
                files.extend(guide_files);
            }
        }
    }
    Ok(files)
}

// Topic bucketing (implemented in the included handler files below): walks
// `content_dir` (and optional `guide_dir`) recursively, parses every `.md`
// file, and groups them into a `BTreeMap<category, Vec<TopicSummary>>`.
// Descends into category subdirectories; subdirectory TOPIC slugs take the
// form `<category>/<stem>`. Files with no `category:` frontmatter (or category
// `root`) are bucketed under `"uncategorised"`.

// Handler implementations split across subfiles for L20 line-count compliance.
// Each subfile is `include!`d here rather than declared as a submodule so all
// items remain in the `server` module scope (same visibility, same imports).
include!("home_handlers.rs");
include!("wiki_handlers.rs");
include!("special_handlers.rs");
include!("misc_handlers.rs");
