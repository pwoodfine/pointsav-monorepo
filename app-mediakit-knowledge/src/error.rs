use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};

#[derive(Debug, thiserror::Error)]
pub enum WikiError {
    #[error("page not found: {0}")]
    NotFound(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("frontmatter parse error: {0}")]
    Frontmatter(#[from] serde_yaml::Error),

    /// Phase 2: slug failed `^[a-z0-9._-]+$` validation (path-traversal,
    /// uppercase, spaces, leading dot, `..` sequence).
    #[error("invalid slug: {0}")]
    SlugInvalid(String),

    /// Phase 2: atomic write to disk failed (temp-file create, write, or
    /// persist/rename).
    #[error("write failed: {0}")]
    WriteFailed(String),

    /// Phase 2: `/create` invoked with a slug that already exists on disk.
    #[error("already exists: {0}")]
    AlreadyExists(String),

    /// Phase 2 Step 5: citation registry file could not be read or parsed.
    #[error("citation registry load failed: {0}")]
    CitationLoadFailed(String),

    /// Phase 3 Step 3.1: tantivy index build, query, or reindex failed.
    #[error("search failed: {0}")]
    SearchFailed(String),

    /// Phase 4 Steps 4.4+4.5: redb link-graph or blake3 hash operation failed.
    #[error("link graph error: {0}")]
    LinkGraph(String),
}

impl IntoResponse for WikiError {
    fn into_response(self) -> Response {
        let status = match &self {
            WikiError::NotFound(_) => StatusCode::NOT_FOUND,
            WikiError::SlugInvalid(_) => StatusCode::BAD_REQUEST,
            WikiError::AlreadyExists(_) => StatusCode::CONFLICT,
            WikiError::CitationLoadFailed(_) => StatusCode::INTERNAL_SERVER_ERROR,
            WikiError::SearchFailed(_) => StatusCode::INTERNAL_SERVER_ERROR,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        tracing::warn!(error = %self, "request error");

        let (heading, body_html) = match &self {
            WikiError::NotFound(slug) => (
                "Page not found".to_owned(),
                format!(
                    "<p class=\"wiki-error-message\">The article \
                     <strong>{}</strong> does not exist in this wiki.</p>\
                     <p class=\"wiki-error-detail\">\
                     You can <a href=\"/\">return to the home page</a> or \
                     <a href=\"/search\">search</a> for what you are looking for.</p>",
                    html_escape(slug)
                ),
            ),
            WikiError::SlugInvalid(slug) => (
                "Invalid page name".to_owned(),
                format!(
                    "<p class=\"wiki-error-message\">\
                     <code>{}</code> is not a valid article name.</p>",
                    html_escape(slug)
                ),
            ),
            WikiError::AlreadyExists(slug) => (
                "Article already exists".to_owned(),
                format!(
                    "<p class=\"wiki-error-message\">\
                     <a href=\"/wiki/{}\">{}</a> already exists.</p>",
                    html_escape(slug),
                    html_escape(slug)
                ),
            ),
            _ => (
                format!("Error {}", status.as_u16()),
                format!(
                    "<p class=\"wiki-error-message\">{}</p>",
                    html_escape(&self.to_string())
                ),
            ),
        };

        let html = format!(
            r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width,initial-scale=1">
  <title>{heading}</title>
  <link rel="stylesheet" href="/static/style.css">
</head>
<body class="wiki-error-body">
  <header class="wiki-error-header">
    <a class="wiki-error-home" href="/">← Home</a>
  </header>
  <main class="wiki-error-page">
    <h1 class="wiki-error-title">{heading}</h1>
    {body_html}
  </main>
</body>
</html>"#,
        );

        (
            status,
            [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
            html,
        )
            .into_response()
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
