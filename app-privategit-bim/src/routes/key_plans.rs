// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

use axum::{
    extract::{Path, State},
    response::{IntoResponse, Redirect, Response},
};

use crate::state::AppState;

/// `/key-plans` is now folded into the unified home catalog's Compositions
/// tab. Redirect legacy links/bookmarks to `/` (302). The download sub-route
/// below is unchanged.
pub async fn key_plans_handler() -> Redirect {
    Redirect::to("/")
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
