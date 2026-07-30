// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

// DESIGN-BUNDLE directory mounts — list, serve, and zip-download an entire
// externally-owned directory (canonical-source-with-downstream-mount, DOCTRINE §IV.e).
// The source directory is never copied; `state.bundle_mounts` only holds a path.
use crate::{render, schema, state::AppState, vault};
use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{Html, IntoResponse, Response},
};
use std::{fs, io::Write, path::Path as StdPath};

#[derive(serde::Serialize)]
struct BundleFile {
    filename: String,
    title: String,
}

/// Lists any file in the mount (not just `.md`) — non-.md files (e.g. the tokens.css /
/// tokens.full.json bundle) get a title derived from the filename instead of frontmatter.
fn list_bundle_files(dir: &StdPath) -> Vec<BundleFile> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<BundleFile> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            let title = if name.ends_with(".md") {
                let raw = fs::read_to_string(e.path()).ok()?;
                let (fm, _) = vault::parse_frontmatter(&raw);
                fm.get("title")
                    .cloned()
                    .unwrap_or_else(|| vault::to_title(name.strip_suffix(".md").unwrap_or(&name)))
            } else {
                name.clone()
            };
            Some(BundleFile {
                filename: name,
                title,
            })
        })
        .collect();
    files.sort_by(|a, b| a.filename.cmp(&b.filename));
    files
}

fn content_type_for(filename: &str) -> &'static str {
    if filename.ends_with(".css") {
        "text/css; charset=utf-8"
    } else if filename.ends_with(".json") {
        "application/json; charset=utf-8"
    } else {
        "text/plain; charset=utf-8"
    }
}

pub async fn list(Path(name): Path<String>, State(state): State<AppState>) -> Response {
    let Some(dir) = state.bundle_mounts.get(&name) else {
        return (StatusCode::NOT_FOUND, "bundle not found").into_response();
    };
    let files = list_bundle_files(dir);
    let title = vault::to_title(&name);

    let list_html = state
        .env
        .get_template("bundle.html")
        .expect("bundle.html missing")
        .render(minijinja::context! {
            name => name,
            file_count => files.len(),
            files => files,
        })
        .expect("render bundle.html failed");

    let nav_html = render::render_nav(&state.env, &state.nav, &state.component_groups, vault::SECTIONS, "", "");
    Html(render::shell(
        &state.env,
        &format!("{title} — PointSav Design System"),
        &nav_html,
        "",
        &title,
        &list_html,
    ))
    .into_response()
}

pub async fn file(
    Path((name, filename)): Path<(String, String)>,
    State(state): State<AppState>,
) -> Response {
    if filename.contains("..") || filename.contains('/') {
        return (StatusCode::BAD_REQUEST, "invalid").into_response();
    }
    let Some(dir) = state.bundle_mounts.get(&name) else {
        return (StatusCode::NOT_FOUND, "bundle not found").into_response();
    };
    let Ok(raw) = fs::read_to_string(dir.join(&filename)) else {
        return (StatusCode::NOT_FOUND, "file not found").into_response();
    };

    if !filename.ends_with(".md") {
        return ([(header::CONTENT_TYPE, content_type_for(&filename))], raw).into_response();
    }

    let (frontmatter, body) = vault::parse_frontmatter(&raw);
    let schema_type = schema::detect(&frontmatter);
    let content = schema::render(schema_type, &frontmatter, &body);

    let nav_html = render::render_nav(&state.env, &state.nav, &state.component_groups, vault::SECTIONS, "", "");
    Html(render::shell(
        &state.env,
        &format!("{filename} — PointSav Design System"),
        &nav_html,
        "",
        "",
        &content,
    ))
    .into_response()
}

pub async fn download(Path(name): Path<String>, State(state): State<AppState>) -> Response {
    let Some(dir) = state.bundle_mounts.get(&name) else {
        return (StatusCode::NOT_FOUND, "bundle not found").into_response();
    };
    let files = list_bundle_files(dir);
    if files.is_empty() {
        return (StatusCode::NOT_FOUND, "bundle is empty").into_response();
    }

    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut buf);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        for f in &files {
            let Ok(bytes) = fs::read(dir.join(&f.filename)) else {
                continue;
            };
            if zip.start_file(&f.filename, options).is_err() {
                continue;
            }
            let _ = zip.write_all(&bytes);
        }
        let _ = zip.finish();
    }

    (
        [
            (header::CONTENT_TYPE, "application/zip".to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{name}.zip\""),
            ),
        ],
        buf.into_inner(),
    )
        .into_response()
}
