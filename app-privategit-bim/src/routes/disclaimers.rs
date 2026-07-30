// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

use axum::{extract::State, response::Html};

use crate::{render, render::shell::esc, state::AppState};

pub async fn disclaimers_handler(State(state): State<AppState>) -> Html<String> {
    let mut sections = String::new();
    for section in state.disclaimers_page.sections.iter() {
        sections.push_str(&format!(
            "<section><h2>{}</h2>{}</section>",
            esc(&section.heading),
            section.body_html,
        ));
    }

    let content = format!(
        r#"<div class="bim-breadcrumbs">
  <a href="/" data-path="/" class="bim-nav-link">Home</a>
</div>
<header class="bim-cat-pagehead">
  <span class="bim-cat-kicker">Important information</span>
  <h1>Disclaimers</h1>
</header>
<article class="bim-article">
  {sections}
</article>"#,
    );

    Html(render::shell::page_shell(
        "Disclaimers",
        "/disclaimers",
        &content,
        &state,
    ))
}
