// Workbench file-browser and editor handlers, nested at /workbench/ in the prototype.
// Handler logic ported from app-privategit-workbench/src/main.rs (read-only vendor source).

use axum::{
    body::Bytes,
    extract::{Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{Html, IntoResponse, Json, Redirect, Response},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, UNIX_EPOCH},
};

use crate::{AppState, Assets, WorkbenchRoot};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(serve_index))
        .route("/file", get(get_file).put(put_file))
        .route("/rename", post(rename_file))
        .route("/duplicate", post(duplicate_file))
        .route("/create", post(create_file))
        .route("/delete", post(delete_file))
        .route("/git-status", get(git_status))
        .route("/document", get(get_document))
        .route("/pdf", get(get_pdf))
        .route("/events", get(get_workbench_events))
        .route("/download", get(download_folder))
        // B2 (Track B) — serve the shared frontend shell-chrome module as embedded assets.
        .route("/shell-chrome.js", get(serve_shell_chrome_js))
        .route("/shell-chrome.css", get(serve_shell_chrome_css))
        // In-workbench AI chat (Cursor-style) — proxies to the SLM Doorman.
        .route("/chat", post(chat))
}

// ---------------------------------------------------------------------------
// AI chat — proxy to the SLM Doorman (service-slm)
// ---------------------------------------------------------------------------

/// Local SLM Doorman inference endpoint. Model "olmo" routes to Tier A (local
/// OLMo 7B, free, SYS-ADR-07-safe — no data leaves the VM).
const DOORMAN_URL: &str = "http://127.0.0.1:9080/v1/chat/completions";

#[derive(Deserialize, Serialize, Clone)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatRequest {
    messages: Vec<ChatMessage>,
}

#[derive(Serialize)]
struct DoormanRequest<'a> {
    model: &'a str,
    messages: &'a [ChatMessage],
    max_tokens: u32,
    stream: bool,
}

#[derive(Deserialize)]
struct DoormanResponse {
    content: Option<String>,
    tier_used: Option<String>,
    inference_ms: Option<u64>,
    cost_usd: Option<f64>,
}

/// POST /workbench/chat — forward the conversation to the Doorman and return the reply.
/// The browser cannot reach 127.0.0.1:9080 itself, so the workbench backend proxies.
async fn chat(Json(req): Json<ChatRequest>) -> Response {
    if req.messages.is_empty() {
        return (StatusCode::BAD_REQUEST, "no messages").into_response();
    }
    let body = DoormanRequest {
        model: "olmo",
        messages: &req.messages,
        max_tokens: 512,
        stream: false,
    };
    let client = reqwest::Client::new();
    let resp = match client
        .post(DOORMAN_URL)
        .json(&body)
        .timeout(std::time::Duration::from_secs(120))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::BAD_GATEWAY,
                format!("SLM Doorman unreachable: {e}"),
            )
                .into_response()
        }
    };
    if !resp.status().is_success() {
        let code = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return (StatusCode::BAD_GATEWAY, format!("Doorman {code}: {text}")).into_response();
    }
    match resp.json::<DoormanResponse>().await {
        Ok(dr) => Json(serde_json::json!({
            "content": dr.content.unwrap_or_default(),
            "tier_used": dr.tier_used,
            "inference_ms": dr.inference_ms,
            "cost_usd": dr.cost_usd,
        }))
        .into_response(),
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            format!("bad Doorman response: {e}"),
        )
            .into_response(),
    }
}

// ---------------------------------------------------------------------------
// Asset serving
// ---------------------------------------------------------------------------

fn serve_embedded(key: &str, content_type: &'static str) -> Response {
    match Assets::get(key) {
        Some(content) => (
            [(header::CONTENT_TYPE, content_type)],
            content.data.into_owned(),
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn serve_shell_chrome_js() -> Response {
    serve_embedded("workbench/shell-chrome.js", "text/javascript; charset=utf-8")
}

async fn serve_shell_chrome_css() -> Response {
    serve_embedded("workbench/shell-chrome.css", "text/css; charset=utf-8")
}

async fn serve_index() -> Response {
    match Assets::get("workbench/index.html") {
        Some(content) => {
            let html = String::from_utf8_lossy(&content.data).into_owned();
            let injected = html.replacen(
                "</head>",
                "<base href=\"/workbench/\">\n</head>",
                1,
            );
            (
                [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
                injected,
            )
                .into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

// ---------------------------------------------------------------------------
// Path resolution
// ---------------------------------------------------------------------------

fn resolve_path(roots: &[WorkbenchRoot], url_path: &str) -> Result<(PathBuf, bool), String> {
    let url_path = url_path.trim_start_matches('/');

    for root in roots {
        let prefix = root.url_prefix.trim_end_matches('/');
        let rest = if url_path == prefix {
            ""
        } else if let Some(r) = url_path.strip_prefix(&format!("{}/", prefix)) {
            r
        } else {
            continue;
        };

        let base = PathBuf::from(&root.fs_path);
        let target = if rest.is_empty() {
            base.clone()
        } else {
            base.join(rest)
        };

        let canonical = if target.exists() {
            target.canonicalize().map_err(|e| e.to_string())?
        } else {
            let parent = target
                .parent()
                .ok_or_else(|| "no parent directory".to_string())?;
            let cp = parent.canonicalize().map_err(|e| e.to_string())?;
            cp.join(
                target
                    .file_name()
                    .ok_or_else(|| "no filename".to_string())?,
            )
        };

        let root_canonical = base.canonicalize().map_err(|e| e.to_string())?;
        if !canonical.starts_with(&root_canonical) {
            return Err("path traversal attempt".to_string());
        }

        return Ok((canonical, root.writable));
    }

    Err(format!("no matching root for path: {}", url_path))
}

fn allowed_write_ext(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    matches!(
        ext.to_lowercase().as_str(),
        "md" | "txt"
            | "html"
            | "css"
            | "js"
            | "ts"
            | "json"
            | "toml"
            | "yaml"
            | "yml"
            | "sh"
            | "rs"
            | "py"
            | "rb"
            | "go"
            | "conf"
            | "ini"
            | "env"
            | "lock"
            | "svg"
    )
}

fn join_parent_url(url_path: &str, new_name: &str) -> String {
    let trimmed = url_path.trim_start_matches('/');
    match trimmed.rsplit_once('/') {
        Some((parent, _)) => format!("{}/{}", parent, new_name),
        None => new_name.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Error helper
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

fn err(status: StatusCode, msg: impl Into<String>) -> Response {
    (status, Json(ErrorBody { error: msg.into() })).into_response()
}

// ---------------------------------------------------------------------------
// GET /workbench/file
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct FileQuery {
    path: String,
    #[serde(default = "default_true")]
    page_numbers: bool,
    #[serde(default)]
    pn_position: Option<String>,
    #[serde(default)]
    paper: Option<String>,
}

fn default_true() -> bool {
    true
}

#[derive(Serialize)]
struct FileResponse {
    content: String,
    mtime: u64,
    writable: bool,
}

async fn get_file(State(state): State<AppState>, Query(q): Query<FileQuery>) -> Response {
    let (fs_path, writable) = match resolve_path(&state.roots, &q.path) {
        Ok(v) => v,
        Err(e) => return err(StatusCode::BAD_REQUEST, e),
    };

    if !fs_path.exists() {
        return err(StatusCode::NOT_FOUND, "file not found");
    }
    if !fs_path.is_file() {
        return err(StatusCode::BAD_REQUEST, "not a file");
    }

    let meta = match fs::metadata(&fs_path) {
        Ok(m) => m,
        Err(e) => return err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };

    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let content = match tokio::fs::read_to_string(&fs_path).await {
        Ok(c) => c,
        Err(e) => return err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };

    Json(FileResponse {
        content,
        mtime,
        writable,
    })
    .into_response()
}

// ---------------------------------------------------------------------------
// PUT /workbench/file
// ---------------------------------------------------------------------------

async fn put_file(
    State(state): State<AppState>,
    Query(q): Query<FileQuery>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if headers
        .get("x-foundry-editor")
        .and_then(|v| v.to_str().ok())
        != Some("1")
    {
        return err(StatusCode::FORBIDDEN, "missing X-Foundry-Editor header");
    }

    let (fs_path, writable) = match resolve_path(&state.roots, &q.path) {
        Ok(v) => v,
        Err(e) => return err(StatusCode::BAD_REQUEST, e),
    };

    if !writable {
        return err(StatusCode::FORBIDDEN, "path is not writable");
    }
    if !allowed_write_ext(&fs_path) {
        return err(StatusCode::FORBIDDEN, "file extension not allowed for writes");
    }
    if body.len() > state.max_bytes {
        return err(StatusCode::PAYLOAD_TOO_LARGE, "file exceeds size limit");
    }

    if let Some(client_mtime_str) = headers.get("x-foundry-mtime").and_then(|v| v.to_str().ok()) {
        if let Ok(client_mtime) = client_mtime_str.parse::<u64>() {
            if fs_path.exists() {
                if let Ok(meta) = fs::metadata(&fs_path) {
                    let server_mtime = meta
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    if server_mtime != client_mtime {
                        return err(StatusCode::CONFLICT, "file modified on disk since last read");
                    }
                }
            }
        }
    }

    let content = match std::str::from_utf8(&body) {
        Ok(s) => s,
        Err(_) => return err(StatusCode::BAD_REQUEST, "body is not valid UTF-8"),
    };

    let ext = fs_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("bin");
    let tmp_path = fs_path.with_extension(format!("{}.tmp", ext));

    let write_result: Result<(), String> = (|| {
        let mut f = fs::File::create(&tmp_path).map_err(|e| e.to_string())?;
        f.write_all(content.as_bytes()).map_err(|e| e.to_string())?;
        f.sync_all().map_err(|e| e.to_string())?;
        fs::rename(&tmp_path, &fs_path).map_err(|e| e.to_string())?;
        Ok(())
    })();

    if let Err(e) = write_result {
        let _ = fs::remove_file(&tmp_path);
        return err(StatusCode::INTERNAL_SERVER_ERROR, e);
    }

    let new_mtime = fs::metadata(&fs_path)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let event_json = format!(r#"{{"event":"changed","path":"{}","mtime":{}}}"#, q.path, new_mtime);
    let _ = state.events_tx.send(event_json);

    let mut resp = HashMap::new();
    resp.insert("ok", serde_json::Value::Bool(true));
    resp.insert("mtime", serde_json::Value::Number(new_mtime.into()));
    Json(resp).into_response()
}

// ---------------------------------------------------------------------------
// GET /workbench/events — SSE stream for real-time file change notifications
// ---------------------------------------------------------------------------

async fn get_workbench_events(State(state): State<AppState>) -> impl IntoResponse {
    use axum::response::sse::{Event, KeepAlive, Sse};
    use std::convert::Infallible;
    use tokio_stream::{wrappers::BroadcastStream, StreamExt as _};

    let rx = state.events_tx.subscribe();
    let stream = BroadcastStream::new(rx)
        .filter_map(|r| r.ok())
        .map(|data| Ok::<_, Infallible>(Event::default().data(data)));
    Sse::new(stream).keep_alive(KeepAlive::default())
}

// ---------------------------------------------------------------------------
// POST /workbench/rename
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct RenameQuery {
    from: String,
    to: String,
}

#[derive(Serialize)]
struct RenameResponse {
    ok: bool,
    new_path: String,
    new_name: String,
}

async fn rename_file(State(state): State<AppState>, Query(q): Query<RenameQuery>) -> Response {
    let new_name = q.to.trim();
    if new_name.is_empty() {
        return err(StatusCode::BAD_REQUEST, "new name is empty");
    }
    if new_name.contains('/') || new_name.contains('\\') {
        return err(StatusCode::BAD_REQUEST, "new name must not contain slashes");
    }

    let (fs_path, writable) = match resolve_path(&state.roots, &q.from) {
        Ok(v) => v,
        Err(e) => return err(StatusCode::BAD_REQUEST, e),
    };

    if !fs_path.exists() {
        return err(StatusCode::NOT_FOUND, "source file not found");
    }
    if !writable {
        return err(StatusCode::FORBIDDEN, "path is not writable");
    }

    let old_name = match fs_path.file_name().and_then(|n| n.to_str()) {
        Some(n) => n,
        None => return err(StatusCode::BAD_REQUEST, "source has no filename"),
    };
    if old_name == new_name {
        return err(StatusCode::BAD_REQUEST, "new name is the same as old name");
    }

    let parent = match fs_path.parent() {
        Some(p) => p,
        None => return err(StatusCode::BAD_REQUEST, "source has no parent directory"),
    };
    let new_fs_path = parent.join(new_name);

    if new_fs_path.exists() {
        return err(StatusCode::CONFLICT, "destination already exists");
    }

    if let Err(e) = fs::rename(&fs_path, &new_fs_path) {
        return err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string());
    }

    let new_url_path = join_parent_url(&q.from, new_name);
    Json(RenameResponse {
        ok: true,
        new_path: new_url_path,
        new_name: new_name.to_string(),
    })
    .into_response()
}

// ---------------------------------------------------------------------------
// POST /workbench/duplicate
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct DuplicateResponse {
    ok: bool,
    new_path: String,
    new_name: String,
}

fn generate_copy_name(parent: &Path, original: &str) -> Option<String> {
    let (stem, ext) = match original.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s.to_string(), Some(e.to_string())),
        _ => (original.to_string(), None),
    };
    let make = |suffix: &str| -> String {
        match &ext {
            Some(e) => format!("{}{}.{}", stem, suffix, e),
            None => format!("{}{}", stem, suffix),
        }
    };
    let first = make("-copy");
    if !parent.join(&first).exists() {
        return Some(first);
    }
    for n in 2..=99 {
        let candidate = make(&format!("-copy-{}", n));
        if !parent.join(&candidate).exists() {
            return Some(candidate);
        }
    }
    None
}

async fn duplicate_file(State(state): State<AppState>, Query(q): Query<FileQuery>) -> Response {
    let (fs_path, writable) = match resolve_path(&state.roots, &q.path) {
        Ok(v) => v,
        Err(e) => return err(StatusCode::BAD_REQUEST, e),
    };

    if !fs_path.exists() {
        return err(StatusCode::NOT_FOUND, "source file not found");
    }
    if !fs_path.is_file() {
        return err(StatusCode::BAD_REQUEST, "source is not a file");
    }
    if !writable {
        return err(StatusCode::FORBIDDEN, "path is not writable");
    }

    let original_name = match fs_path.file_name().and_then(|n| n.to_str()) {
        Some(n) => n,
        None => return err(StatusCode::BAD_REQUEST, "source has no filename"),
    };
    let parent = match fs_path.parent() {
        Some(p) => p,
        None => return err(StatusCode::BAD_REQUEST, "source has no parent directory"),
    };

    let new_name = match generate_copy_name(parent, original_name) {
        Some(n) => n,
        None => return err(StatusCode::CONFLICT, "could not find available copy name"),
    };
    let new_fs_path = parent.join(&new_name);

    if let Err(e) = fs::copy(&fs_path, &new_fs_path) {
        return err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string());
    }

    let new_url_path = join_parent_url(&q.path, &new_name);
    Json(DuplicateResponse {
        ok: true,
        new_path: new_url_path,
        new_name,
    })
    .into_response()
}

// ---------------------------------------------------------------------------
// POST /workbench/create
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct CreateResponse {
    ok: bool,
    path: String,
    name: String,
}

async fn create_file(State(state): State<AppState>, Query(q): Query<FileQuery>) -> Response {
    let (fs_path, writable) = match resolve_path(&state.roots, &q.path) {
        Ok(v) => v,
        Err(e) => return err(StatusCode::BAD_REQUEST, e),
    };

    if !writable {
        return err(StatusCode::FORBIDDEN, "path is not writable");
    }
    if !allowed_write_ext(&fs_path) {
        return err(StatusCode::FORBIDDEN, "file extension not allowed for writes");
    }
    if fs_path.exists() {
        return err(StatusCode::CONFLICT, "file already exists");
    }

    if let Err(e) = fs::write(&fs_path, b"") {
        return err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string());
    }

    let name = fs_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();

    Json(CreateResponse {
        ok: true,
        path: q.path.trim_start_matches('/').to_string(),
        name,
    })
    .into_response()
}

// ---------------------------------------------------------------------------
// POST /workbench/delete
// ---------------------------------------------------------------------------

async fn delete_file(State(state): State<AppState>, Query(q): Query<FileQuery>) -> Response {
    let (fs_path, writable) = match resolve_path(&state.roots, &q.path) {
        Ok(v) => v,
        Err(e) => return err(StatusCode::BAD_REQUEST, e),
    };

    if !writable {
        return err(StatusCode::FORBIDDEN, "path is not writable");
    }
    if !fs_path.exists() {
        return err(StatusCode::NOT_FOUND, "file not found");
    }

    let result = if fs_path.is_dir() {
        fs::remove_dir_all(&fs_path)
    } else {
        fs::remove_file(&fs_path)
    };
    if let Err(e) = result {
        return err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string());
    }

    let mut resp = HashMap::new();
    resp.insert("ok", serde_json::Value::Bool(true));
    Json(resp).into_response()
}

// ---------------------------------------------------------------------------
// GET /workbench/git-status
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct GitStatusQuery {
    root: String,
}

async fn git_status(State(state): State<AppState>, Query(q): Query<GitStatusQuery>) -> Response {
    let mut empty: HashMap<&str, serde_json::Value> = HashMap::new();
    empty.insert("files", serde_json::json!({}));

    let (fs_path, _writable) = match resolve_path(&state.roots, &q.root) {
        Ok(v) => v,
        Err(_) => return Json(&empty).into_response(),
    };

    if !fs_path.is_dir() || !fs_path.join(".git").exists() {
        return Json(&empty).into_response();
    }

    let (tx, rx) = mpsc::channel();
    let path_clone = fs_path.clone();
    thread::spawn(move || {
        let out = Command::new("git")
            .args([
                "-c",
                "core.quotePath=off",
                "-c",
                "safe.directory=*",
                "status",
                "--porcelain",
                "--untracked-files=all",
            ])
            .current_dir(&path_clone)
            .output();
        let _ = tx.send(out);
    });

    let output = match rx.recv_timeout(Duration::from_secs(5)) {
        Ok(Ok(o)) => o,
        _ => return Json(&empty).into_response(),
    };

    if !output.status.success() {
        return Json(&empty).into_response();
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut files: HashMap<String, String> = HashMap::new();
    for line in stdout.lines() {
        if line.len() < 3 {
            continue;
        }
        let code = &line[..2];
        let rest = line[3..].trim();
        let path = if let Some(idx) = rest.find(" -> ") {
            rest[idx + 4..].to_string()
        } else {
            rest.to_string()
        };
        if path.is_empty() {
            continue;
        }
        let status = match code {
            "??" => "untracked",
            "A " | "AM" => "staged",
            "D " | " D" => "deleted",
            "M " | " M" | "MM" => "modified",
            _ => {
                let chars: Vec<char> = code.chars().collect();
                if chars.contains(&'D') {
                    "deleted"
                } else if chars.contains(&'A') {
                    "staged"
                } else if chars.contains(&'M') {
                    "modified"
                } else {
                    continue;
                }
            }
        };
        files.insert(path, status.to_string());
    }

    let mut resp: HashMap<&str, serde_json::Value> = HashMap::new();
    resp.insert(
        "files",
        serde_json::to_value(files).unwrap_or(serde_json::json!({})),
    );
    Json(resp).into_response()
}

// ---------------------------------------------------------------------------
// GET /workbench/document
// ---------------------------------------------------------------------------

#[derive(Debug, PartialEq)]
enum DocType {
    HtmlDoc,
    GeoJson,
    Proforma,
    Other,
}

fn detect_doc_type(path: &Path, content_peek: Option<&str>) -> DocType {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    match ext.as_str() {
        "html" | "htm" => DocType::HtmlDoc,
        "geojson" => DocType::GeoJson,
        "json" => {
            if let Some(peek) = content_peek {
                let p = &peek[..peek.len().min(6144)];
                if p.contains("\"proforma_version\"") {
                    return DocType::Proforma;
                }
                if p.contains("\"entity\"")
                    && p.contains("\"date\"")
                    && (p.contains("\"income\"")
                        || p.contains("\"years\"")
                        || p.contains("\"areas\""))
                {
                    return DocType::Proforma;
                }
            }
            DocType::Other
        }
        _ => DocType::Other,
    }
}

async fn get_document(State(state): State<AppState>, Query(q): Query<FileQuery>) -> Response {
    let (fs_path, _writable) = match resolve_path(&state.roots, &q.path) {
        Ok(v) => v,
        Err(e) => return err(StatusCode::BAD_REQUEST, e),
    };

    if !fs_path.exists() || !fs_path.is_file() {
        return err(StatusCode::NOT_FOUND, "file not found");
    }

    let content = match fs::read_to_string(&fs_path) {
        Ok(s) => s,
        Err(e) => return err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };

    let doc_type = detect_doc_type(&fs_path, Some(&content));

    match doc_type {
        DocType::HtmlDoc => {
            let browse_path = format!("/{}", q.path.trim_start_matches('/'));
            Redirect::temporary(&browse_path).into_response()
        }
        DocType::Proforma => render_proforma_document(&content, &fs_path),
        DocType::GeoJson => render_geojson_placeholder(&content, &fs_path),
        DocType::Other => {
            let browse_path = format!("/{}", q.path.trim_start_matches('/'));
            Redirect::temporary(&browse_path).into_response()
        }
    }
}

fn render_proforma_document(content: &str, path: &Path) -> Response {
    let html_path = path.with_extension("html");
    if html_path.exists() {
        if let Ok(html) = fs::read_to_string(&html_path) {
            return Html(html).into_response();
        }
    }

    let fname = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("proforma.json");
    let title = extract_json_string_field(content, "title").unwrap_or_else(|| fname.to_string());
    let entity = extract_json_string_field(content, "entity").unwrap_or_default();
    let date = extract_json_string_field(content, "date").unwrap_or_default();
    let sep = if !entity.is_empty() && !date.is_empty() { " — " } else { "" };

    Html(format!(
        r#"<!DOCTYPE html>
<html lang="en"><head><meta charset="UTF-8">
<title>{title}</title>
<style>body{{font-family:sans-serif;padding:32px 48px}}
.notice{{background:#f6f8fa;border:1px solid #e1e4e8;border-radius:6px;padding:16px;font-size:13px}}</style>
</head><body>
<h1>{title}</h1><div style="font-size:12px;color:#666;margin-bottom:24px">{entity}{sep}{date}</div>
<div class="notice">No rendered companion .html found alongside this proforma.<br>
Run tool-proforma to generate the companion HTML, then reload.</div>
</body></html>"#,
        title = esc_html(&title),
        entity = esc_html(&entity),
        sep = sep,
        date = esc_html(&date),
    )).into_response()
}

fn render_geojson_placeholder(content: &str, path: &Path) -> Response {
    let fname = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("data.geojson");
    let feature_count = content.matches("\"Feature\"").count();

    Html(format!(
        r#"<!DOCTYPE html>
<html lang="en"><head><meta charset="UTF-8"><title>{fname}</title>
<style>body{{font-family:sans-serif;padding:32px 48px}}</style>
</head><body>
<div style="font-size:11px;font-weight:700;text-transform:uppercase;color:#28a745;margin-bottom:12px">GeoJSON</div>
<h1>{fname}</h1>
<div style="font-size:12px;color:#666;margin-bottom:32px">{feature_count} feature{pl}</div>
<div style="background:#f6f8fa;border:1px solid #e1e4e8;border-radius:6px;padding:16px;font-size:13px">
Interactive map viewer (MapLibre GL) coming in Stage 7.</div>
</body></html>"#,
        fname = esc_html(fname),
        feature_count = feature_count,
        pl = if feature_count == 1 { "" } else { "s" },
    )).into_response()
}

// ---------------------------------------------------------------------------
// GET /workbench/pdf
// ---------------------------------------------------------------------------

fn inject_page_css(html: &str, q: &FileQuery) -> String {
    let size = match q.paper.as_deref() {
        Some("a4") => "a4",
        _ => "letter",
    };
    let pn_rule = if q.page_numbers {
        let prop = match q.pn_position.as_deref() {
            Some("bottom-right") => "@bottom-right",
            Some("top-right") => "@top-right",
            _ => "@bottom-center",
        };
        format!(
            "  {} {{ content: counter(page) \" / \" counter(pages); font-size: 9pt; color: #666; }}\n",
            prop
        )
    } else {
        String::new()
    };
    let css = format!(
        "<style>\n@page {{ size: {}; margin: 2cm 2cm 2.5cm 2cm;\n{}}}\n@page :first {{ margin-top: 3cm; }}\n</style>\n",
        size, pn_rule
    );
    if html.contains("</head>") {
        html.replacen("</head>", &format!("{}</head>", css), 1)
    } else {
        format!("{}{}", css, html)
    }
}

fn run_weasyprint(wp: PathBuf, html: String) -> Result<Vec<u8>, String> {
    let mut child = Command::new(&wp)
        .args(["-", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("spawning weasyprint at {:?}: {}", wp, e))?;

    if let Some(stdin) = child.stdin.as_mut() {
        stdin
            .write_all(html.as_bytes())
            .map_err(|e| format!("writing to weasyprint stdin: {}", e))?;
    }
    drop(child.stdin.take());

    let output = child
        .wait_with_output()
        .map_err(|e| format!("weasyprint wait_with_output: {}", e))?;

    if !output.status.success() {
        return Err(format!("weasyprint exited {}", output.status));
    }
    if output.stdout.is_empty() {
        return Err("weasyprint produced empty output".to_string());
    }
    Ok(output.stdout)
}

async fn get_pdf(State(state): State<AppState>, Query(q): Query<FileQuery>) -> Response {
    let wp_path = match &*state.weasyprint {
        Some(p) => p.clone(),
        None => return err(StatusCode::SERVICE_UNAVAILABLE, "WeasyPrint not configured"),
    };

    let (fs_path, _writable) = match resolve_path(&state.roots, &q.path) {
        Ok(v) => v,
        Err(e) => return err(StatusCode::BAD_REQUEST, e),
    };

    if !fs_path.exists() || !fs_path.is_file() {
        return err(StatusCode::NOT_FOUND, "file not found");
    }

    let content = match fs::read_to_string(&fs_path) {
        Ok(s) => s,
        Err(e) => return err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };

    let doc_type = detect_doc_type(&fs_path, Some(&content));

    let html = match doc_type {
        DocType::HtmlDoc => inject_page_css(&content, &q),
        DocType::Proforma => {
            let html_path = fs_path.with_extension("html");
            if html_path.exists() {
                match fs::read_to_string(&html_path) {
                    Ok(companion) => inject_page_css(&companion, &q),
                    Err(e) => return err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
                }
            } else {
                let title = extract_json_string_field(&content, "title")
                    .or_else(|| fs_path.file_stem().and_then(|s| s.to_str()).map(|s| s.to_string()))
                    .unwrap_or_else(|| "Proforma".to_string());
                let body = format!(
                    "<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"UTF-8\"><title>{}</title></head><body><h1>{}</h1><p>No companion .html found. Run tool-proforma and re-export.</p></body></html>",
                    esc_html(&title),
                    esc_html(&title),
                );
                inject_page_css(&body, &q)
            }
        }
        DocType::GeoJson => {
            let fname = fs_path.file_name().and_then(|n| n.to_str()).unwrap_or("data.geojson");
            let feature_count = content.matches("\"Feature\"").count();
            format!(
                "<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"UTF-8\"><title>{}</title><style>@page{{size:letter landscape}}</style></head><body><h1>{}</h1><p>{} feature{}</p></body></html>",
                esc_html(fname),
                esc_html(fname),
                feature_count,
                if feature_count == 1 { "" } else { "s" },
            )
        }
        DocType::Other => {
            return err(
                StatusCode::UNPROCESSABLE_ENTITY,
                "PDF export not supported for this file type",
            )
        }
    };

    let stem = fs_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("document")
        .to_string();

    match tokio::task::spawn_blocking(move || run_weasyprint(wp_path, html)).await {
        Ok(Ok(pdf_bytes)) => {
            let cd = format!("attachment; filename=\"{}.pdf\"", stem);
            let mut headers = HeaderMap::new();
            headers.insert("content-type", "application/pdf".parse().unwrap());
            headers.insert("content-disposition", cd.parse().unwrap());
            (StatusCode::OK, headers, Bytes::from(pdf_bytes)).into_response()
        }
        Ok(Err(e)) => err(StatusCode::INTERNAL_SERVER_ERROR, format!("PDF render failed: {}", e)),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, format!("PDF render task panicked: {}", e)),
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn esc_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn extract_json_string_field(json: &str, field: &str) -> Option<String> {
    let needle = format!("\"{}\"", field);
    let pos = json.find(&needle)?;
    let after = json[pos + needle.len()..].trim_start();
    let after = after.strip_prefix(':')?.trim_start();
    let after = after.strip_prefix('"')?;
    let end = after.find('"')?;
    Some(after[..end].to_string())
}

// ---------------------------------------------------------------------------
// Folder download as ZIP
// ---------------------------------------------------------------------------

// Translate /_api/<prefix>/... paths (from the file-browser data-api attributes) to a
// filesystem path via the configured workbench roots.  The /_api/ layer uses bare names
// like "command" and "clones" while the workbench roots use "_command" and "_clones", so
// we try both the bare form and the underscore-prefixed form.
fn resolve_download_path(roots: &[WorkbenchRoot], raw: &str) -> Result<PathBuf, String> {
    let stripped = raw
        .trim_start_matches('/')
        .trim_start_matches("_api/");
    if let Ok((p, _)) = resolve_path(roots, stripped) {
        return Ok(p);
    }
    let with_underscore = format!("_{}", stripped);
    resolve_path(roots, &with_underscore)
        .map(|(p, _)| p)
        .map_err(|_| format!("no matching root for download path: {}", raw))
}

async fn download_folder(
    State(state): State<AppState>,
    Query(q): Query<FileQuery>,
) -> Response {
    let fs_path = match resolve_download_path(&state.roots, &q.path) {
        Ok(p) => p,
        Err(e) => return (StatusCode::BAD_REQUEST, e).into_response(),
    };
    if !fs_path.is_dir() {
        return (StatusCode::BAD_REQUEST, "path is not a directory").into_response();
    }
    let folder_name = fs_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let zip_bytes = match tokio::task::spawn_blocking(move || build_zip(&fs_path)).await {
        Ok(Ok(b)) => b,
        Ok(Err(e)) => return (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };

    Response::builder()
        .header("Content-Type", "application/zip")
        .header(
            "Content-Disposition",
            format!("attachment; filename=\"{}.zip\"", folder_name),
        )
        .body(axum::body::Body::from(zip_bytes))
        .unwrap()
}

fn build_zip(root: &std::path::Path) -> Result<Vec<u8>, String> {
    let cursor = std::io::Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(cursor);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    add_dir_to_zip(&mut zip, root, root, &options)?;
    let cursor = zip.finish().map_err(|e| e.to_string())?;
    Ok(cursor.into_inner())
}

fn add_dir_to_zip(
    zip: &mut zip::ZipWriter<std::io::Cursor<Vec<u8>>>,
    root: &std::path::Path,
    dir: &std::path::Path,
    options: &zip::write::SimpleFileOptions,
) -> Result<(), String> {
    use std::io::Write;
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok())
        .collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let path = entry.path();
        let fname = entry.file_name();
        let fname_str = fname.to_string_lossy();
        if fname_str.starts_with('.') || fname_str == "target" {
            continue;
        }
        let rel = path.strip_prefix(root).map_err(|e| e.to_string())?;
        let rel_str = rel.to_string_lossy().replace('\\', "/");

        if path.is_dir() {
            zip.add_directory(format!("{}/", rel_str), *options)
                .map_err(|e| e.to_string())?;
            add_dir_to_zip(zip, root, &path, options)?;
        } else if path.is_file() {
            if path.metadata().map(|m| m.len()).unwrap_or(0) > 50 * 1024 * 1024 {
                continue;
            }
            zip.start_file(&rel_str, *options).map_err(|e| e.to_string())?;
            let data = std::fs::read(&path).map_err(|e| e.to_string())?;
            zip.write_all(&data).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
