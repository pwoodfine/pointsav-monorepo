use axum::{
    extract::{Path, State},
    http::HeaderMap,
    response::{Html, IntoResponse, Response},
};
use std::io::Write;
use zip::{write::FileOptions, CompressionMethod, ZipWriter};

use crate::{render, state::AppState};

pub async fn furniture_handler(headers: HeaderMap, State(state): State<AppState>) -> Html<String> {
    let content = render::card::render_furniture(&state);
    if is_fragment(&headers) {
        Html(content)
    } else {
        Html(render::shell::page_shell(
            "Furniture Library",
            "/furniture",
            &content,
            &state,
        ))
    }
}

pub async fn furniture_fragment(State(state): State<AppState>) -> Html<String> {
    Html(render::card::render_furniture(&state))
}

pub async fn bundle_handler(State(state): State<AppState>) -> Response {
    let lib_dir = state.config.library_dir.join("components");
    match build_zip_bundle(&lib_dir) {
        Ok(bytes) => (
            axum::http::StatusCode::OK,
            [
                ("Content-Type", "application/zip"),
                (
                    "Content-Disposition",
                    "attachment; filename=\"bim-furniture-bundle.zip\"",
                ),
            ],
            bytes,
        )
            .into_response(),
        Err(e) => {
            eprintln!("warn: bundle zip failed: {e}");
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "bundle error",
            )
                .into_response()
        }
    }
}

pub async fn single_handler(
    Path(filename): Path<String>,
    State(state): State<AppState>,
) -> Response {
    let safe_name = filename.replace("..", "").replace('/', "");
    let file_path = state.config.library_dir.join("components").join(&safe_name);
    match std::fs::read(&file_path) {
        Ok(bytes) => (
            axum::http::StatusCode::OK,
            [
                ("Content-Type", "application/x-step"),
                (
                    "Content-Disposition",
                    &format!("attachment; filename=\"{safe_name}\""),
                ),
            ],
            bytes,
        )
            .into_response(),
        Err(_) => (axum::http::StatusCode::NOT_FOUND, "file not found").into_response(),
    }
}

fn build_zip_bundle(dir: &std::path::Path) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let buf = Vec::new();
    let cursor = std::io::Cursor::new(buf);
    let mut zip = ZipWriter::new(cursor);
    let opts: FileOptions<()> =
        FileOptions::default().compression_method(CompressionMethod::Deflated);

    if dir.exists() {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("ifc") {
                let name = path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("file.ifc");
                zip.start_file(name, opts)?;
                let bytes = std::fs::read(&path)?;
                zip.write_all(&bytes)?;
            }
        }
    }

    let cursor = zip.finish()?;
    Ok(cursor.into_inner())
}

fn is_fragment(headers: &HeaderMap) -> bool {
    headers.get("x-fragment").is_some()
}
