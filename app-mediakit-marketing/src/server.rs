//! HTTP surface and shared application state.

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{get, post},
    Json, Router,
};

use app_mediakit_shell::{render_page, Brand};

use crate::content::{self, LoadError};
use crate::pending::Queue;

/// Shared, immutable-after-startup application state.
pub struct AppState {
    pub content_dir: std::path::PathBuf,
    pub brand: Brand,
    /// The active DTCG token bundle, resolved once at startup.
    pub tokens_css: String,
    pub pending: Queue,
    pub mcp_enabled: bool,
}

/// Build the axum router for an instance.
pub fn router(state: AppState) -> Router {
    let state = Arc::new(state);
    Router::new()
        .route("/", get(home))
        .route("/page/{slug}", get(page))
        .route("/es", get(home_es))
        .route("/es/page/{slug}", get(page_es))
        .route("/healthz", get(healthz))
        .route("/api/mcp", post(crate::mcp::handler))
        .route("/api/pending", get(list_pending))
        .route("/api/pending/{id}/manifest", get(pending_manifest))
        .route("/api/pending/{id}/approve", post(approve_pending))
        .with_state(state)
}

async fn healthz() -> &'static str {
    "ok"
}

async fn home(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    render(&state, "home", "en", "/")
}

async fn page(State(state): State<Arc<AppState>>, Path(slug): Path<String>) -> impl IntoResponse {
    let path = format!("/page/{slug}");
    render(&state, &slug, "en", &path)
}

async fn home_es(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    render(&state, "home", "es", "/es")
}

async fn page_es(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
) -> impl IntoResponse {
    let path = format!("/es/page/{slug}");
    render(&state, &slug, "es", &path)
}

/// Render a slug to a full HTML document (or an error response).
fn render(state: &AppState, slug: &str, lang: &str, path: &str) -> Response {
    match content::load_page_lang(&state.content_dir, slug, lang) {
        Ok(page) => Html(render_page(&state.brand, &page, &state.tokens_css, path)).into_response(),
        Err(LoadError::NotFound) => (StatusCode::NOT_FOUND, "Not found").into_response(),
        Err(LoadError::InvalidSlug) => (StatusCode::BAD_REQUEST, "Invalid path").into_response(),
        Err(LoadError::Invalid(e)) => {
            tracing::error!(slug, error = %e, "invalid page manifest");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Page manifest is invalid",
            )
                .into_response()
        }
    }
}

type Response = axum::response::Response;

async fn list_pending(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    Json(serde_json::json!({ "pending": state.pending.list() }))
}

async fn pending_manifest(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.pending.read(&id) {
        Ok(yaml) => (StatusCode::OK, yaml).into_response(),
        Err(e) => (StatusCode::NOT_FOUND, e).into_response(),
    }
}

/// Approve a staged proposal — the human F12. Persists into the content tree.
async fn approve_pending(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.pending.approve(&state.content_dir, &id) {
        Ok(path) => Json(serde_json::json!({
            "approved": id,
            "written": path.display().to_string(),
            "note": "Working-tree change written. Commit it with a signed operator commit."
        }))
        .into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, e).into_response(),
    }
}
