use anyhow::{anyhow, Context, Result};
use axum::{
    body::Bytes,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::sse::{Event, KeepAlive, Sse},
    response::{Html, IntoResponse, Json, Redirect, Response},
    routing::{get, post},
    Router,
};
use notify::{recommended_watcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    convert::Infallible,
    fs,
    io::{Cursor, Write},
    net::SocketAddr,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{mpsc, Arc, Mutex},
    thread,
    time::{Duration, UNIX_EPOCH},
};
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tokio_stream::{wrappers::BroadcastStream, StreamExt as _};
use walkdir::WalkDir;

mod mcp;
mod schema_bim;
mod schema_files;
mod schema_gis;
mod schema_presentation;
mod schema_proforma;
mod schema_schedule;

const SPA_HTML: &str = include_str!("assets/index.html");

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

#[derive(Deserialize, Debug, Clone)]
struct RootEntry {
    url_prefix: String,
    fs_path: String,
    #[serde(default)]
    writable: bool,
}

#[derive(Deserialize, Debug)]
struct Config {
    bind: String,
    #[serde(default = "default_max_bytes")]
    max_bytes: usize,
    #[serde(default = "default_module_id")]
    module_id: String,
    log_dir: Option<String>,
    #[serde(rename = "root")]
    roots: Vec<RootEntry>,
}

fn default_max_bytes() -> usize {
    2 * 1024 * 1024
}

fn default_module_id() -> String {
    "workbench".to_string()
}

impl Config {
    fn load(path: &str) -> Result<Self> {
        let s = fs::read_to_string(path).context("reading config.toml")?;
        toml::from_str(&s).context("parsing config.toml")
    }
}

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct AppState {
    roots: Arc<Vec<RootEntry>>,
    max_bytes: usize,
    spa_html: Arc<String>,
    events_tx: broadcast::Sender<String>,
    log_dir: Option<PathBuf>,
    pending_edits: mcp::PendingEdits,
}

// ---------------------------------------------------------------------------
// Path resolution helper
// ---------------------------------------------------------------------------

/// Resolve a URL-style path (e.g. "_sandbox-jennifer/foo/bar.txt") to a
/// canonical fs path, checking that it stays within the declared root.
/// Returns (fs_path, writable).
fn resolve_path(roots: &[RootEntry], url_path: &str) -> Result<(PathBuf, bool)> {
    resolve_path_with_root(roots, url_path).map(|(p, w, _)| (p, w))
}

/// Like `resolve_path` but also returns the canonicalised root fs path.
/// Returns (fs_path, writable, root_canonical).
fn resolve_path_with_root(roots: &[RootEntry], url_path: &str) -> Result<(PathBuf, bool, PathBuf)> {
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

        // Canonicalize to resolve symlinks and check containment.
        // The target may not exist yet for write; canonicalize the parent instead.
        let canonical = if target.exists() {
            target.canonicalize().context("canonicalize target")?
        } else {
            let parent = target.parent().ok_or_else(|| anyhow!("no parent"))?;
            let cp = parent.canonicalize().context("canonicalize parent")?;
            cp.join(target.file_name().ok_or_else(|| anyhow!("no filename"))?)
        };

        let root_canonical = base.canonicalize().context("canonicalize root")?;
        if !canonical.starts_with(&root_canonical) {
            return Err(anyhow!("path traversal attempt"));
        }

        return Ok((canonical, root.writable, root_canonical));
    }

    Err(anyhow!("no matching root for path: {}", url_path))
}

// ---------------------------------------------------------------------------
// Allowed write extensions
// ---------------------------------------------------------------------------

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

/// Join the parent of a url_path with a new filename.
/// e.g. ("_clones/foo/bar/baz.md", "qux.md") -> "_clones/foo/bar/qux.md"
/// e.g. ("baz.md", "qux.md") -> "qux.md"
fn join_parent_url(url_path: &str, new_name: &str) -> String {
    let trimmed = url_path.trim_start_matches('/');
    match trimmed.rsplit_once('/') {
        Some((parent, _)) => format!("{}/{}", parent, new_name),
        None => new_name.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

async fn get_spa(State(state): State<AppState>) -> Html<String> {
    Html((*state.spa_html).clone())
}

#[derive(Deserialize)]
struct FileQuery {
    path: String,
    #[serde(default = "default_true")]
    page_numbers: bool,
    #[serde(default)]
    pn_position: Option<String>, // "bottom-center" | "bottom-right" | "top-right"
    #[serde(default)]
    paper: Option<String>, // "letter" | "a4"
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

#[derive(Serialize)]
struct DirEntry {
    name: String,
    is_dir: bool,
    mtime: u64,
    size: u64,
}

#[derive(Serialize)]
struct DirResponse {
    entries: Vec<DirEntry>,
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

fn err(status: StatusCode, msg: impl Into<String>) -> Response {
    (status, Json(ErrorBody { error: msg.into() })).into_response()
}

/// Append one JSONL record to the daily activity log (fire-and-forget; errors are silent).
fn log_activity(log_dir: &Option<PathBuf>, action: &str, path: &str, meta: serde_json::Value) {
    let Some(dir) = log_dir else { return };
    let now = std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let ts_ms = now.as_millis() as u64;
    let days = now.as_secs() / 86400;
    let filename = format!("activity-{days}.jsonl");
    let record = serde_json::json!({"ts": ts_ms, "action": action, "path": path, "meta": meta});
    let mut line = record.to_string();
    line.push('\n');
    let fpath = dir.join(filename);
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&fpath)
        .and_then(|mut f| {
            use std::io::Write;
            f.write_all(line.as_bytes())
        });
}

/// GET /file?path=<url_path>
/// - Empty path  → {entries: [{name, is_dir:true}]} listing all configured roots
/// - Directory   → {entries: [{name, is_dir}]} listing directory contents
/// - File        → {content, mtime, writable}
async fn get_file(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<FileQuery>,
) -> Response {
    // Empty path — return all configured roots as top-level directory entries.
    if q.path.trim_matches('/').is_empty() {
        let mut entries: Vec<DirEntry> = state
            .roots
            .iter()
            .map(|r| DirEntry {
                name: r.url_prefix.trim_end_matches('/').to_string(),
                is_dir: true,
                mtime: 0,
                size: 0,
            })
            .collect();
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        return Json(DirResponse { entries }).into_response();
    }

    if headers.get("x-wb-source").and_then(|v| v.to_str().ok()) == Some("user") {
        let ext = q.path.rsplit('.').next().unwrap_or("").to_string();
        log_activity(
            &state.log_dir,
            "open",
            &q.path,
            serde_json::json!({"ext": ext}),
        );
    }
    let (fs_path, writable) = match resolve_path(&state.roots, &q.path) {
        Ok(v) => v,
        Err(e) => return err(StatusCode::BAD_REQUEST, e.to_string()),
    };

    if !fs_path.exists() {
        return err(StatusCode::NOT_FOUND, "file not found");
    }

    // Directory — list contents.
    if fs_path.is_dir() {
        let rd = match fs::read_dir(&fs_path) {
            Ok(r) => r,
            Err(e) => return err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        };
        let mut entries: Vec<DirEntry> = rd
            .filter_map(|de| {
                let de = de.ok()?;
                let name = de.file_name().to_string_lossy().to_string();
                if name.starts_with('.') {
                    return None;
                }
                let meta = de.metadata().ok()?;
                let is_dir = meta.is_dir();
                let mtime = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                let size = if is_dir { 0 } else { meta.len() };
                Some(DirEntry { name, is_dir, mtime, size })
            })
            .collect();
        entries.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.cmp(&b.name)));
        return Json(DirResponse { entries }).into_response();
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

/// PUT /file?path=<url_path>
/// Body: raw UTF-8 text. Header X-Foundry-Editor: 1 required.
/// Optional header X-Foundry-Mtime: <u64> — if provided and mtime differs, returns 409.
async fn put_file(
    State(state): State<AppState>,
    Query(q): Query<FileQuery>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    // CSRF guard
    if headers
        .get("x-foundry-editor")
        .and_then(|v| v.to_str().ok())
        != Some("1")
    {
        return err(StatusCode::FORBIDDEN, "missing X-Foundry-Editor header");
    }

    let (fs_path, writable) = match resolve_path(&state.roots, &q.path) {
        Ok(v) => v,
        Err(e) => return err(StatusCode::BAD_REQUEST, e.to_string()),
    };

    if !writable {
        return err(StatusCode::FORBIDDEN, "path is not writable");
    }

    if !allowed_write_ext(&fs_path) {
        return err(
            StatusCode::FORBIDDEN,
            "file extension not allowed for writes",
        );
    }

    if body.len() > state.max_bytes {
        return err(
            StatusCode::PAYLOAD_TOO_LARGE,
            "file exceeds max_bytes limit",
        );
    }

    // mtime conflict check
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
                        return err(
                            StatusCode::CONFLICT,
                            "file modified on disk since last read",
                        );
                    }
                }
            }
        }
    }

    let content = match std::str::from_utf8(&body) {
        Ok(s) => s,
        Err(_) => return err(StatusCode::BAD_REQUEST, "body is not valid UTF-8"),
    };

    // Atomic write: write to .tmp, then rename
    let tmp_path = fs_path.with_extension(format!(
        "{}.tmp",
        fs_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("bin")
    ));

    let write_result: Result<()> = (|| {
        let mut f = fs::File::create(&tmp_path)?;
        f.write_all(content.as_bytes())?;
        f.sync_all()?;
        fs::rename(&tmp_path, &fs_path)?;
        Ok(())
    })();

    if let Err(e) = write_result {
        let _ = fs::remove_file(&tmp_path);
        return err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string());
    }

    // Return new mtime
    let new_mtime = fs::metadata(&fs_path)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);

    // Notify other open tabs that this file changed.
    let event_json = format!(
        r#"{{"event":"changed","path":"{}","mtime":{}}}"#,
        q.path, new_mtime
    );
    let _ = state.events_tx.send(event_json);

    log_activity(
        &state.log_dir,
        "save",
        &q.path,
        serde_json::json!({"bytes": body.len()}),
    );

    let mut resp_map = HashMap::new();
    resp_map.insert("ok", serde_json::Value::Bool(true));
    resp_map.insert("mtime", serde_json::Value::Number(new_mtime.into()));
    Json(resp_map).into_response()
}

// ---------------------------------------------------------------------------
// GET /events — SSE stream for real-time file change notifications
// ---------------------------------------------------------------------------

async fn get_events(State(state): State<AppState>) -> impl IntoResponse {
    let rx = state.events_tx.subscribe();
    let stream = BroadcastStream::new(rx)
        .filter_map(|r| r.ok())
        .map(|data| Ok::<_, Infallible>(Event::default().data(data)));
    Sse::new(stream).keep_alive(KeepAlive::default())
}

// ---------------------------------------------------------------------------
// Rename
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

/// POST /rename?from=<url_path>&to=<new_filename>
/// Renames the file within its current directory. `to` must be a bare
/// filename (no slashes). Source must resolve to a writable root.
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
        Err(e) => return err(StatusCode::BAD_REQUEST, e.to_string()),
    };

    if !fs_path.exists() {
        return err(StatusCode::NOT_FOUND, "source file not found");
    }
    if !writable {
        return err(StatusCode::FORBIDDEN, "path is not writable");
    }

    // Same-name check (before any disk activity)
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
    log_activity(
        &state.log_dir,
        "rename",
        &q.from,
        serde_json::json!({"new_name": new_name}),
    );
    Json(RenameResponse {
        ok: true,
        new_path: new_url_path,
        new_name: new_name.to_string(),
    })
    .into_response()
}

// ---------------------------------------------------------------------------
// Move
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct MoveResponse {
    ok: bool,
    new_path: String,
}

/// POST /move?from=<file_url_path>&to=<dest_dir_url_path>
/// Moves a file into a different directory. `to` must resolve to an existing
/// directory. Both source and destination must be in writable roots.
async fn move_file(State(state): State<AppState>, Query(q): Query<RenameQuery>) -> Response {
    let (src_fs, src_writable) = match resolve_path(&state.roots, &q.from) {
        Ok(v) => v,
        Err(e) => return err(StatusCode::BAD_REQUEST, e.to_string()),
    };
    if !src_fs.exists() {
        return err(StatusCode::NOT_FOUND, "source file not found");
    }
    if !src_fs.is_file() {
        return err(StatusCode::BAD_REQUEST, "source is not a file");
    }
    if !src_writable {
        return err(StatusCode::FORBIDDEN, "source path is not writable");
    }

    let (dst_fs, dst_writable) = match resolve_path(&state.roots, &q.to) {
        Ok(v) => v,
        Err(e) => return err(StatusCode::BAD_REQUEST, e.to_string()),
    };
    if !dst_writable {
        return err(StatusCode::FORBIDDEN, "destination is not writable");
    }
    if !dst_fs.is_dir() {
        return err(StatusCode::BAD_REQUEST, "destination is not a directory");
    }

    let filename = match src_fs.file_name() {
        Some(n) => n.to_owned(),
        None => return err(StatusCode::BAD_REQUEST, "cannot determine filename"),
    };
    let final_path = dst_fs.join(&filename);

    // No-op: already in the right place
    if src_fs == final_path {
        return Json(MoveResponse {
            ok: true,
            new_path: q.from.clone(),
        })
        .into_response();
    }
    if final_path.exists() {
        return err(
            StatusCode::CONFLICT,
            "a file with that name already exists in destination",
        );
    }

    // Atomic rename; fall back to copy+delete for cross-filesystem moves
    if fs::rename(&src_fs, &final_path).is_err() {
        if let Err(e) = fs::copy(&src_fs, &final_path) {
            return err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string());
        }
        let _ = fs::remove_file(&src_fs);
    }

    let dest_prefix = q.to.trim_start_matches('/').trim_end_matches('/');
    let new_path = format!("{}/{}", dest_prefix, filename.to_str().unwrap_or(""));
    log_activity(
        &state.log_dir,
        "move",
        &q.from,
        serde_json::json!({"to": &q.to}),
    );
    Json(MoveResponse { ok: true, new_path }).into_response()
}

// ---------------------------------------------------------------------------
// Duplicate
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct DuplicateResponse {
    ok: bool,
    new_path: String,
    new_name: String,
}

/// Insert "-copy" (or "-copy-N") before the extension. Returns a filename
/// that does not currently exist in `parent`. Tries N=2..=99.
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

    // First try "-copy"
    let first = make("-copy");
    if !parent.join(&first).exists() {
        return Some(first);
    }

    // Then "-copy-2".."-copy-99"
    for n in 2..=99 {
        let candidate = make(&format!("-copy-{}", n));
        if !parent.join(&candidate).exists() {
            return Some(candidate);
        }
    }

    None
}

/// POST /duplicate?path=<url_path>
/// Copies the file in place with "-copy" inserted before the extension.
/// Source must resolve to a writable root.
async fn duplicate_file(State(state): State<AppState>, Query(q): Query<FileQuery>) -> Response {
    let (fs_path, writable) = match resolve_path(&state.roots, &q.path) {
        Ok(v) => v,
        Err(e) => return err(StatusCode::BAD_REQUEST, e.to_string()),
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
        None => {
            return err(
                StatusCode::CONFLICT,
                "could not find an available copy name (tried -copy through -copy-99)",
            )
        }
    };
    let new_fs_path = parent.join(&new_name);

    if let Err(e) = fs::copy(&fs_path, &new_fs_path) {
        return err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string());
    }

    let new_url_path = join_parent_url(&q.path, &new_name);
    log_activity(
        &state.log_dir,
        "duplicate",
        &q.path,
        serde_json::json!({"new_name": &new_name}),
    );
    Json(DuplicateResponse {
        ok: true,
        new_path: new_url_path,
        new_name,
    })
    .into_response()
}

// ---------------------------------------------------------------------------
// Create file
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct CreateResponse {
    ok: bool,
    path: String,
    name: String,
}

/// POST /create?path=<url_path>
/// Creates an empty file at the given path. Path must resolve to a writable
/// root and the extension must be allowed for writes. 409 if file exists.
async fn create_file(State(state): State<AppState>, Query(q): Query<FileQuery>) -> Response {
    let (fs_path, writable) = match resolve_path(&state.roots, &q.path) {
        Ok(v) => v,
        Err(e) => return err(StatusCode::BAD_REQUEST, e.to_string()),
    };

    if !writable {
        return err(StatusCode::FORBIDDEN, "path is not writable");
    }

    if !allowed_write_ext(&fs_path) {
        return err(
            StatusCode::FORBIDDEN,
            "file extension not allowed for writes",
        );
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

    log_activity(&state.log_dir, "create", &q.path, serde_json::json!({}));
    Json(CreateResponse {
        ok: true,
        path: q.path.trim_start_matches('/').to_string(),
        name,
    })
    .into_response()
}

// ---------------------------------------------------------------------------
// Delete file
// ---------------------------------------------------------------------------

/// POST /delete?path=<url_path>
/// Deletes the file at the given path. Path must resolve to a writable root.
/// 400 if it resolves to a directory.
async fn delete_file(State(state): State<AppState>, Query(q): Query<FileQuery>) -> Response {
    let (fs_path, writable) = match resolve_path(&state.roots, &q.path) {
        Ok(v) => v,
        Err(e) => return err(StatusCode::BAD_REQUEST, e.to_string()),
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

    log_activity(&state.log_dir, "delete", &q.path, serde_json::json!({}));
    let mut resp_map = HashMap::new();
    resp_map.insert("ok", serde_json::Value::Bool(true));
    Json(resp_map).into_response()
}

// ---------------------------------------------------------------------------
// Trash (soft-delete) + Restore
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct TrashResponse {
    ok: bool,
    trash_key: String,
    original_path: String,
}

/// POST /trash?path=<url_path>
/// Moves the item into <root>/.wb-trash/<timestamp_ms>_<leaf> instead of
/// permanently deleting it. Returns {ok, trash_key, original_path} so the
/// frontend can offer an Undo button that calls /restore.
async fn trash_file(State(state): State<AppState>, Query(q): Query<FileQuery>) -> Response {
    let (fs_path, writable, root_canonical) = match resolve_path_with_root(&state.roots, &q.path) {
        Ok(v) => v,
        Err(e) => return err(StatusCode::BAD_REQUEST, e.to_string()),
    };

    if !writable {
        return err(StatusCode::FORBIDDEN, "path is not writable");
    }

    if !fs_path.exists() {
        return err(StatusCode::NOT_FOUND, "file not found");
    }

    let trash_dir = root_canonical.join(".wb-trash");
    if let Err(e) = fs::create_dir_all(&trash_dir) {
        return err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string());
    }

    let ts = std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();

    let leaf = fs_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("deleted");
    let trash_key = format!("{ts}_{leaf}");
    let trash_path = trash_dir.join(&trash_key);

    if let Err(e) = fs::rename(&fs_path, &trash_path) {
        return err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string());
    }

    let original_path = q.path.trim_start_matches('/').to_string();
    log_activity(
        &state.log_dir,
        "trash",
        &original_path,
        serde_json::json!({"key": &trash_key}),
    );
    Json(TrashResponse {
        ok: true,
        trash_key,
        original_path,
    })
    .into_response()
}

#[derive(Deserialize)]
struct RestoreQuery {
    trash_key: String,
    original_path: String,
}

/// POST /restore?trash_key=<key>&original_path=<url_path>
/// Moves an item from <root>/.wb-trash/<key> back to its original location.
async fn restore_file(State(state): State<AppState>, Query(q): Query<RestoreQuery>) -> Response {
    let (fs_dest, writable, root_canonical) =
        match resolve_path_with_root(&state.roots, &q.original_path) {
            Ok(v) => v,
            Err(e) => return err(StatusCode::BAD_REQUEST, e.to_string()),
        };

    if !writable {
        return err(StatusCode::FORBIDDEN, "path is not writable");
    }

    if q.trash_key.contains('/') || q.trash_key.contains("..") || q.trash_key.is_empty() {
        return err(StatusCode::BAD_REQUEST, "invalid trash key");
    }

    let trash_path = root_canonical.join(".wb-trash").join(&q.trash_key);

    if !trash_path.exists() {
        return err(StatusCode::NOT_FOUND, "trash entry not found");
    }

    if fs_dest.exists() {
        return err(StatusCode::CONFLICT, "destination already exists");
    }

    if let Err(e) = fs::rename(&trash_path, &fs_dest) {
        return err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string());
    }

    log_activity(
        &state.log_dir,
        "restore",
        &q.original_path,
        serde_json::json!({}),
    );
    let mut resp_map = HashMap::new();
    resp_map.insert("ok", serde_json::Value::Bool(true));
    Json(resp_map).into_response()
}

// ---------------------------------------------------------------------------
// Activity log — client-side events
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct ActivityBody {
    action: String,
    path: Option<String>,
    meta: Option<serde_json::Value>,
}

/// POST /activity
/// Receives client-side-only events (search, tab switch, keyboard shortcuts, etc.)
/// and appends them to the activity log.
async fn post_activity(State(state): State<AppState>, Json(body): Json<ActivityBody>) -> Response {
    log_activity(
        &state.log_dir,
        &body.action,
        body.path.as_deref().unwrap_or(""),
        body.meta
            .unwrap_or(serde_json::Value::Object(Default::default())),
    );
    Json(serde_json::json!({"ok": true})).into_response()
}

// ---------------------------------------------------------------------------
// Git status
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct GitStatusQuery {
    root: String,
}

/// GET /git-status?root=<url_prefix>
/// Runs `git status --porcelain --untracked-files=all` in the resolved
/// directory. Returns a map of relative-path → status. Empty map on any
/// failure (not a git repo, git absent, timeout, etc.).
async fn git_status(State(state): State<AppState>, Query(q): Query<GitStatusQuery>) -> Response {
    let mut empty: HashMap<&str, serde_json::Value> = HashMap::new();
    empty.insert("files", serde_json::json!({}));

    let (fs_path, _writable) = match resolve_path(&state.roots, &q.root) {
        Ok(v) => v,
        Err(_) => return Json(&empty).into_response(),
    };

    if !fs_path.is_dir() {
        return Json(&empty).into_response();
    }

    // Only report status if fs_path itself is a git work tree (has a .git
    // directory or file). Without this, `git status` walks upward and
    // surfaces noise from an enclosing repo (e.g. ~/Foundry).
    if !fs_path.join(".git").exists() {
        return Json(&empty).into_response();
    }

    // Run git in a worker thread with a 5-second timeout.
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
        // Handle rename "old -> new" — take the new name
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
                // Generic: any M in either column → modified; A → staged; D → deleted
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
// Document type detection
// ---------------------------------------------------------------------------

#[derive(Debug, PartialEq)]
enum DocType {
    HtmlDoc,  // .html / .htm — inject @page CSS then WeasyPrint
    GeoJson,  // .geojson
    Proforma, // .json with "proforma_version" key
    Other,    // fall through to existing code-view behavior
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
                // 6 KB is enough to find all discriminating fields in every known proforma variant
                let p = &peek[..peek.len().min(6144)];
                if p.contains("\"proforma_version\"") {
                    return DocType::Proforma;
                }
                // Bespoke tool-proforma output (no version field): entity + date + data section key
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

// ---------------------------------------------------------------------------
// Platform PDF rendering helpers
// ---------------------------------------------------------------------------

const WEASYPRINT_BIN: &str = "/usr/bin/weasyprint";

/// Inject a minimal @page CSS block before </head>.
/// Respects print config from the request query: paper size, page numbers, position.
/// The document content and all existing styles are untouched.
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
            "  {} {{\n    content: counter(page) \" / \" counter(pages);\n    font-size: 9pt;\n    color: #666;\n    font-family: \"Liberation Sans\", Arial, sans-serif;\n  }}\n",
            prop
        )
    } else {
        String::new()
    };
    let css = format!(
        "<style>\n@page {{\n  size: {};\n  margin: 2cm 2cm 2.5cm 2cm;\n{}}}\n@page :first {{ margin-top: 3cm; }}\n</style>\n",
        size, pn_rule
    );
    if html.contains("</head>") {
        html.replacen("</head>", &format!("{}</head>", css), 1)
    } else if html.contains("<body") {
        html.replacen("<body", &format!("<head>{}</head>\n<body", css), 1)
    } else {
        format!("{}{}", css, html)
    }
}

/// Spawn weasyprint and pipe HTML → PDF bytes.
/// Blocking — call via spawn_blocking from async context.
fn run_weasyprint(html: String) -> Result<Vec<u8>> {
    let mut child = Command::new(WEASYPRINT_BIN)
        .args(["-", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .context("spawning weasyprint — is it installed at /usr/bin/weasyprint?")?;

    if let Some(stdin) = child.stdin.as_mut() {
        stdin
            .write_all(html.as_bytes())
            .context("writing HTML to weasyprint stdin")?;
    }
    drop(child.stdin.take());

    let output = child
        .wait_with_output()
        .context("weasyprint wait_with_output")?;

    if !output.status.success() {
        return Err(anyhow!("weasyprint exited {}", output.status));
    }
    if output.stdout.is_empty() {
        return Err(anyhow!("weasyprint produced empty output"));
    }
    Ok(output.stdout)
}

// ---------------------------------------------------------------------------
// GET /document — renders a document as HTML for the iframe viewer
// ---------------------------------------------------------------------------

/// GET /document?path=<url_path>
/// Returns a standalone HTML page for known document types.
/// Unknown types redirect to the raw browse path.
async fn get_document(State(state): State<AppState>, Query(q): Query<FileQuery>) -> Response {
    let (fs_path, _writable) = match resolve_path(&state.roots, &q.path) {
        Ok(v) => v,
        Err(e) => return err(StatusCode::BAD_REQUEST, e.to_string()),
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
            // HTML files already render correctly via nginx static serving.
            // Redirect to the raw browse path so the iframe loads it directly.
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
    // Prefer companion .html produced by tool-proforma — correct by definition.
    let html_path = path.with_extension("html");
    if html_path.exists() {
        if let Ok(html) = fs::read_to_string(&html_path) {
            return Html(html).into_response();
        }
    }

    // Fallback: no companion HTML found
    let fname = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("proforma.json");
    let title = extract_json_string_field(content, "title").unwrap_or_else(|| fname.to_string());
    let entity = extract_json_string_field(content, "entity").unwrap_or_default();
    let date = extract_json_string_field(content, "date").unwrap_or_default();
    let sep = if !entity.is_empty() && !date.is_empty() {
        " — "
    } else {
        ""
    };

    let html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>{title}</title>
<style>
body {{ font-family: "Liberation Sans", Arial, sans-serif;
       margin: 0; padding: 32px 48px; background: #fff; color: #24292e; }}
h1 {{ font-size: 1.5em; margin: 0 0 4px; }}
.meta {{ font-size: 12px; color: #666; margin-bottom: 24px; }}
.notice {{ background: #f6f8fa; border: 1px solid #e1e4e8; border-radius: 6px;
           padding: 16px 20px; font-size: 13px; color: #444; }}
</style>
</head>
<body>
<h1>{title}</h1>
<div class="meta">{entity}{sep}{date}</div>
<div class="notice">
  No rendered companion .html found alongside this proforma.<br>
  Run tool-proforma to generate the companion HTML, then reload.
</div>
</body>
</html>"#,
        title = esc_html(&title),
        entity = esc_html(&entity),
        sep = sep,
        date = esc_html(&date),
    );

    Html(html).into_response()
}

fn render_geojson_placeholder(content: &str, path: &Path) -> Response {
    let fname = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("data.geojson");

    // Count features by counting occurrences of "\"type\":\"Feature\""
    let feature_count = content.matches("\"Feature\"").count();

    let html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<title>{fname}</title>
<style>
body {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
       margin: 0; padding: 32px 48px; background: #fff; color: #24292e; }}
.badge {{ display: inline-block; background: #28a74522; color: #28a745;
          border: 1px solid #28a74544; font-size: 11px; font-weight: 700;
          text-transform: uppercase; letter-spacing: .06em;
          padding: 2px 8px; border-radius: 3px; margin-bottom: 12px; }}
h1 {{ font-size: 1.4em; margin: 0 0 4px; }}
.meta {{ font-size: 12px; color: #666; margin-bottom: 32px; }}
.notice {{ background: #f6f8fa; border: 1px solid #e1e4e8; border-radius: 6px;
           padding: 16px 20px; font-size: 13px; color: #444; }}
</style>
</head>
<body>
<div class="badge">GeoJSON</div>
<h1>{fname}</h1>
<div class="meta">{feature_count} feature{pl}</div>
<div class="notice">
  Interactive map viewer (MapLibre GL) is coming in the next sprint.
</div>
</body>
</html>"#,
        fname = esc_html(fname),
        feature_count = feature_count,
        pl = if feature_count == 1 { "" } else { "s" },
    );

    Html(html).into_response()
}

/// Minimal HTML entity escaping for injecting values into HTML attributes/text.
fn esc_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Extract a top-level string field value from JSON without a full parse.
/// Returns None if not found. Only suitable for simple flat string fields.
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
// GET /pdf — platform PDF rendering via WeasyPrint subprocess
// ---------------------------------------------------------------------------

/// GET /pdf?path=<url_path>
/// Renders the file to PDF via WeasyPrint and returns it as a download.
/// For HTML files: injects @page CSS then renders.
/// For proforma JSON: renders the proforma viewer HTML then renders.
/// For other types: returns 422 Unprocessable Content.
async fn get_pdf(State(state): State<AppState>, Query(q): Query<FileQuery>) -> Response {
    let (fs_path, _writable) = match resolve_path(&state.roots, &q.path) {
        Ok(v) => v,
        Err(e) => return err(StatusCode::BAD_REQUEST, e.to_string()),
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
            // Prefer companion .html produced by tool-proforma; inject @page CSS for pagination.
            let html_path = fs_path.with_extension("html");
            if html_path.exists() {
                match fs::read_to_string(&html_path) {
                    Ok(companion_html) => inject_page_css(&companion_html, &q),
                    Err(e) => return err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
                }
            } else {
                // Fallback when no companion HTML exists
                let title = extract_json_string_field(&content, "title")
                    .or_else(|| {
                        fs_path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .map(|s| s.to_string())
                    })
                    .unwrap_or_else(|| "Proforma".to_string());
                let entity = extract_json_string_field(&content, "entity").unwrap_or_default();
                let date = extract_json_string_field(&content, "date").unwrap_or_default();
                let fname = fs_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("proforma.json");
                let sep = if !entity.is_empty() && !date.is_empty() {
                    " — "
                } else {
                    ""
                };

                let body = format!(
                    r#"<!DOCTYPE html>
<html lang="en">
<head><meta charset="UTF-8"><title>{title}</title>
<style>
body {{ font-family: "Liberation Sans", Arial, sans-serif;
        margin: 0; padding: 32px 48px; color: #24292e; }}
h1 {{ font-size: 1.5em; margin: 0 0 4px; }}
.meta {{ font-size: 12px; color: #666; margin-bottom: 32px; }}
p {{ font-size: 13px; color: #444; }}
</style>
</head>
<body>
<h1>{title}</h1>
<div class="meta">{entity}{sep}{date} — {fname}</div>
<p>Run tool-proforma to generate the companion .html, then re-export this PDF.</p>
</body>
</html>"#,
                    title = esc_html(&title),
                    entity = esc_html(&entity),
                    sep = sep,
                    date = esc_html(&date),
                    fname = esc_html(fname),
                );
                inject_page_css(&body, &q)
            }
        }
        DocType::GeoJson => {
            // PDF = feature properties as a data table (MapLibre is WebGL; WeasyPrint cannot render it)
            let feature_count = content.matches("\"Feature\"").count();
            let fname = fs_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("data.geojson");

            format!(
                r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<title>{fname}</title>
<style>
@page {{ size: letter landscape; margin: 2cm;
  @bottom-center {{ content: counter(page) " / " counter(pages);
    font-size: 9pt; color: #666; font-family: "Liberation Sans", Arial, sans-serif; }} }}
body {{ font-family: "Liberation Sans", Arial, sans-serif;
        margin: 0; padding: 24px; color: #24292e; }}
h1 {{ font-size: 1.3em; margin: 0 0 8px; }}
.meta {{ font-size: 12px; color: #666; margin-bottom: 24px; }}
pre {{ background: #f6f8fa; border: 1px solid #e1e4e8; border-radius: 6px;
       padding: 16px; font-size: 10px; white-space: pre-wrap; word-break: break-word; }}
</style>
</head>
<body>
<h1>{fname}</h1>
<div class="meta">{feature_count} feature{pl}</div>
<pre>{json_preview}</pre>
</body>
</html>"#,
                fname = esc_html(fname),
                feature_count = feature_count,
                pl = if feature_count == 1 { "" } else { "s" },
                json_preview = esc_html(&content[..content.len().min(8000)]),
            )
        }
        DocType::Other => {
            return err(
                StatusCode::UNPROCESSABLE_ENTITY,
                "PDF export is not supported for this file type",
            );
        }
    };

    let stem = fs_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("document")
        .to_string();

    match tokio::task::spawn_blocking(move || run_weasyprint(html)).await {
        Ok(Ok(pdf_bytes)) => {
            let cd = format!("attachment; filename=\"{}.pdf\"", stem);
            let mut headers = HeaderMap::new();
            headers.insert("content-type", "application/pdf".parse().unwrap());
            headers.insert("content-disposition", cd.parse().unwrap());
            (StatusCode::OK, headers, Bytes::from(pdf_bytes)).into_response()
        }
        Ok(Err(e)) => err(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("PDF render failed: {}", e),
        ),
        Err(e) => err(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("PDF render task panicked: {}", e),
        ),
    }
}

// ---------------------------------------------------------------------------
// GET /section — AST-aware block snap
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct SectionQuery {
    path: String,
    offset: u64,
}

/// GET /section?path=<url_path>&offset=<byte_offset>
/// Returns the block boundaries that contain `offset` in the given text file.
async fn get_section(State(state): State<AppState>, Query(q): Query<SectionQuery>) -> Response {
    use moonshot_docengine::{Document, Span};

    let (fs_path, _writable) = match resolve_path(&state.roots, &q.path) {
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

    let offset = (q.offset as usize).min(src.len());
    let doc = Document::parse(&src);
    let sel = Span::new(offset, offset);
    let snapped = doc.section_span(sel);
    let content = src
        .get(snapped.start..snapped.end)
        .unwrap_or("")
        .to_string();
    let block_kind = doc
        .block_at(snapped.start)
        .and_then(|i| doc.blocks().get(i))
        .map(|b| format!("{:?}", b.kind))
        .unwrap_or_else(|| "Unknown".to_string());

    Json(serde_json::json!({
        "start": snapped.start,
        "end": snapped.end,
        "content": content,
        "block_kind": block_kind,
    }))
    .into_response()
}

// ---------------------------------------------------------------------------
// GET /download — recursive ZIP download of a directory
// ---------------------------------------------------------------------------

/// GET /download?path=<url_path>
/// Zips the directory at the given path and returns it as a download.
/// Read-only paths are allowed (ZIP is a read operation).
async fn zip_download(State(state): State<AppState>, Query(q): Query<FileQuery>) -> Response {
    if q.path.trim_matches('/').is_empty() {
        return err(StatusCode::BAD_REQUEST, "path is required");
    }
    let (fs_path, _writable) = match resolve_path(&state.roots, &q.path) {
        Ok(v) => v,
        Err(e) => return err(StatusCode::BAD_REQUEST, e.to_string()),
    };
    if !fs_path.exists() {
        return err(StatusCode::NOT_FOUND, "path not found");
    }
    if !fs_path.is_dir() {
        return err(StatusCode::BAD_REQUEST, "path is not a directory");
    }

    let dir_name = fs_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("download")
        .to_string();

    let result = tokio::task::spawn_blocking(move || -> Result<Vec<u8>> {
        let buf = Cursor::new(Vec::new());
        let mut zip = zip::ZipWriter::new(buf);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        for entry in WalkDir::new(&fs_path).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            let rel = match path.strip_prefix(&fs_path) {
                Ok(r) => r,
                Err(_) => continue,
            };
            if rel == Path::new("") {
                continue;
            }
            // Skip hidden files/dirs
            if rel.components().any(|c| {
                c.as_os_str().to_str().unwrap_or("").starts_with('.')
            }) {
                continue;
            }

            let zip_path = rel.to_string_lossy().to_string();
            if path.is_dir() {
                zip.add_directory(format!("{}/", zip_path), options)?;
            } else {
                zip.start_file(&zip_path, options)?;
                let bytes = fs::read(path)?;
                use std::io::Write as _;
                zip.write_all(&bytes)?;
            }
        }

        let result = zip.finish()?;
        Ok(result.into_inner())
    })
    .await;

    match result {
        Ok(Ok(bytes)) => {
            let cd = format!("attachment; filename=\"{}.zip\"", dir_name);
            let mut headers = HeaderMap::new();
            headers.insert("content-type", "application/zip".parse().unwrap());
            headers.insert("content-disposition", cd.parse().unwrap());
            (StatusCode::OK, headers, Bytes::from(bytes)).into_response()
        }
        Ok(Err(e)) => err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        Err(e) => err(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("zip task panicked: {}", e),
        ),
    }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<()> {
    let config_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "config.toml".to_string());

    let config = Config::load(&config_path)?;

    // Inject module_id into SPA HTML as a bootstrap <meta> tag
    let spa_html = SPA_HTML.replacen(
        "<head>",
        &format!(
            "<head>\n<meta name=\"workbench-module-id\" content=\"{}\">",
            config.module_id
        ),
        1,
    );

    // SSE broadcast channel + inotify watcher.
    // Watches ALL configured roots so inotify fires for external writes
    // (e.g. Claude Code editing files in _clones/).
    let (events_tx, _) = broadcast::channel::<String>(64);
    let watcher_tx = events_tx.clone();
    let root_mappings: Vec<(String, String)> = config
        .roots
        .iter()
        .map(|r| (r.url_prefix.clone(), r.fs_path.clone()))
        .collect();
    let roots_for_watcher = root_mappings.clone();
    tokio::spawn(async move {
        let (inner_tx, mut inner_rx) = tokio::sync::mpsc::channel::<String>(8);
        let mut watcher = match recommended_watcher(move |res: notify::Result<notify::Event>| {
            if let Ok(event) = res {
                let path_str = event
                    .paths
                    .first()
                    .and_then(|p| p.to_str())
                    .unwrap_or("")
                    .to_string();
                let _ = inner_tx.blocking_send(path_str);
            }
        }) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("warning: file watcher init failed: {}", e);
                return;
            }
        };
        for (_, fs_path) in &roots_for_watcher {
            if let Err(e) = watcher.watch(std::path::Path::new(fs_path), RecursiveMode::Recursive) {
                eprintln!("warning: could not watch {:?}: {}", fs_path, e);
            }
        }
        while let Some(path_str) = inner_rx.recv().await {
            if path_str.is_empty() {
                continue;
            }
            let rel = roots_for_watcher
                .iter()
                .find_map(|(prefix, base)| {
                    let base = base.trim_end_matches('/');
                    if let Some(rest_stripped) = path_str.strip_prefix(base) {
                        let rest = rest_stripped.trim_start_matches('/');
                        Some(format!("{}/{}", prefix, rest))
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| path_str.clone());
            let mtime = std::fs::metadata(&path_str)
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let msg = format!(
                r#"{{"event":"changed","path":"{}","mtime":{}}}"#,
                rel, mtime
            );
            let _ = watcher_tx.send(msg);
        }
    });

    let log_dir: Option<PathBuf> = config.log_dir.as_deref().map(PathBuf::from);
    if let Some(ref d) = log_dir {
        let _ = fs::create_dir_all(d);
    }

    let state = AppState {
        roots: Arc::new(config.roots),
        max_bytes: config.max_bytes,
        spa_html: Arc::new(spa_html),
        events_tx,
        log_dir,
        pending_edits: Arc::new(Mutex::new(HashMap::new())),
    };

    let app = Router::new()
        .route("/", get(get_spa))
        .route("/file", get(get_file).put(put_file))
        .route("/rename", post(rename_file))
        .route("/move", post(move_file))
        .route("/duplicate", post(duplicate_file))
        .route("/create", post(create_file))
        .route("/delete", post(delete_file))
        .route("/trash", post(trash_file))
        .route("/restore", post(restore_file))
        .route("/activity", post(post_activity))
        .route("/git-status", get(git_status))
        .route("/document", get(get_document))
        .route("/pdf", get(get_pdf))
        .route("/events", get(get_events))
        .route("/mcp", post(mcp::mcp_handler))
        .route("/section", get(get_section))
        .route(
            "/api/presentation/files",
            get(schema_presentation::list_files),
        )
        .route("/api/presentation/render", get(schema_presentation::render))
        .route("/api/schedule/files", get(schema_schedule::list_files))
        .route(
            "/api/schedule/syntax-hints",
            get(schema_schedule::syntax_hints),
        )
        .route("/api/gis/files", get(schema_gis::list_files))
        .route("/api/gis/feature-count", get(schema_gis::feature_count))
        .route("/api/bim/files", get(schema_bim::list_files))
        .route("/api/bim/parse", get(schema_bim::parse_file))
        .route("/api/bim/instances", get(schema_bim::list_instances))
        .route("/api/bim/create", post(schema_bim::create))
        .route("/api/files", get(schema_files::list_files))
        .route("/api/files/create", post(schema_files::create))
        .route("/api/proforma/files", get(schema_proforma::list_files))
        .route("/api/proforma/create", post(schema_proforma::create))
        .route("/download", get(zip_download))
        .with_state(state);

    let addr: SocketAddr = config.bind.parse().context("parsing bind address")?;
    println!("app-privategit-workbench listening on {}", addr);

    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
