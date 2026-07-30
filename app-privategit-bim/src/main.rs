mod config;
mod mcp;
mod render;
mod routes;
mod schema;
mod state;

use axum::{
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use tower_http::services::ServeDir;

#[tokio::main]
async fn main() {
    let config = config::Config::from_env();
    let app_state = state::AppState::new(&config)
        .await
        .expect("AppState init failed");

    state::spawn_file_watcher(app_state.clone(), &config);

    let static_dir = config.static_dir.clone();

    let app = Router::new()
        // Full-page routes
        .route("/", get(routes::home::home_handler))
        .route("/tokens", get(routes::tokens::tokens_index_handler))
        .route(
            "/tokens/{name}",
            get(routes::tokens::token_category_handler),
        )
        .route("/key-plans", get(routes::key_plans::key_plans_handler))
        .route(
            "/key-plans/download/{filename}",
            get(routes::key_plans::kp_download_handler),
        )
        .route("/furniture", get(routes::furniture::furniture_handler))
        .route(
            "/furniture/download/bundle.zip",
            get(routes::furniture::bundle_handler),
        )
        .route(
            "/furniture/download/{filename}",
            get(routes::furniture::single_handler),
        )
        .route("/research", get(routes::research::research_index_handler))
        .route(
            "/research/{slug}",
            get(routes::research::research_item_handler),
        )
        .route("/edit/{slug}", get(routes::editor::edit_get))
        .route("/edit/{slug}", post(routes::editor::edit_post))
        // Fragment routes (content-only; same handlers, X-Fragment header also works)
        .route(
            "/fragment/tokens",
            get(routes::tokens::tokens_index_fragment),
        )
        .route(
            "/fragment/tokens/{name}",
            get(routes::tokens::token_category_fragment),
        )
        .route("/fragment/key-plans", get(routes::key_plans::kp_fragment))
        .route(
            "/fragment/furniture",
            get(routes::furniture::furniture_fragment),
        )
        .route(
            "/fragment/research",
            get(routes::research::research_fragment),
        )
        // API
        .route("/api/events", get(routes::api::sse_handler))
        .route("/api/tokens.json", get(routes::api::tokens_json_handler))
        .route("/api/validate", post(routes::api::validate_handler))
        .route("/healthz", get(routes::api::healthz))
        .route("/readyz", get(routes::api::readyz))
        // MCP endpoint (JSON-RPC 2.0 over HTTP)
        .route("/mcp", post(mcp::mcp_handler))
        // Static assets
        .nest_service("/static", ServeDir::new(static_dir))
        .with_state(app_state);

    let addr: SocketAddr = config.bind;
    println!("app-privategit-bim listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
