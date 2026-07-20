// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

use crate::{state::AppState, vault};
use axum::{extract::State, http::header, response::IntoResponse};

pub async fn robots_txt(State(state): State<AppState>) -> impl IntoResponse {
    let body = format!(
        "User-agent: *\nAllow: /\nSitemap: {}/sitemap.xml\n",
        state.site_origin
    );
    ([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], body)
}

fn push_url(body: &mut String, site_origin: &str, path: &str) {
    body.push_str("  <url><loc>");
    body.push_str(site_origin);
    body.push_str(path);
    body.push_str("</loc></url>\n");
}

/// GET /sitemap.xml — generated from the real route table (`vault::SECTIONS` +
/// `state.nav`'s discovered slugs + `vault::discover_tabs`' discovered tabs), not a
/// hand-maintained list — stays correct as vault content is added or removed.
pub async fn sitemap_xml(State(state): State<AppState>) -> impl IntoResponse {
    let mut body = String::new();
    body.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    body.push_str("<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n");

    push_url(&mut body, &state.site_origin, "/");
    push_url(&mut body, &state.site_origin, "/es");
    push_url(&mut body, &state.site_origin, "/tokens");
    push_url(&mut body, &state.site_origin, "/adoption");

    for (section, _, _) in vault::SECTIONS {
        let Some(slugs) = state.nav.get(*section) else {
            continue;
        };
        for slug in slugs {
            for tab in vault::discover_tabs(&state.vault, section, slug) {
                push_url(&mut body, &state.site_origin, &format!("/{section}/{slug}/{tab}"));
            }
        }
    }

    body.push_str("</urlset>\n");
    ([(header::CONTENT_TYPE, "application/xml; charset=utf-8")], body)
}
