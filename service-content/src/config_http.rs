use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use std::sync::Arc;

use crate::http::HttpState;
use crate::taxonomy::{
    archetypes_to_entities, coa_to_entities, domains_to_entities, glossary_to_entities,
    guides_to_entities, parse_archetypes, parse_coa, parse_domain, parse_glossary, parse_guides,
    parse_themes, parse_topics, serialize_domains, themes_to_entities, topics_to_entities,
};

// ── CSV response helper ───────────────────────────────────────────────────────

fn csv_response(body: String) -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/csv; charset=utf-8")],
        body,
    )
        .into_response()
}

fn err500(msg: impl std::fmt::Display) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, msg.to_string())
}

fn err422(msg: impl std::fmt::Display) -> (StatusCode, String) {
    (StatusCode::UNPROCESSABLE_ENTITY, msg.to_string())
}

fn err404(name: &str) -> (StatusCode, String) {
    (
        StatusCode::NOT_FOUND,
        format!("no {name} taxonomy file found"),
    )
}

// ── GET handlers ─────────────────────────────────────────────────────────────

async fn get_archetypes(
    State(state): State<Arc<HttpState>>,
) -> Result<Response, (StatusCode, String)> {
    let path = format!("{}/archetypes.csv", state.ontology_dir);
    let csv = std::fs::read_to_string(&path).map_err(|_| err404("archetypes"))?;
    Ok(csv_response(csv))
}

async fn get_coa(State(state): State<Arc<HttpState>>) -> Result<Response, (StatusCode, String)> {
    let path = format!("{}/chart_of_accounts.csv", state.ontology_dir);
    let csv = std::fs::read_to_string(&path).map_err(|_| err404("chart_of_accounts"))?;
    Ok(csv_response(csv))
}

async fn get_domains(
    State(state): State<Arc<HttpState>>,
) -> Result<Response, (StatusCode, String)> {
    let mut all = Vec::new();
    for domain in &["corporate", "documentation", "projects"] {
        let path = format!("{}/domains/domain_{}.csv", state.ontology_dir, domain);
        if let Ok(csv) = std::fs::read_to_string(&path) {
            let rows = parse_domain(&csv).map_err(err422)?;
            all.extend(rows);
        }
    }
    Ok(csv_response(serialize_domains(&all)))
}

async fn get_glossary(
    State(state): State<Arc<HttpState>>,
    Path(domain): Path<String>,
) -> Result<Response, (StatusCode, String)> {
    let valid = ["corporate", "documentation", "projects"];
    if !valid.contains(&domain.as_str()) {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("domain must be one of: {}", valid.join(", ")),
        ));
    }
    let path = format!("{}/glossary/glossary_{}.csv", state.ontology_dir, domain);
    let csv = std::fs::read_to_string(&path).map_err(|_| err404(&format!("glossary/{domain}")))?;
    Ok(csv_response(csv))
}

async fn get_themes(State(state): State<Arc<HttpState>>) -> Result<Response, (StatusCode, String)> {
    let path = format!("{}/themes.csv", state.ontology_dir);
    let csv = std::fs::read_to_string(&path).map_err(|_| err404("themes"))?;
    Ok(csv_response(csv))
}

async fn get_topics(
    State(state): State<Arc<HttpState>>,
    Path(domain): Path<String>,
) -> Result<Response, (StatusCode, String)> {
    let valid = ["corporate", "documentation", "projects"];
    if !valid.contains(&domain.as_str()) {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("domain must be one of: {}", valid.join(", ")),
        ));
    }
    let path = format!("{}/topics/topics_{}.csv", state.ontology_dir, domain);
    let csv = std::fs::read_to_string(&path).map_err(|_| err404(&format!("topics/{domain}")))?;
    Ok(csv_response(csv))
}

// ── POST handlers ─────────────────────────────────────────────────────────────

async fn post_archetypes(
    State(state): State<Arc<HttpState>>,
    body: String,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let rows = parse_archetypes(&body).map_err(err422)?;
    let entities = archetypes_to_entities(&rows);
    state
        .graph
        .delete_by_classification("__taxonomy__", "archetype")
        .map_err(err500)?;
    let count = state
        .graph
        .upsert_entities("__taxonomy__", &entities)
        .map_err(err500)?;
    Ok(Json(
        serde_json::json!({"loaded": count, "classification": "archetype"}),
    ))
}

async fn post_coa(
    State(state): State<Arc<HttpState>>,
    body: String,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let rows = parse_coa(&body).map_err(err422)?;
    let entities = coa_to_entities(&rows);
    state
        .graph
        .delete_by_classification("__taxonomy__", "coa-profile")
        .map_err(err500)?;
    let count = state
        .graph
        .upsert_entities("__taxonomy__", &entities)
        .map_err(err500)?;
    Ok(Json(
        serde_json::json!({"loaded": count, "classification": "coa-profile"}),
    ))
}

async fn post_domains(
    State(state): State<Arc<HttpState>>,
    body: String,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let rows = parse_domain(&body).map_err(err422)?;
    let entities = domains_to_entities(&rows);
    state
        .graph
        .delete_by_classification("__taxonomy__", "domain")
        .map_err(err500)?;
    let count = state
        .graph
        .upsert_entities("__taxonomy__", &entities)
        .map_err(err500)?;
    Ok(Json(
        serde_json::json!({"loaded": count, "classification": "domain"}),
    ))
}

async fn post_glossary(
    State(state): State<Arc<HttpState>>,
    Path(domain): Path<String>,
    body: String,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let valid = ["corporate", "documentation", "projects"];
    if !valid.contains(&domain.as_str()) {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("domain must be one of: {}", valid.join(", ")),
        ));
    }
    let rows = parse_glossary(&body).map_err(err422)?;
    let entities = glossary_to_entities(&rows);
    let classification = format!("glossary-{}", domain);
    state
        .graph
        .delete_by_classification("__taxonomy__", &classification)
        .map_err(err500)?;
    let count = state
        .graph
        .upsert_entities("__taxonomy__", &entities)
        .map_err(err500)?;
    Ok(Json(
        serde_json::json!({"loaded": count, "classification": classification}),
    ))
}

async fn post_themes(
    State(state): State<Arc<HttpState>>,
    body: String,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let rows = parse_themes(&body).map_err(err422)?;
    let entities = themes_to_entities(&rows);
    state
        .graph
        .delete_by_classification("__taxonomy__", "theme")
        .map_err(err500)?;
    let count = state
        .graph
        .upsert_entities("__taxonomy__", &entities)
        .map_err(err500)?;
    Ok(Json(
        serde_json::json!({"loaded": count, "classification": "theme"}),
    ))
}

async fn post_topics(
    State(state): State<Arc<HttpState>>,
    Path(domain): Path<String>,
    body: String,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let valid = ["corporate", "documentation", "projects"];
    if !valid.contains(&domain.as_str()) {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("domain must be one of: {}", valid.join(", ")),
        ));
    }
    let rows = parse_topics(&body).map_err(err422)?;
    let entities = topics_to_entities(&rows);
    state
        .graph
        .delete_by_classification_and_location("__taxonomy__", "topic", &domain)
        .map_err(err500)?;
    let count = state
        .graph
        .upsert_entities("__taxonomy__", &entities)
        .map_err(err500)?;
    Ok(Json(
        serde_json::json!({"loaded": count, "classification": "topic", "domain": domain}),
    ))
}

// ── Guides ────────────────────────────────────────────────────────────────────

async fn get_guides(State(state): State<Arc<HttpState>>) -> Result<Response, (StatusCode, String)> {
    let path = format!("{}/guides/guides_documentation.csv", state.ontology_dir);
    let csv = std::fs::read_to_string(&path).map_err(|_| err404("guides"))?;
    Ok(csv_response(csv))
}

async fn post_guides(
    State(state): State<Arc<HttpState>>,
    body: String,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let rows = parse_guides(&body).map_err(err422)?;
    let entities = guides_to_entities(&rows);
    state
        .graph
        .delete_by_classification("__taxonomy__", "guide")
        .map_err(err500)?;
    let count = state
        .graph
        .upsert_entities("__taxonomy__", &entities)
        .map_err(err500)?;
    Ok(Json(
        serde_json::json!({"loaded": count, "classification": "guide"}),
    ))
}

// Reload all guides from the on-disk CSV and re-seed into the graph.
// Used by graph-cleanup.sh as the canonical no-restart reload path.
async fn reload_guides(
    State(state): State<Arc<HttpState>>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let path = format!("{}/guides/guides_documentation.csv", state.ontology_dir);
    let csv = std::fs::read_to_string(&path).map_err(|_| err404("guides"))?;
    // Pass full CSV including header row — csv ReaderBuilder has has_headers=true by default,
    // which consumes the header row and iterates only data rows via .records().
    let rows = parse_guides(&csv).map_err(err422)?;
    let entities = guides_to_entities(&rows);
    state
        .graph
        .delete_by_classification("__taxonomy__", "guide")
        .map_err(err500)?;
    let count = state
        .graph
        .upsert_entities("__taxonomy__", &entities)
        .map_err(err500)?;
    Ok(Json(
        serde_json::json!({"reloaded": count, "classification": "guide", "source": path}),
    ))
}

// ── Router ────────────────────────────────────────────────────────────────────

pub fn config_routes() -> Router<Arc<HttpState>> {
    Router::new()
        .route(
            "/v1/config/archetypes",
            get(get_archetypes).post(post_archetypes),
        )
        .route("/v1/config/coa", get(get_coa).post(post_coa))
        .route("/v1/config/domains", get(get_domains).post(post_domains))
        .route("/v1/config/themes", get(get_themes).post(post_themes))
        .route(
            "/v1/config/glossary/:domain",
            get(get_glossary).post(post_glossary),
        )
        .route(
            "/v1/config/topics/:domain",
            get(get_topics).post(post_topics),
        )
        .route("/v1/config/guides", get(get_guides).post(post_guides))
        .route(
            "/v1/config/guides/reload",
            axum::routing::post(reload_guides),
        )
}
