use axum::{extract::State, response::Html};

use crate::{render, state::AppState};

pub async fn home_handler(State(state): State<AppState>) -> Html<String> {
    let content = render::card::render_home(&state);
    Html(render::shell::page_shell(
        "",
        "/",
        &content,
        &state,
    ))
}
