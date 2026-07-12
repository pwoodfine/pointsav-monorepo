// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

use axum::extract::{Query, State};
use axum::response::Html;
use std::collections::HashMap;

use crate::{render, state::AppState};

pub async fn search_handler(
    Query(params): Query<HashMap<String, String>>,
    State(state): State<AppState>,
) -> Html<String> {
    let query = params.get("q").cloned().unwrap_or_default();
    let content = render::search::render_search_results(&query, &state);
    Html(render::shell::page_shell(
        "Search", "/search", &content, &state,
    ))
}
