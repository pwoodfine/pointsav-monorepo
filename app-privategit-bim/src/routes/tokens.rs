use axum::{
    extract::{Path, State},
    http::HeaderMap,
    response::Html,
};

use crate::{render, state::AppState};

pub async fn tokens_index_handler(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Html<String> {
    let content = render::card::render_tokens_index(&state);
    if is_fragment(&headers) {
        Html(content)
    } else {
        Html(render::shell::page_shell(
            "BIM Object Catalog",
            "/tokens",
            &content,
            &state,
        ))
    }
}

pub async fn token_category_handler(
    Path(name): Path<String>,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Html<String> {
    let content = render::card::render_token_page(&name, &state);
    if is_fragment(&headers) {
        Html(content)
    } else {
        Html(render::shell::page_shell(
            &format!("{name} — BIM Objects"),
            &format!("/tokens/{name}"),
            &content,
            &state,
        ))
    }
}

pub async fn token_category_fragment(
    Path(name): Path<String>,
    State(state): State<AppState>,
) -> Html<String> {
    Html(render::card::render_token_page(&name, &state))
}

pub async fn tokens_index_fragment(State(state): State<AppState>) -> Html<String> {
    Html(render::card::render_tokens_index(&state))
}

fn is_fragment(headers: &HeaderMap) -> bool {
    headers.get("x-fragment").is_some()
}
