/// Schedule schema handlers — TaskJuggler DSL files (*.tjp).
///
/// GET /api/schedule/files  → list *.tjp files under configured roots
///
/// Server-side render is out of scope for v0: TaskJuggler produces HTML from
/// a separate `tj3` binary invocation. The viewer renders plain text with
/// structure hints (project/task/resource declarations highlighted).
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use serde::Deserialize;
use std::fs;

use crate::{err, resolve_path, AppState};

#[derive(Deserialize)]
pub struct ScheduleQuery {
    path: String,
}

/// GET /api/schedule/files
/// Lists *.tjp files from all configured roots.
pub async fn list_files(State(state): State<AppState>) -> Response {
    let mut files: Vec<String> = Vec::new();
    for root in state.roots.iter() {
        collect_by_ext(&root.fs_path, &root.url_prefix, "tjp", &mut files);
    }
    Json(serde_json::json!({ "files": files })).into_response()
}

/// GET /api/schedule/syntax-hints?path=
/// Returns a list of structure-hint offsets for TaskJuggler keywords
/// (project, task, resource, milestone). Used by the viewer to add
/// visual markers without server-side rendering.
pub async fn syntax_hints(
    State(state): State<AppState>,
    Query(q): Query<ScheduleQuery>,
) -> Response {
    let (fs_path, _) = match resolve_path(&state.roots, &q.path) {
        Ok(v) => v,
        Err(e) => return err(StatusCode::BAD_REQUEST, e.to_string()),
    };

    if !fs_path.is_file() {
        return err(StatusCode::NOT_FOUND, "file not found");
    }

    let src = match fs::read_to_string(&fs_path) {
        Ok(s) => s,
        Err(e) => return err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };

    let keywords = ["project", "task", "resource", "milestone", "macro"];
    let mut hints: Vec<serde_json::Value> = Vec::new();
    for line in src.lines().enumerate().map(|(i, l)| (i + 1, l)) {
        let (lineno, text) = line;
        let trimmed = text.trim_start();
        for kw in &keywords {
            if trimmed.starts_with(kw) {
                hints.push(serde_json::json!({ "line": lineno, "keyword": kw }));
                break;
            }
        }
    }

    Json(serde_json::json!({ "hints": hints, "line_count": src.lines().count() })).into_response()
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
