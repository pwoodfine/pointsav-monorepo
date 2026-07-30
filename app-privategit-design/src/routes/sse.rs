// D4 — SSE live-reload sidebar.
// GET /sidebar/sse streams nav HTML fragments when the vault changes.
// Client replaces nav.sidebar innerHTML on each event; no full reload needed.

use crate::{render, state::AppState, vault::SECTIONS};
use axum::{
    extract::State,
    response::sse::{Event, KeepAlive, Sse},
};
use futures_util::stream;
use std::convert::Infallible;

pub async fn sidebar_sse(
    State(state): State<AppState>,
) -> Sse<impl stream::Stream<Item = Result<Event, Infallible>>> {
    let rx = state.watch_tx.subscribe();
    let nav = state.nav.clone();
    let env = state.env.clone();

    let s = stream::unfold((rx, nav, env), |(mut rx, nav, env)| async move {
        if rx.changed().await.is_err() {
            return None;
        }
        let html = render::render_nav(&env, &nav, SECTIONS, "", "");
        Some((Ok(Event::default().data(html)), (rx, nav, env)))
    });

    Sse::new(s).keep_alive(KeepAlive::default())
}
