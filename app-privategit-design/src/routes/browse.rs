use crate::{render, schema, state::AppState, vault};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
};
use std::fs;

pub async fn index(State(state): State<AppState>) -> Html<String> {
    let nav_html = render::render_nav(&state.env, &state.nav, vault::SECTIONS, "", "");
    let content = "<div class=\"home-body\"><h1>PointSav Design System</h1>\
                   <p>Select an element from the sidebar.</p></div>";
    Html(render::shell(
        &state.env,
        "PointSav Design System",
        &nav_html,
        "",
        "",
        content,
    ))
}

pub async fn element_redirect(Path(slug): Path<String>, State(state): State<AppState>) -> Response {
    if slug.contains("..") || slug.contains('/') {
        return (StatusCode::BAD_REQUEST, "invalid").into_response();
    }
    let tabs = vault::discover_tabs(&state.vault, "elements", &slug);
    let first = tabs
        .into_iter()
        .next()
        .unwrap_or_else(|| "overview".to_string());
    Redirect::permanent(&format!("/elements/{}/{}", slug, first)).into_response()
}

pub async fn element_tab(
    Path((slug, tab)): Path<(String, String)>,
    State(state): State<AppState>,
) -> Response {
    if slug.contains("..") || slug.contains('/') || tab.contains("..") || tab.contains('/') {
        return (StatusCode::BAD_REQUEST, "invalid").into_response();
    }
    let tabs = vault::discover_tabs(&state.vault, "elements", &slug);
    if tabs.is_empty() {
        return (StatusCode::NOT_FOUND, "element not found").into_response();
    }
    let md_path = state
        .vault
        .join("elements")
        .join(&slug)
        .join(format!("{}.md", tab));
    let raw = match fs::read_to_string(&md_path) {
        Ok(s) => s,
        Err(_) => return (StatusCode::NOT_FOUND, "tab not found").into_response(),
    };

    let (frontmatter, body) = vault::parse_frontmatter(&raw);
    let schema_type = schema::detect(&frontmatter);
    let content = schema::render(schema_type, &frontmatter, &body);

    let nav_html = render::render_nav(&state.env, &state.nav, vault::SECTIONS, "elements", &slug);
    let tab_bar = render::render_tab_bar(&state.env, "elements", &slug, &tabs, &tab);
    let label = vault::to_title(&slug);

    Html(render::shell(
        &state.env,
        &format!("{} — PointSav Design System", label),
        &nav_html,
        &tab_bar,
        &label,
        &content,
    ))
    .into_response()
}
