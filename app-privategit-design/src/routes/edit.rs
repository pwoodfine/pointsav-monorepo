// D3 — WYSIWYG edit overlay: raw markdown GET + authenticated PUT save-back.

use crate::state::AppState;
use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
};
use std::fs;

/// Serve the raw markdown source for a vault file.
/// GET /vault/elements/:slug/:tab/raw
pub async fn get_raw(
    Path((slug, tab)): Path<(String, String)>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    if bad_path(&slug) || bad_path(&tab) {
        return (StatusCode::BAD_REQUEST, "invalid path").into_response();
    }
    let path = state
        .vault
        .join("elements")
        .join(&slug)
        .join(format!("{}.md", tab));
    match fs::read_to_string(&path) {
        Ok(s) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            s,
        )
            .into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "not found").into_response(),
    }
}

/// Save edited markdown back to vault.
/// PUT /vault/elements/:slug/:tab
/// Requires Authorization: Bearer <edit_token> header.
/// SYS-ADR-10: operator reviews changes in the textarea before clicking Confirm.
pub async fn put_save(
    Path((slug, tab)): Path<(String, String)>,
    State(state): State<AppState>,
    headers: HeaderMap,
    body: String,
) -> impl IntoResponse {
    // Auth check
    let auth = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");
    if auth != state.edit_token.as_str() {
        return StatusCode::UNAUTHORIZED.into_response();
    }

    if bad_path(&slug) || bad_path(&tab) {
        return (StatusCode::BAD_REQUEST, "invalid path").into_response();
    }

    let path = state
        .vault
        .join("elements")
        .join(&slug)
        .join(format!("{}.md", tab));

    // Confirm file already exists — no new file creation via PUT
    if !path.exists() {
        return (StatusCode::NOT_FOUND, "file not found").into_response();
    }

    if let Err(e) = fs::write(&path, body.as_bytes()) {
        eprintln!("edit PUT write error: {}", e);
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    // Re-index updated file and notify SSE clients
    let (fm, body_text) = crate::vault::parse_frontmatter(&body);
    let title = fm
        .get("name")
        .or_else(|| fm.get("title"))
        .cloned()
        .unwrap_or_else(|| tab.clone());
    let doc = moonshot_index::Document {
        id: path.to_string_lossy().to_string(),
        title,
        body: body_text,
    };
    state.index.write().await.insert(doc);
    let _ = state.watch_tx.send(());

    StatusCode::OK.into_response()
}

fn bad_path(s: &str) -> bool {
    s.contains("..") || s.contains('/') || s.contains('\\')
}
