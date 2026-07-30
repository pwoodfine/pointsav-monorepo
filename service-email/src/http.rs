// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Axum router and application state for service-email.
//!
//! Endpoints:
//!   GET  /healthz   → liveness, always 200
//!   GET  /readyz    → readiness; 200 with module_id
//!   POST /mcp       → MCP JSON-RPC 2.0 tool interface (email.ingest)

use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::Router;
use serde::Serialize;

use crate::fs_client::FsClient;
use crate::mcp::mcp_handler;

pub struct AppState {
    pub module_id: String,
    pub fs_client: FsClient,
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .route("/mcp", post(mcp_handler))
        .with_state(state)
}

async fn healthz() -> &'static str {
    "ok"
}

#[derive(Serialize)]
struct ReadyzBody {
    ready: bool,
    module_id: String,
}

async fn readyz(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    (
        StatusCode::OK,
        axum::Json(ReadyzBody {
            ready: true,
            module_id: state.module_id.clone(),
        }),
    )
}
