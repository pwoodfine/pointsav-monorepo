use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Json},
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{render, schema::validator, state::AppState};

pub async fn edit_get(Path(slug): Path<String>, State(state): State<AppState>) -> Html<String> {
    let token_json = state.tokens.get(&slug).cloned().unwrap_or(Value::Null);
    let content = render::editor::render_editor_panel(&slug, &token_json);
    Html(render::shell::page_shell(
        &format!("Edit: {slug}"),
        &format!("/edit/{slug}"),
        &content,
        &state,
    ))
}

#[derive(Deserialize)]
pub struct EditQuery {
    pub confirm: Option<String>,
}

pub async fn edit_post(
    Path(slug): Path<String>,
    State(state): State<AppState>,
    Query(q): Query<EditQuery>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    // PBS-1 validation gate
    if let Err(errors) = validator::validate_dtcg(&body) {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({ "valid": false, "errors": errors })),
        )
            .into_response();
    }

    // SYS-ADR-10 F12 gate: must send ?confirm=1
    let confirmed = q.confirm.as_deref() == Some("1");
    if !confirmed {
        return (
            StatusCode::OK,
            Json(json!({ "dry_run": true, "valid": true, "errors": [] })),
        )
            .into_response();
    }

    let bim_dir = state.config.design_system_dir.join("tokens").join("bim");
    let filename = format!("{slug}.dtcg.json");
    let out_path = bim_dir.join(&filename);

    let pretty = match serde_json::to_string_pretty(&body) {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("serialization failed: {e}") })),
            )
                .into_response();
        }
    };

    if let Err(e) = std::fs::write(&out_path, &pretty) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("write failed: {e}") })),
        )
            .into_response();
    }

    // Broadcast SSE event so connected browsers auto-refresh
    let msg = format!(
        r#"{{"event":"token-updated","path":"{}"}}"#,
        out_path.display()
    );
    let _ = state.events_tx.send(msg);

    (
        StatusCode::OK,
        Json(json!({ "saved": true, "file": filename })),
    )
        .into_response()
}
