/// MCP (Model Context Protocol) server — native JSON-RPC 2.0, no vendor SDK.
///
/// Transport: `POST /mcp`
/// Protocol:  JSON-RPC 2.0 over HTTP (open spec — no `rmcp` crate used).
///
/// Implemented methods:
///   initialize / initialized
///   tools/list · tools/call
///   resources/list · resources/read
///   prompts/list · prompts/get
///
/// Tools:
///   create_topic, propose_edit, link_citation
///
/// Resources: wiki://topic/{slug}
///
/// Prompts: cite-this-page, summarize-topic, draft-related-topic
use std::sync::Arc;

use axum::{
    extract::State,
    http::{header, HeaderValue},
    response::IntoResponse,
    Json,
};
use serde_json::{json, Value};

use crate::server::AppState;

// ─── Public axum handler ────────────────────────────────────────────────────

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

    let body = match dispatch(&state, &method, &params).await {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err((code, msg)) => json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": code, "message": msg }
        }),
    };

    (
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        )],
        axum::Json(body),
    )
}

// ─── Method dispatch ────────────────────────────────────────────────────────

async fn dispatch(state: &AppState, method: &str, params: &Value) -> Result<Value, (i32, String)> {
    match method {
        "initialize" => initialize(params),
        "initialized" | "notifications/initialized" => Ok(Value::Null),
        "tools/list" => tools_list(),
        "tools/call" => tools_call(state, params).await,
        "resources/list" => resources_list(state).await,
        "resources/read" => resources_read(state, params).await,
        "prompts/list" => prompts_list(),
        "prompts/get" => prompts_get(params),
        "query_claims" => query_claims(state, params),
        "query_page" => query_page(state, params).await,
        "search" => mcp_search(state, params).await,
        // Phase 7: federated search — fans out to peer instances and merges results.
        "knowledge/search" => federation_search(state, params).await,
        "list_pages" => list_pages(state, params).await,
        "get_links" => get_links(state, params).await,
        "get_citations" => get_citations(state, params).await,
        _ => Err((-32601, format!("method not found: {method}"))),
    }
}

// ─── initialize ─────────────────────────────────────────────────────────────

fn initialize(_params: &Value) -> Result<Value, (i32, String)> {
    Ok(json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {
            "tools": {},
            "resources": {},
            "prompts": {}
        },
        "serverInfo": {
            "name": "app-mediakit-knowledge",
            "version": env!("CARGO_PKG_VERSION")
        }
    }))
}

// ─── tools/list ─────────────────────────────────────────────────────────────

fn tools_list() -> Result<Value, (i32, String)> {
    Ok(json!({ "tools": [
        {
            "name": "create_topic",
            "description": "Propose a new wiki article. Per SYS-ADR-10, proposals are not auto-committed — the operator must press F12 to persist.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "slug":     { "type": "string" },
                    "title":    { "type": "string" },
                    "category": { "type": "string" },
                    "body":     { "type": "string", "description": "Article body in Markdown" }
                },
                "required": ["slug", "title", "category", "body"]
            }
        },
        {
            "name": "propose_edit",
            "description": "Propose an edit to an existing article body. Per SYS-ADR-10, proposals are not auto-committed — the operator must press F12 to persist.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "slug":    { "type": "string" },
                    "body":    { "type": "string", "description": "Full replacement body in Markdown" },
                    "summary": { "type": "string", "description": "Edit summary (optional)" }
                },
                "required": ["slug", "body"]
            }
        },
        {
            "name": "link_citation",
            "description": "Search the workspace citation registry by ID or keyword. Returns matching citation entries.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Citation ID (e.g. 'ni-51-102') or keyword" }
                },
                "required": ["query"]
            }
        }
    ]}))
}

// ─── tools/call ─────────────────────────────────────────────────────────────

async fn tools_call(state: &AppState, params: &Value) -> Result<Value, (i32, String)> {
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| (-32602i32, "missing param: name".to_string()))?;
    let args = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| Value::Object(serde_json::Map::new()));

    let text = match name {
        "create_topic" => tool_create_topic(&args)?,
        "propose_edit" => tool_propose_edit(&args)?,
        "link_citation" => tool_link_citation(state, &args).await?,
        _ => return Err((-32601, format!("unknown tool: {name}"))),
    };

    Ok(json!({ "content": [{ "type": "text", "text": text }], "isError": false }))
}

fn tool_create_topic(args: &Value) -> Result<String, (i32, String)> {
    let slug = args
        .get("slug")
        .and_then(|v| v.as_str())
        .ok_or_else(|| (-32602i32, "missing param: slug".to_string()))?;
    Ok(format!(
        "Proposed topic '{}' recorded. Requires operator F12 commit (SYS-ADR-10) before persisting.",
        slug
    ))
}

fn tool_propose_edit(args: &Value) -> Result<String, (i32, String)> {
    let slug = args
        .get("slug")
        .and_then(|v| v.as_str())
        .ok_or_else(|| (-32602i32, "missing param: slug".to_string()))?;
    Ok(format!(
        "Edit proposal for '{}' recorded. Requires operator F12 commit (SYS-ADR-10) before persisting.",
        slug
    ))
}

async fn tool_link_citation(state: &AppState, args: &Value) -> Result<String, (i32, String)> {
    let query = args
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or_else(|| (-32602i32, "missing param: query".to_string()))?;
    let entries = crate::citations::load_registry(&state.citations_yaml)
        .await
        .map_err(|e| (-32000i32, format!("citations error: {e}")))?;
    let q_lower = query.to_lowercase();
    let matches: Vec<Value> = entries
        .iter()
        .filter(|c| {
            c.id.to_lowercase().contains(&q_lower) || c.title.to_lowercase().contains(&q_lower)
        })
        .take(10)
        .map(|c| json!({ "id": c.id, "title": c.title, "url": c.url }))
        .collect();
    Ok(serde_json::to_string_pretty(
        &json!({ "query": query, "count": matches.len(), "matches": matches }),
    )
    .unwrap())
}

// ─── resources/list ─────────────────────────────────────────────────────────

async fn resources_list(state: &AppState) -> Result<Value, (i32, String)> {
    let topic_files =
        crate::server::collect_all_topic_files(state.primary_path(), &state.guide_dirs_arr())
            .await
            .map_err(|e| (-32000i32, format!("io error: {e}")))?;

    let resources: Vec<Value> = topic_files
        .iter()
        .take(500)
        .map(|tf| {
            json!({
                "uri": format!("wiki://topic/{}", tf.slug),
                "name": tf.slug.clone(),
                "mimeType": "text/markdown"
            })
        })
        .collect();
    Ok(json!({ "resources": resources }))
}

// ─── resources/read ─────────────────────────────────────────────────────────

async fn resources_read(state: &AppState, params: &Value) -> Result<Value, (i32, String)> {
    let uri = params
        .get("uri")
        .and_then(|v| v.as_str())
        .ok_or_else(|| (-32602i32, "missing param: uri".to_string()))?;
    let slug = uri
        .strip_prefix("wiki://topic/")
        .ok_or_else(|| (-32602i32, format!("unsupported URI scheme: {uri}")))?;
    if slug.contains("..") {
        return Err((-32602, "invalid slug".to_string()));
    }
    let topic_files =
        crate::server::collect_all_topic_files(state.primary_path(), &state.guide_dirs_arr())
            .await
            .map_err(|e| (-32000i32, format!("io error: {e}")))?;
    let tf = topic_files
        .into_iter()
        .find(|tf| tf.slug == slug)
        .ok_or_else(|| (-32000i32, format!("resource not found: {uri}")))?;
    let text = tokio::fs::read_to_string(&tf.path)
        .await
        .map_err(|e| (-32000i32, format!("read error: {e}")))?;
    Ok(json!({
        "contents": [{ "uri": uri, "mimeType": "text/markdown", "text": text }]
    }))
}

// ─── prompts/list ───────────────────────────────────────────────────────────

fn prompts_list() -> Result<Value, (i32, String)> {
    Ok(json!({ "prompts": [
        {
            "name": "cite-this-page",
            "description": "Generate a formatted citation for a wiki article.",
            "arguments": [
                { "name": "slug", "description": "Article slug", "required": true }
            ]
        },
        {
            "name": "summarize-topic",
            "description": "Write a concise 2–3 sentence summary of an article.",
            "arguments": [
                { "name": "slug", "description": "Article slug", "required": true }
            ]
        },
        {
            "name": "draft-related-topic",
            "description": "Draft a new article related to an existing one.",
            "arguments": [
                { "name": "slug",      "description": "Existing article slug to base the draft on", "required": true },
                { "name": "new_title", "description": "Title for the new article",                  "required": true }
            ]
        }
    ]}))
}

// ─── prompts/get ────────────────────────────────────────────────────────────

fn prompts_get(params: &Value) -> Result<Value, (i32, String)> {
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| (-32602i32, "missing param: name".to_string()))?;
    let arg = |key: &str| -> &str {
        params
            .get("arguments")
            .and_then(|a| a.get(key))
            .and_then(|v| v.as_str())
            .unwrap_or("{slug}")
    };
    match name {
        "cite-this-page" => {
            let slug = arg("slug");
            Ok(json!({
                "description": "Citation generator for a wiki article",
                "messages": [{
                    "role": "user",
                    "content": {
                        "type": "text",
                        "text": format!(
                            "Generate a formatted academic citation for the wiki article \
                             at slug: {slug}. Include title, publisher (PointSav Digital \
                             Systems), URL, and access date."
                        )
                    }
                }]
            }))
        }
        "summarize-topic" => {
            let slug = arg("slug");
            Ok(json!({
                "description": "Article summarizer",
                "messages": [{
                    "role": "user",
                    "content": {
                        "type": "text",
                        "text": format!(
                            "Read the wiki article '{slug}' and write a concise 2–3 \
                             sentence summary suitable for its lead paragraph."
                        )
                    }
                }]
            }))
        }
        "draft-related-topic" => {
            let slug = arg("slug");
            let new_title = params
                .get("arguments")
                .and_then(|a| a.get("new_title"))
                .and_then(|v| v.as_str())
                .unwrap_or("New Article");
            Ok(json!({
                "description": "Related topic drafter",
                "messages": [{
                    "role": "user",
                    "content": {
                        "type": "text",
                        "text": format!(
                            "Based on the wiki article '{slug}', draft a new article \
                             titled '{new_title}'. Follow the same frontmatter schema \
                             and maintain the same encyclopedic tone."
                        )
                    }
                }]
            }))
        }
        _ => Err((-32601, format!("unknown prompt: {name}"))),
    }
}

// ─── query_page ──────────────────────────────────────────────────────────────

/// `query_page` — fetch a single article by slug.
///
/// params: `{ slug: String }`
/// returns: `{ slug, title, category, status, summary, last_edited, html_body, relates_to, backlinks }`
async fn query_page(state: &AppState, params: &Value) -> Result<Value, (i32, String)> {
    let slug = params
        .get("slug")
        .and_then(|v| v.as_str())
        .ok_or_else(|| (-32602i32, "missing param: slug".to_string()))?;
    if slug.contains("..") {
        return Err((-32602, "invalid slug".to_string()));
    }

    let topic_files =
        crate::server::collect_all_topic_files(state.primary_path(), &state.guide_dirs_arr())
            .await
            .map_err(|e| (-32000i32, format!("io error: {e}")))?;

    let tf = topic_files
        .into_iter()
        .find(|tf| tf.slug == slug)
        .ok_or_else(|| (-32000i32, format!("page not found: {slug}")))?;

    let text = tokio::fs::read_to_string(&tf.path)
        .await
        .map_err(|e| (-32000i32, format!("read error: {e}")))?;

    let parsed =
        crate::render::parse_page(&text).map_err(|e| (-32000i32, format!("parse error: {e}")))?;

    let title = parsed
        .frontmatter
        .title
        .clone()
        .unwrap_or_else(|| slug.to_string());
    let category = parsed.frontmatter.category.clone().unwrap_or_default();
    let status = parsed
        .frontmatter
        .status
        .clone()
        .unwrap_or_else(|| "unknown".to_string());
    let summary = crate::feeds::first_paragraph_snippet(&parsed.body_md, 300);
    let last_edited = parsed.frontmatter.last_edited.clone().unwrap_or_default();
    let relates_to: Vec<String> = parsed
        .frontmatter
        .extra
        .get("relates_to")
        .and_then(|v| v.as_sequence())
        .map(|seq| {
            seq.iter()
                .filter_map(|v| v.as_str())
                .map(|s| s.to_string())
                .collect()
        })
        .unwrap_or_default();
    let html_body =
        crate::render::render_html_raw(&parsed.body_md, state.primary_path(), &state.link_roots());
    let backlinks = state.links.backlinks(slug).unwrap_or_default();

    Ok(json!({
        "slug": slug,
        "title": title,
        "category": category,
        "status": status,
        "summary": summary,
        "last_edited": last_edited,
        "html_body": html_body,
        "relates_to": relates_to,
        "backlinks": backlinks,
    }))
}

// ─── search ──────────────────────────────────────────────────────────────────

/// `search` — full-text BM25 search over the wiki index.
///
/// params: `{ query: String, limit?: usize }`
/// returns: `{ results: [{ slug, title, category, lede, score }] }`
async fn mcp_search(state: &AppState, params: &Value) -> Result<Value, (i32, String)> {
    let query = params
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or_else(|| (-32602i32, "missing param: query".to_string()))?;
    let limit = params
        .get("limit")
        .and_then(|v| v.as_u64())
        .map(|n| n as usize)
        .unwrap_or(10);

    let hits = crate::search::search(&state.search, query, limit)
        .map_err(|e| (-32000i32, format!("search error: {e}")))?;

    let results: Vec<Value> = hits
        .into_iter()
        .map(|h| {
            json!({
                "slug":  h.slug,
                "title": h.title,
                "lede":  h.snippet,
                "score": h.score,
            })
        })
        .collect();

    Ok(json!({ "results": results }))
}

// ─── list_pages ───────────────────────────────────────────────────────────────

/// `list_pages` — enumerate articles with optional category / status filter.
///
/// params: `{ category?: String, status?: String, limit?: usize }`
/// returns: `{ articles: [{ slug, title, category, status, last_edited }] }`
async fn list_pages(state: &AppState, params: &Value) -> Result<Value, (i32, String)> {
    let filter_category = params
        .get("category")
        .and_then(|v| v.as_str())
        .map(|s| s.to_lowercase());
    let filter_status = params
        .get("status")
        .and_then(|v| v.as_str())
        .map(|s| s.to_lowercase());
    let limit = params
        .get("limit")
        .and_then(|v| v.as_u64())
        .map(|n| n as usize)
        .unwrap_or(100);

    let topic_files =
        crate::server::collect_all_topic_files(state.primary_path(), &state.guide_dirs_arr())
            .await
            .map_err(|e| (-32000i32, format!("io error: {e}")))?;

    let mut articles = Vec::new();
    for tf in topic_files {
        if articles.len() >= limit {
            break;
        }
        let text = match tokio::fs::read_to_string(&tf.path).await {
            Ok(t) => t,
            Err(_) => continue,
        };
        let parsed = match crate::render::parse_page(&text) {
            Ok(p) => p,
            Err(_) => continue,
        };
        let category = parsed.frontmatter.category.clone().unwrap_or_default();
        let status = parsed
            .frontmatter
            .status
            .clone()
            .unwrap_or_else(|| "unknown".to_string());

        if let Some(ref fc) = filter_category {
            if !category.to_lowercase().contains(fc.as_str()) {
                continue;
            }
        }
        if let Some(ref fs) = filter_status {
            if status.to_lowercase() != fs.as_str() {
                continue;
            }
        }
        let title = parsed
            .frontmatter
            .title
            .clone()
            .unwrap_or_else(|| tf.slug.clone());
        let last_edited = parsed.frontmatter.last_edited.clone().unwrap_or_default();
        articles.push(json!({
            "slug":       tf.slug,
            "title":      title,
            "category":   category,
            "status":     status,
            "last_edited": last_edited,
        }));
    }

    Ok(json!({ "articles": articles }))
}

// ─── get_links ───────────────────────────────────────────────────────────────

/// `get_links` — return forward or backward wikilinks for a slug.
///
/// params: `{ slug: String, direction: "forward" | "backward" }`
/// returns: `{ slug, links: [String] }`
async fn get_links(state: &AppState, params: &Value) -> Result<Value, (i32, String)> {
    let slug = params
        .get("slug")
        .and_then(|v| v.as_str())
        .ok_or_else(|| (-32602i32, "missing param: slug".to_string()))?;
    if slug.contains("..") {
        return Err((-32602, "invalid slug".to_string()));
    }
    let direction = params
        .get("direction")
        .and_then(|v| v.as_str())
        .unwrap_or("backward");

    let links: Vec<String> = match direction {
        "forward" => {
            // Forward links: read outlinks stored in the link graph for this slug.
            // The link graph stores keys as "from\x00to" — scan the prefix.
            state.links.forward_links(slug).unwrap_or_default()
        }
        _ => {
            // backward (default)
            state.links.backlinks(slug).unwrap_or_default()
        }
    };

    Ok(json!({ "slug": slug, "links": links }))
}

// ─── get_citations ────────────────────────────────────────────────────────────

/// `get_citations` — look up citation registry entries by ID.
///
/// params: `{ ids: [String] }`
/// returns: `{ citations: [{ id, title, authors, year, url }] }`
async fn get_citations(state: &AppState, params: &Value) -> Result<Value, (i32, String)> {
    let ids: Vec<String> = params
        .get("ids")
        .and_then(|v| v.as_array())
        .ok_or_else(|| (-32602i32, "missing param: ids (array)".to_string()))?
        .iter()
        .filter_map(|v| v.as_str().map(|s| s.to_string()))
        .collect();

    let registry = crate::citations::load_registry(&state.citations_yaml)
        .await
        .map_err(|e| (-32000i32, format!("citations error: {e}")))?;

    let id_set: std::collections::HashSet<&str> = ids.iter().map(|s| s.as_str()).collect();
    let citations: Vec<Value> = registry
        .iter()
        .filter(|c| id_set.contains(c.id.as_str()))
        .map(|c| {
            json!({
                "id":      c.id,
                "title":   c.title,
                "url":     c.url,
                "publisher": c.publisher,
                "year":    c.extra.get("year").and_then(|v| v.as_str()),
            })
        })
        .collect();

    Ok(json!({ "citations": citations }))
}

// ─── query_claims ────────────────────────────────────────────────────────────

/// Phase 11 — `query_claims(topic, asof)`.
/// Returns all citation/claim records for the given article slug.
/// `asof` is a reserved ISO 8601 date string; date-filtering is planned for
/// Phase 11 full implementation once the citations table carries timestamps.
fn query_claims(state: &AppState, params: &Value) -> Result<Value, (i32, String)> {
    let topic = params
        .get("topic")
        .and_then(|v| v.as_str())
        .ok_or_else(|| (-32602i32, "missing param: topic".to_string()))?;
    if topic.contains("..") {
        return Err((-32602, "invalid slug".to_string()));
    }
    let asof = params.get("asof").and_then(|v| v.as_str());
    let records = state.links.citations_for_slug(topic, asof);
    let claims: Vec<Value> = records
        .into_iter()
        .filter_map(|(cite_id, blob)| {
            let parsed: serde_json::Value = serde_json::from_str(&blob).ok()?;
            Some(json!({
                "claim_id":      cite_id,
                "status":        parsed.get("status").and_then(|v| v.as_str()).unwrap_or("unknown"),
                "cite_url":      parsed.get("url").and_then(|v| v.as_str()).unwrap_or(""),
                "cite_title":    parsed.get("title").and_then(|v| v.as_str()).unwrap_or(""),
                "last_verified": null
            }))
        })
        .collect();
    Ok(json!({ "claims": claims, "topic": topic, "asof": asof }))
}

// ─── knowledge/search (Phase 7 federation) ───────────────────────────────────

/// `knowledge/search` — federated full-text search.
///
/// Calls local BM25 search and fans out to all configured `[[peer]]` instances.
/// Results are merged and deduplicated by slug; each result carries an `instance`
/// label identifying its source.
///
/// params: `{ query: String, limit?: usize }`
/// returns: `{ results: [{ slug, title, lede, score, instance }] }`
pub(crate) async fn federation_search(
    state: &AppState,
    params: &Value,
) -> Result<Value, (i32, String)> {
    let query = params
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or_else(|| (-32602i32, "missing param: query".to_string()))?;
    let limit = params
        .get("limit")
        .and_then(|v| v.as_u64())
        .map(|n| n as usize)
        .unwrap_or(10);

    // Local results labelled by site_title.
    let local_hits = crate::search::search(&state.search, query, limit)
        .map_err(|e| (-32000i32, format!("local search error: {e}")))?;
    let local_label = state.site_title.clone();
    let mut results: Vec<Value> = local_hits
        .into_iter()
        .map(|h| {
            json!({
                "slug":     h.slug,
                "title":    h.title,
                "lede":     h.snippet,
                "score":    h.score,
                "instance": local_label,
            })
        })
        .collect();

    // Fan out to peer instances. Each peer is queried independently; failures
    // are logged and skipped so local results are always returned.
    let client = reqwest::Client::new();
    let peer_request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "search",
        "params": { "query": query, "limit": limit }
    });
    let mut seen_slugs: std::collections::HashSet<String> = results
        .iter()
        .filter_map(|r| r["slug"].as_str().map(|s| s.to_string()))
        .collect();

    for peer in &state.peers {
        let url = format!("{}/mcp", peer.url);
        let label = peer.label.clone();
        match client
            .post(&url)
            .json(&peer_request)
            .timeout(std::time::Duration::from_secs(3))
            .send()
            .await
        {
            Ok(resp) => match resp.json::<Value>().await {
                Ok(body) => {
                    if let Some(peer_results) = body
                        .get("result")
                        .and_then(|r| r.get("results"))
                        .and_then(|r| r.as_array())
                    {
                        for hit in peer_results {
                            let slug = hit["slug"].as_str().unwrap_or("").to_string();
                            if !slug.is_empty() && seen_slugs.insert(slug.clone()) {
                                results.push(json!({
                                    "slug":     slug,
                                    "title":    hit["title"],
                                    "lede":     hit["lede"],
                                    "score":    hit["score"],
                                    "instance": label,
                                }));
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(peer = %url, err = %e, "federation_search: JSON parse error");
                }
            },
            Err(e) => {
                tracing::warn!(peer = %url, err = %e, "federation_search: request error");
            }
        }
    }

    // Sort by score descending, then truncate to limit.
    results.sort_by(|a, b| {
        let sa = a["score"].as_f64().unwrap_or(0.0);
        let sb = b["score"].as_f64().unwrap_or(0.0);
        sb.partial_cmp(&sa).unwrap_or(std::cmp::Ordering::Equal)
    });
    results.truncate(limit);

    Ok(json!({ "results": results }))
}
