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
                "serverInfo": { "name": "app-privategit-bim", "version": "0.1.0" },
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
        "tools/call" => match tools::call_tool(&req.params, &state) {
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
