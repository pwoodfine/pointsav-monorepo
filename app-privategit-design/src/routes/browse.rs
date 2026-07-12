// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

use crate::{
    component_meta, component_preview, render, schema, state::AppState, tokens_gallery, vault,
};
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
};
use serde::Serialize;
use std::{fs, io::Write};

pub async fn index(State(state): State<AppState>) -> Html<String> {
    let nav_html = render::render_nav(
        &state.env,
        &state.nav,
        &state.component_groups,
        vault::SECTIONS,
        "",
        "",
    );

    // Part C (2026-07-04): the CTA no longer renders as a grid card — a real button
    // next to the lede reads as an action, not a wayfinding tile; the grid below is now
    // a uniform set of true-peer sections instead of one card carrying different weight.
    let mut cards = String::new();
    let mut item_total = 0usize;
    for (section, _, _) in vault::SECTIONS {
        let Some(slugs) = state.nav.get(*section) else {
            continue;
        };
        if slugs.is_empty() {
            continue;
        }
        item_total += slugs.len();
        let first = &slugs[0];
        let tab = vault::default_tab(section);
        let n = slugs.len();
        let noun = if n == 1 { "item" } else { "items" };
        cards.push_str(&format!(
            "<a class=\"home-card\" href=\"/{section}/{first}/{tab}\">\
             <h2>{}</h2><p>{n} {noun}</p></a>\n",
            vault::to_title(section),
        ));
    }

    let tiers = tokens_gallery::load_and_flatten(&state.vault);
    let token_count: usize = tiers
        .iter()
        .flat_map(|tier| &tier.groups)
        .map(|group| group.entries.len())
        .sum();

    // Phase 5 — a live palette strip, not decoration: real swatches pulled from the same
    // primitive.color group the /tokens gallery renders, not invented graphics. Picks one
    // representative mid-tone from each named color family, in a fixed, meaningful order.
    let palette_paths = [
        "color.primary-60",
        "color.positive-60",
        "color.critical-60",
        "color.caution-60",
        "color.neutral-60",
    ];
    let all_entries: Vec<&tokens_gallery::TokenEntry> = tiers
        .iter()
        .flat_map(|tier| &tier.groups)
        .flat_map(|group| &group.entries)
        .collect();
    let found_palette: Vec<&&tokens_gallery::TokenEntry> = palette_paths
        .iter()
        .filter_map(|p| all_entries.iter().find(|e| e.path == *p))
        .collect();
    // Part C (2026-07-04): the swatches attach directly to the token-count stat instead
    // of forming their own row with a separate caption sentence — one visual idea, not
    // three stacked widgets. Real values, same as before; no invented graphics.
    let palette: String = found_palette
        .iter()
        .map(|e| {
            format!(
                "<span class=\"home-swatch\" style=\"background:{}\" title=\"{}\"></span>",
                e.value, e.path
            )
        })
        .collect();

    let content = format!(
        "<div class=\"home-body\">\
         <div class=\"home-hero\">\
         <p class=\"home-eyebrow\">Token documentation &amp; component library</p>\
         <h1>One governed token graph, not a components folder every team forks and drifts&nbsp;from.</h1>\
         <p class=\"home-lede\">Most design systems ship a components folder — files, a Storybook, a package to \
         install. The moment a team needs something the library doesn't have, they fork it, and the fork drifts \
         from the source. This system ships the token graph itself: DTCG-native, self-hostable, and readable \
         directly by the codegen agents that consume it, not just by designers.</p>\
         <p class=\"home-lede\">Every token, every component recipe, and every research decision behind it lives \
         in one versioned source — browse it below, or <a class=\"home-cta-button\" href=\"/bundles/tokens\">download \
         the whole graph as a bundle</a>.</p>\
         <div class=\"home-stats\">\
         <div class=\"home-stat\"><span class=\"home-stat-value\">{item_total}</span>\
         <span class=\"home-stat-label\">components &amp; elements</span></div>\
         <div class=\"home-stat\"><span class=\"home-stat-value\">{token_count}</span>\
         <span class=\"home-stat-label\">tokens</span>\
         <span class=\"home-stat-swatches\" aria-hidden=\"true\">{palette}</span></div>\
         <span class=\"home-stat-tag\">DTCG-native</span>\
         <span class=\"home-stat-tag\">Apache-2.0 tokens &amp; bundles</span>\
         </div>\
         </div>\
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

    let nav_html = render::render_nav(
        &state.env,
        &state.nav,
        &state.component_groups,
        vault::SECTIONS,
        "",
        "",
    );
    Html(render::shell(
        &state.env,
        "Tokens — PointSav Design System",
        &nav_html,
        "",
        "Tokens",
        &body,
    ))
}

#[derive(Serialize)]
struct AdoptionGroup {
    consumer: String,
    count: usize,
    slugs: Vec<String>,
}

/// Phase 4 — a real, verifiable "who uses this" view, not an invented usage metric.
/// Reuses the same `recipe.json` `category` field `discover_component_groups` already
/// computes for the sidebar grouping (Phase 2) and the per-component origin badge
/// (Phase 3) — this is the third and final consumer of that one authored fact.
pub async fn adoption_page(State(state): State<AppState>) -> Html<String> {
    let generic_count = state
        .component_groups
        .iter()
        .find(|(label, _)| label.is_empty())
        .map(|(_, slugs)| slugs.len())
        .unwrap_or(0);

    let consumers: Vec<AdoptionGroup> = state
        .component_groups
        .iter()
        .filter(|(label, _)| !label.is_empty())
        .map(|(label, slugs)| AdoptionGroup {
            consumer: label
                .strip_prefix("Also used on ")
                .or_else(|| label.strip_prefix("Also used by "))
                .unwrap_or(label)
                .to_string(),
            count: slugs.len(),
            slugs: slugs.clone(),
        })
        .collect();

    let body = state
        .env
        .get_template("adoption.html")
        .expect("adoption.html missing")
        .render(minijinja::context! { generic_count => generic_count, consumers => consumers })
        .expect("render adoption.html failed");

    let nav_html = render::render_nav(
        &state.env,
        &state.nav,
        &state.component_groups,
        vault::SECTIONS,
        "",
        "",
    );
    Html(render::shell(
        &state.env,
        "Adoption — PointSav Design System",
        &nav_html,
        "",
        "Adoption",
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
    // A flat section (research/developing/designing/about) has exactly one canonical
    // tab per slug — reject any other tab value in the URL rather than silently serving
    // the same content at every tab (would otherwise be a duplicate-content URL smell).
    if vault::is_flat_section(&section) && tab != vault::default_tab(&section) {
        return (StatusCode::NOT_FOUND, "tab not found").into_response();
    }
    let tabs = vault::discover_tabs(&state.vault, &section, &slug);
    if tabs.is_empty() {
        return (StatusCode::NOT_FOUND, "item not found").into_response();
    }
    let md_path = vault::content_path(&state.vault, &section, &slug, &tab);
    let raw = match fs::read_to_string(&md_path) {
        Ok(s) => s,
        Err(_) => return (StatusCode::NOT_FOUND, "tab not found").into_response(),
    };

    let (frontmatter, body) = vault::parse_frontmatter(&raw);
    let schema_type = schema::detect(&frontmatter);
    let mut content = schema::render(schema_type, &frontmatter, &body);

    // P1.1 — live component preview (recipe.json variants, sandboxed via iframe).
    // Phase 3 — origin + freshness meta badges, ahead of the preview.
    if section == "components" {
        let badges =
            component_meta::render_meta_badges(&state.component_groups, &state.vault, &slug);
        if let Some(preview) = component_preview::render_preview(&state.vault, &slug) {
            content = format!("{badges}{preview}{content}");
        } else {
            content = format!("{badges}{content}");
        }
    }

    let nav_html = render::render_nav(
        &state.env,
        &state.nav,
        &state.component_groups,
        vault::SECTIONS,
        &section,
        &slug,
    );
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
