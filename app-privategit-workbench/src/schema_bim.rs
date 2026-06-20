/// BIM schema handlers — IFC-SPF files (*.ifc) and BIM JSON manifests (*.bim.json).
///
/// GET /api/bim/files           → list *.ifc + *.bim.json files under configured roots
/// GET /api/bim/parse?path=     → parse IFC-SPF header + instance counts via moonshot-bim-engine
/// GET /api/bim/instances?path=&entity=  → list instances of a given entity type
///
/// SYS-ADR-07: IFC is structured geometric/property data. It is never passed through
/// any AI inference layer. This handler calls the deterministic Rust parser only.
///
/// Note: the app-privategit-bim service (port 9204) is a planned dependency for
/// geometry streaming and glTF cache; this handler covers headless structural inspection
/// using moonshot-bim-engine directly, with no external service dependency.
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use serde::{Deserialize, Serialize};
use std::fs;

use crate::{err, resolve_path, AppState};
use moonshot_bim_engine as bim;

#[derive(Deserialize)]
pub struct BimQuery {
    path: String,
}

#[derive(Deserialize)]
pub struct BimInstanceQuery {
    path: String,
    entity: String,
}

/// GET /api/bim/files
/// Lists *.ifc and *.bim.json files from all configured roots.
pub async fn list_files(State(state): State<AppState>) -> Response {
    let mut files: Vec<serde_json::Value> = Vec::new();
    for root in state.roots.iter() {
        collect_bim(&root.fs_path, &root.url_prefix, &mut files);
    }
    Json(serde_json::json!({ "files": files })).into_response()
}

/// GET /api/bim/parse?path=
/// Parses the IFC-SPF header and returns entity-type counts.
/// Does not stream geometry — that is a planned app-privategit-bim service concern.
pub async fn parse_file(State(state): State<AppState>, Query(q): Query<BimQuery>) -> Response {
    let (fs_path, _) = match resolve_path(&state.roots, &q.path) {
        Ok(v) => v,
        Err(e) => return err(StatusCode::BAD_REQUEST, e.to_string()),
    };

    if !fs_path.is_file() {
        return err(StatusCode::NOT_FOUND, "file not found");
    }

    let ext = fs_path.extension().and_then(|e| e.to_str()).unwrap_or("");

    if ext == "json" {
        // *.bim.json — return as structured metadata pass-through (SYS-ADR-07: no AI)
        let content = match fs::read_to_string(&fs_path) {
            Ok(s) => s,
            Err(e) => return err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        };
        match serde_json::from_str::<serde_json::Value>(&content) {
            Ok(v) => {
                return Json(serde_json::json!({ "format": "bim.json", "data": v })).into_response()
            }
            Err(e) => {
                return err(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    format!("JSON parse error: {}", e),
                )
            }
        }
    }

    // *.ifc — parse via moonshot-bim-engine
    let source = match fs::read_to_string(&fs_path) {
        Ok(s) => s,
        Err(e) => return err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };

    let step = match bim::parse(&source) {
        Ok(s) => s,
        Err(e) => {
            return err(
                StatusCode::UNPROCESSABLE_ENTITY,
                format!("IFC parse error: {:?}", e),
            )
        }
    };

    // Build header metadata from HEADER records
    let mut schema_identifiers: Vec<String> = Vec::new();
    let mut file_description = String::new();
    for rec in &step.header {
        match rec.keyword.as_str() {
            "FILE_SCHEMA" => {
                // params: (('IFC4',))  — extract inner string
                let s = rec
                    .params
                    .trim_matches(|c| c == '(' || c == ')')
                    .trim()
                    .to_string();
                schema_identifiers.push(s);
            }
            "FILE_DESCRIPTION" => {
                file_description = rec.params.clone();
            }
            _ => {}
        }
    }

    // Count entity types
    let mut entity_counts: std::collections::HashMap<&str, usize> =
        std::collections::HashMap::new();
    for inst in &step.data {
        *entity_counts.entry(inst.entity.as_str()).or_insert(0) += 1;
    }
    let mut entity_list: Vec<serde_json::Value> = entity_counts
        .iter()
        .map(|(k, v)| serde_json::json!({"entity": k, "count": v}))
        .collect();
    entity_list.sort_by(|a, b| {
        b["count"]
            .as_u64()
            .unwrap_or(0)
            .cmp(&a["count"].as_u64().unwrap_or(0))
    });

    Json(serde_json::json!({
        "format": "ifc-spf",
        "path": q.path,
        "schema": schema_identifiers,
        "file_description": file_description,
        "total_instances": step.data.len(),
        "header_records": step.header.len(),
        "entity_counts": entity_list,
    }))
    .into_response()
}

/// GET /api/bim/instances?path=&entity=
/// Returns all instances of a given entity type (e.g. IfcWall, IfcSpace).
/// SYS-ADR-07: returns raw structural data only; no AI layer.
pub async fn list_instances(
    State(state): State<AppState>,
    Query(q): Query<BimInstanceQuery>,
) -> Response {
    let (fs_path, _) = match resolve_path(&state.roots, &q.path) {
        Ok(v) => v,
        Err(e) => return err(StatusCode::BAD_REQUEST, e.to_string()),
    };

    if !fs_path.is_file() {
        return err(StatusCode::NOT_FOUND, "file not found");
    }

    let source = match fs::read_to_string(&fs_path) {
        Ok(s) => s,
        Err(e) => return err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };

    let step = match bim::parse(&source) {
        Ok(s) => s,
        Err(e) => {
            return err(
                StatusCode::UNPROCESSABLE_ENTITY,
                format!("IFC parse error: {:?}", e),
            )
        }
    };

    let entity_upper = q.entity.to_uppercase();
    let matches: Vec<serde_json::Value> = step
        .data
        .iter()
        .filter(|inst| inst.entity.eq_ignore_ascii_case(&entity_upper))
        .map(|inst| {
            serde_json::json!({
                "id": inst.id,
                "entity": inst.entity,
                "params": inst.params,
            })
        })
        .collect();

    Json(serde_json::json!({
        "entity": entity_upper,
        "count": matches.len(),
        "instances": matches,
    }))
    .into_response()
}

fn collect_bim(dir: &str, prefix: &str, out: &mut Vec<serde_json::Value>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() {
                let name = p
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();
                let is_ifc = name.ends_with(".ifc");
                let is_bim_json = name.ends_with(".bim.json");
                if is_ifc || is_bim_json {
                    let fmt = if is_ifc { "ifc-spf" } else { "bim.json" };
                    out.push(serde_json::json!({
                        "path": format!("{}/{}", prefix, name),
                        "name": name,
                        "format": fmt,
                    }));
                }
            }
        }
    }
}

#[derive(Deserialize)]
pub struct CreateBody {
    name: String,
}

#[derive(Serialize)]
struct CreateResponse {
    ok: bool,
    path: String,
}

/// POST /api/bim/create  body: {"name": "<workspace>.bim.json"}
/// Creates a blank BIM workspace JSON file in the first writable root.
pub async fn create(
    State(state): State<AppState>,
    axum::Json(body): axum::Json<CreateBody>,
) -> Response {
    let mut name = body.name.trim().to_string();
    if name.is_empty() {
        return err(StatusCode::BAD_REQUEST, "name is required");
    }
    if name.contains('/') || name.contains("..") {
        return err(StatusCode::BAD_REQUEST, "invalid name");
    }
    if !name.ends_with(".bim.json") {
        name.push_str(".bim.json");
    }

    let root = match state.roots.iter().find(|r| r.writable) {
        Some(r) => r,
        None => return err(StatusCode::FORBIDDEN, "no writable root configured"),
    };

    let fs_path = std::path::PathBuf::from(&root.fs_path).join(&name);
    if fs_path.exists() {
        return err(StatusCode::CONFLICT, "file already exists");
    }

    if let Err(e) = fs::write(&fs_path, b"{}") {
        return err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string());
    }

    let path = format!(
        "{}/{}",
        root.url_prefix.trim_end_matches('/'),
        name
    );
    Json(CreateResponse { ok: true, path }).into_response()
}
