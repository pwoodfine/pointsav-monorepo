//! Application state and axum Router. Grows each phase — P0 mounts only
//! `/healthz` and `/static/*path`; content routes land in P1, chrome in P2/P3.

use std::path::PathBuf;
use std::sync::Arc;

use axum::routing::get;
use axum::Router;

use crate::config::Config;

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
        .with_state(state)
}

async fn healthz() -> &'static str {
    "ok"
}
