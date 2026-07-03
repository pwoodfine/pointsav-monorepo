use crate::{component_preview, render, schema, state::AppState, tokens_gallery, vault};
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
};
use std::{fs, io::Write};

pub async fn index(State(state): State<AppState>) -> Html<String> {
    let nav_html = render::render_nav(&state.env, &state.nav, vault::SECTIONS, "", "");

    let mut cards = String::new();
    for (section, _) in vault::SECTIONS {
        let Some(slugs) = state.nav.get(*section) else {
            continue;
        };
        if slugs.is_empty() {
            continue;
        }
        let first = &slugs[0];
        let tab = vault::default_tab(section);
        cards.push_str(&format!(
            "<a class=\"home-card\" href=\"/{section}/{first}/{tab}\">\
             <h2>{}</h2><p>{} items</p></a>\n",
            vault::to_title(section),
            slugs.len()
        ));
    }

    let content = format!(
        "<div class=\"home-body\"><h1>PointSav Design System</h1>\
         <p>DTCG-native design tokens and components, self-hostable and machine-readable. \
         Pick a section below, search above, or <a href=\"/bundles/tokens\">download the token bundle</a>.</p>\
         <div class=\"home-grid\">{cards}</div></div>"
    );

    Html(render::shell(
        &state.env,
        "PointSav Design System",
        &nav_html,
        "",
        "",
        &content,
    ))
}

pub async fn tokens_gallery_page(State(state): State<AppState>) -> Html<String> {
    let tiers = tokens_gallery::load_and_flatten(&state.vault);
    let body = state
        .env
        .get_template("tokens.html")
        .expect("tokens.html missing")
        .render(minijinja::context! { tiers => tiers })
        .expect("render tokens.html failed");

    let nav_html = render::render_nav(&state.env, &state.nav, vault::SECTIONS, "", "");
    Html(render::shell(
        &state.env,
        "Tokens — PointSav Design System",
        &nav_html,
        "",
        "Tokens",
        &body,
    ))
}

pub async fn item_redirect(
    Path((section, slug)): Path<(String, String)>,
    State(state): State<AppState>,
) -> Response {
    if slug.contains("..") || slug.contains('/') {
        return (StatusCode::BAD_REQUEST, "invalid").into_response();
    }
    if !vault::is_known_section(&section) {
        return (StatusCode::NOT_FOUND, "unknown section").into_response();
    }
    let tabs = vault::discover_tabs(&state.vault, &section, &slug);
    let first = tabs
        .into_iter()
        .next()
        .unwrap_or_else(|| vault::default_tab(&section).to_string());
    Redirect::permanent(&format!("/{}/{}/{}", section, slug, first)).into_response()
}

pub async fn item_tab(
    Path((section, slug, tab)): Path<(String, String, String)>,
    State(state): State<AppState>,
) -> Response {
    if slug.contains("..") || slug.contains('/') || tab.contains("..") || tab.contains('/') {
        return (StatusCode::BAD_REQUEST, "invalid").into_response();
    }
    if !vault::is_known_section(&section) {
        return (StatusCode::NOT_FOUND, "unknown section").into_response();
    }
    let tabs = vault::discover_tabs(&state.vault, &section, &slug);
    if tabs.is_empty() {
        return (StatusCode::NOT_FOUND, "item not found").into_response();
    }
    let md_path = state
        .vault
        .join(&section)
        .join(&slug)
        .join(format!("{}.md", tab));
    let raw = match fs::read_to_string(&md_path) {
        Ok(s) => s,
        Err(_) => return (StatusCode::NOT_FOUND, "tab not found").into_response(),
    };

    let (frontmatter, body) = vault::parse_frontmatter(&raw);
    let schema_type = schema::detect(&frontmatter);
    let mut content = schema::render(schema_type, &frontmatter, &body);

    // P1.1 — live component preview (recipe.json variants, sandboxed via iframe).
    if section == "components" {
        if let Some(preview) = component_preview::render_preview(&state.vault, &slug) {
            content = format!("{preview}{content}");
        }
    }

    let nav_html = render::render_nav(&state.env, &state.nav, vault::SECTIONS, &section, &slug);
    let tab_bar = render::render_tab_bar(&state.env, &section, &slug, &tabs, &tab);
    let label = vault::to_title(&slug);

    // P2.2 — breadcrumb wayfinding (Home > Section > Item), especially useful in the
    // mobile drawer where the sidebar is collapsed by default.
    let breadcrumb = format!(
        "<nav class=\"breadcrumb\" aria-label=\"Breadcrumb\">\
         <a href=\"/\">Home</a><span aria-hidden=\"true\"> / </span>\
         <span>{}</span><span aria-hidden=\"true\"> / </span>\
         <span aria-current=\"page\">{}</span></nav>",
        vault::to_title(&section),
        label
    );
    let content = format!("{breadcrumb}{content}");

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

/// GET /elements/:slug/download — ZIP all non-.md members from vault/elements/<slug>/
pub async fn bundle_download(Path(slug): Path<String>, State(state): State<AppState>) -> Response {
    if slug.contains("..") || slug.contains('/') {
        return (StatusCode::BAD_REQUEST, "invalid").into_response();
    }
    let elem_dir = state.vault.join("elements").join(&slug);
    let Ok(entries) = fs::read_dir(&elem_dir) else {
        return (StatusCode::NOT_FOUND, "bundle not found").into_response();
    };

    let buf = Vec::new();
    let cursor = std::io::Cursor::new(buf);
    let mut zip_writer = zip::ZipWriter::new(cursor);
    let zip_opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    for entry in entries.filter_map(|e| e.ok()) {
        let name = entry.file_name().to_string_lossy().to_string();
        // include all files; .md are the vault doc, skip them in the download
        if name.ends_with(".md") {
            continue;
        }
        let Ok(content) = fs::read(entry.path()) else {
            continue;
        };
        let _ = zip_writer.start_file(&name, zip_opts);
        let _ = zip_writer.write_all(&content);
    }
    let Ok(cursor) = zip_writer.finish() else {
        return (StatusCode::INTERNAL_SERVER_ERROR, "zip error").into_response();
    };
    let body = cursor.into_inner();
    let disposition = format!("attachment; filename=\"{}.zip\"", slug);
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/zip")
        .header(header::CONTENT_DISPOSITION, &disposition)
        .body(Body::from(body))
        .unwrap_or_else(|_| (StatusCode::INTERNAL_SERVER_ERROR, "response error").into_response())
}
