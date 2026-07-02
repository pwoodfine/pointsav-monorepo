//! Application state and axum Router. P0 mounted `/healthz` and
//! `/static/*path`; P1 added the content pipeline; P3 wires real chrome
//! (masthead/hero/footer/drawer, see `ui`) in place of the P1/P2 bare render.

use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Path as AxumPath, State};
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
}

pub type AppState = Arc<AppStateInner>;

pub fn build_state(cfg: &Config) -> AppState {
    Arc::new(AppStateInner {
        content_dir: cfg.content_dir.clone(),
        module_id: cfg.module_id.clone(),
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
    Ok(page_shell(&tenant, &page, &state.module_id))
}
