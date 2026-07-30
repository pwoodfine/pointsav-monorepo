use axum::{
    extract::State,
    http::StatusCode,
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Json,
    },
};
use futures_util::StreamExt;
use serde_json::{json, Value};
use tokio_stream::wrappers::BroadcastStream;

use crate::{schema::validator, state::AppState};

pub async fn sse_handler(
    State(state): State<AppState>,
) -> Sse<impl futures_util::Stream<Item = Result<Event, std::convert::Infallible>>> {
    let rx = state.events_tx.subscribe();
    let stream = BroadcastStream::new(rx)
        .filter_map(|r| async move { r.ok() })
        .map(|data| Ok::<_, std::convert::Infallible>(Event::default().data(data)));
    Sse::new(stream).keep_alive(KeepAlive::default())
}

pub async fn tokens_json_handler(State(state): State<AppState>) -> Json<Value> {
    let combined: serde_json::Map<String, Value> = state
        .tokens
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    Json(Value::Object(combined))
}

pub async fn validate_handler(
    State(_state): State<AppState>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    match validator::validate_dtcg(&body) {
        Ok(()) => (StatusCode::OK, Json(json!({ "valid": true, "errors": [] }))),
        Err(errors) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({ "valid": false, "errors": errors })),
        ),
    }
}

pub async fn healthz(State(state): State<AppState>) -> Json<Value> {
    Json(json!({
        "status": "ok",
        "token_count": state.token_count,
        "components_count": state.components_count,
    }))
}

pub async fn readyz() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}
