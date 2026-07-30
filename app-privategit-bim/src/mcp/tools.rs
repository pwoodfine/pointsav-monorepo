use crate::state::AppState;
use serde_json::{json, Value};

pub fn list_tools() -> Vec<Value> {
    vec![
        json!({
            "name": "get_bim_object",
            "description": "Get a BIM Object entity by its token path (e.g. 'bim.key-plans.private-office-small')",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "token_path": {
                        "type": "string",
                        "description": "Dot-separated path: bim.<category>.<slug>"
                    }
                },
                "required": ["token_path"]
            }
        }),
        json!({
            "name": "list_objects_by_category",
            "description": "List all BIM Object token paths in a named category file",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "category": {
                        "type": "string",
                        "description": "Category stem (e.g. 'key-plans', 'spatial', 'elements')"
                    }
                },
                "required": ["category"]
            }
        }),
        json!({
            "name": "get_ifc_entity",
            "description": "Get ifc_class and ifc_anchor for a BIM Object",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "token_path": { "type": "string" }
                },
                "required": ["token_path"]
            }
        }),
        json!({
            "name": "get_compliance_requirements",
            "description": "Get compliance fields from a BIM Object $value",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "token_path": { "type": "string" }
                },
                "required": ["token_path"]
            }
        }),
        json!({
            "name": "get_object_property_sets",
            "description": "Get property_sets array from a BIM Object $value",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "token_path": { "type": "string" }
                },
                "required": ["token_path"]
            }
        }),
    ]
}

pub fn call_tool(params: &Option<Value>, state: &AppState) -> Result<Value, String> {
    let p = params.as_ref().ok_or("missing params")?;
    let name = p
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or("missing tool name")?;
    let args = p.get("arguments").cloned().unwrap_or(Value::Null);

    match name {
        "get_bim_object" => {
            let path = args
                .get("token_path")
                .and_then(|v| v.as_str())
                .ok_or("missing token_path")?;
            let entity = resolve_token_path(path, state)?;
            Ok(json!({
                "content": [{ "type": "text", "text": entity.to_string() }]
            }))
        }
        "list_objects_by_category" => {
            let cat = args
                .get("category")
                .and_then(|v| v.as_str())
                .ok_or("missing category")?;
            let file_val = state
                .tokens
                .get(cat)
                .ok_or_else(|| format!("category '{cat}' not found"))?;
            let bim = file_val
                .get("bim")
                .and_then(|v| v.as_object())
                .ok_or("no 'bim' root in token file")?;
            let paths: Vec<String> = bim
                .iter()
                .flat_map(|(grp, grp_val)| {
                    grp_val
                        .as_object()
                        .map(|o| {
                            o.keys()
                                .map(|k| format!("bim.{cat}.{grp}.{k}"))
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default()
                })
                .collect();
            Ok(json!({
                "content": [{ "type": "text", "text": serde_json::to_string(&paths).unwrap() }]
            }))
        }
        "get_ifc_entity" => {
            let path = args
                .get("token_path")
                .and_then(|v| v.as_str())
                .ok_or("missing token_path")?;
            let entity = resolve_token_path(path, state)?;
            let val = entity.get("$value").cloned().unwrap_or(Value::Null);
            Ok(json!({
                "content": [{
                    "type": "text",
                    "text": json!({
                        "ifc_class": val.get("ifc_class"),
                        "ifc_anchor": val.get("ifc_anchor")
                    }).to_string()
                }]
            }))
        }
        "get_compliance_requirements" => {
            let path = args
                .get("token_path")
                .and_then(|v| v.as_str())
                .ok_or("missing token_path")?;
            let entity = resolve_token_path(path, state)?;
            let compliance = entity
                .get("$value")
                .and_then(|v| v.get("compliance"))
                .cloned()
                .unwrap_or(Value::Null);
            Ok(json!({
                "content": [{ "type": "text", "text": compliance.to_string() }]
            }))
        }
        "get_object_property_sets" => {
            let path = args
                .get("token_path")
                .and_then(|v| v.as_str())
                .ok_or("missing token_path")?;
            let entity = resolve_token_path(path, state)?;
            let psets = entity
                .get("$value")
                .and_then(|v| v.get("property_sets"))
                .cloned()
                .unwrap_or(Value::Array(vec![]));
            Ok(json!({
                "content": [{ "type": "text", "text": psets.to_string() }]
            }))
        }
        other => Err(format!("unknown tool: {other}")),
    }
}

fn resolve_token_path(path: &str, state: &AppState) -> Result<Value, String> {
    // Accepts: bim.<file>.<cat>.<slug> or <file>.<cat>.<slug> or <file>.<slug>
    let parts: Vec<&str> = path.split('.').collect();
    let (file_stem, rest) = match parts.as_slice() {
        ["bim", file, rest @ ..] => (*file, rest.to_vec()),
        [file, rest @ ..] => (*file, rest.to_vec()),
        _ => return Err(format!("invalid token_path: {path}")),
    };

    let file_val = state
        .tokens
        .get(file_stem)
        .ok_or_else(|| format!("no token file for '{file_stem}'"))?;

    let bim = file_val
        .get("bim")
        .and_then(|v| v.as_object())
        .ok_or("no 'bim' root")?;

    // Navigate remaining path segments through the JSON tree
    let mut cur: &Value = &Value::Object(bim.clone());
    for key in &rest {
        cur = cur
            .get(key)
            .ok_or_else(|| format!("key '{key}' not found in path '{path}'"))?;
    }
    Ok(cur.clone())
}
