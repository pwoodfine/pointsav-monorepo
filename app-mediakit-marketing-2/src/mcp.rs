// SPDX-License-Identifier: FSL-1.1-ALv2
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

//! Native JSON-RPC 2.0 MCP surface — no vendor SDK, matching the retired
//! engine's contract. Exposes the typed section vocabulary and the
//! read/validate/propose/list-pending operations to an AI authoring agent.
//! `propose_page` only ever stages to the review queue (`pending`) — it can
//! never write into the content tree. Only a human-triggered `POST
//! /api/pending/{id}/approve` call does that (SYS-ADR-10/19).

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::app::AppStateInner;
use crate::content;

#[derive(Debug, Deserialize)]
pub struct RpcRequest {
    #[allow(dead_code)]
    pub jsonrpc: String,
    pub method: String,
    #[serde(default)]
    pub params: Value,
    pub id: Value,
}

#[derive(Debug, Serialize)]
pub struct RpcResponse {
    pub jsonrpc: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
    pub id: Value,
}

#[derive(Debug, Serialize)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
}

#[allow(dead_code)] // standard JSON-RPC 2.0 error code; malformed JSON never
                    // reaches this module (axum's Json extractor rejects it
                    // first) — kept for documentation of the code space.
const PARSE_ERROR: i32 = -32700;
const METHOD_NOT_FOUND: i32 = -32601;
const INVALID_PARAMS: i32 = -32602;
const INTERNAL_ERROR: i32 = -32603;

pub fn handle(state: &AppStateInner, req: RpcRequest) -> RpcResponse {
    let id = req.id.clone();
    match dispatch(state, &req) {
        Ok(result) => RpcResponse {
            jsonrpc: "2.0",
            result: Some(result),
            error: None,
            id,
        },
        Err((code, message)) => RpcResponse {
            jsonrpc: "2.0",
            result: None,
            error: Some(RpcError { code, message }),
            id,
        },
    }
}

fn dispatch(state: &AppStateInner, req: &RpcRequest) -> Result<Value, (i32, String)> {
    match req.method.as_str() {
        "list_section_types" => Ok(section_type_catalog()),

        "read_page" => {
            let slug = str_param(&req.params, "slug")?;
            let lang = opt_str_param(&req.params, "lang");
            let page = content::load_page(&state.content_dir, &slug, lang.as_deref())
                .map_err(|e| (INTERNAL_ERROR, e.to_string()))?;
            serde_json::to_value(page).map_err(|e| (INTERNAL_ERROR, e.to_string()))
        }

        "validate_manifest" => {
            let yaml = str_param(&req.params, "manifest_yaml")?;
            match serde_yaml::from_str::<content::Page>(&yaml) {
                Ok(_) => Ok(json!({ "valid": true })),
                Err(e) => Ok(json!({ "valid": false, "error": e.to_string() })),
            }
        }

        "propose_page" => {
            let slug = str_param(&req.params, "slug")?;
            let lang = opt_str_param(&req.params, "lang").unwrap_or_else(|| "en".to_string());
            let yaml = str_param(&req.params, "manifest_yaml")?;
            let id = state
                .pending
                .stage(&slug, &lang, &yaml)
                .map_err(|e| (INVALID_PARAMS, e.to_string()))?;
            Ok(json!({ "id": id, "status": "pending" }))
        }

        "list_pending" => {
            let items = state
                .pending
                .list()
                .map_err(|e| (INTERNAL_ERROR, e.to_string()))?;
            serde_json::to_value(items).map_err(|e| (INTERNAL_ERROR, e.to_string()))
        }

        other => Err((METHOD_NOT_FOUND, format!("unknown method: {other}"))),
    }
}

fn str_param(params: &Value, key: &str) -> Result<String, (i32, String)> {
    params
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| {
            (
                INVALID_PARAMS,
                format!("missing or non-string param: {key}"),
            )
        })
}

fn opt_str_param(params: &Value, key: &str) -> Option<String> {
    params.get(key).and_then(Value::as_str).map(str::to_string)
}

fn section_type_catalog() -> Value {
    json!([
        {
            "type": "hero",
            "fields": [
                { "name": "headline", "kind": "string", "required": true },
                { "name": "subhead", "kind": "string", "required": false }
            ]
        },
        {
            "type": "card-grid",
            "fields": [
                { "name": "columns", "kind": "integer", "required": true },
                {
                    "name": "cards",
                    "kind": "array",
                    "required": true,
                    "item_fields": [
                        { "name": "title", "kind": "string", "required": true },
                        { "name": "body", "kind": "string", "required": false },
                        { "name": "href", "kind": "string", "required": false }
                    ]
                }
            ]
        },
        {
            "type": "prose",
            "fields": [
                { "name": "body", "kind": "string (markdown)", "required": true }
            ]
        }
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pending::Queue;

    fn test_state() -> (tempfile::TempDir, tempfile::TempDir, AppStateInner) {
        let content_dir = tempfile::tempdir().unwrap();
        let state_dir = tempfile::tempdir().unwrap();
        let home_dir = content_dir.path().join("home");
        std::fs::create_dir_all(&home_dir).unwrap();
        std::fs::write(
            home_dir.join("page.yaml"),
            "title: Home\nslug: home\ndescription: Test.\nsections:\n  - type: hero\n    headline: Hi\n",
        )
        .unwrap();
        let pending = Queue::open(state_dir.path()).unwrap();
        let state = AppStateInner {
            content_dir: content_dir.path().to_path_buf(),
            module_id: "woodfine".to_string(),
            google_verify: None,
            pending,
        };
        (content_dir, state_dir, state)
    }

    fn req(method: &str, params: Value) -> RpcRequest {
        RpcRequest {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
            id: json!(1),
        }
    }

    #[test]
    fn list_section_types_returns_three_types() {
        let (_c, _s, state) = test_state();
        let resp = handle(&state, req("list_section_types", Value::Null));
        let result = resp.result.unwrap();
        assert_eq!(result.as_array().unwrap().len(), 3);
    }

    #[test]
    fn read_page_returns_real_content() {
        let (_c, _s, state) = test_state();
        let resp = handle(&state, req("read_page", json!({ "slug": "home" })));
        assert!(resp.error.is_none());
        assert_eq!(resp.result.unwrap()["title"], "Home");
    }

    #[test]
    fn propose_page_never_touches_content_tree() {
        let (content_dir, _s, state) = test_state();
        let before = std::fs::read_to_string(content_dir.path().join("home/page.yaml")).unwrap();

        let resp = handle(
            &state,
            req(
                "propose_page",
                json!({
                    "slug": "home",
                    "manifest_yaml": "title: Home\nslug: home\ndescription: Changed.\nsections: []\n"
                }),
            ),
        );
        assert!(resp.error.is_none());
        assert!(resp.result.unwrap()["id"].as_str().is_some());

        // The content tree must be byte-for-byte unchanged — propose is not publish.
        let after = std::fs::read_to_string(content_dir.path().join("home/page.yaml")).unwrap();
        assert_eq!(before, after);
        assert_eq!(state.pending.list().unwrap().len(), 1);
    }

    #[test]
    fn unknown_method_returns_method_not_found() {
        let (_c, _s, state) = test_state();
        let resp = handle(&state, req("delete_everything", Value::Null));
        assert_eq!(resp.error.unwrap().code, METHOD_NOT_FOUND);
    }

    #[test]
    fn validate_manifest_reports_invalid_yaml() {
        let (_c, _s, state) = test_state();
        let resp = handle(
            &state,
            req(
                "validate_manifest",
                json!({ "manifest_yaml": "not: [valid" }),
            ),
        );
        assert_eq!(resp.result.unwrap()["valid"], false);
    }
}
