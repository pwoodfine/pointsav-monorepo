// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

use crate::{state::AppState, tokens_gallery};
use serde_json::{json, Value};

pub fn list_tools() -> Vec<Value> {
    vec![
        json!({
            "name": "get_component_recipe",
            "description": "Get a component's full recipe (HTML, CSS, ARIA guidance, tokens consumed, variants) by slug — the same recipe.json this site renders live previews from",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Component slug, e.g. 'button'" }
                },
                "required": ["name"]
            }
        }),
        json!({
            "name": "list_components",
            "description": "List all component slugs, optionally filtered by origin category",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "category": {
                        "type": "string",
                        "description": "Filter by origin: 'components' (generic substrate), 'map' (GIS-origin), 'wiki' (wiki-engine-origin). Omit for all."
                    }
                }
            }
        }),
        json!({
            "name": "get_token",
            "description": "Resolve a single design token by its CSS custom property name (e.g. '--cds-interactive') or DTCG path (e.g. 'semantic.interactive-primary')",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "--cds-* custom property name or dot-path" }
                },
                "required": ["name"]
            }
        }),
        json!({
            "name": "search_design_system",
            "description": "Full-text search across every indexed vault document — components, tokens, research, guidelines, developing, designing, about",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": { "type": "string" }
                },
                "required": ["query"]
            }
        }),
    ]
}

pub async fn call_tool(params: &Option<Value>, state: &AppState) -> Result<Value, String> {
    let p = params.as_ref().ok_or("missing params")?;
    let name = p
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or("missing tool name")?;
    let args = p.get("arguments").cloned().unwrap_or(Value::Null);

    match name {
        "get_component_recipe" => {
            let slug = args
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or("missing name")?;
            if slug.contains("..") || slug.contains('/') {
                return Err("invalid name".to_string());
            }
            let path = state
                .vault
                .join("components")
                .join(slug)
                .join("recipe.json");
            let raw = std::fs::read_to_string(&path)
                .map_err(|_| format!("no recipe.json for component '{slug}'"))?;
            let recipe: Value = serde_json::from_str(&raw)
                .map_err(|_| "recipe.json is not valid JSON".to_string())?;
            Ok(json!({ "content": [{ "type": "text", "text": recipe.to_string() }] }))
        }
        "list_components" => {
            let category_filter = args.get("category").and_then(|v| v.as_str());
            let slugs: Vec<String> = match category_filter {
                None => state.nav.get("components").cloned().unwrap_or_default(),
                Some(cat) => state
                    .component_groups
                    .iter()
                    .find(|(label, _)| label_matches_category(label, cat))
                    .map(|(_, slugs)| slugs.clone())
                    .unwrap_or_default(),
            };
            Ok(
                json!({ "content": [{ "type": "text", "text": serde_json::to_string(&slugs).unwrap() }] }),
            )
        }
        "get_token" => {
            let query = args
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or("missing name")?;
            let tiers = tokens_gallery::load_and_flatten(&state.vault);
            let hit = tiers
                .iter()
                .flat_map(|tier| &tier.groups)
                .flat_map(|group| &group.entries)
                .find(|e| e.css_var == query || e.path == query);
            match hit {
                Some(entry) => Ok(json!({
                    "content": [{
                        "type": "text",
                        "text": json!({
                            "path": entry.path,
                            "css_var": entry.css_var,
                            "value": entry.value,
                            "kind": entry.kind,
                            "description": entry.description,
                        }).to_string()
                    }]
                })),
                None => Err(format!("no token found matching '{query}'")),
            }
        }
        "search_design_system" => {
            let query = args
                .get("query")
                .and_then(|v| v.as_str())
                .ok_or("missing query")?;
            let idx = state.index.read().await;
            let hits: Vec<Value> = idx
                .search(query)
                .into_iter()
                .take(20)
                .map(|doc| json!({ "id": doc.id, "title": doc.title }))
                .collect();
            Ok(
                json!({ "content": [{ "type": "text", "text": serde_json::to_string(&hits).unwrap() }] }),
            )
        }
        other => Err(format!("unknown tool: {other}")),
    }
}

/// `component_groups` labels are human-readable prose ("Also used by the wiki engine"),
/// not the raw category string — match on the same keywords `vault::discover_component_groups`
/// uses to build them, plus the empty-label generic group.
fn label_matches_category(label: &str, category: &str) -> bool {
    match category {
        "components" => label.is_empty(),
        "map" => label.contains("gis.woodfinegroup.com"),
        "wiki" => label.contains("wiki engine"),
        other => label.to_lowercase().contains(&other.to_lowercase()),
    }
}
