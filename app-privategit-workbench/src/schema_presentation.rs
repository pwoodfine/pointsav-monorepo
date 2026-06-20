/// Presentation schema handlers — JSON slide decks.
///
/// GET /api/presentation/files  → list *.json files under the first writable root
/// GET /api/presentation/render?path=  → emit HTML slide sequence from JSON
/// PUT /api/presentation/file?path=    → atomic save (reuses AppState write path)
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Json, Response},
};
use serde::Deserialize;
use serde_json::Value;
use std::fs;

use crate::{err, resolve_path, AppState};

#[derive(Deserialize)]
pub struct PresentationQuery {
    path: String,
}

/// GET /api/presentation/files
/// Lists *.json files from all configured roots.
pub async fn list_files(State(state): State<AppState>) -> Response {
    let mut files: Vec<String> = Vec::new();
    for root in state.roots.iter() {
        if let Ok(entries) = fs::read_dir(&root.fs_path) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file() && p.extension().and_then(|e| e.to_str()) == Some("json") {
                    if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                        files.push(format!("{}/{}", root.url_prefix, name));
                    }
                }
            }
        }
    }
    Json(serde_json::json!({ "files": files })).into_response()
}

/// GET /api/presentation/render?path=
/// Reads a JSON slide deck and emits a standalone HTML presentation.
pub async fn render(State(state): State<AppState>, Query(q): Query<PresentationQuery>) -> Response {
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

    let deck: Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(e) => return err(StatusCode::UNPROCESSABLE_ENTITY, format!("JSON parse error: {}", e)),
    };

    let title = deck["title"].as_str().unwrap_or("Presentation").to_string();
    let slides = deck["slides"].as_array().cloned().unwrap_or_default();

    let mut slides_html = String::new();
    for (i, slide) in slides.iter().enumerate() {
        let heading = slide["heading"].as_str().unwrap_or("").to_string();
        let body = slide["body"].as_str().unwrap_or("").to_string();
        slides_html.push_str(&format!(
            r#"<section class="slide" id="slide-{i}"><h2>{heading}</h2><p>{body}</p></section>"#,
            i = i,
            heading = esc(&heading),
            body = esc(&body),
        ));
    }

    let html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head><meta charset="UTF-8"><title>{title}</title>
<style>
body {{ font-family: -apple-system, sans-serif; margin: 0; background: #f5f5f5; }}
.slide {{ min-height: 400px; padding: 60px 80px; background: #fff;
          border-radius: 8px; margin: 24px auto; max-width: 720px;
          box-shadow: 0 2px 8px rgba(0,0,0,.12); page-break-after: always; }}
h1 {{ text-align: center; margin: 0 0 16px; font-size: 1.6em; color: #222; }}
h2 {{ font-size: 1.4em; margin: 0 0 20px; color: #24292e; }}
p {{ font-size: 1em; line-height: 1.6; color: #444; }}
.deck-title {{ text-align: center; padding: 40px; font-size: 2em; font-weight: 700; }}
</style>
</head>
<body>
<div class="slide deck-title">{title}</div>
{slides_html}
</body>
</html>"#,
        title = esc(&title),
        slides_html = slides_html,
    );

    Html(html).into_response()
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
