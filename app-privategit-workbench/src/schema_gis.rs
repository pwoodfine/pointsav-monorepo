/// GIS schema handlers — GeoJSON files (*.geojson).
///
/// GET /api/gis/files          → list *.geojson files under configured roots
/// GET /api/gis/feature-count?path=  → count GeoJSON features (for status bar)
///
/// SYS-ADR-07: GeoJSON is structured geometric data. It is never passed through
/// any AI inference layer. The feature-count endpoint counts features
/// deterministically from the raw bytes.
///
/// Viewer: JSON pretty-print in the workbench frontend. MapLibre GL rendering is
/// a planned browser-side capability — the server provides no tile service.
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use serde::Deserialize;
use std::fs;

use crate::{err, resolve_path, AppState};

#[derive(Deserialize)]
pub struct GisQuery {
    path: String,
}

/// GET /api/gis/files
pub async fn list_files(State(state): State<AppState>) -> Response {
    let mut files: Vec<String> = Vec::new();
    for root in state.roots.iter() {
        collect_by_ext(&root.fs_path, &root.url_prefix, "geojson", &mut files);
    }
    Json(serde_json::json!({ "files": files })).into_response()
}

/// GET /api/gis/feature-count?path=
/// Counts the number of GeoJSON `Feature` objects by scanning raw bytes.
/// Does not parse the full JSON (SYS-ADR-07 — no AI layer; but also no
/// need to build a full DOM for a count).
pub async fn feature_count(
    State(state): State<AppState>,
    Query(q): Query<GisQuery>,
) -> Response {
    let (fs_path, _) = match resolve_path(&state.roots, &q.path) {
        Ok(v) => v,
        Err(e) => return err(StatusCode::BAD_REQUEST, e.to_string()),
    };

    if !fs_path.is_file() {
        return err(StatusCode::NOT_FOUND, "file not found");
    }

    let content = match fs::read_to_string(&fs_path) {
        Ok(s) => s,
        Err(e) => return err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };

    // Count occurrences of `"type":"Feature"` and `"type": "Feature"` (both forms).
    let count = content.matches("\"Feature\"").count();

    let file_bytes = content.len();
    Json(serde_json::json!({
        "path": q.path,
        "feature_count": count,
        "file_bytes": file_bytes,
    }))
    .into_response()
}

fn collect_by_ext(dir: &str, prefix: &str, ext: &str, out: &mut Vec<String>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() && p.extension().and_then(|e| e.to_str()) == Some(ext) {
                if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                    out.push(format!("{}/{}", prefix, name));
                }
            }
        }
    }
}
