// SPDX-License-Identifier: Apache-2.0 OR MIT

//! MCP-server interface layer for service-input.
//!
//! Implements the Anthropic/Cloudflare 2026 Model Context Protocol
//! (MCP) spec over JSON-RPC 2.0 on top of the axum surface. Mounted
//! at `POST /mcp` (Streamable HTTP transport — single JSON response).
//!
//! Per `~/Foundry/conventions/three-ring-architecture.md` §"MCP
//! boundary at Ring 1": Ring 1 services expose an MCP server
//! interface; `service-extraction` (Ring 2) consumes them as an MCP
//! client.
//!
//! Capabilities exposed:
//!   Tools (write surface):
//!     document.ingest — detect format, parse bytes, write to
//!       service-fs via FsClient. Arguments:
//!         filename (string) — used for format detection (extension
//!           first, per ADR-07 deterministic detection).
//!         source_id (string) — forwarded as the ledger payload_id.
//!         bytes_base64 (string) — base64-encoded document bytes.
//!       Returns: { cursor: u64 } — the ledger cursor assigned by
//!         service-fs.
//!
//!   Resources: none (service-input is write-only; reads happen at
//!     service-fs via its `ledger://entries` resource).
//!
//! Authentication: `X-Foundry-Module-ID` header required on all MCP
//! requests (per-tenant boundary enforcement; mismatch returns a
//! JSON-RPC error).
//!
//! ADR-07 compliant — no AI inference; format detection is
//! deterministic (extension-first, magic-byte fallback).

use std::sync::Arc;

use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Json};
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::http::AppState;
use crate::{detect_format, ParseError};

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

// ── MCP handler ─────────────────────────────────────────────────────

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

    if let Err(msg) = check_module_id(&state, &headers) {
        return Json(JsonRpcResponse::err(id, INVALID_REQUEST, msg));
    }

    let result = match req.method.as_str() {
        "initialize" => handle_initialize(),
        "notifications/initialized" => serde_json::json!({}),
        "tools/list" => handle_tools_list(),
        "tools/call" => match handle_tools_call(&state, req.params).await {
            Ok(v) => v,
            Err((code, msg)) => return Json(JsonRpcResponse::err(id, code, msg)),
        },
        "resources/list" => handle_resources_list(),
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
            "name": "service-input",
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
                "name": "document.ingest",
                "description": "Detect format, parse bytes, and write the parsed document \
                    into service-fs's WORM ledger. Returns the assigned ledger cursor.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "filename": {
                            "type": "string",
                            "description": "Original filename; used for format detection \
                                (extension-first, magic-byte fallback per ADR-07)."
                        },
                        "source_id": {
                            "type": "string",
                            "description": "Caller-side document identifier; forwarded as \
                                the ledger payload_id."
                        },
                        "bytes_base64": {
                            "type": "string",
                            "description": "Base64-encoded document bytes (standard encoding)."
                        }
                    },
                    "required": ["filename", "source_id", "bytes_base64"]
                }
            }
        ]
    })
}

async fn handle_tools_call(
    state: &Arc<AppState>,
    params: Option<Value>,
) -> Result<Value, (i64, String)> {
    let params = params
        .ok_or_else(|| (INVALID_PARAMS, "params required for tools/call".to_string()))?;
    let name = params["name"]
        .as_str()
        .ok_or_else(|| (INVALID_PARAMS, "params.name is required".to_string()))?;

    match name {
        "document.ingest" => {
            let args = &params["arguments"];

            let filename = args["filename"]
                .as_str()
                .ok_or_else(|| (INVALID_PARAMS, "arguments.filename is required".to_string()))?;
            let source_id = args["source_id"]
                .as_str()
                .ok_or_else(|| (INVALID_PARAMS, "arguments.source_id is required".to_string()))?;
            let bytes_b64 = args["bytes_base64"]
                .as_str()
                .ok_or_else(|| (INVALID_PARAMS, "arguments.bytes_base64 is required".to_string()))?;

            let bytes = base64::engine::general_purpose::STANDARD
                .decode(bytes_b64)
                .map_err(|e| (INVALID_PARAMS, format!("bytes_base64 decode error: {e}")))?;

            let format = detect_format(filename, &bytes).ok_or_else(|| {
                (
                    INVALID_PARAMS,
                    format!("format undetected for filename '{filename}'"),
                )
            })?;

            let doc = state
                .dispatcher
                .dispatch(format, source_id, &bytes)
                .map_err(|e: ParseError| (INTERNAL_ERROR, e.to_string()))?;

            let cursor = state
                .fs_client
                .submit(&doc)
                .map_err(|e| (INTERNAL_ERROR, e.to_string()))?;

            let result_text = serde_json::to_string(&serde_json::json!({
                "cursor": cursor,
                "source_id": source_id,
                "format": format!("{format:?}"),
            }))
            .unwrap_or_default();

            Ok(serde_json::json!({
                "content": [{"type": "text", "text": result_text}],
                "isError": false
            }))
        }
        other => Err((INVALID_PARAMS, format!("unknown tool '{other}'"))),
    }
}

fn handle_resources_list() -> Value {
    serde_json::json!({
        "resources": []
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DocxParser, MarkdownParser, PdfParser, XlsxParser};
    use axum::http::Request;
    use tower::ServiceExt;

    fn make_state(module_id: &str) -> Arc<AppState> {
        use crate::fs_client::FsClient;
        use crate::Dispatcher;

        Arc::new(AppState {
            module_id: module_id.to_string(),
            dispatcher: Dispatcher::new()
                .with_pdf(Box::new(PdfParser))
                .with_markdown(Box::new(MarkdownParser))
                .with_docx(Box::new(DocxParser))
                .with_xlsx(Box::new(XlsxParser)),
            fs_client: FsClient::new("http://127.0.0.1:0", module_id),
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
    async fn initialize_returns_server_info() {
        let state = make_state("m1");
        let app = crate::http::router(state);
        let resp = mcp_call(
            app,
            "m1",
            serde_json::json!({
                "jsonrpc": "2.0", "id": 1,
                "method": "initialize", "params": {}
            }),
        )
        .await;
        assert_eq!(resp["jsonrpc"], "2.0");
        assert_eq!(resp["result"]["serverInfo"]["name"], "service-input");
        assert!(resp["result"]["capabilities"]["tools"].is_object());
    }

    #[tokio::test]
    async fn tools_list_includes_document_ingest() {
        let state = make_state("m2");
        let app = crate::http::router(state);
        let resp = mcp_call(
            app,
            "m2",
            serde_json::json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
        )
        .await;
        let tools = resp["result"]["tools"].as_array().unwrap();
        assert!(
            tools.iter().any(|t| t["name"] == "document.ingest"),
            "document.ingest must be listed"
        );
    }

    #[tokio::test]
    async fn tools_call_ingest_markdown_returns_error_without_fs() {
        // With no real service-fs behind it, the ingest tool should
        // parse the Markdown (success) then fail at FsClient::submit
        // (transport error — port 0 refuses connections). The RPC
        // response must be a JSON-RPC error, not a panic.
        let state = make_state("m3");
        let app = crate::http::router(state);
        let bytes_b64 =
            base64::engine::general_purpose::STANDARD.encode(b"# Hello\n\nBody.");
        let resp = mcp_call(
            app,
            "m3",
            serde_json::json!({
                "jsonrpc": "2.0", "id": 3,
                "method": "tools/call",
                "params": {
                    "name": "document.ingest",
                    "arguments": {
                        "filename": "doc.md",
                        "source_id": "test-doc",
                        "bytes_base64": bytes_b64
                    }
                }
            }),
        )
        .await;
        // Expect an error result (transport error) — not a panic.
        assert!(
            resp["error"].is_object(),
            "expected JSON-RPC error when fs_client has no server, got: {resp}"
        );
    }

    #[tokio::test]
    async fn unknown_module_id_returns_rpc_error() {
        let state = make_state("m4");
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
                            "jsonrpc": "2.0", "id": 4,
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

    #[tokio::test]
    async fn unknown_format_returns_rpc_error() {
        let state = make_state("m5");
        let app = crate::http::router(state);
        let bytes_b64 =
            base64::engine::general_purpose::STANDARD.encode(b"arbitrary bytes");
        let resp = mcp_call(
            app,
            "m5",
            serde_json::json!({
                "jsonrpc": "2.0", "id": 5,
                "method": "tools/call",
                "params": {
                    "name": "document.ingest",
                    "arguments": {
                        "filename": "unknown.bin",
                        "source_id": "x",
                        "bytes_base64": bytes_b64
                    }
                }
            }),
        )
        .await;
        assert!(
            resp["error"].is_object(),
            "expected JSON-RPC error for undetected format"
        );
    }
}
