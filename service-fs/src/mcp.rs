// SPDX-License-Identifier: Apache-2.0 OR MIT

//! MCP-server interface layer for service-fs.
//!
//! Implements the Anthropic/Cloudflare 2026 Model Context Protocol
//! (MCP) spec over JSON-RPC 2.0 on top of the existing axum surface.
//! The underlying `/v1/append` and `/v1/entries` routes remain
//! unchanged; this layer adds a `/mcp` endpoint that speaks MCP
//! JSON-RPC 2.0 (Streamable HTTP transport — single JSON response,
//! no SSE required for the initial landing).
//!
//! Per `~/Foundry/conventions/three-ring-architecture.md` §"MCP
//! boundary at Ring 1": Ring 1 services expose an MCP server
//! interface. Ring 2 (`service-extraction`) consumes these as an
//! MCP client over the same wire.
//!
//! Capabilities exposed:
//!   Tools (write surface):
//!     ledger.append — append a payload to the WORM ledger. Arguments:
//!       payload_id (string), payload (object).
//!
//!   Resources (read surface):
//!     ledger://entries — read ledger entries since a cursor. URI form:
//!       `ledger://entries?since=N` (default N=0).
//!
//! Authentication: the same `X-Foundry-Module-ID` header is required
//! on all MCP requests (same per-tenant boundary as /v1/append and
//! /v1/entries; mismatch returns a JSON-RPC error, not a 403, so the
//! MCP client gets a protocol-level error).

use std::sync::Arc;

use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Json};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::http::AppState;
use crate::ledger::LedgerError;

// ── JSON-RPC 2.0 envelope types ────────────────────────────────────

#[derive(Deserialize)]
pub struct JsonRpcRequest {
    #[allow(dead_code)]
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    pub params: Option<Value>,
}

#[derive(Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: &'static str,
    pub id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Serialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl JsonRpcResponse {
    fn ok(id: Value, result: Value) -> Self {
        Self { jsonrpc: "2.0", id, result: Some(result), error: None }
    }

    fn err(id: Value, code: i64, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(JsonRpcError { code, message: message.into(), data: None }),
        }
    }
}

// Standard JSON-RPC 2.0 error codes.
const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;
const INTERNAL_ERROR: i64 = -32603;

// ── MCP method handlers ─────────────────────────────────────────────

/// `/mcp` POST handler. Dispatches JSON-RPC 2.0 requests to the
/// appropriate MCP method handler.
pub async fn mcp_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    let req: JsonRpcRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => {
            return Json(JsonRpcResponse::err(
                Value::Null,
                PARSE_ERROR,
                format!("parse error: {e}"),
            ));
        }
    };

    let id = req.id.clone().unwrap_or(Value::Null);

    // Per-tenant boundary enforcement — same rule as /v1/append and
    // /v1/entries. A moduleId mismatch returns a JSON-RPC error so
    // the MCP client gets a protocol-level response rather than a
    // bare 403.
    if let Err(msg) = check_module_id(&state, &headers) {
        return Json(JsonRpcResponse::err(id, INVALID_REQUEST, msg));
    }

    let result = match req.method.as_str() {
        "initialize" => handle_initialize(),
        "notifications/initialized" => {
            // Client notification; no response needed per spec, but
            // we return an empty result to satisfy JSON-RPC 2.0.
            serde_json::json!({})
        }
        "tools/list" => handle_tools_list(),
        "tools/call" => match handle_tools_call(&state, req.params) {
            Ok(v) => v,
            Err(e) => return Json(JsonRpcResponse::err(id, INTERNAL_ERROR, e)),
        },
        "resources/list" => handle_resources_list(),
        "resources/read" => match handle_resources_read(&state, req.params) {
            Ok(v) => v,
            Err(e) => return Json(JsonRpcResponse::err(id, INVALID_PARAMS, e)),
        },
        _ => {
            return Json(JsonRpcResponse::err(
                id,
                METHOD_NOT_FOUND,
                format!("method '{}' not found", req.method),
            ));
        }
    };

    Json(JsonRpcResponse::ok(id, result))
}

fn check_module_id(state: &AppState, headers: &HeaderMap) -> Result<(), String> {
    let supplied = headers
        .get("x-foundry-module-id")
        .and_then(|v| v.to_str().ok());
    match supplied {
        Some(s) if s == state.module_id => Ok(()),
        Some(other) => Err(format!(
            "X-Foundry-Module-ID '{other}' does not match this daemon's tenant '{}' \
             (per-tenant boundary, Doctrine §IV.b)",
            state.module_id
        )),
        None => Err(
            "X-Foundry-Module-ID header is required (per-tenant boundary)".to_string(),
        ),
    }
}

fn handle_initialize() -> Value {
    serde_json::json!({
        "protocolVersion": "2024-11-05",
        "serverInfo": {
            "name": "service-fs",
            "version": env!("CARGO_PKG_VERSION"),
        },
        "capabilities": {
            "tools": {},
            "resources": {},
        },
    })
}

fn handle_tools_list() -> Value {
    serde_json::json!({
        "tools": [
            {
                "name": "ledger.append",
                "description": "Append a payload to the WORM ledger. \
                    Returns the assigned monotonic cursor. The entry is \
                    permanent — no API surface can remove or modify it.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "payload_id": {
                            "type": "string",
                            "description": "Caller-side identifier (e.g., source-document id)."
                        },
                        "payload": {
                            "type": "object",
                            "description": "Payload to persist. Any JSON object."
                        }
                    },
                    "required": ["payload_id", "payload"]
                }
            }
        ]
    })
}

fn handle_tools_call(
    state: &Arc<AppState>,
    params: Option<Value>,
) -> Result<Value, String> {
    let params = params.ok_or_else(|| "params required for tools/call".to_string())?;
    let name = params["name"]
        .as_str()
        .ok_or_else(|| "params.name is required".to_string())?;

    match name {
        "ledger.append" => {
            let args = &params["arguments"];
            let payload_id = args["payload_id"]
                .as_str()
                .ok_or_else(|| "arguments.payload_id is required".to_string())?;
            let payload = args
                .get("payload")
                .ok_or_else(|| "arguments.payload is required".to_string())?;

            let cursor = state
                .ledger
                .append(payload_id, payload)
                .map_err(|e: LedgerError| e.to_string())?;

            let result_text =
                serde_json::to_string(&serde_json::json!({ "cursor": cursor, "payload_id": payload_id }))
                    .unwrap_or_default();

            Ok(serde_json::json!({
                "content": [{"type": "text", "text": result_text}],
                "isError": false
            }))
        }
        other => Err(format!("unknown tool '{other}'")),
    }
}

fn handle_resources_list() -> Value {
    serde_json::json!({
        "resources": [
            {
                "uri": "ledger://entries",
                "name": "Ledger Entries",
                "description": "Read WORM ledger entries since a cursor. \
                    Use URI `ledger://entries?since=N` (default N=0).",
                "mimeType": "application/json"
            }
        ]
    })
}

fn handle_resources_read(
    state: &Arc<AppState>,
    params: Option<Value>,
) -> Result<Value, String> {
    let uri = params
        .as_ref()
        .and_then(|p| p["uri"].as_str())
        .ok_or_else(|| "params.uri is required".to_string())?;

    // Parse: ledger://entries?since=N
    if !uri.starts_with("ledger://entries") {
        return Err(format!("unknown resource URI '{uri}'"));
    }

    let since: u64 = uri
        .find("since=")
        .and_then(|pos| uri[pos + 6..].split('&').next())
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    let entries = state
        .ledger
        .read_since(since)
        .map_err(|e: LedgerError| e.to_string())?;

    let payload_json = serde_json::to_string(&serde_json::json!({
        "module_id": state.module_id,
        "since": since,
        "entries": entries,
    }))
    .unwrap_or_default();

    Ok(serde_json::json!({
        "contents": [
            {
                "uri": uri,
                "mimeType": "application/json",
                "text": payload_json
            }
        ]
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger::InMemoryLedger;
    use axum::http::Request;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use tower::ServiceExt;

    static TMPCTR: AtomicU64 = AtomicU64::new(0);

    fn tmpdir() -> PathBuf {
        let n = TMPCTR.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir()
            .join(format!("svc-fs-mcp-test-{}-{}", std::process::id(), n));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn make_state(module_id: &str) -> Arc<AppState> {
        let d = tmpdir();
        Arc::new(AppState {
            module_id: module_id.to_string(),
            ledger: Box::new(InMemoryLedger::open(d.join("main"), module_id).unwrap()),
            audit_ledger: Box::new(
                InMemoryLedger::open(d.join("audit"), "audit-log").unwrap(),
            ),
        })
    }

    async fn mcp_call(
        app: axum::Router,
        module_id: &str,
        body: serde_json::Value,
    ) -> serde_json::Value {
        use axum::body::to_bytes;
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/mcp")
                    .header("content-type", "application/json")
                    .header("x-foundry-module-id", module_id)
                    .body(axum::body::Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn initialize_returns_capabilities() {
        let state = make_state("t1");
        let app = crate::http::router(state);
        let resp = mcp_call(
            app,
            "t1",
            serde_json::json!({
                "jsonrpc": "2.0", "id": 1,
                "method": "initialize",
                "params": {}
            }),
        )
        .await;
        assert_eq!(resp["jsonrpc"], "2.0");
        assert!(resp["result"]["capabilities"]["tools"].is_object());
        assert!(resp["result"]["capabilities"]["resources"].is_object());
    }

    #[tokio::test]
    async fn tools_list_includes_ledger_append() {
        let state = make_state("t2");
        let app = crate::http::router(state);
        let resp = mcp_call(
            app,
            "t2",
            serde_json::json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
        )
        .await;
        let tools = resp["result"]["tools"].as_array().unwrap();
        assert!(
            tools.iter().any(|t| t["name"] == "ledger.append"),
            "ledger.append must be listed"
        );
    }

    #[tokio::test]
    async fn tools_call_append_returns_cursor() {
        let state = make_state("t3");
        let app = crate::http::router(state);
        let resp = mcp_call(
            app,
            "t3",
            serde_json::json!({
                "jsonrpc": "2.0", "id": 3,
                "method": "tools/call",
                "params": {
                    "name": "ledger.append",
                    "arguments": {
                        "payload_id": "doc-mcp",
                        "payload": {"k": 1}
                    }
                }
            }),
        )
        .await;
        assert!(resp["error"].is_null(), "no error: {resp}");
        let content_text = resp["result"]["content"][0]["text"].as_str().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(content_text).unwrap();
        assert!(parsed["cursor"].as_u64().unwrap() >= 1);
        assert_eq!(parsed["payload_id"], "doc-mcp");
    }

    #[tokio::test]
    async fn resources_read_returns_appended_entry() {
        let state = make_state("t4");
        let app = crate::http::router(state.clone());

        // Append via main ledger directly.
        state.ledger.append("r1", &serde_json::json!({"x": 42})).unwrap();

        let resp = mcp_call(
            app,
            "t4",
            serde_json::json!({
                "jsonrpc": "2.0", "id": 4,
                "method": "resources/read",
                "params": {"uri": "ledger://entries?since=0"}
            }),
        )
        .await;

        assert!(resp["error"].is_null(), "no error: {resp}");
        let text = resp["result"]["contents"][0]["text"].as_str().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
        let entries = parsed["entries"].as_array().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["payload_id"], "r1");
    }

    #[tokio::test]
    async fn unknown_module_id_returns_rpc_error() {
        let state = make_state("t5");
        let app = crate::http::router(state);
        use axum::body::to_bytes;
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/mcp")
                    .header("content-type", "application/json")
                    .header("x-foundry-module-id", "wrong-tenant")
                    .body(axum::body::Body::from(
                        serde_json::to_vec(&serde_json::json!({
                            "jsonrpc": "2.0", "id": 5,
                            "method": "initialize", "params": {}
                        }))
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(body["error"].is_object(), "expected RPC error for wrong module_id");
    }
}
