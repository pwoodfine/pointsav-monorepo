//! MCP (Model Context Protocol) server — native JSON-RPC 2.0, no vendor SDK.
//!
//! This is the agent-first authoring surface: AI authors discover the typed
//! section vocabulary, read pages, validate a manifest against the contract,
//! and *propose* a page. Proposals stage to the review queue — they are never
//! auto-committed (SYS-ADR-10); a human approves (F12) before anything
//! persists, and there is no automated publish path (SYS-ADR-19).
//!
//! Transport: `POST /api/mcp`. Method set mirrors `app-mediakit-knowledge`.

use std::sync::Arc;

use axum::{
    extract::State,
    http::{header, HeaderValue},
    response::IntoResponse,
    Json,
};
use serde_json::{json, Value};

use app_mediakit_shell::{section::section_catalog, Page};

use crate::content;
use crate::server::AppState;

pub async fn handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<Value>,
) -> impl IntoResponse {
    let id = req.get("id").cloned().unwrap_or(Value::Null);
    let method = req
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let params = req
        .get("params")
        .cloned()
        .unwrap_or_else(|| Value::Object(serde_json::Map::new()));

    let body = if !state.mcp_enabled {
        json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32000, "message": "MCP disabled on this instance" } })
    } else {
        match dispatch(&state, &method, &params).await {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Err((code, msg)) => {
                json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": msg } })
            }
        }
    };

    (
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        )],
        Json(body),
    )
}

async fn dispatch(state: &AppState, method: &str, params: &Value) -> Result<Value, (i32, String)> {
    match method {
        "initialize" => Ok(initialize()),
        "initialized" | "notifications/initialized" => Ok(Value::Null),
        "tools/list" => Ok(tools_list()),
        "tools/call" => tools_call(state, params).await,
        "resources/list" => Ok(resources_list(state)),
        "resources/read" => resources_read(state, params),
        _ => Err((-32601, format!("method not found: {method}"))),
    }
}

fn initialize() -> Value {
    json!({
        "protocolVersion": "2024-11-05",
        "capabilities": { "tools": {}, "resources": {} },
        "serverInfo": { "name": "app-mediakit-marketing", "version": env!("CARGO_PKG_VERSION") }
    })
}

fn tools_list() -> Value {
    json!({ "tools": [
        {
            "name": "list_section_types",
            "description": "List the typed section vocabulary a page may be composed from. This is the contract: a manifest may only use these section types and their fields.",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "read_page",
            "description": "Read the current manifest (YAML) for a page slug.",
            "inputSchema": {
                "type": "object",
                "properties": { "slug": { "type": "string" } },
                "required": ["slug"]
            }
        },
        {
            "name": "validate_manifest",
            "description": "Validate a page manifest (YAML) against the section contract WITHOUT staging it. Returns ok or the structural error.",
            "inputSchema": {
                "type": "object",
                "properties": { "manifest": { "type": "string", "description": "Page manifest in YAML" } },
                "required": ["manifest"]
            }
        },
        {
            "name": "propose_page",
            "description": "Propose a page manifest. It is validated and staged to the human review queue. Per SYS-ADR-10 it is NOT published — a human must approve (F12) before it persists.",
            "inputSchema": {
                "type": "object",
                "properties": { "manifest": { "type": "string", "description": "Page manifest in YAML (must include a slug)" } },
                "required": ["manifest"]
            }
        },
        {
            "name": "list_pending",
            "description": "List page proposals currently awaiting human approval.",
            "inputSchema": { "type": "object", "properties": {} }
        }
    ]})
}

async fn tools_call(state: &AppState, params: &Value) -> Result<Value, (i32, String)> {
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or((-32602i32, "missing param: name".to_string()))?;
    let args = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| Value::Object(serde_json::Map::new()));

    let text = match name {
        "list_section_types" => serde_json::to_string_pretty(&section_catalog()).unwrap(),
        "read_page" => {
            let slug = str_arg(&args, "slug")?;
            content::load_page(&state.content_dir, slug)
                .map_err(|e| (-32000i32, format!("{e:?}")))
                .and_then(|p| p.to_yaml().map_err(|e| (-32000i32, e)))?
        }
        "validate_manifest" => {
            let manifest = str_arg(&args, "manifest")?;
            match Page::from_yaml(manifest) {
                Ok(p) => format!("valid: {} section(s)", p.sections.len()),
                Err(e) => format!("INVALID: {e}"),
            }
        }
        "propose_page" => {
            let manifest = str_arg(&args, "manifest")?;
            let id = state.pending.stage(manifest).map_err(|e| (-32000i32, e))?;
            format!(
                "Proposal staged as '{id}'. Per SYS-ADR-10 it is NOT published — a human must approve (F12) before it persists."
            )
        }
        "list_pending" => serde_json::to_string_pretty(&json!(state.pending.list())).unwrap(),
        _ => return Err((-32601, format!("unknown tool: {name}"))),
    };

    Ok(json!({ "content": [{ "type": "text", "text": text }], "isError": false }))
}

fn resources_list(state: &AppState) -> Value {
    let resources: Vec<Value> = content::list_slugs(&state.content_dir)
        .into_iter()
        .map(|slug| {
            json!({
                "uri": format!("marketing://page/{slug}"),
                "name": slug,
                "mimeType": "application/yaml"
            })
        })
        .collect();
    json!({ "resources": resources })
}

fn resources_read(state: &AppState, params: &Value) -> Result<Value, (i32, String)> {
    let uri = params
        .get("uri")
        .and_then(|v| v.as_str())
        .ok_or((-32602i32, "missing param: uri".to_string()))?;
    let slug = uri
        .strip_prefix("marketing://page/")
        .ok_or((-32602i32, format!("unsupported URI scheme: {uri}")))?;
    let page =
        content::load_page(&state.content_dir, slug).map_err(|e| (-32000i32, format!("{e:?}")))?;
    let yaml = page.to_yaml().map_err(|e| (-32000i32, e))?;
    Ok(json!({
        "contents": [{ "uri": uri, "mimeType": "application/yaml", "text": yaml }]
    }))
}

fn str_arg<'a>(args: &'a Value, key: &str) -> Result<&'a str, (i32, String)> {
    args.get(key)
        .and_then(|v| v.as_str())
        .ok_or((-32602i32, format!("missing param: {key}")))
}
