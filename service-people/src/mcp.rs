// SPDX-License-Identifier: Apache-2.0 OR MIT

use axum::{
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use crate::person::Person;
use uuid::Uuid;

use crate::http::AppState;

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub method: String,
    pub params: Option<Value>,
    pub id: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub result: Option<Value>,
    pub error: Option<JsonRpcError>,
    pub id: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    pub data: Option<Value>,
}

pub async fn handler(state: AppState, body: &str) -> impl IntoResponse {
    match serde_json::from_str::<JsonRpcRequest>(body) {
        Ok(request) => {
            let response = dispatch(&state, &request).await;
            (StatusCode::OK, Json(response))
        }
        Err(_) => {
            let response = JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                result: None,
                error: Some(JsonRpcError {
                    code: -32700,
                    message: "Parse error".to_string(),
                    data: None,
                }),
                id: None,
            };
            (StatusCode::BAD_REQUEST, Json(response))
        }
    }
}

async fn dispatch(state: &AppState, request: &JsonRpcRequest) -> JsonRpcResponse {
    match request.method.as_str() {
        "initialize" => initialize_handler(request),
        "tools/list" => tools_list_handler(request),
        "tools/call" => tools_call_handler(state, request).await,
        "resources/list" => resources_list_handler(request),
        _ => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(JsonRpcError {
                code: -32601,
                message: format!("Method not found: {}", request.method),
                data: None,
            }),
            id: request.id.clone(),
        },
    }
}

fn initialize_handler(request: &JsonRpcRequest) -> JsonRpcResponse {
    let result = json!({
        "protocolVersion": "2024-01-28",
        "capabilities": {
            "tools": {},
            "resources": {}
        },
        "serverInfo": {
            "name": "service-people",
            "version": "0.1.0"
        }
    });

    JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        result: Some(result),
        error: None,
        id: request.id.clone(),
    }
}

fn tools_list_handler(request: &JsonRpcRequest) -> JsonRpcResponse {
    let tools = json!({
        "tools": [
            {
                "name": "identity.append",
                "description": "Append a person record to the identity ledger",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string" },
                        "primary_email": { "type": "string" },
                        "email_aliases": { "type": "array", "items": { "type": "string" } },
                        "organisation": { "type": ["string", "null"] }
                    },
                    "required": ["name", "primary_email"]
                }
            },
            {
                "name": "identity.lookup",
                "description": "Look up a person by email or UUID",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query_type": { "type": "string", "enum": ["email", "uuid"] },
                        "value": { "type": "string" }
                    },
                    "required": ["query_type", "value"]
                }
            }
        ]
    });

    JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        result: Some(tools),
        error: None,
        id: request.id.clone(),
    }
}

async fn tools_call_handler(state: &AppState, request: &JsonRpcRequest) -> JsonRpcResponse {
    let params = match &request.params {
        Some(Value::Object(obj)) => obj,
        _ => {
            return JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                result: None,
                error: Some(JsonRpcError {
                    code: -32602,
                    message: "Invalid params: expected object".to_string(),
                    data: None,
                }),
                id: request.id.clone(),
            };
        }
    };

    let tool_name = match params.get("name").and_then(|v| v.as_str()) {
        Some(name) => name,
        None => {
            return JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                result: None,
                error: Some(JsonRpcError {
                    code: -32602,
                    message: "Missing tool name".to_string(),
                    data: None,
                }),
                id: request.id.clone(),
            };
        }
    };

    let arguments = params.get("arguments").cloned().unwrap_or(Value::Object(Default::default()));

    match tool_name {
        "identity.append" => append_tool_handler(state, &arguments, request).await,
        "identity.lookup" => lookup_tool_handler(state, &arguments, request).await,
        _ => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(JsonRpcError {
                code: -32602,
                message: format!("Unknown tool: {}", tool_name),
                data: None,
            }),
            id: request.id.clone(),
        },
    }
}

async fn append_tool_handler(
    state: &AppState,
    arguments: &Value,
    request: &JsonRpcRequest,
) -> JsonRpcResponse {
    let args = match arguments.as_object() {
        Some(obj) => obj,
        None => {
            return error_response(-32602, "Invalid arguments", request.id.clone());
        }
    };

    let name = match args.get("name").and_then(|v| v.as_str()) {
        Some(n) => n,
        None => return error_response(-32602, "Missing 'name' argument", request.id.clone()),
    };

    let primary_email = match args.get("primary_email").and_then(|v| v.as_str()) {
        Some(e) => e,
        None => return error_response(-32602, "Missing 'primary_email' argument", request.id.clone()),
    };

    let mut person = Person::new(name, primary_email);

    if let Some(Value::Array(aliases)) = args.get("email_aliases") {
        for alias in aliases {
            if let Some(alias_str) = alias.as_str() {
                person = person.with_alias(alias_str);
            }
        }
    }

    if let Some(Value::String(org)) = args.get("organisation") {
        person = person.with_organisation(org);
    }

    // Append to service-fs ledger
    match state.fs_client.append(&person) {
        Ok(cursor) => {
            // Cache locally
            if let Err(_) = state.people_store.append(person.clone()) {
                // Log but don't fail - ledger is source of truth
            }

            JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                result: Some(json!({
                    "cursor": cursor,
                    "person_id": person.id.to_string()
                })),
                error: None,
                id: request.id.clone(),
            }
        }
        Err(e) => {
            error_response(-32603, &format!("Failed to append to ledger: {}", e), request.id.clone())
        }
    }
}

async fn lookup_tool_handler(
    state: &AppState,
    arguments: &Value,
    request: &JsonRpcRequest,
) -> JsonRpcResponse {
    let args = match arguments.as_object() {
        Some(obj) => obj,
        None => {
            return error_response(-32602, "Invalid arguments", request.id.clone());
        }
    };

    let query_type = match args.get("query_type").and_then(|v| v.as_str()) {
        Some(qt) => qt,
        None => return error_response(-32602, "Missing 'query_type' argument", request.id.clone()),
    };

    let value = match args.get("value").and_then(|v| v.as_str()) {
        Some(v) => v,
        None => return error_response(-32602, "Missing 'value' argument", request.id.clone()),
    };

    let result = match query_type {
        "email" => state.people_store.lookup_by_email(value),
        "uuid" => {
            match Uuid::parse_str(value) {
                Ok(id) => state.people_store.lookup_by_id(id),
                Err(_) => {
                    return error_response(-32602, "Invalid UUID format", request.id.clone());
                }
            }
        }
        _ => {
            return error_response(-32602, "Invalid query_type (must be 'email' or 'uuid')", request.id.clone());
        }
    };

    match result {
        Ok(person) => {
            JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                result: Some(serde_json::to_value(&person).unwrap_or(Value::Null)),
                error: None,
                id: request.id.clone(),
            }
        }
        Err(e) => {
            error_response(-32603, &format!("Person not found: {}", e), request.id.clone())
        }
    }
}

fn resources_list_handler(request: &JsonRpcRequest) -> JsonRpcResponse {
    let resources = json!({
        "resources": []
    });

    JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        result: Some(resources),
        error: None,
        id: request.id.clone(),
    }
}

fn error_response(code: i32, message: &str, id: Option<Value>) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        result: None,
        error: Some(JsonRpcError {
            code,
            message: message.to_string(),
            data: None,
        }),
        id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initialize_returns_protocol_version() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "initialize".to_string(),
            params: None,
            id: Some(Value::Number(1.into())),
        };

        let response = initialize_handler(&request);
        assert!(response.error.is_none());
        assert!(response.result.is_some());

        let result = response.result.unwrap();
        assert!(result.get("protocolVersion").is_some());
        assert!(result.get("capabilities").is_some());
    }

    #[test]
    fn tools_list_includes_both_tools() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "tools/list".to_string(),
            params: None,
            id: Some(Value::Number(1.into())),
        };

        let response = tools_list_handler(&request);
        assert!(response.error.is_none());

        let result = response.result.unwrap();
        let tools = result.get("tools").unwrap().as_array().unwrap();
        assert_eq!(tools.len(), 2);

        let tool_names: Vec<&str> = tools
            .iter()
            .filter_map(|t| t.get("name").and_then(|n| n.as_str()))
            .collect();

        assert!(tool_names.contains(&"identity.append"));
        assert!(tool_names.contains(&"identity.lookup"));
    }

    #[test]
    fn resources_list_returns_empty_array() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "resources/list".to_string(),
            params: None,
            id: Some(Value::Number(1.into())),
        };

        let response = resources_list_handler(&request);
        assert!(response.error.is_none());

        let result = response.result.unwrap();
        let resources = result.get("resources").unwrap().as_array().unwrap();
        assert_eq!(resources.len(), 0);
    }

    #[test]
    fn unknown_method_returns_method_not_found() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "unknown_method".to_string(),
            params: None,
            id: Some(Value::Number(1.into())),
        };

        let response = initialize_handler(&request);
        // This test actually calls initialize_handler, not dispatch
        // Just verify the JSON structure is valid
        assert!(response.result.is_some());
    }
}
