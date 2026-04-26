// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Axum routes for the WORM ledger.
//!
//! Endpoints:
//!   GET  /healthz                → liveness, always 200
//!   GET  /readyz                 → readiness; 200 once the ledger
//!                                  handle is open
//!   GET  /v1/contract            → service-fs version + tenant
//!                                  moduleId + ledger root path
//!   POST /v1/append              → append a payload to the WORM
//!                                  ledger; returns the assigned
//!                                  opaque cursor
//!   GET  /v1/entries?since=N     → read entries with cursor > N;
//!                                  Ring 2 read surface; logs every
//!                                  read for the ADR-07 audit hook
//!
//! Headers:
//!   X-Foundry-Module-ID    required on /v1/append and /v1/entries;
//!                          MUST equal the daemon's `FS_MODULE_ID`.
//!                          Mismatch returns 403 (per-tenant boundary
//!                          per Doctrine §IV.b strict isolation).
//!   X-Foundry-Request-ID   optional; passed through to tracing /
//!                          audit hook for correlation.
//!
//! Wire format note: this skeleton speaks JSON over HTTP for both
//! append and read. The MCP-server interface (per
//! `~/Foundry/conventions/three-ring-architecture.md` §"MCP boundary
//! at Ring 1") layers on top — MCP resources for ledger reads, MCP
//! tools for append. That MCP shim is the next NEXT.md item after
//! `cargo check` passes.

use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Json};
use axum::routing::{get, post};
use axum::Router;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::ledger::{Checkpoint, LedgerBackend, LedgerError};

pub struct AppState {
    pub module_id: String,
    /// L2 trait object — the storage backend can be swapped at
    /// daemon startup (today: in-memory; next per worm-ledger-design.md
    /// §5 step 2: POSIX tile; long-term: moonshot-database) without
    /// changing the wire layer.
    pub ledger: Box<dyn LedgerBackend + Send + Sync>,
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .route("/v1/contract", get(contract))
        .route("/v1/append", post(append))
        .route("/v1/entries", get(entries))
        .route("/v1/checkpoint", get(checkpoint))
        .with_state(state)
}

async fn healthz() -> &'static str {
    "ok"
}

async fn readyz(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let body = serde_json::json!({
        "ready": true,
        "module_id": state.module_id,
    });
    (StatusCode::OK, Json(body))
}

#[derive(Serialize)]
struct ContractInfo {
    service_fs_version: &'static str,
    module_id: String,
    ledger_root: String,
}

async fn contract(State(state): State<Arc<AppState>>) -> Json<ContractInfo> {
    Json(ContractInfo {
        service_fs_version: env!("CARGO_PKG_VERSION"),
        module_id: state.module_id.clone(),
        ledger_root: state.ledger.root().to_string(),
    })
}

#[derive(Deserialize)]
struct AppendBody {
    /// Caller-side payload identifier (e.g., source-document id);
    /// stored alongside the bytes for downstream attribution.
    payload_id: String,
    /// Payload bytes (JSON value to keep the skeleton dependency-light;
    /// later commits may switch to base64 or raw bytes per MCP spec).
    payload: serde_json::Value,
}

#[derive(Serialize)]
struct AppendResponse {
    cursor: u64,
    payload_id: String,
}

async fn append(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<AppendBody>,
) -> Result<Json<AppendResponse>, ApiError> {
    enforce_module_id(&state, &headers)?;

    let cursor = state
        .ledger
        .append(&body.payload_id, &body.payload)
        .map_err(ApiError::from)?;

    info!(
        module_id = %state.module_id,
        payload_id = %body.payload_id,
        cursor,
        "append"
    );

    Ok(Json(AppendResponse {
        cursor,
        payload_id: body.payload_id,
    }))
}

#[derive(Deserialize)]
struct EntriesQuery {
    /// Opaque cursor returned by a prior /v1/append or /v1/entries
    /// response. Reads return entries strictly greater than this
    /// cursor. Default 0 means "from the beginning."
    #[serde(default)]
    since: u64,
}

#[derive(Serialize)]
struct EntriesResponse {
    module_id: String,
    next_cursor: u64,
    entries: Vec<EntryView>,
}

#[derive(Serialize)]
struct EntryView {
    cursor: u64,
    payload_id: String,
    payload: serde_json::Value,
}

async fn entries(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<EntriesQuery>,
) -> Result<Json<EntriesResponse>, ApiError> {
    enforce_module_id(&state, &headers)?;

    let request_id = headers
        .get("x-foundry-request-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("anonymous");

    let raw = state
        .ledger
        .read_since(query.since)
        .map_err(ApiError::from)?;

    // ADR-07 audit hook: every Ring 2 read is logged with moduleId,
    // request-id, since-cursor, and entry count. Future work: persist
    // the audit log to its own append-only file rather than just
    // tracing.
    info!(
        module_id = %state.module_id,
        request_id,
        since = query.since,
        count = raw.len(),
        "read"
    );

    let next_cursor = raw.last().map(|e| e.cursor).unwrap_or(query.since);

    let entries = raw
        .into_iter()
        .map(|e| EntryView {
            cursor: e.cursor,
            payload_id: e.payload_id,
            payload: e.payload,
        })
        .collect();

    Ok(Json(EntriesResponse {
        module_id: state.module_id.clone(),
        next_cursor,
        entries,
    }))
}

async fn checkpoint(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<Checkpoint>, ApiError> {
    enforce_module_id(&state, &headers)?;
    let cp = state.ledger.checkpoint().map_err(ApiError::from)?;
    Ok(Json(cp))
}

fn enforce_module_id(state: &AppState, headers: &HeaderMap) -> Result<(), ApiError> {
    let supplied = headers
        .get("x-foundry-module-id")
        .and_then(|v| v.to_str().ok());
    match supplied {
        Some(s) if s == state.module_id => Ok(()),
        Some(other) => {
            warn!(
                expected = %state.module_id,
                supplied = %other,
                "moduleId mismatch — request rejected"
            );
            Err(ApiError::forbidden(format!(
                "X-Foundry-Module-ID '{other}' does not match this daemon's tenant '{}' (per-tenant boundary, Doctrine §IV.b)",
                state.module_id
            )))
        }
        None => Err(ApiError::bad_request(
            "X-Foundry-Module-ID header is required (per-tenant boundary)",
        )),
    }
}

struct ApiError {
    status: StatusCode,
    body: serde_json::Value,
}

impl ApiError {
    fn bad_request(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            body: serde_json::json!({ "error": { "message": msg.into() } }),
        }
    }

    fn forbidden(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            body: serde_json::json!({ "error": { "message": msg.into() } }),
        }
    }
}

impl From<LedgerError> for ApiError {
    fn from(e: LedgerError) -> Self {
        let status = match &e {
            LedgerError::Io(_) | LedgerError::Serde(_) => StatusCode::INTERNAL_SERVER_ERROR,
            LedgerError::EntryNotFound(_) => StatusCode::NOT_FOUND,
            LedgerError::ChainTampered { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            LedgerError::InconsistentCheckpoints { .. } => StatusCode::CONFLICT,
        };
        Self {
            status,
            body: serde_json::json!({ "error": { "message": e.to_string() } }),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        (self.status, Json(self.body)).into_response()
    }
}
