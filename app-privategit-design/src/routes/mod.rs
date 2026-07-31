// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

pub mod ai;
pub mod browse;
pub mod bundle;
pub mod edit;
pub mod search;
pub mod seo;
pub mod sse;

use crate::state::AppState;
use axum::{
    routing::{get, post, put},
    Router,
};
use tower_http::services::ServeDir;

pub fn build_router(state: AppState) -> Router {
    let static_dir = state.static_dir.clone();

    Router::new()
        .route("/healthz", get(healthz))
        .route("/robots.txt", get(seo::robots_txt))
        .route("/sitemap.xml", get(seo::sitemap_xml))
        .route("/", get(browse::index))
        .route("/es", get(browse::index_es))
        .route("/tokens", get(browse::tokens_gallery_page))
        .route("/adoption", get(browse::adoption_page))
        .route("/elements/:slug/download", get(browse::bundle_download))
        .route("/components/:slug/recipe.json", get(browse::component_recipe))
        .route("/:section/:slug", get(browse::item_redirect))
        .route("/:section/:slug/:tab", get(browse::item_tab))
        .route("/tokens/search", get(search::token_search))
        .route("/bundles/:name", get(bundle::list))
        .route("/bundles/:name/download", get(bundle::download))
        .route("/bundles/:name/:filename", get(bundle::file))
        .route("/sidebar/sse", get(sse::sidebar_sse))
        .route("/vault/:section/:slug/:tab/raw", get(edit::get_raw))
        .route("/vault/:section/:slug/:tab", put(edit::put_save))
        .route("/ai/session", post(ai::post_session))
        .route("/mcp", post(crate::mcp::mcp_handler))
        .nest_service("/static", ServeDir::new(static_dir))
        .with_state(state)
}

async fn healthz() -> &'static str {
    "ok"
}
