//! Application state and axum Router. P0 mounted `/healthz` and
//! `/static/*path`; P1 added the content pipeline; P3 wired real chrome;
//! P4 adds SEO/discovery (canonical/OG/Twitter/JSON-LD, robots, sitemap).

use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Path as AxumPath, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use maud::Markup;

use crate::config::Config;
use crate::content;
use crate::error::MarketingError;
use crate::ui::{page_shell, Tenant};

pub struct AppStateInner {
    pub content_dir: PathBuf,
    pub module_id: String,
    /// `SERVICE_MARKETING_GOOGLE_VERIFY` — read directly from the
    /// environment (not a clap flag) to match the retired engine's contract.
    pub google_verify: Option<String>,
}

pub type AppState = Arc<AppStateInner>;

pub fn build_state(cfg: &Config) -> AppState {
    Arc::new(AppStateInner {
        content_dir: cfg.content_dir.clone(),
        module_id: cfg.module_id.clone(),
        google_verify: std::env::var("SERVICE_MARKETING_GOOGLE_VERIFY").ok(),
    })
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/static/{*path}", get(crate::assets::serve))
        .route("/", get(home))
        .route("/es", get(home_es))
        .route("/page/{slug}", get(page))
        .route("/es/page/{slug}", get(page_es))
        .route("/robots.txt", get(robots_txt))
        .route("/sitemap.xml", get(sitemap_xml))
        .with_state(state)
}

async fn healthz() -> &'static str {
    "ok"
}

async fn home(State(state): State<AppState>) -> Result<Markup, MarketingError> {
    render_slug(&state, "home", None)
}

async fn home_es(State(state): State<AppState>) -> Result<Markup, MarketingError> {
    render_slug(&state, "home", Some("es"))
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

fn render_slug(state: &AppStateInner, slug: &str, lang: Option<&str>) -> Result<Markup, MarketingError> {
    let page = content::load_page(&state.content_dir, slug, lang)?;
    let tenant = Tenant::by_module_id(&state.module_id);
    let (en_path, es_path) = slug_paths(slug);
    Ok(page_shell(
        &tenant,
        &page,
        &state.module_id,
        &en_path,
        &es_path,
        state.google_verify.as_deref(),
    ))
}

fn slug_paths(slug: &str) -> (String, String) {
    if slug == "home" {
        ("/".to_string(), "/es".to_string())
    } else {
        (format!("/page/{slug}"), format!("/es/page/{slug}"))
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
