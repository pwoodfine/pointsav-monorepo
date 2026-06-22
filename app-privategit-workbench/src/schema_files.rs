/// Memo/document schema handlers — HTML document files.
///
/// GET /api/files           → list *.html + *.md files from all roots
/// POST /api/files/create   → create a blank HTML document in first writable root
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

/// GET /api/files
/// Lists *.html and *.md files from all configured roots.
pub async fn list_files(State(state): State<AppState>) -> Response {
    let mut files: Vec<serde_json::Value> = Vec::new();
    for root in state.roots.iter() {
        if let Ok(entries) = fs::read_dir(&root.fs_path) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file() {
                    let ext = p
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    if ext == "html" || ext == "md" {
                        if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                            files.push(serde_json::json!({
                                "path": format!("{}/{}", root.url_prefix, name),
                                "name": name,
                                "format": ext,
                            }));
                        }
                    }
                }
            }
        }
    }
    Json(serde_json::json!({ "files": files })).into_response()
}

/// POST /api/files/create  body: {"name": "<doc>.html"}
/// Creates a blank HTML document in the first writable root.
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
    if !name.ends_with(".html") && !name.ends_with(".md") {
        name.push_str(".html");
    }

    let root = match state.roots.iter().find(|r| r.writable) {
        Some(r) => r,
        None => return err(StatusCode::FORBIDDEN, "no writable root configured"),
    };

    let fs_path = std::path::PathBuf::from(&root.fs_path).join(&name);
    if fs_path.exists() {
        return err(StatusCode::CONFLICT, "file already exists");
    }

    let template = b"<!DOCTYPE html>\n<html lang=\"en\">\n<head><meta charset=\"UTF-8\"><title>New Document</title></head>\n<body>\n\n</body>\n</html>\n";
    if let Err(e) = fs::write(&fs_path, template) {
        return err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string());
    }

    let path = format!("{}/{}", root.url_prefix.trim_end_matches('/'), name);
    Json(CreateResponse { ok: true, path }).into_response()
}
