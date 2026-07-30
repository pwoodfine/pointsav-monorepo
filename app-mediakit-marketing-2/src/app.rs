// SPDX-License-Identifier: FSL-1.1-ALv2
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

//! Application state and axum Router. P0 mounted `/healthz` and
//! `/static/*path`; P1 added the content pipeline; P3 wired real chrome;
//! P4 added SEO/discovery; P5 adds the MCP JSON-RPC surface + review queue
//! (mounted only when `enable_mcp` is set).

use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Path as AxumPath, State};
use axum::http::header;
use axum::response::{IntoResponse, Json, Response};
use axum::routing::{get, post};
use axum::Router;
use maud::Markup;

use crate::config::Config;
use crate::content;
use crate::error::MarketingError;
use crate::mcp::{self, RpcRequest};
use crate::pending::Queue;
use crate::ui::{page_shell, Tenant};

pub struct AppStateInner {
    pub content_dir: PathBuf,
    pub module_id: String,
    /// `SERVICE_MARKETING_GOOGLE_VERIFY` — read directly from the
    /// environment (not a clap flag) to match the retired engine's contract.
    pub google_verify: Option<String>,
    pub pending: Queue,
}

pub type AppState = Arc<AppStateInner>;

pub fn build_state(cfg: &Config) -> Result<AppState, MarketingError> {
    Ok(Arc::new(AppStateInner {
        content_dir: cfg.content_dir.clone(),
        module_id: cfg.module_id.clone(),
        google_verify: std::env::var("SERVICE_MARKETING_GOOGLE_VERIFY").ok(),
        pending: Queue::open(&cfg.state_dir)?,
    }))
}

pub fn router(state: AppState, enable_mcp: bool) -> Router {
    // No /es route for the home page — operator call 2026-07-02 (English
    // only on the home pages for now). `page.es.yaml` files for `home` stay
    // on disk, just unrouted, so this is reversible. Other pages
    // (/page/{slug}) keep their /es/page/{slug} variant where content exists.
    let mut router = Router::new()
        .route("/healthz", get(healthz))
        .route("/static/{*path}", get(crate::assets::serve))
        .route("/", get(home))
        .route("/page/{slug}", get(page))
        .route("/es/page/{slug}", get(page_es))
        .route("/robots.txt", get(robots_txt))
        .route("/sitemap.xml", get(sitemap_xml));

    if enable_mcp {
        router = router
            .route("/api/mcp", post(mcp_rpc))
            .route("/api/pending", get(list_pending))
            .route("/api/pending/{id}/manifest", get(pending_manifest))
            .route("/api/pending/{id}/approve", post(approve_pending));
    }

    router.with_state(state)
}

async fn healthz() -> &'static str {
    "ok"
}

async fn home(State(state): State<AppState>) -> Result<Markup, MarketingError> {
    render_slug(&state, "home", None)
}

async fn page(
    State(state): State<AppState>,
    AxumPath(slug): AxumPath<String>,
) -> Result<Markup, MarketingError> {
    render_slug(&state, &slug, None)
}

async fn page_es(
    State(state): State<AppState>,
    AxumPath(slug): AxumPath<String>,
) -> Result<Markup, MarketingError> {
    render_slug(&state, &slug, Some("es"))
}

fn render_slug(
    state: &AppStateInner,
    slug: &str,
    lang: Option<&str>,
) -> Result<Markup, MarketingError> {
    let page = content::load_page(&state.content_dir, slug, lang)?;
    let tenant = Tenant::by_module_id(&state.module_id);
    let (en_path, es_path) = slug_paths(slug);
    Ok(page_shell(
        &tenant,
        &page,
        &state.module_id,
        &en_path,
        es_path.as_deref(),
        state.google_verify.as_deref(),
    ))
}

/// `home` has no `/es` route (operator call 2026-07-02) — every other slug
/// keeps its `/es/page/{slug}` variant.
fn slug_paths(slug: &str) -> (String, Option<String>) {
    if slug == "home" {
        ("/".to_string(), None)
    } else {
        (format!("/page/{slug}"), Some(format!("/es/page/{slug}")))
    }
}

async fn robots_txt(State(state): State<AppState>) -> Response {
    let tenant = Tenant::by_module_id(&state.module_id);
    let body = format!(
        "User-agent: *\nAllow: /\nSitemap: {}/sitemap.xml\n",
        tenant.canonical_base
    );
    ([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], body).into_response()
}

async fn sitemap_xml(State(state): State<AppState>) -> Response {
    let tenant = Tenant::by_module_id(&state.module_id);
    let mut slugs = content::list_slugs(&state.content_dir);
    slugs.sort();
    let mut body = String::from(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
    body.push_str(r#"<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">"#);
    for slug in &slugs {
        let (en_path, _) = slug_paths(slug);
        body.push_str(&format!(
            "<url><loc>{}{}</loc></url>",
            tenant.canonical_base, en_path
        ));
    }
    body.push_str("</urlset>");
    (
        [(header::CONTENT_TYPE, "application/xml; charset=utf-8")],
        body,
    )
        .into_response()
}

// ---------------------------------------------------------------- P5: MCP

async fn mcp_rpc(
    State(state): State<AppState>,
    Json(req): Json<RpcRequest>,
) -> Json<mcp::RpcResponse> {
    Json(mcp::handle(&state, req))
}

async fn list_pending(State(state): State<AppState>) -> Result<Response, MarketingError> {
    let items = state.pending.list()?;
    Ok(Json(items).into_response())
}

async fn pending_manifest(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Response, MarketingError> {
    let manifest = state.pending.manifest(&id)?;
    Ok(([(header::CONTENT_TYPE, "application/yaml")], manifest).into_response())
}

/// The F12 human-approval endpoint. Nothing in this codebase calls this
/// automatically — it exists only to be triggered by an explicit
/// human/operator action (a UI button, a curl command a human runs).
async fn approve_pending(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Response, MarketingError> {
    state.pending.approve(&id, &state.content_dir)?;
    Ok(Json(serde_json::json!({ "status": "approved", "id": id })).into_response())
}
