// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

use axum::{
    extract::{Path, State},
    response::{IntoResponse, Redirect, Response},
};
use std::io::Write;
use zip::{write::FileOptions, CompressionMethod, ZipWriter};

use crate::state::AppState;

/// `/furniture` is now folded into the unified home catalog's Objects tab.
/// Redirect legacy links/bookmarks to `/` (302). The download and bundle
/// sub-routes below are unchanged.
pub async fn furniture_handler() -> Redirect {
    Redirect::to("/")
}

pub async fn bundle_handler(State(state): State<AppState>) -> Response {
    let lib_dir = state.config.library_dir.join("blocks").join("furniture");
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
    let file_path = state
        .config
        .library_dir
        .join("blocks")
        .join("furniture")
        .join(&safe_name);
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
