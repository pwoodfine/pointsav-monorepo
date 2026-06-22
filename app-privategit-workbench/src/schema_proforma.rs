/// Proforma schema handlers — JSON proforma files.
///
/// GET /api/proforma/files      → list *.json proforma files from all roots
/// POST /api/proforma/create    → create a blank proforma JSON in first writable root
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use serde::{Deserialize, Serialize};
use std::fs;

use crate::{err, AppState};

#[derive(Deserialize)]
pub struct CreateBody {
    name: String,
}

#[derive(Serialize)]
struct CreateResponse {
    ok: bool,
    path: String,
}

/// GET /api/proforma/files
/// Lists *.json files (excluding *.bim.json) from all configured roots.
pub async fn list_files(State(state): State<AppState>) -> Response {
    let mut files: Vec<serde_json::Value> = Vec::new();
    for root in state.roots.iter() {
        if let Ok(entries) = fs::read_dir(&root.fs_path) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file() {
                    let name = p
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .to_string();
                    if name.ends_with(".json") && !name.ends_with(".bim.json") {
                        files.push(serde_json::json!({
                            "path": format!("{}/{}", root.url_prefix, name),
                            "name": name,
                            "format": "json",
                        }));
                    }
                }
            }
        }
    }
    Json(serde_json::json!({ "files": files })).into_response()
}

/// POST /api/proforma/create  body: {"name": "<report>.json"}
/// Creates a blank proforma JSON file in the first writable root.
pub async fn create(
    State(state): State<AppState>,
    axum::Json(body): axum::Json<CreateBody>,
) -> Response {
    let mut name = body.name.trim().to_string();
    if name.is_empty() {
        return err(StatusCode::BAD_REQUEST, "name is required");
    }
    if name.contains('/') || name.contains("..") {
        return err(StatusCode::BAD_REQUEST, "invalid name");
    }
    if !name.ends_with(".json") {
        name.push_str(".json");
    }

    let root = match state.roots.iter().find(|r| r.writable) {
        Some(r) => r,
        None => return err(StatusCode::FORBIDDEN, "no writable root configured"),
    };

    let fs_path = std::path::PathBuf::from(&root.fs_path).join(&name);
    if fs_path.exists() {
        return err(StatusCode::CONFLICT, "file already exists");
    }

    if let Err(e) = fs::write(&fs_path, b"{}") {
        return err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string());
    }

    let path = format!("{}/{}", root.url_prefix.trim_end_matches('/'), name);
    Json(CreateResponse { ok: true, path }).into_response()
}
