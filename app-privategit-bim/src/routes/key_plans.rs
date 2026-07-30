use axum::{
    extract::{Path, State},
    http::HeaderMap,
    response::{Html, IntoResponse, Response},
};

use crate::{render, state::AppState};

pub async fn key_plans_handler(headers: HeaderMap, State(state): State<AppState>) -> Html<String> {
    let content = render::card::render_key_plans(&state);
    if is_fragment(&headers) {
        Html(content)
    } else {
        Html(render::shell::page_shell(
            "Key Plans",
            "/key-plans",
            &content,
            &state,
        ))
    }
}

pub async fn kp_fragment(State(state): State<AppState>) -> Html<String> {
    Html(render::card::render_key_plans(&state))
}

pub async fn kp_download_handler(
    Path(filename): Path<String>,
    State(state): State<AppState>,
) -> Response {
    let kp_dir = state.config.library_dir.join("key-plans");
    let safe_name = filename.replace("..", "").replace('/', "");
    let file_path = kp_dir.join(&safe_name);
    match std::fs::read(&file_path) {
        Ok(bytes) => {
            let content_type = if safe_name.ends_with(".ifc") {
                "application/x-step"
            } else if safe_name.ends_with(".dxf") {
                "image/vnd.dxf"
            } else {
                "application/octet-stream"
            };
            (
                axum::http::StatusCode::OK,
                [
                    ("Content-Type", content_type),
                    (
                        "Content-Disposition",
                        &format!("attachment; filename=\"{safe_name}\""),
                    ),
                ],
                bytes,
            )
                .into_response()
        }
        Err(_) => (axum::http::StatusCode::NOT_FOUND, "file not found").into_response(),
    }
}

fn is_fragment(headers: &HeaderMap) -> bool {
    headers.get("x-fragment").is_some()
}
