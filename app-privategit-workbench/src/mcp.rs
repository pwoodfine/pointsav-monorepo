/// MCP bridge — JSON-RPC 2.0 over HTTP POST /mcp.
///
/// Implements the Model Context Protocol (MCP) server side for the three
/// workbench tools: read_selection, propose_edit, commit_edit.
///
/// SYS-ADR-07 guard bars structured-data extensions (.json, .geojson,
/// .bim.json, .schedule) from any AI-mediated editing path.
use axum::{extract::State, response::Json};
use moonshot_docengine::{Document, Span};
use serde_json::{json, Value};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::UNIX_EPOCH,
};

use crate::AppState;

// ---------------------------------------------------------------------------
// Pending edit store
// ---------------------------------------------------------------------------

pub struct PendingEdit {
    pub fs_path: PathBuf,
    pub start: usize,
    pub end: usize,
    pub new_text: String,
    pub mtime_secs: u64,
}

pub type PendingEdits = Arc<Mutex<std::collections::HashMap<String, PendingEdit>>>;

static PROPOSAL_COUNTER: AtomicU64 = AtomicU64::new(1);

fn new_proposal_id() -> String {
    let ts = std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let seq = PROPOSAL_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("prop-{}-{}", ts, seq)
}

// ---------------------------------------------------------------------------
// ADR-07 guard
// ---------------------------------------------------------------------------

fn adr07_blocked(path: &Path) -> Option<String> {
    let name = path.to_string_lossy().to_lowercase();
    if name.ends_with(".bim.json") {
        return Some("ADR-07: .bim.json structured data is not eligible for AI editing".into());
    }
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        match ext.to_lowercase().as_str() {
            "json" => return Some("ADR-07: .json structured data is not eligible for AI editing".into()),
            "geojson" => return Some("ADR-07: .geojson structured data is not eligible for AI editing".into()),
            "schedule" => return Some("ADR-07: .schedule structured data is not eligible for AI editing".into()),
            _ => {}
        }
    }
    None
}

// ---------------------------------------------------------------------------
// mtime helper
// ---------------------------------------------------------------------------

fn file_mtime(path: &Path) -> u64 {
    fs::metadata(path)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Atomic write helper (mirrors put_file pattern)
// ---------------------------------------------------------------------------

fn atomic_write(fs_path: &Path, content: &str) -> Result<u64, String> {
    let ext = fs_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("bin");
    let tmp_path = fs_path.with_extension(format!("{}.tmp", ext));
    let result: Result<(), std::io::Error> = (|| {
        let mut f = fs::File::create(&tmp_path)?;
        f.write_all(content.as_bytes())?;
        f.sync_all()?;
        fs::rename(&tmp_path, fs_path)?;
        Ok(())
    })();
    match result {
        Ok(()) => Ok(file_mtime(fs_path)),
        Err(e) => {
            let _ = fs::remove_file(&tmp_path);
            Err(e.to_string())
        }
    }
}

// ---------------------------------------------------------------------------
// JSON-RPC helpers
// ---------------------------------------------------------------------------

fn rpc_ok(id: &Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn rpc_err(id: &Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

// ---------------------------------------------------------------------------
// Tool: read_selection
// ---------------------------------------------------------------------------

fn tool_read_selection(
    state: &AppState,
    args: &Value,
) -> Result<Value, (i64, String)> {
    let file_url = args["file"]
        .as_str()
        .ok_or_else(|| (-32602i64, "missing 'file' argument".to_string()))?;
    let start_byte = args["start_byte"]
        .as_u64()
        .ok_or_else(|| (-32602i64, "missing 'start_byte' argument".to_string()))? as usize;
    let end_byte = args["end_byte"]
        .as_u64()
        .ok_or_else(|| (-32602i64, "missing 'end_byte' argument".to_string()))? as usize;

    let (fs_path, _writable) = crate::resolve_path(&state.roots, file_url)
        .map_err(|e| (-32602i64, format!("path error: {}", e)))?;

    if let Some(msg) = adr07_blocked(&fs_path) {
        return Err((-32600, msg));
    }

    let src = fs::read_to_string(&fs_path)
        .map_err(|e| (-32603i64, format!("read error: {}", e)))?;

    let doc = Document::parse(&src);
    let sel = Span::new(start_byte.min(src.len()), end_byte.min(src.len()));
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

    Ok(json!({
        "file": file_url,
        "start": snapped.start,
        "end": snapped.end,
        "content": content,
        "block_kind": block_kind,
    }))
}

// ---------------------------------------------------------------------------
// Tool: propose_edit
// ---------------------------------------------------------------------------

fn tool_propose_edit(
    state: &AppState,
    pending: &PendingEdits,
    args: &Value,
) -> Result<Value, (i64, String)> {
    let file_url = args["file"]
        .as_str()
        .ok_or_else(|| (-32602i64, "missing 'file' argument".to_string()))?;
    let start_byte = args["start_byte"]
        .as_u64()
        .ok_or_else(|| (-32602i64, "missing 'start_byte' argument".to_string()))? as usize;
    let end_byte = args["end_byte"]
        .as_u64()
        .ok_or_else(|| (-32602i64, "missing 'end_byte' argument".to_string()))? as usize;
    let new_text = args["new_text"]
        .as_str()
        .ok_or_else(|| (-32602i64, "missing 'new_text' argument".to_string()))?
        .to_string();

    let (fs_path, writable) = crate::resolve_path(&state.roots, file_url)
        .map_err(|e| (-32602i64, format!("path error: {}", e)))?;

    if !writable {
        return Err((-32600, "file is in a read-only root".to_string()));
    }

    if let Some(msg) = adr07_blocked(&fs_path) {
        return Err((-32600, msg));
    }

    let mtime_secs = file_mtime(&fs_path);
    let proposal_id = new_proposal_id();

    pending
        .lock()
        .unwrap()
        .insert(
            proposal_id.clone(),
            PendingEdit {
                fs_path,
                start: start_byte,
                end: end_byte,
                new_text,
                mtime_secs,
            },
        );

    Ok(json!({ "proposal_id": proposal_id }))
}

// ---------------------------------------------------------------------------
// Tool: commit_edit
// ---------------------------------------------------------------------------

fn tool_commit_edit(
    state: &AppState,
    pending: &PendingEdits,
    args: &Value,
) -> Result<Value, (i64, String)> {
    let proposal_id = args["proposal_id"]
        .as_str()
        .ok_or_else(|| (-32602i64, "missing 'proposal_id' argument".to_string()))?;

    let edit = {
        let mut map = pending.lock().unwrap();
        map.remove(proposal_id)
            .ok_or_else(|| (-32602i64, format!("unknown proposal_id: {}", proposal_id)))?
    };

    let current_mtime = file_mtime(&edit.fs_path);
    if current_mtime != edit.mtime_secs {
        return Err((-32600, "conflict: file was modified since the proposal was made".to_string()));
    }

    let src = fs::read_to_string(&edit.fs_path)
        .map_err(|e| (-32603i64, format!("read error: {}", e)))?;

    let end = edit.end.min(src.len());
    let start = edit.start.min(end);
    let new_content = format!("{}{}{}", &src[..start], edit.new_text, &src[end..]);

    let new_mtime = atomic_write(&edit.fs_path, &new_content)
        .map_err(|e| (-32603i64, format!("write error: {}", e)))?;

    // Broadcast SSE change event
    let rel_path = edit
        .fs_path
        .to_string_lossy()
        .to_string();
    let _ = state.events_tx.send(format!(
        r#"{{"event":"changed","path":"{}","mtime":{}}}"#,
        rel_path, new_mtime
    ));

    Ok(json!({ "ok": true, "new_mtime": new_mtime }))
}

// ---------------------------------------------------------------------------
// Tool list (for tools/list)
// ---------------------------------------------------------------------------

fn tools_list() -> Value {
    json!({ "tools": [
        {
            "name": "read_selection",
            "description": "Read a byte range from a text file, snapped to AST block boundaries. Blocked for structured-data files (ADR-07).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "URL-style path within a configured workbench root" },
                    "start_byte": { "type": "integer", "description": "Start byte offset (inclusive)" },
                    "end_byte": { "type": "integer", "description": "End byte offset (exclusive)" }
                },
                "required": ["file", "start_byte", "end_byte"]
            }
        },
        {
            "name": "propose_edit",
            "description": "Stage an edit to a text file for human review via commit_edit. Blocked for structured-data files (ADR-07).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "URL-style path within a configured workbench root" },
                    "start_byte": { "type": "integer", "description": "Start byte offset (inclusive)" },
                    "end_byte": { "type": "integer", "description": "End byte offset (exclusive)" },
                    "new_text": { "type": "string", "description": "Replacement text" }
                },
                "required": ["file", "start_byte", "end_byte", "new_text"]
            }
        },
        {
            "name": "commit_edit",
            "description": "Apply a staged edit. Fails with conflict error if the file has been modified since propose_edit.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "proposal_id": { "type": "string", "description": "ID returned by propose_edit" }
                },
                "required": ["proposal_id"]
            }
        }
    ]})
}

// ---------------------------------------------------------------------------
// MCP HTTP handler
// ---------------------------------------------------------------------------

pub async fn mcp_handler(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let id = body.get("id").cloned().unwrap_or(Value::Null);
    let method = body["method"].as_str().unwrap_or("");

    let response = match method {
        "initialize" => rpc_ok(&id, json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "workbench", "version": "0.0.1" }
        })),
        "initialized" => {
            return Json(Value::Null);
        }
        "tools/list" => rpc_ok(&id, tools_list()),
        "tools/call" => {
            let name = body["params"]["name"].as_str().unwrap_or("");
            let args = &body["params"]["arguments"];
            let result = match name {
                "read_selection" => {
                    tool_read_selection(&state, args)
                        .map(|v| json!({ "content": [{ "type": "text", "text": v.to_string() }] }))
                }
                "propose_edit" => {
                    tool_propose_edit(&state, &state.pending_edits, args)
                        .map(|v| json!({ "content": [{ "type": "text", "text": v.to_string() }] }))
                }
                "commit_edit" => {
                    tool_commit_edit(&state, &state.pending_edits, args)
                        .map(|v| json!({ "content": [{ "type": "text", "text": v.to_string() }] }))
                }
                other => Err((-32601i64, format!("unknown tool: {}", other))),
            };
            match result {
                Ok(v) => rpc_ok(&id, v),
                Err((code, msg)) => rpc_err(&id, code, &msg),
            }
        }
        other => rpc_err(&id, -32601, &format!("method not found: {}", other)),
    };

    Json(response)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn adr07_blocks_json() {
        assert!(adr07_blocked(Path::new("proforma.json")).is_some());
    }

    #[test]
    fn adr07_blocks_geojson() {
        assert!(adr07_blocked(Path::new("parcels.geojson")).is_some());
    }

    #[test]
    fn adr07_blocks_bim_json() {
        assert!(adr07_blocked(Path::new("model.bim.json")).is_some());
    }

    #[test]
    fn adr07_blocks_schedule() {
        assert!(adr07_blocked(Path::new("project.schedule")).is_some());
    }

    #[test]
    fn adr07_allows_markdown() {
        assert!(adr07_blocked(Path::new("readme.md")).is_none());
    }

    #[test]
    fn adr07_allows_rust() {
        assert!(adr07_blocked(Path::new("lib.rs")).is_none());
    }

    #[test]
    fn proposal_id_is_unique() {
        let a = new_proposal_id();
        let b = new_proposal_id();
        assert_ne!(a, b);
    }

    #[test]
    fn atomic_write_round_trips() {
        let dir = std::env::temp_dir().join("workbench-mcp-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test-roundtrip.md");
        let content = "# Hello\n\nWorld.\n";
        atomic_write(&path, content).unwrap();
        let read_back = fs::read_to_string(&path).unwrap();
        assert_eq!(read_back, content);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn pending_edit_store_insert_remove() {
        let store: PendingEdits = Arc::new(Mutex::new(std::collections::HashMap::new()));
        let id = new_proposal_id();
        store.lock().unwrap().insert(
            id.clone(),
            PendingEdit {
                fs_path: PathBuf::from("/tmp/x.md"),
                start: 0,
                end: 5,
                new_text: "Hello".into(),
                mtime_secs: 12345,
            },
        );
        assert!(store.lock().unwrap().contains_key(&id));
        let removed = store.lock().unwrap().remove(&id);
        assert!(removed.is_some());
        assert!(store.lock().unwrap().is_empty());
    }
}
