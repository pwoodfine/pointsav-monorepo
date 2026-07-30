// D5 — AI bridge: POST /ai/session streams AI completions as SSE.
// X-Model header selects adapter: "doorman" (local OLMo) or "claude" (ClaudeCloud).
// X-Api-Key header provides the Claude API credential; never stored server-side.

use crate::{ai, state::AppState};
use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use futures_util::StreamExt;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct AiSessionRequest {
    pub selection: String,
    pub schema: String,
    pub context: String,
}

pub async fn post_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<AiSessionRequest>,
) -> Response {
    let model = headers
        .get("x-model")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("doorman");

    let req = ai::AiRequest {
        selection: body.selection,
        schema: body.schema,
        context: body.context,
    };

    let chunk_stream = match model {
        "claude" => {
            let api_key = headers
                .get("x-api-key")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_string();
            if api_key.is_empty() {
                return (
                    StatusCode::BAD_REQUEST,
                    "X-Api-Key required for claude model",
                )
                    .into_response();
            }
            ai::claude::stream_completion(&api_key, req).await
        }
        _ => ai::doorman::stream_completion(&state.doorman_url, req).await,
    };

    let body_stream =
        chunk_stream.map(|chunk| Ok::<String, std::convert::Infallible>(chunk.to_sse()));

    axum::response::Response::builder()
        .status(200)
        .header(header::CONTENT_TYPE, "text/event-stream")
        .header(header::CACHE_CONTROL, "no-cache")
        .header("X-Accel-Buffering", "no")
        .body(axum::body::Body::from_stream(body_stream))
        .unwrap()
}
