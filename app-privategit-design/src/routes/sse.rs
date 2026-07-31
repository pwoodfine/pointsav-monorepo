// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

// D4 — SSE live-reload sidebar.
// GET /sidebar/sse streams nav HTML fragments when the vault changes.
// Client replaces nav.sidebar innerHTML on each event; no full reload needed.

use crate::{render, state::AppState};
use axum::{
    extract::{Query, State},
    response::sse::{Event, KeepAlive, Sse},
};
use futures_util::stream;
use std::{collections::HashMap, convert::Infallible};

/// `?section=components&slug=button` keeps a live-editing visitor's sidebar scoped to
/// their own component's category across a vault-change push — shell.html's inline
/// script only opens this connection at all when a sidebar is actually present (i.e.
/// `section=components`), so `render_nav` here mirrors the same real-slug lookup the
/// initial page load already did rather than broadcasting one shared tree to everyone.
pub async fn sidebar_sse(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Sse<impl stream::Stream<Item = Result<Event, Infallible>>> {
    let section = params.get("section").cloned().unwrap_or_default();
    let slug = params.get("slug").cloned().unwrap_or_default();
    let rx = state.watch_tx.subscribe();
    let env = state.env.clone();
    let component_groups = state.component_groups.clone();

    let s = stream::unfold(
        (rx, env, component_groups, section, slug),
        move |(mut rx, env, component_groups, section, slug)| async move {
            if rx.changed().await.is_err() {
                return None;
            }
            let html = render::render_nav(&env, &component_groups, &section, &slug);
            Some((
                Ok(Event::default().data(html)),
                (rx, env, component_groups, section, slug),
            ))
        },
    );

    Sse::new(s).keep_alive(KeepAlive::default())
}
