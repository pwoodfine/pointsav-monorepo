use axum::{
    extract::{Path, State},
    http::HeaderMap,
    response::Html,
};

use crate::{render, state::AppState};

pub async fn research_index_handler(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Html<String> {
    let content = render::card::render_research_index(&state);
    if is_fragment(&headers) {
        Html(content)
    } else {
        Html(render::shell::page_shell(
            "Research",
            "/research",
            &content,
            &state,
        ))
    }
}

pub async fn research_item_handler(
    Path(slug): Path<String>,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Html<String> {
    let content = render::card::render_research_item(&slug, &state);
    if is_fragment(&headers) {
        Html(content)
    } else {
        Html(render::shell::page_shell(
            &format!("{slug} — Research"),
            &format!("/research/{slug}"),
            &content,
            &state,
        ))
    }
}

pub async fn research_fragment(State(state): State<AppState>) -> Html<String> {
    Html(render::card::render_research_index(&state))
}

fn is_fragment(headers: &HeaderMap) -> bool {
    headers.get("x-fragment").is_some()
}
