//! Application state and axum router assembly.
//!
//! All routes are declared in one place (`router`) so the URL surface is
//! legible at a glance. Handlers are added phase by phase; through P1 this
//! wires the liveness probe, static assets, and a raw (chrome-less) article
//! view so the content pipeline can be exercised end to end.

use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{header, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::Router;

use crate::assets::StaticAssets;
use crate::config::Config;
use crate::content::{self, ContentIndex, Lang, MountSet};

/// Shared, immutable-after-startup application state.
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub mounts: Arc<MountSet>,
    pub index: Arc<ContentIndex>,
}

impl AppState {
    /// Build state from parsed config, walking the mounts to build the index.
    pub fn build(config: Config) -> Self {
        let mounts = MountSet::from_config(&config);
        let index = ContentIndex::build(&mounts);
        tracing::info!("indexed {} article(s)", index.article_count());
        Self {
            config: Arc::new(config),
            mounts: Arc::new(mounts),
            index: Arc::new(index),
        }
    }
}

/// Build the router. The route map is the engine's public contract.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/health", get(healthz))
        .route("/wiki/{*slug}", get(wiki_raw))
        .route("/static/{*path}", get(static_asset))
        .with_state(state)
}

/// Liveness probe.
async fn healthz() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

/// Raw article view — rendered body only, no chrome yet (chrome arrives in P2/P3).
/// Proves the content pipeline: slug resolution → parse → render.
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
        .unwrap_or_else(|| doc.title.clone());
    // Minimal, unstyled document — real chrome is authored in P2/P3.
    let page = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>{title}</title></head>\
         <body><h1>{title}</h1>{body}</body></html>",
        title = html_escape(&title),
        body = rendered.html,
    );
    Html(page).into_response()
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

/// Minimal HTML text escaping for interpolated titles.
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
