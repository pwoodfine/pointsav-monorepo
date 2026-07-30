use crate::state::AppState;
use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    response::IntoResponse,
};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct SearchQuery {
    q: Option<String>,
}

pub async fn token_search(
    Query(params): Query<SearchQuery>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let q = match params.q.as_deref().filter(|s| !s.is_empty()) {
        Some(q) => q.to_string(),
        None => {
            return (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "application/json")],
                "[]".to_string(),
            );
        }
    };

    let idx = state.index.read().await;
    let hits = idx.search(&q);

    let json = {
        let items: Vec<String> = hits
            .into_iter()
            .take(20)
            .map(|doc| {
                let snippet = snippet(&doc.body, &q);
                format!(
                    "{{\"id\":{},\"title\":{},\"snippet\":{}}}",
                    json_str(&doc.id),
                    json_str(&doc.title),
                    json_str(&snippet)
                )
            })
            .collect();
        format!("[{}]", items.join(","))
    };

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        json,
    )
}

fn snippet(body: &str, query: &str) -> String {
    let q_lower = query.to_lowercase();
    let b_lower = body.to_lowercase();
    if let Some(pos) = b_lower.find(q_lower.split_whitespace().next().unwrap_or("")) {
        let start = pos.saturating_sub(40);
        let end = (pos + 120).min(body.len());
        let s = &body[start..end];
        s.replace('\n', " ").trim().to_string()
    } else {
        body.chars()
            .take(120)
            .collect::<String>()
            .replace('\n', " ")
    }
}

fn json_str(s: &str) -> String {
    let escaped = s
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t");
    format!("\"{}\"", escaped)
}
