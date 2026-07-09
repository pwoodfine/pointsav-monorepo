// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

// Phase 3 (hyperscaler-tier initiative) — MCP JSON-RPC endpoint. Closes a dead nginx
// proxy rule (nginx-design.conf already forwarded /mcp to this binary; no route existed).
// Follows app-privategit-bim's mcp/{mod,protocol,tools}.rs split as the reference
// implementation — same JSON-RPC 2.0 envelope, same initialize/tools:list/tools:call
// dispatch shape. Self-hosted MCP context (no cloud SaaS middleman) is the same
// sovereignty-over-scale positioning this whole substrate already argues for; zeroheight
// and Supernova made "expose the design system to AI agents via MCP" their 2026 flagship
// feature — this closes that gap rather than inventing new positioning.
mod protocol;
mod tools;

use axum::{extract::State, http::StatusCode, Json};
use protocol::{JsonRpcError, JsonRpcRequest, JsonRpcResponse};
use serde_json::json;

use crate::state::AppState;

pub async fn mcp_handler(
    State(state): State<AppState>,
    body: axum::body::Bytes,
) -> (StatusCode, Json<JsonRpcResponse>) {
    let req: JsonRpcRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(_) => {
            return (
                StatusCode::OK,
                Json(JsonRpcResponse {
                    jsonrpc: "2.0".into(),
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32700,
                        message: "Parse error".into(),
                    }),
                    id: serde_json::Value::Null,
                }),
            );
        }
    };

    let id = req.id.clone();
    let resp = match req.method.as_str() {
        "initialize" => JsonRpcResponse {
            jsonrpc: "2.0".into(),
            result: Some(json!({
                "protocolVersion": "2025-11-25",
                "serverInfo": { "name": "app-privategit-design", "version": env!("CARGO_PKG_VERSION") },
                "capabilities": { "tools": {} }
            })),
            error: None,
            id,
        },
        "tools/list" => JsonRpcResponse {
            jsonrpc: "2.0".into(),
            result: Some(json!({ "tools": tools::list_tools() })),
            error: None,
            id,
        },
        "tools/call" => match tools::call_tool(&req.params, &state).await {
            Ok(v) => JsonRpcResponse {
                jsonrpc: "2.0".into(),
                result: Some(v),
                error: None,
                id,
            },
            Err(e) => JsonRpcResponse {
                jsonrpc: "2.0".into(),
                result: None,
                error: Some(JsonRpcError {
                    code: -32603,
                    message: e,
                }),
                id,
            },
        },
        other => JsonRpcResponse {
            jsonrpc: "2.0".into(),
            result: None,
            error: Some(JsonRpcError {
                code: -32601,
                message: format!("method '{other}' not found"),
            }),
            id,
        },
    };

    (StatusCode::OK, Json(resp))
}
