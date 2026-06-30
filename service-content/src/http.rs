use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::config_http::config_routes;
use crate::entity_filter;
use crate::graph::{GraphEntity, GraphStore};
use crate::pairing::{NonceCache, PairingKeypair, PairingRecord, PairingStore};

// ── shared server state ───────────────────────────────────────────────────────

pub struct HttpState {
    pub graph: Arc<dyn GraphStore>,
    pub doorman_endpoint: String,
    pub ontology_dir: String,
    pub corpus_dir: String,
    #[allow(dead_code)]
    pub graph_dir: String,
    pub pairing_store: Mutex<PairingStore>,
    pub nonce_cache: NonceCache,
    pub pairing_key: PairingKeypair,
}

// ── pairing request / response ────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct PairRequest {
    /// `<base64url(payload_json)>.<base64url(ed25519_sig)>`
    pub token: String,
    /// Base64url Ed25519 verifying key (32 bytes) of the issuing node.
    pub public_key: String,
    pub node_label: String,
}

#[derive(Debug, Serialize)]
pub struct PairResponse {
    pub status: &'static str,
    pub paired_on: String,
    pub role: String,
    pub archive_scope: Vec<String>,
}

/// Issue a new pairing invite token.
#[derive(Debug, Deserialize)]
pub struct PairTokenQuery {
    pub role: String,
    #[serde(default)]
    pub node_label: String,
    /// Comma-separated archive scope IDs (optional).
    #[serde(default)]
    pub archive_scope: String,
}

#[derive(Debug, Serialize)]
pub struct PairTokenResponse {
    pub token: String,
    pub public_key: String,
}

// ── request / response types ──────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ContextQuery {
    pub q: String,
    pub module_id: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
    /// When >0, follow RelatedTo edges transitively (clamped to 1-4 hops).
    #[serde(default)]
    pub hops: usize,
}

fn default_limit() -> usize {
    20
}

/// Query params for GET /v1/graph/delta
#[derive(Debug, Deserialize)]
pub struct DeltaQuery {
    /// ISO 8601 timestamp — return entities created at or after this instant.
    /// Example: `2026-06-29T00:00:00Z`
    pub since: String,
    pub module_id: String,
    #[serde(default = "default_delta_limit")]
    pub limit: usize,
}

fn default_delta_limit() -> usize {
    500
}

#[derive(Debug, Deserialize)]
pub struct MutateRequest {
    pub module_id: String,
    pub entities: Vec<GraphEntity>,
}

#[derive(Debug, Serialize)]
pub struct MutateResponse {
    pub upserted: usize,
}

#[derive(Debug, Deserialize)]
pub struct DraftRequest {
    pub module_id: String,
    #[serde(default)]
    pub query_hint: String,
    /// Purpose forwarded to Doorman audit_proxy allowlist check.
    /// Must be one of: initial-graph-build, entity-disambiguation,
    /// citation-grounding, editorial-refinement.
    #[serde(default = "default_purpose")]
    pub purpose: String,
    #[serde(default = "default_max_entities")]
    pub max_entities: usize,
}

fn default_purpose() -> String {
    "initial-graph-build".to_string()
}

fn default_max_entities() -> usize {
    20
}

#[derive(Debug, Serialize)]
pub struct DraftResponse {
    pub draft: String,
    pub entity_count: usize,
    pub audit_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub entity_count: usize,
}

fn default_dry_run() -> bool {
    true
}

#[derive(Debug, Deserialize)]
pub struct CleanupQuery {
    pub module_id: String,
    /// When true (default), report what would be deleted without deleting anything.
    /// Must be explicitly set to false to trigger deletions.
    #[serde(default = "default_dry_run")]
    pub dry_run: bool,
}

#[derive(Debug, Serialize)]
pub struct CleanupSample {
    pub entity_name: String,
    pub classification: String,
    pub reason: String,
}

#[derive(Debug, Serialize)]
pub struct CleanupReport {
    pub module_id: String,
    pub scanned: usize,
    pub flagged: usize,
    pub deleted: usize,
    pub dry_run: bool,
    /// Up to 100 flagged entity samples for inspection.
    pub samples: Vec<CleanupSample>,
}

#[derive(Debug, Deserialize)]
pub struct IngestRequest {
    pub text: String,
    #[serde(default)]
    pub module_id: String,
    #[serde(default)]
    pub doc_id: String,
    #[serde(default)]
    pub source_type: String,
}

#[derive(Debug, Serialize)]
pub struct IngestResponse {
    pub doc_id: String,
    pub queued: bool,
}

// ── handlers ──────────────────────────────────────────────────────────────────

async fn healthz(State(state): State<Arc<HttpState>>) -> Json<HealthResponse> {
    let (status, entity_count) = match state.graph.count_all() {
        Ok(n) => ("ok", n),
        Err(_) => ("degraded", 0),
    };
    Json(HealthResponse {
        status,
        entity_count,
    })
}

async fn graph_context(
    State(state): State<Arc<HttpState>>,
    Query(params): Query<ContextQuery>,
) -> Result<Json<Vec<GraphEntity>>, (StatusCode, String)> {
    let result = if params.hops > 0 {
        state.graph.query_context_transitive(
            &params.module_id,
            &params.q,
            params.limit,
            params.hops,
        )
    } else {
        state
            .graph
            .query_context(&params.module_id, &params.q, params.limit)
    };
    result
        .map(Json)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

async fn graph_mutate(
    State(state): State<Arc<HttpState>>,
    Json(body): Json<MutateRequest>,
) -> Result<Json<MutateResponse>, (StatusCode, String)> {
    state
        .graph
        .upsert_entities(&body.module_id, &body.entities)
        .map(|upserted| Json(MutateResponse { upserted }))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

/// Tier C Drafting Pipeline.
///
/// Queries the LadybugDB graph for entities matching the request's module and
/// query_hint, packages them into a ≤2 000-token structured prompt, then
/// proxies the request to Claude 3.5 Sonnet via Doorman /v1/audit/proxy.
///
/// Pre-D4: the Doorman will return 503 (provider unconfigured). The handler
/// surfaces the upstream status and message rather than masking it.
async fn draft_generate(
    State(state): State<Arc<HttpState>>,
    Json(body): Json<DraftRequest>,
) -> Result<Json<DraftResponse>, (StatusCode, String)> {
    // 1. Fetch graph entities.
    let entities = state
        .graph
        .query_context(&body.module_id, &body.query_hint, body.max_entities)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("graph query failed: {e}"),
            )
        })?;

    let entity_count = entities.len();

    // 2. Package entities into a structured prompt (≤2 000 tokens ≈ 8 000 chars).
    let entity_block = format_entity_block(&entities);

    let system_prompt = "You are a knowledge graph analyst for a real estate property \
        management archive. Given a list of extracted entities from the Totebox Archive, \
        produce a concise structured summary document. \
        Include: key people and their roles, active projects, notable locations, \
        and company relationships. Write in precise professional prose. \
        Maximum 600 words.";

    let user_message = format!(
        "Module: {module_id}\nQuery context: {hint}\n\n{entity_block}",
        module_id = body.module_id,
        hint = if body.query_hint.is_empty() {
            "(all entities)"
        } else {
            &body.query_hint
        },
        entity_block = entity_block
    );

    // Truncate to keep under 8 000 chars (~2 000 tokens).
    let user_message = if user_message.len() > 8000 {
        format!(
            "{}\n\n[truncated — {entity_count} entities total]",
            &user_message[..7900]
        )
    } else {
        user_message
    };

    // 3. Build the AuditProxyRequest JSON (matches slm-core wire format).
    let request_id = format!(
        "sc-draft-{}-{}",
        body.module_id,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos()
    );

    let proxy_body = serde_json::json!({
        "provider": "anthropic",
        "model": "anthropic:claude-haiku-4-5-20251001",
        "purpose": body.purpose,
        "caller_request_id": request_id,
        "messages": [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": user_message}
        ],
        "max_tokens": 1024,
        "temperature": 0.3
    });

    // 4. POST to Doorman /v1/audit/proxy.
    let url = format!("{}/v1/audit/proxy", state.doorman_endpoint);
    let client = reqwest::Client::new();
    let res = client
        .post(&url)
        .header("X-Foundry-Module-ID", &body.module_id)
        .header("X-Foundry-Request-ID", &request_id)
        .json(&proxy_body)
        .timeout(Duration::from_secs(120))
        .send()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("Doorman unreachable: {e}")))?;

    let status = res.status();
    let resp_json: serde_json::Value = res
        .json()
        .await
        .unwrap_or_else(|_| serde_json::json!({"error": "invalid JSON from Doorman"}));

    if !status.is_success() {
        let msg = resp_json
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("upstream error")
            .to_string();
        return Err((
            StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY),
            format!("audit_proxy {}: {}", status.as_u16(), msg),
        ));
    }

    // 5. Extract the draft text from the completion.
    let audit_id = resp_json
        .get("audit_id")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    let draft = resp_json
        .pointer("/choices/0/message/content")
        .or_else(|| resp_json.get("content"))
        .and_then(|v| v.as_str())
        .unwrap_or("(no content returned)")
        .to_string();

    Ok(Json(DraftResponse {
        draft,
        entity_count,
        audit_id,
    }))
}

/// `POST /v1/ingest` — submit a document for DataGraph enrichment.
///
/// Writes a `CORPUS_<doc_id>_<ts>.json` file to the watched corpus directory.
/// The file watcher picks it up within milliseconds and routes it through the
/// Tier A → Tier B enrichment cascade, writing extracted entities to LadybugDB.
///
/// Returns 202 Accepted with `{ "doc_id": "...", "queued": true }`.
async fn ingest_document(
    State(state): State<Arc<HttpState>>,
    Json(body): Json<IngestRequest>,
) -> Result<(axum::http::StatusCode, Json<IngestResponse>), (axum::http::StatusCode, String)> {
    if body.text.trim().is_empty() {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            "text must not be empty".to_string(),
        ));
    }

    let doc_id = if body.doc_id.is_empty() {
        format!(
            "{:x}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_nanos()
        )
    } else {
        // Sanitise: allow only alphanumeric + hyphens/underscores
        body.doc_id
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
            .collect()
    };

    let module_id = if body.module_id.is_empty() {
        "woodfine"
    } else {
        body.module_id.as_str()
    };
    let source_type = if body.source_type.is_empty() {
        "ingest-api"
    } else {
        body.source_type.as_str()
    };

    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();

    let worm_id = format!("DOC_{}_{}", doc_id, ts);
    let corpus_json = serde_json::json!({
        "worm_id": worm_id,
        "corpus": body.text,
        "module_id": module_id,
        "source_type": source_type,
    });

    let filename = format!("CORPUS_{}_{}.json", doc_id, ts);
    let path = std::path::Path::new(&state.corpus_dir).join(&filename);

    tokio::fs::write(&path, corpus_json.to_string())
        .await
        .map_err(|e| {
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to write corpus file: {e}"),
            )
        })?;

    Ok((
        axum::http::StatusCode::ACCEPTED,
        Json(IngestResponse {
            doc_id,
            queued: true,
        }),
    ))
}

/// `GET /v1/graph/cleanup` — scan and optionally delete noise entities.
///
/// Default is dry_run=true (safe). Pass dry_run=false to apply deletions.
/// Example: curl 'http://127.0.0.1:9081/v1/graph/cleanup?module_id=jennifer&dry_run=true'
/// GET /v1/graph/delta — federation delta sync.
/// Returns entities created at or after `?since=<ISO-8601>` for a given `module_id`.
/// Consumers (app-orchestration-graph) call this to pull incremental updates.
async fn graph_delta(
    State(state): State<Arc<HttpState>>,
    Query(params): Query<DeltaQuery>,
) -> Result<Json<Vec<GraphEntity>>, (StatusCode, String)> {
    let entities = state
        .graph
        .query_entities_since(&params.module_id, &params.since, params.limit)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(entities))
}

///
/// Uses the same noise filters as the ingest gate so cleanup matches exactly what
/// the hardened binary would have rejected on ingestion.
async fn graph_cleanup(
    State(state): State<Arc<HttpState>>,
    Query(params): Query<CleanupQuery>,
) -> Result<Json<CleanupReport>, (StatusCode, String)> {
    let entities = state.graph.list_entities(&params.module_id).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("list_entities failed: {e}"),
        )
    })?;

    let scanned = entities.len();
    let mut flagged = 0usize;
    let mut deleted = 0usize;
    let mut samples: Vec<CleanupSample> = Vec::new();

    for entity in &entities {
        let reason: Option<&'static str> =
            if entity_filter::is_noise_entity_name(&entity.entity_name) {
                Some("noise-name")
            } else if entity.entity_name.split_whitespace().count() > 8 {
                Some("fragment-wordcount")
            } else if entity_filter::coerce_classification(
                &entity.entity_name,
                &entity.classification,
            )
            .is_none()
            {
                Some("type-incoherent")
            } else if !entity_filter::ALLOWED_CLASSIFICATIONS
                .contains(&entity.classification.as_str())
            {
                Some("oov-classification")
            } else {
                None
            };

        if let Some(r) = reason {
            flagged += 1;
            if samples.len() < 100 {
                samples.push(CleanupSample {
                    entity_name: entity.entity_name.clone(),
                    classification: entity.classification.clone(),
                    reason: r.to_string(),
                });
            }
            if !params.dry_run {
                match state
                    .graph
                    .delete_entity(&params.module_id, &entity.entity_name)
                {
                    Ok(()) => deleted += 1,
                    Err(e) => eprintln!(
                        "[CLEANUP] delete_entity failed for '{}': {e}",
                        entity.entity_name
                    ),
                }
            }
        }
    }

    Ok(Json(CleanupReport {
        module_id: params.module_id,
        scanned,
        flagged,
        deleted,
        dry_run: params.dry_run,
        samples,
    }))
}

// ── enrichment endpoint ───────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct EnrichQuery {
    pub module_id: String,
    /// Only enrich entities of this classification (default: Person).
    #[serde(default = "default_enrich_classification")]
    pub classification: String,
    /// Max entities to enrich per call (default: 10). Gate: total ≤ 50.
    #[serde(default = "default_enrich_limit")]
    pub limit: usize,
    /// When true (default), report which entities WOULD be enriched without calling Tier A.
    #[serde(default = "default_dry_run")]
    pub dry_run: bool,
}

fn default_enrich_classification() -> String {
    "Person".to_string()
}

fn default_enrich_limit() -> usize {
    10
}

#[derive(Debug, Serialize)]
pub struct EnrichResponse {
    pub module_id: String,
    pub classification: String,
    pub null_vector_count: usize,
    pub enriched: usize,
    pub unchanged: usize,
    pub dry_run: bool,
    pub samples: Vec<String>,
}

/// `POST /v1/graph/enrich` — retroactively fill NULL role_vector entries via Tier A inference.
///
/// Queries LadybugDB for Person entities (or the requested classification) whose
/// `role_vector` is NULL, then asks Tier A (local OLMo) "what is X's role?" for
/// each entity and writes back any non-empty answer.
///
/// Default: dry_run=true — reports how many entities would be enriched without
/// calling Tier A. Set dry_run=false to trigger live enrichment.
///
/// Note (I4, 2026-06-27): if the source corpus contains only technical text
/// (code diffs, commit messages), Tier A will answer "unknown" for most entities.
/// The real unlock is source diversification — routing CRM/people data through
/// the extraction pipeline. This endpoint provides the infrastructure for when
/// richer sources become available.
///
/// Example: curl -X POST 'http://127.0.0.1:9081/v1/graph/enrich?module_id=jennifer&dry_run=false&limit=5'
async fn graph_enrich(
    State(state): State<Arc<HttpState>>,
    Query(params): Query<EnrichQuery>,
) -> Result<Json<EnrichResponse>, (StatusCode, String)> {
    let limit = params.limit.min(50);

    // Find all entities for this module; filter NULL role_vector + matching classification.
    let all = state.graph.list_entities(&params.module_id).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("list_entities: {e}"),
        )
    })?;

    let candidates: Vec<_> = all
        .into_iter()
        .filter(|e| e.role_vector.is_none() && e.classification == params.classification)
        .take(limit)
        .collect();

    let null_vector_count = candidates.len();
    let mut samples: Vec<String> = candidates.iter().map(|e| e.entity_name.clone()).collect();
    samples.truncate(20);

    if params.dry_run || candidates.is_empty() {
        return Ok(Json(EnrichResponse {
            module_id: params.module_id,
            classification: params.classification,
            null_vector_count,
            enriched: 0,
            unchanged: 0,
            dry_run: true,
            samples,
        }));
    }

    // Live enrichment: ask Tier A for each entity's role.
    let client = reqwest::Client::new();
    let chat_url = format!("{}/v1/chat/completions", state.doorman_endpoint);
    let mut enriched = 0usize;
    let mut unchanged = 0usize;

    for entity in candidates {
        let prompt = format!(
            "What is the professional role or job title of {}? \
             Answer in 3-7 words (e.g. \"Senior Software Engineer\" or \"CEO, Woodfine Management Corp.\"). \
             If unknown, answer exactly: unknown",
            entity.entity_name
        );
        let body = serde_json::json!({
            "model": "local",
            "messages": [{"role": "user", "content": prompt}],
            "max_tokens": 30,
            "temperature": 0.0
        });
        let res = client
            .post(&chat_url)
            .header("X-Foundry-Module-ID", &params.module_id)
            .json(&body)
            .timeout(Duration::from_secs(60))
            .send()
            .await;

        let role_text = match res {
            Ok(r) if r.status().is_success() => {
                let j: serde_json::Value = r.json().await.unwrap_or_default();
                j.pointer("/choices/0/message/content")
                    .and_then(|v| v.as_str())
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty() && s.to_lowercase() != "unknown")
            }
            _ => None,
        };

        match role_text {
            Some(role) => {
                let mut updated = entity.clone();
                updated.role_vector = Some(role);
                if state
                    .graph
                    .upsert_entities(&params.module_id, &[updated])
                    .is_ok()
                {
                    enriched += 1;
                } else {
                    unchanged += 1;
                }
            }
            None => {
                unchanged += 1;
            }
        }
    }

    Ok(Json(EnrichResponse {
        module_id: params.module_id,
        classification: params.classification,
        null_vector_count,
        enriched,
        unchanged,
        dry_run: false,
        samples,
    }))
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn format_entity_block(entities: &[GraphEntity]) -> String {
    if entities.is_empty() {
        return "(no entities found in graph)".to_string();
    }
    let mut out = String::from("## Extracted Entities\n\n");
    for e in entities {
        out.push_str(&format!("- **{}** ({})", e.entity_name, e.classification));
        if let Some(r) = &e.role_vector {
            out.push_str(&format!("; role: {r}"));
        }
        if let Some(l) = &e.location_vector {
            out.push_str(&format!("; location: {l}"));
        }
        if let Some(c) = &e.contact_vector {
            out.push_str(&format!("; contact: {c}"));
        }
        out.push('\n');
    }
    out
}

// ── pairing handlers ─────────────────────────────────────────────────────────

async fn pair_peer(
    State(state): State<Arc<HttpState>>,
    Json(body): Json<PairRequest>,
) -> Result<Json<PairResponse>, (StatusCode, String)> {
    use crate::pairing::{verify_pair_token, PairError};

    let payload = verify_pair_token(&body.token, &body.public_key).map_err(|e| match e {
        PairError::Malformed => (StatusCode::BAD_REQUEST, e.to_string()),
        PairError::BadSignature => (StatusCode::UNAUTHORIZED, e.to_string()),
        PairError::Expired => (StatusCode::UNAUTHORIZED, "token expired".into()),
        PairError::NonceReused => (StatusCode::CONFLICT, "nonce already used".into()),
    })?;

    // already_paired check before nonce uniqueness — a re-submit of an existing
    // pairing must return already_paired even if the nonce was previously used.
    {
        let store = state.pairing_store.lock().unwrap();
        if let Some(existing) = store.get(&body.public_key) {
            return Ok(Json(PairResponse {
                status: "already_paired",
                paired_on: existing.paired_on.clone(),
                role: existing.role.clone(),
                archive_scope: existing.archive_scope.clone(),
            }));
        }
    }

    if !state.nonce_cache.try_insert(&payload.nonce) {
        return Err((StatusCode::CONFLICT, "nonce already used".into()));
    }

    let paired_on = Utc::now().to_rfc3339();
    let rec = PairingRecord {
        public_key: body.public_key.clone(),
        issuer: payload.issuer.clone(),
        peer_type: payload.peer_type.clone(),
        role: payload.role.clone(),
        archive_scope: payload.archive_scope.clone(),
        node_label: body.node_label.clone(),
        paired_on: paired_on.clone(),
        nonce: payload.nonce.clone(),
    };

    state
        .pairing_store
        .lock()
        .unwrap()
        .insert(rec)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(PairResponse {
        status: "paired",
        paired_on,
        role: payload.role,
        archive_scope: payload.archive_scope,
    }))
}

/// Issue a new signed invite token (Totebox → caller).
async fn issue_pair_token(
    State(state): State<Arc<HttpState>>,
    Query(q): Query<PairTokenQuery>,
) -> Result<Json<PairTokenResponse>, (StatusCode, String)> {
    let scope: Vec<String> = if q.archive_scope.is_empty() {
        vec![]
    } else {
        q.archive_scope
            .split(',')
            .map(|s| s.trim().to_string())
            .collect()
    };
    let label = if q.node_label.is_empty() {
        "totebox"
    } else {
        &q.node_label
    };
    let token = state.pairing_key.issue_token(&q.role, scope, label);
    Ok(Json(PairTokenResponse {
        token,
        public_key: state.pairing_key.verifying_key_b64.clone(),
    }))
}

// ── server entrypoint ─────────────────────────────────────────────────────────

pub async fn run_server(
    store: Arc<dyn GraphStore>,
    bind_addr: String,
    doorman_endpoint: String,
    ontology_dir: String,
    corpus_dir: String,
    graph_dir: String,
) {
    let pairing_store = PairingStore::load(&graph_dir).unwrap_or_else(|e| {
        eprintln!("[HTTP] pairing store load failed: {e}; starting empty");
        PairingStore::load("/tmp").expect("fallback pairing store")
    });
    let pairing_key = PairingKeypair::load_or_generate(&graph_dir).unwrap_or_else(|e| {
        eprintln!("[HTTP] pairing keypair init failed: {e}");
        PairingKeypair::load_or_generate("/tmp").expect("fallback keypair")
    });

    let state = Arc::new(HttpState {
        graph: store,
        doorman_endpoint,
        ontology_dir,
        corpus_dir,
        graph_dir,
        pairing_store: Mutex::new(pairing_store),
        nonce_cache: NonceCache::new(),
        pairing_key,
    });

    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/v1/graph/context", get(graph_context))
        .route("/v1/graph/delta", get(graph_delta))
        .route("/v1/graph/mutate", post(graph_mutate))
        .route("/v1/graph/cleanup", get(graph_cleanup))
        .route("/v1/graph/enrich", post(graph_enrich))
        .route("/v1/draft/generate", post(draft_generate))
        .route("/v1/ingest", post(ingest_document))
        .route("/v1/pair", post(pair_peer))
        .route("/v1/pair/token", get(issue_pair_token))
        .merge(config_routes())
        .with_state(state);

    let listener = match tokio::net::TcpListener::bind(&bind_addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("[HTTP] Failed to bind {}: {}", bind_addr, e);
            return;
        }
    };
    println!("[HTTP] Graph API listening on {}", bind_addr);
    if let Err(e) = axum::serve(listener, app).await {
        eprintln!("[HTTP] Server error: {}", e);
    }
}
