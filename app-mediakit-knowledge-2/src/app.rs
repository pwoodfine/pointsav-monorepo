//! Application state and axum router assembly.
//!
//! All routes are declared in one place (`router`) so the URL surface is
//! legible at a glance. Handlers are added phase by phase; P0 wires the
//! liveness probe and static-asset serving only.

use std::sync::Arc;

use axum::body::Body;
use axum::extract::Path;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;

use crate::assets::StaticAssets;
use crate::config::Config;

/// Shared, immutable-after-startup application state.
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        Self {
            config: Arc::new(config),
        }
    }
}

/// Build the router. The route map is the engine's public contract.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/health", get(healthz))
        .route("/static/{*path}", get(static_asset))
        .with_state(state)
}

/// Liveness probe.
async fn healthz() -> impl IntoResponse {
    (StatusCode::OK, "ok")
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
