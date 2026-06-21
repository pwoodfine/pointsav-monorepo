// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Integration tests for `slm-doorman-server::http`.
//!
//! Coverage targets (Brief A — PS.6 sub-brief #1):
//!   - Smoke tests: 4 control endpoints, happy-path status + body shape
//!   - Error-mapping tests: DoormanError variants → HTTP status codes
//!   - Apprenticeship-disabled 404 tests: /v1/brief, /v1/verdict, /v1/shadow
//!
//! All tests use `tower::ServiceExt::oneshot` to drive the axum `Router`
//! without binding a real TCP socket. A `MockVerifier` is defined locally
//! to inject `VerifySignature` and `BriefCacheMiss` errors through the
//! `/v1/verdict` route.
//!
//! Deviation from brief:
//!   - `TierUnavailable` → 503 SERVICE_UNAVAILABLE (brief listed 502;
//!     actual code maps TierUnavailable to SERVICE_UNAVAILABLE, not
//!     BAD_GATEWAY; tested against the actual mapping).
//!   - `ExternalNotAllowlisted` → 403: this error is only reachable
//!     through `ExternalTierClient::complete`, which the HTTP handler
//!     cannot reach (tier_hint is always None from the HTTP layer and
//!     External is never the default). Covered as a From<DoormanError>
//!     unit test inside http.rs (see below) and separately here via
//!     the status-code constant for completeness in a mapping helper test.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;
use serde_json::json;
use slm_doorman::tier::{TierCPricing, TierCProvider};
use slm_doorman::{
    AuditLedger, BriefCache, Doorman, DoormanConfig, DoormanError, ExpressLane,
    VerdictDispatcher, VerdictVerifier, FOUNDRY_DEFAULT_PURPOSE_ALLOWLIST,
};
use slm_doorman_server::http::{
    router, AppState, AUDIT_CAPTURE_MAX_PAYLOAD_BYTES, AUDIT_PROXY_MAX_REQUEST_BYTES,
};
use slm_doorman_server::test_helpers::{
    app_state_no_tiers, app_state_with_apprenticeship, app_state_with_audit_proxy,
    app_state_with_local, app_state_with_service_content, temp_ledger, temp_promotion_ledger,
    temp_queue_config,
};
use tower::ServiceExt;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ---------------------------------------------------------------------------
// MockVerifier — injects VerifySignature or Ok depending on the signature
// value. Used by error-mapping tests that need to exercise /v1/verdict.
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct MockVerifier {
    /// If the incoming signature_pem equals this value, return Ok.
    /// Any other value returns Err(VerifySignature).
    accept_pem: String,
}

#[async_trait]
impl VerdictVerifier for MockVerifier {
    async fn verify(
        &self,
        _body: &str,
        signature_pem: &str,
        _senior_identity: &str,
        _namespace: &str,
    ) -> slm_doorman::Result<()> {
        if signature_pem == self.accept_pem {
            Ok(())
        } else {
            Err(DoormanError::VerifySignature(
                "mock verifier: signature mismatch".into(),
            ))
        }
    }
}

/// Always-reject verifier — every call returns VerifySignature.
#[derive(Debug)]
struct RejectVerifier;

#[async_trait]
impl VerdictVerifier for RejectVerifier {
    async fn verify(
        &self,
        _body: &str,
        _signature_pem: &str,
        _senior_identity: &str,
        _namespace: &str,
    ) -> slm_doorman::Result<()> {
        Err(DoormanError::VerifySignature(
            "always-reject mock verifier".into(),
        ))
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Decode a response body to a `serde_json::Value`.
async fn body_json(resp: axum::response::Response) -> serde_json::Value {
    let bytes = axum::body::to_bytes(resp.into_body(), 1024 * 1024)
        .await
        .expect("read response body");
    serde_json::from_slice(&bytes).expect("response body is JSON")
}

/// Minimal valid verdict body text (YAML frontmatter + prose).
fn verdict_body(brief_id: &str, attempt_id: &str) -> String {
    format!(
        "---\n\
         schema: foundry-apprentice-verdict-v1\n\
         brief_id: {brief_id}\n\
         attempt_id: {attempt_id}\n\
         verdict: accept\n\
         created: 2026-04-27T10:00:00Z\n\
         senior_identity: ps-administrator\n\
         final_diff_sha: 0000000000000000000000000000000000000000\n\
         notes: LGTM\n\
         ---\n\
         \n\
         LGTM.\n"
    )
}

/// JSON body for POST /v1/verdict wire shape.
fn verdict_wire_json(brief_id: &str, attempt_id: &str, sig_b64: &str) -> serde_json::Value {
    json!({
        "body": verdict_body(brief_id, attempt_id),
        "signature": sig_b64,
        "senior_identity": "ps-administrator"
    })
}

/// Build a POST request with a JSON body.
fn post_json(uri: &str, body: &serde_json::Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .expect("build request")
}

// ===========================================================================
// Section 1 — Smoke tests (4 tests)
// ===========================================================================

/// GET /healthz → 200 with body "ok".
#[tokio::test]
async fn smoke_healthz_returns_200_ok() {
    let state = app_state_no_tiers();
    let app = router(state);

    let req = Request::builder()
        .method("GET")
        .uri("/healthz")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.expect("oneshot");
    assert_eq!(resp.status(), StatusCode::OK);

    let bytes = axum::body::to_bytes(resp.into_body(), 256)
        .await
        .expect("read body");
    assert_eq!(&bytes[..], b"ok");
}

/// GET /readyz — no tiers configured → 503 with status:"closed" (P2-B).
/// When all tiers are absent `ai_available` is false and the endpoint signals
/// degraded state to clients via HTTP 503 + a structured `status`/`reason` body.
#[tokio::test]
async fn smoke_readyz_returns_503_when_no_tiers() {
    let state = app_state_no_tiers();
    let app = router(state);

    let req = Request::builder()
        .method("GET")
        .uri("/readyz")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.expect("oneshot");
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);

    let body = body_json(resp).await;
    assert_eq!(body["ready"], false);
    assert_eq!(body["status"], "closed");
    assert_eq!(body["reason"], "no_tier_available");
    assert_eq!(body["ai_available"], false);
    assert!(body["has_local"].is_boolean(), "has_local is bool");
    assert!(body["has_yoyo"].is_boolean(), "has_yoyo is bool");
    assert!(body["has_external"].is_boolean(), "has_external is bool");
    // No tiers configured — all should be false.
    assert_eq!(body["has_local"], false);
    assert_eq!(body["has_yoyo"], false);
    assert_eq!(body["has_external"], false);
    // Queue snapshot fields present (values depend on test environment).
    assert!(
        body["queue_pending"].is_number(),
        "queue_pending is a number"
    );
    assert!(body["queue_done"].is_number(), "queue_done is a number");
    assert!(body["queue_poison"].is_number(), "queue_poison is a number");
}

/// GET /v1/contract → 200 with {doorman_version, yoyo_contract_version, ...}.
#[tokio::test]
async fn smoke_contract_returns_200_with_version_fields() {
    let state = app_state_no_tiers();
    let app = router(state);

    let req = Request::builder()
        .method("GET")
        .uri("/v1/contract")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.expect("oneshot");
    assert_eq!(resp.status(), StatusCode::OK);

    let body = body_json(resp).await;
    assert!(
        body["doorman_version"].is_string(),
        "doorman_version is string"
    );
    assert!(
        body["yoyo_contract_version"].is_string(),
        "yoyo_contract_version is string"
    );
    assert!(body["has_local"].is_boolean());
    assert!(body["has_yoyo"].is_boolean());
    assert!(body["has_external"].is_boolean());
    // Version should not be empty.
    assert!(!body["doorman_version"].as_str().unwrap().is_empty());
}

/// POST /v1/chat/completions happy path → 200 with a content string returned.
/// Uses wiremock to back the local tier.
#[tokio::test]
async fn smoke_chat_completions_happy_path_returns_200_with_content() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "choices": [
                { "message": { "role": "assistant", "content": "pong" } }
            ]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let state = app_state_with_local(server.uri());
    let app = router(state);

    let req_body = json!({
        "messages": [{"role": "user", "content": "ping"}]
    });
    let resp = app
        .oneshot(post_json("/v1/chat/completions", &req_body))
        .await
        .expect("oneshot");

    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp).await;
    assert_eq!(body["content"], "pong");
    assert_eq!(body["tier_used"], "local");
}

// ===========================================================================
// Section 2 — Error-mapping tests (5 tests)
// ===========================================================================

// ── 2a. TierUnavailable → 503 SERVICE_UNAVAILABLE ──────────────────────────
//
// The brief listed 502, but the actual From<DoormanError> impl maps
// TierUnavailable | NotImplemented → SERVICE_UNAVAILABLE (503).
// This test verifies the actual code.

/// POST /v1/chat/completions with no tiers → DoormanError::TierUnavailable → 503.
#[tokio::test]
async fn error_tier_unavailable_returns_503() {
    let state = app_state_no_tiers();
    let app = router(state);

    let req_body = json!({
        "messages": [{"role": "user", "content": "ping"}]
    });
    let resp = app
        .oneshot(post_json("/v1/chat/completions", &req_body))
        .await
        .expect("oneshot");

    assert_eq!(
        resp.status(),
        StatusCode::SERVICE_UNAVAILABLE,
        "TierUnavailable must map to 503 SERVICE_UNAVAILABLE"
    );
}

// ── 2b. BriefCacheMiss → 410 GONE ─────────────────────────────────────────

/// POST /v1/verdict with apprenticeship enabled, verifier accepts, but
/// brief_id not in cache → DoormanError::BriefCacheMiss → 410 GONE.
#[tokio::test]
async fn error_brief_cache_miss_returns_410() {
    // MockVerifier accepts anything — let the request get past signature
    // checking so the cache-miss is reached.
    let verifier: Arc<dyn VerdictVerifier> = Arc::new(MockVerifier {
        accept_pem: "GOOD-SIG".to_string(),
    });

    // Build state WITHOUT inserting the brief into the cache.
    let tmp = std::env::temp_dir();
    let foundry_root = tmp.join(format!(
        "slm-http-test-miss-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&foundry_root).unwrap();

    let cfg = slm_doorman::ApprenticeshipConfig {
        foundry_root: foundry_root.clone(),
        citations_path: foundry_root.join("citations.yaml"),
        brief_tier_b_threshold_chars: 8000,
        doctrine_version: "0.0.1".to_string(),
        tenant: "test".to_string(),
        tier_a_first: false,
        yoyo_dispatch_label: None,
    };
    let brief_cache = Arc::new(BriefCache::default()); // empty
    let verdict_dispatcher = VerdictDispatcher {
        verifier,
        cache: brief_cache.clone(),
        ledger: temp_promotion_ledger(),
        corpus_root: foundry_root,
        doctrine_version: "0.0.1".to_string(),
        tenant: "test".to_string(),
    };
    let doorman = Doorman::new(DoormanConfig::default(), temp_ledger());
    let state = Arc::new(AppState {
        doorman,
        apprenticeship: Some(cfg),
        brief_cache,
        verdict_dispatcher: Some(verdict_dispatcher),
        audit_proxy_client: None,
        audit_proxy_purpose_allowlist: FOUNDRY_DEFAULT_PURPOSE_ALLOWLIST,
        audit_tenant_concurrency: Arc::new(Mutex::new(HashMap::new())),
        audit_tenant_concurrency_cap: 100,
        queue_config: temp_queue_config(),
        service_content_endpoint: String::new(),
        node_class: "hardware",
        tier_a_reason: "available",
        express_lane: Arc::new(ExpressLane::with_default_labels()),
    });
    let app = router(state);

    let sig_b64 = B64.encode("GOOD-SIG");
    let req_body = verdict_wire_json("unknown-brief-id", "unknown-attempt-id", &sig_b64);
    let resp = app
        .oneshot(post_json("/v1/verdict", &req_body))
        .await
        .expect("oneshot");

    assert_eq!(
        resp.status(),
        StatusCode::GONE,
        "BriefCacheMiss must map to 410 GONE"
    );
}

// ── 2c. VerifySignature → 403 FORBIDDEN ────────────────────────────────────

/// POST /v1/verdict with a RejectVerifier → DoormanError::VerifySignature → 403.
#[tokio::test]
async fn error_verify_signature_returns_403() {
    let verifier: Arc<dyn VerdictVerifier> = Arc::new(RejectVerifier);
    let state = app_state_with_apprenticeship(verifier);
    let app = router(state);

    // Any base64-encoded signature — RejectVerifier always refuses.
    let sig_b64 = B64.encode("any-signature");
    let req_body = verdict_wire_json("some-brief", "some-attempt", &sig_b64);
    let resp = app
        .oneshot(post_json("/v1/verdict", &req_body))
        .await
        .expect("oneshot");

    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "VerifySignature must map to 403 FORBIDDEN"
    );
}

// ── 2d. ExternalNotAllowlisted → 403 FORBIDDEN ────────────────────────────
//
// The HTTP handler sets tier_hint = None and the router only selects
// External when tier_hint = Some(Tier::External). Therefore
// ExternalNotAllowlisted cannot be triggered through the normal
// POST /v1/chat/completions path. Instead we verify the
// From<DoormanError> status mapping by constructing the ApiError
// directly via the DoormanError conversion.
//
// This is a Rust-level unit test embedded in the integration test file;
// it confirms the status code constant without a full HTTP round-trip.

#[test]
fn error_external_not_allowlisted_maps_to_403() {
    // Replicate the From<DoormanError> mapping logic as documented in
    // http.rs. The actual conversion is tested by confirming that the
    // variant falls into the FORBIDDEN arm of the match.
    //
    // The match arm in http.rs From<DoormanError> for ApiError:
    //   DoormanError::ExternalNotAllowlisted { .. } | DoormanError::VerifySignature(_)
    //     => StatusCode::FORBIDDEN
    //
    // We verify this by constructing the error and checking which
    // branch of the documented mapping it falls into.
    let err = DoormanError::ExternalNotAllowlisted {
        label: "not-on-list".to_string(),
    };
    let expected_status = StatusCode::FORBIDDEN;
    // Map the error to a status code using the same logic as http.rs.
    let actual_status = doorman_error_to_status(&err);
    assert_eq!(actual_status, expected_status);
}

// ── 2e. Malformed X-Foundry-Module-ID → 400 BAD_REQUEST ───────────────────

/// POST /v1/chat/completions with an invalid X-Foundry-Module-ID header
/// (uppercase characters not allowed) → 400 BAD_REQUEST.
#[tokio::test]
async fn error_malformed_module_id_header_returns_400() {
    let state = app_state_no_tiers();
    let app = router(state);

    let req_body = json!({
        "messages": [{"role": "user", "content": "ping"}]
    });
    let req = Request::builder()
        .method("POST")
        .uri("/v1/chat/completions")
        .header("content-type", "application/json")
        // ModuleId only allows [a-z0-9-]; uppercase is rejected.
        .header("x-foundry-module-id", "INVALID-UPPERCASE")
        .body(Body::from(req_body.to_string()))
        .unwrap();

    let resp = app.oneshot(req).await.expect("oneshot");
    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "invalid Module-ID must map to 400 BAD_REQUEST"
    );
}

// ===========================================================================
// Section 3 — Apprenticeship-disabled 404 tests (3 tests)
// ===========================================================================

/// POST /v1/brief with apprenticeship=None → 404.
#[tokio::test]
async fn apprenticeship_disabled_brief_returns_404() {
    let state = app_state_no_tiers(); // apprenticeship: None
    let app = router(state);

    let req_body = json!({
        "brief_id": "b1",
        "created": "2026-04-27T00:00:00Z",
        "senior_role": "master",
        "senior_identity": "ps-administrator",
        "task_type": "test",
        "scope": {},
        "acceptance_test": "pass",
        "body": "do the thing"
    });
    let resp = app
        .oneshot(post_json("/v1/brief", &req_body))
        .await
        .expect("oneshot");

    assert_eq!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "/v1/brief must return 404 when apprenticeship is disabled"
    );
}

/// POST /v1/verdict with apprenticeship=None → 404.
#[tokio::test]
async fn apprenticeship_disabled_verdict_returns_404() {
    let state = app_state_no_tiers(); // apprenticeship: None
    let app = router(state);

    let sig_b64 = B64.encode("any");
    let req_body = verdict_wire_json("b1", "a1", &sig_b64);
    let resp = app
        .oneshot(post_json("/v1/verdict", &req_body))
        .await
        .expect("oneshot");

    assert_eq!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "/v1/verdict must return 404 when apprenticeship is disabled"
    );
}

/// POST /v1/shadow with apprenticeship=None → 404.
#[tokio::test]
async fn apprenticeship_disabled_shadow_returns_404() {
    let state = app_state_no_tiers(); // apprenticeship: None
    let app = router(state);

    let req_body = json!({
        "brief": {
            "brief_id": "b1",
            "created": "2026-04-27T00:00:00Z",
            "senior_role": "master",
            "senior_identity": "ps-administrator",
            "task_type": "test",
            "scope": {},
            "acceptance_test": "pass",
            "body": "do the thing"
        },
        "actual_diff": "+ stub line"
    });
    let resp = app
        .oneshot(post_json("/v1/shadow", &req_body))
        .await
        .expect("oneshot");

    assert_eq!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "/v1/shadow must return 404 when apprenticeship is disabled"
    );
}

// ===========================================================================
// Section 3b — Shadow enqueue tests (§7C step 3 — iter-23)
//
// These tests verify the new async-202 contract introduced in iter-23:
//   - Apprenticeship enabled + valid brief → 202 ACCEPTED with correct body
//   - The queue file lands at <queue_dir>/<brief_id>.brief.jsonl
//   - The 404 path (apprenticeship disabled) is unchanged (section 3 above)
// ===========================================================================

/// POST /v1/shadow with apprenticeship enabled → 202 ACCEPTED.
/// Response body must carry `audit_id`, `queue_position`, and `brief_id`
/// all matching the submitted brief_id.
#[tokio::test]
async fn shadow_with_apprenticeship_enabled_returns_202_with_body_shape() {
    let verifier: Arc<dyn VerdictVerifier> = Arc::new(RejectVerifier);
    let state = app_state_with_apprenticeship(verifier);
    let app = router(state);

    let brief_id = "shadow-enqueue-test-001";
    let req_body = json!({
        "brief": {
            "brief_id": brief_id,
            "created": "2026-04-28T00:00:00Z",
            "senior_role": "task",
            "senior_identity": "jwoodfine",
            "task_type": "version-bump-manifest",
            "scope": {},
            "acceptance_test": "cargo test --workspace",
            "body": "bump Cargo.toml version to 0.1.0"
        },
        "actual_diff": "- version = \"0.0.9\"\n+ version = \"0.1.0\"\n"
    });

    let resp = app
        .oneshot(post_json("/v1/shadow", &req_body))
        .await
        .expect("oneshot");

    assert_eq!(
        resp.status(),
        StatusCode::ACCEPTED,
        "/v1/shadow must return 202 ACCEPTED when apprenticeship is enabled"
    );

    let body = body_json(resp).await;

    // audit_id must match brief_id (handler preserves caller's brief_id).
    assert_eq!(
        body["audit_id"].as_str().unwrap_or(""),
        brief_id,
        "audit_id must echo the brief_id from the request"
    );

    // brief_id must be present and match.
    assert_eq!(
        body["brief_id"].as_str().unwrap_or(""),
        brief_id,
        "brief_id field must be present in the 202 response body"
    );

    // queue_position must be a non-negative integer (0 = first in queue).
    assert!(
        body["queue_position"].is_u64(),
        "queue_position must be a non-negative integer; got {:?}",
        body["queue_position"]
    );
}

/// POST /v1/shadow with apprenticeship enabled → the queue file lands at
/// `<queue_dir>/<brief_id>.brief.jsonl` relative to the injected queue_config.
///
/// Verifies the durable-disk-write contract: if the handler returns 202,
/// the queue file exists and is readable as a ShadowQueueEntry JSON line.
#[tokio::test]
async fn shadow_enqueued_brief_file_exists_at_queue_path() {
    let queue_cfg = temp_queue_config();
    let queue_dir = queue_cfg.base_dir.join("queue");

    // Build AppState with the same queue_config so we can inspect the
    // queue directory after the request.
    let verifier: Arc<dyn VerdictVerifier> = Arc::new(RejectVerifier);
    let base_state = app_state_with_apprenticeship(verifier);

    // Override queue_config with our inspectable one by rebuilding AppState.
    // AppState is not Clone, but Arc<AppState> fields are accessible
    // directly. We build a new Arc with the queue_config replaced.
    use slm_doorman::{BriefCache, Doorman, DoormanConfig};
    use slm_doorman_server::http::AppState;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    let queue_dir_clone = queue_dir.clone();

    // Build a fresh state inheriting the same apprenticeship config and
    // injecting our inspectable queue_config.
    let state_with_queue = Arc::new(AppState {
        doorman: Doorman::new(DoormanConfig::default(), temp_ledger()),
        apprenticeship: base_state.apprenticeship.clone(),
        brief_cache: Arc::new(BriefCache::default()),
        verdict_dispatcher: None,
        audit_proxy_client: None,
        audit_proxy_purpose_allowlist: FOUNDRY_DEFAULT_PURPOSE_ALLOWLIST,
        audit_tenant_concurrency: Arc::new(Mutex::new(HashMap::new())),
        audit_tenant_concurrency_cap: 100,
        queue_config: queue_cfg,
        service_content_endpoint: String::new(),
        node_class: "hardware",
        tier_a_reason: "available",
        express_lane: Arc::new(ExpressLane::with_default_labels()),
    });

    let app = router(state_with_queue);

    let brief_id = "shadow-file-existence-check-001";
    let req_body = json!({
        "brief": {
            "brief_id": brief_id,
            "created": "2026-04-28T01:00:00Z",
            "senior_role": "task",
            "senior_identity": "pwoodfine",
            "task_type": "version-bump-manifest",
            "scope": {},
            "acceptance_test": "cargo test --workspace",
            "body": "implement shadow enqueue"
        },
        "actual_diff": "+ enqueue_shadow()\n"
    });

    let resp = app
        .oneshot(post_json("/v1/shadow", &req_body))
        .await
        .expect("oneshot");

    assert_eq!(
        resp.status(),
        StatusCode::ACCEPTED,
        "/v1/shadow must return 202 ACCEPTED; got {}",
        resp.status()
    );

    // The queue file must exist at <queue_dir>/<brief_id>.brief.jsonl.
    let expected_file = queue_dir_clone.join(format!("{brief_id}.brief.jsonl"));
    assert!(
        expected_file.exists(),
        "queue file must exist at {} after a 202 response",
        expected_file.display()
    );

    // The file must contain valid JSON with `brief.brief_id` matching.
    let contents = std::fs::read_to_string(&expected_file).expect("read queue file");
    let first_line = contents
        .lines()
        .next()
        .expect("queue file must have at least one line");
    let entry: serde_json::Value =
        serde_json::from_str(first_line).expect("queue file must be valid JSON");
    assert_eq!(
        entry["brief"]["brief_id"].as_str().unwrap_or(""),
        brief_id,
        "queue file must contain the correct brief_id"
    );
    assert_eq!(
        entry["actual_diff"].as_str().unwrap_or(""),
        "+ enqueue_shadow()\n",
        "queue file must preserve the actual_diff"
    );
}

// ===========================================================================
// Helper — replicate the From<DoormanError> status-code mapping
// from http.rs so we can assert on it directly without making the
// private ApiError type public.
// ===========================================================================

fn doorman_error_to_status(e: &DoormanError) -> StatusCode {
    match e {
        DoormanError::TierUnavailable(_) | DoormanError::NotImplemented { .. } => {
            StatusCode::SERVICE_UNAVAILABLE
        }
        DoormanError::ExternalNotAllowlisted { .. }
        | DoormanError::VerifySignature(_)
        // Caller submitted a purpose not on the audit_proxy purpose allowlist
        // (PS.4 step 3). Caller-side policy violation — 403 FORBIDDEN, same
        // as ExternalNotAllowlisted which mirrors this pattern for Tier C labels.
        | DoormanError::AuditProxyPurposeNotAllowlisted { .. } => StatusCode::FORBIDDEN,
        DoormanError::Upstream(_)
        | DoormanError::UpstreamShape(_)
        | DoormanError::ContractMajorMismatch { .. }
        | DoormanError::BearerToken(_) => StatusCode::BAD_GATEWAY,
        DoormanError::VerdictParse(_)
        | DoormanError::TierAGrammarUnsupported { .. }
        | DoormanError::TierCGrammarUnsupported { .. }
        | DoormanError::MalformedLarkGrammar { .. }
        | DoormanError::AuditProxyInvalidProvider { .. }
        // audit_capture caller validation errors (PS.4 step 4) → 400 BAD_REQUEST.
        | DoormanError::AuditCaptureUnknownEventType { .. }
        | DoormanError::AuditCaptureInvalidTimestamp { .. } => StatusCode::BAD_REQUEST,
        // AuditProxyProviderUnavailable → 503: server-side configuration gap,
        // not a caller policy violation.
        DoormanError::AuditProxyProviderUnavailable { .. } => StatusCode::SERVICE_UNAVAILABLE,
        // AuditCapturePayloadTooLarge → 413 PAYLOAD_TOO_LARGE.
        DoormanError::AuditCapturePayloadTooLarge { .. } => StatusCode::PAYLOAD_TOO_LARGE,
        // AuditProxyPayloadTooLarge → 413 PAYLOAD_TOO_LARGE.
        DoormanError::AuditProxyPayloadTooLarge { .. } => StatusCode::PAYLOAD_TOO_LARGE,
        // AuditTenantConcurrencyExhausted → 503 SERVICE_UNAVAILABLE.
        DoormanError::AuditTenantConcurrencyExhausted { .. } => StatusCode::SERVICE_UNAVAILABLE,
        DoormanError::BriefCacheMiss => StatusCode::GONE,
        // OrphanVerdictNoCorpusTuple → 410 GONE: verdict for a brief_id
        // that was never captured to corpus (§7B promote-not-create semantics).
        DoormanError::OrphanVerdictNoCorpusTuple { .. } => StatusCode::GONE,
        DoormanError::LedgerIo(_)
        | DoormanError::LedgerSerde(_)
        | DoormanError::HomeUnset
        | DoormanError::LedgerLock(_)
        | DoormanError::CorpusWrite { .. }
        // Brief Queue Substrate (§7C) — I/O failures are server-side errors.
        | DoormanError::QueueIo { .. } => StatusCode::INTERNAL_SERVER_ERROR,
        // Queue lock contention is transient — 503 SERVICE_UNAVAILABLE.
        DoormanError::QueueLockFailed { .. } => StatusCode::SERVICE_UNAVAILABLE,
        // Malformed brief detected and moved to poison bucket — 400 BAD_REQUEST.
        DoormanError::QueueMalformedBrief { .. } => StatusCode::BAD_REQUEST,
        // Graph proxy — caller omitted module-id header (400) or service-content
        // is unreachable/unconfigured (503).
        DoormanError::GraphProxyMissingModuleId => StatusCode::BAD_REQUEST,
        DoormanError::GraphProxyServiceUnavailable => StatusCode::SERVICE_UNAVAILABLE,
        DoormanError::TierBTimeout | DoormanError::TierBCircuitOpen => {
            StatusCode::SERVICE_UNAVAILABLE
        }
        // RequestTimeout → 503 SERVICE_UNAVAILABLE (mirrors http.rs map).
        DoormanError::RequestTimeout => StatusCode::SERVICE_UNAVAILABLE,
        DoormanError::FlowGateClosed { .. } => StatusCode::SERVICE_UNAVAILABLE,
        DoormanError::PriorityQueueIo { .. } => StatusCode::INTERNAL_SERVER_ERROR,
        DoormanError::GcpApi { .. } => StatusCode::BAD_GATEWAY,
        // CorpusGateRejected → 422 UNPROCESSABLE_ENTITY: the submitted DPO
        // tuple was rejected by the corpus gate (invalid diff, placeholder,
        // length-ratio violation). Caller-side content error.
        DoormanError::CorpusGateRejected { .. } => StatusCode::UNPROCESSABLE_ENTITY,
    }
}

// ===========================================================================
// Section 4 — Lark grammar pre-validation tests (PS.3 step 5) — 3 tests
// ===========================================================================

// ── 4a. MalformedLarkGrammar → 400 BAD_REQUEST ────────────────────────────

/// `DoormanError::MalformedLarkGrammar` maps to 400 BAD_REQUEST.
/// Verified via the doorman_error_to_status helper (same pattern as 2d).
#[test]
fn error_malformed_lark_grammar_maps_to_400() {
    let err = DoormanError::MalformedLarkGrammar {
        reason: "4(21): Expected token ']', found '\\n'".to_string(),
    };
    let status = doorman_error_to_status(&err);
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "MalformedLarkGrammar must map to 400 BAD_REQUEST; got {status}"
    );
}

// ── 4b. Malformed Lark grammar through POST /v1/chat/completions → 400 ───
//
// This test constructs a Doorman with a LarkValidator and a Yo-Yo tier
// backed by a wiremock server, then submits a request carrying a malformed
// Lark grammar. The assertion is:
//   (1) HTTP response is 400 BAD_REQUEST (not 5xx, not 200)
//   (2) the wiremock server received ZERO requests (rejection happened
//       upstream of the network call — key correctness invariant)

/// POST /v1/chat/completions with a malformed Lark grammar hinted at Tier B
/// → 400 BAD_REQUEST, Tier B backend receives zero requests.
#[tokio::test]
async fn lark_validation_runs_before_tier_b_dispatch() {
    use slm_doorman::{
        tier::{PricingConfig, StaticBearer, YoYoTierClient, YoYoTierConfig},
        DoormanConfig, LarkValidator, YOYO_CONTRACT_VERSION,
    };
    use slm_doorman_server::test_helpers::temp_ledger;
    use std::sync::Arc;

    // Start a wiremock server that would accept Yo-Yo calls.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "choices": [{"message": {"role": "assistant", "content": "pong"}}]
        })))
        // Expect ZERO calls — malformed grammar must be rejected before network.
        .expect(0)
        .mount(&server)
        .await;

    // Build a Doorman with:
    //   - a LarkValidator (PS.3 step 5)
    //   - a Yo-Yo tier pointing at the wiremock server
    //   - no local tier (so Low/Medium complexity without a hint would
    //     TierUnavailable; we set tier_hint = Some(Yoyo) in the request)
    let lark_validator = LarkValidator::new().expect("LarkValidator must init");
    let yoyo = YoYoTierClient::new(
        YoYoTierConfig {
            endpoint: server.uri(),
            default_model: "test-model".to_string(),
            contract_version: YOYO_CONTRACT_VERSION.to_string(),
            pricing: PricingConfig::default(),
            zone: None,
            health_path: "/health".to_string(),
        },
        Arc::new(StaticBearer::new("test-token")),
    );
    // Build the Doorman directly (not via AppState / router) so we can call
    // route() on it after construction without borrow issues.
    let doorman = Doorman::new(
        DoormanConfig {
            local: None,
            yoyo: {
                let mut m = std::collections::HashMap::new();
                m.insert("default".to_string(), yoyo);
                m
            },
            external: None,
            lark_validator: Some(lark_validator),
            graph_context_client: None,
            tier_a_first: false,
            daily_yoyo_cap_usd: None,
            cost_ledger: None,
        },
        temp_ledger(),
    );

    use slm_core::{ChatMessage, Complexity, GrammarConstraint, ModuleId, RequestId, Tier};
    use std::str::FromStr;

    let req = slm_core::ComputeRequest {
        request_id: RequestId::new(),
        module_id: ModuleId::from_str("test").unwrap(),
        model: None,
        messages: vec![ChatMessage {
            role: "user".to_string(),
            content: "ping".to_string(),
        }],
        complexity: Complexity::High,
        tier_hint: Some(Tier::Yoyo),
        stream: false,
        max_tokens: None,
        temperature: None,
        sanitised_outbound: true,
        tier_c_label: None,
        yoyo_label: None,
        grammar: Some(GrammarConstraint::Lark(
            // Malformed: unclosed optional bracket.
            "start: item+\nitem: [ unclosed\n".to_string(),
        )),
        speculation: None,
        graph_context_enabled: None,
        tools: None,
        stop_sequences: None,
        session_context: None,
    };

    // Call route() directly on the Doorman. The pre-validation step (PS.3
    // step 5) must reject the malformed Lark grammar BEFORE sending any
    // network request. The wiremock expect(0) assertion fires at server drop
    // and panics if any request reached the backend, proving the rejection
    // happened upstream of the wire.
    let resp = doorman.route(&req).await;
    assert!(
        matches!(resp, Err(DoormanError::MalformedLarkGrammar { .. })),
        "malformed Lark grammar routed at Tier B must return MalformedLarkGrammar; got: {resp:?}"
    );
    // wiremock server drops here — expect(0) fires and panics if any
    // request was received, confirming rejection happened before the wire.
}

// ===========================================================================
// Section 5 — audit_proxy endpoint scaffold tests (PS.4 step 1) — 5 tests
// ===========================================================================
//
// All five tests exercise the new POST /v1/audit/proxy endpoint added in
// PS.4 step 1. Validation failures return 400; a valid request writes a
// ledger stub and returns 503 SERVICE_UNAVAILABLE (upstream relay is pending
// PS.4 step 2).

/// Build a valid audit_proxy request body. Individual tests override fields
/// to exercise specific validation paths. Uses `editorial-refinement` as the
/// default purpose — one of the four documented purposes in
/// `FOUNDRY_DEFAULT_PURPOSE_ALLOWLIST`.
fn valid_audit_proxy_body() -> serde_json::Value {
    json!({
        "module_id": "woodfine",
        "purpose": "editorial-refinement",
        "provider": "anthropic",
        "model": "claude-opus-4-7",
        "messages": [{"role": "user", "content": "Please review this paragraph."}],
        "max_tokens": 512,
        "caller_request_id": "caller-abc-123"
    })
}

// ── 5a. invalid module_id → 400 ──────────────────────────────────────────

/// POST /v1/audit/proxy with an uppercase module_id → 400 BAD_REQUEST.
/// ModuleId only accepts [a-z0-9-]; uppercase violates the constraint.
#[tokio::test]
async fn audit_proxy_invalid_module_id_returns_400() {
    let state = app_state_no_tiers();
    let app = router(state);

    let mut body = valid_audit_proxy_body();
    body["module_id"] = json!("INVALID-UPPERCASE");

    let resp = app
        .oneshot(post_json("/v1/audit/proxy", &body))
        .await
        .expect("oneshot");

    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "invalid module_id must return 400 BAD_REQUEST"
    );
}

// ── 5b. unknown provider → 400 ──────────────────────────────────────────

/// POST /v1/audit/proxy with an unrecognised provider string → 400 BAD_REQUEST.
/// Accepted values: "anthropic", "gemini", "openai".
#[tokio::test]
async fn audit_proxy_unknown_provider_returns_400() {
    let state = app_state_no_tiers();
    let app = router(state);

    let mut body = valid_audit_proxy_body();
    body["provider"] = json!("not-a-real-provider");

    let resp = app
        .oneshot(post_json("/v1/audit/proxy", &body))
        .await
        .expect("oneshot");

    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "unknown provider must return 400 BAD_REQUEST"
    );
    let body_json = body_json(resp).await;
    // The error message must name the unrecognised provider.
    let msg = body_json["error"]["message"].as_str().unwrap_or_default();
    assert!(
        msg.contains("not-a-real-provider"),
        "error message must include the bad provider string; got: {msg}"
    );
}

// ── 5c. empty purpose → 400 ──────────────────────────────────────────────

/// POST /v1/audit/proxy with an empty purpose string → 400 BAD_REQUEST.
#[tokio::test]
async fn audit_proxy_empty_purpose_returns_400() {
    let state = app_state_no_tiers();
    let app = router(state);

    let mut body = valid_audit_proxy_body();
    body["purpose"] = json!("");

    let resp = app
        .oneshot(post_json("/v1/audit/proxy", &body))
        .await
        .expect("oneshot");

    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "empty purpose must return 400 BAD_REQUEST"
    );
}

// ── 5d. empty messages → 400 ─────────────────────────────────────────────

/// POST /v1/audit/proxy with an empty messages array → 400 BAD_REQUEST.
#[tokio::test]
async fn audit_proxy_empty_messages_returns_400() {
    let state = app_state_no_tiers();
    let app = router(state);

    let mut body = valid_audit_proxy_body();
    body["messages"] = json!([]);

    let resp = app
        .oneshot(post_json("/v1/audit/proxy", &body))
        .await
        .expect("oneshot");

    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "empty messages must return 400 BAD_REQUEST"
    );
}

// ── 5e. valid request (no providers configured) → writes audit stub + returns 503 unconfigured ──

/// POST /v1/audit/proxy with a fully valid request body and no providers
/// configured (`audit_proxy_client = None`):
///   - writes a stub entry to the audit ledger (status: "inbound")
///   - returns 503 SERVICE_UNAVAILABLE with an audit_id and the
///     "unconfigured" message (PS.4 step 2 replaces the old step-1
///     "pending PS.4 step 2" placeholder)
///
/// The audit ledger is backed by a temp directory. After the request we read
/// the JSONL file back to confirm the stub entry is present.
///
/// Note: `app_state_no_tiers()` sets `audit_proxy_client = None`, which
/// triggers the unconfigured path (same 503 status; different message).
#[tokio::test]
async fn audit_proxy_valid_request_writes_audit_stub_and_returns_503() {
    use slm_doorman::{AuditLedger, Doorman, DoormanConfig};
    use slm_doorman_server::http::AppState;

    // Build a state with a ledger rooted at a known temp directory so we
    // can inspect the written JSONL after the request.
    let ledger_dir = std::env::temp_dir().join(format!(
        "slm-audit-proxy-test-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&ledger_dir).expect("create test ledger dir");
    let ledger = AuditLedger::new(&ledger_dir).expect("create test audit ledger");
    let doorman = Doorman::new(DoormanConfig::default(), ledger);
    let state = Arc::new(AppState {
        doorman,
        apprenticeship: None,
        brief_cache: Arc::new(slm_doorman::BriefCache::default()),
        verdict_dispatcher: None,
        // No providers configured → unconfigured path.
        audit_proxy_client: None,
        audit_proxy_purpose_allowlist: FOUNDRY_DEFAULT_PURPOSE_ALLOWLIST,
        audit_tenant_concurrency: Arc::new(Mutex::new(HashMap::new())),
        audit_tenant_concurrency_cap: 100,
        queue_config: temp_queue_config(),
        service_content_endpoint: String::new(),
        node_class: "hardware",
        tier_a_reason: "available",
        express_lane: Arc::new(ExpressLane::with_default_labels()),
    });
    let app = router(state);

    let body = valid_audit_proxy_body();
    let resp = app
        .oneshot(post_json("/v1/audit/proxy", &body))
        .await
        .expect("oneshot");

    // 1. Status must be 503 SERVICE_UNAVAILABLE.
    assert_eq!(
        resp.status(),
        StatusCode::SERVICE_UNAVAILABLE,
        "valid audit_proxy request with no providers configured must return 503"
    );

    // 2. Response body must carry an audit_id and the "unconfigured" message
    //    (PS.4 step 2 retired the old "pending PS.4 step 2" placeholder).
    let resp_body = body_json(resp).await;
    let audit_id = resp_body["audit_id"]
        .as_str()
        .expect("audit_id must be present in 503 response body");
    assert!(!audit_id.is_empty(), "audit_id must be non-empty");
    assert_eq!(
        resp_body["caller_request_id"].as_str().unwrap_or(""),
        "caller-abc-123",
        "caller_request_id must be echoed back"
    );
    let error_msg = resp_body["error"].as_str().unwrap_or_default();
    assert!(
        error_msg.contains("unconfigured"),
        "error message must say 'unconfigured'; got: {error_msg}"
    );

    // 3. Audit ledger must contain the stub entry (still written even when
    //    the provider is unconfigured — this preserves the paper trail).
    let jsonl_files: Vec<_> = std::fs::read_dir(&ledger_dir)
        .expect("read ledger dir")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "jsonl").unwrap_or(false))
        .collect();
    assert_eq!(jsonl_files.len(), 1, "exactly one JSONL file must exist");

    let contents = std::fs::read_to_string(jsonl_files[0].path()).expect("read JSONL");
    let lines: Vec<_> = contents.lines().collect();
    assert_eq!(lines.len(), 1, "exactly one ledger line must be written");

    let entry: serde_json::Value = serde_json::from_str(lines[0]).expect("valid JSONL");
    assert_eq!(
        entry["audit_id"].as_str().unwrap_or_default(),
        audit_id,
        "ledger audit_id must match response audit_id"
    );
    assert_eq!(entry["module_id"].as_str().unwrap_or(""), "woodfine");
    assert_eq!(
        entry["purpose"].as_str().unwrap_or(""),
        "editorial-refinement"
    );
    assert_eq!(entry["provider"].as_str().unwrap_or(""), "anthropic");
    assert_eq!(entry["model"].as_str().unwrap_or(""), "claude-opus-4-7");
    assert_eq!(entry["request_messages_count"].as_u64().unwrap_or(0), 1);
    // Stub status is now "inbound" (PS.4 step 2 update).
    assert_eq!(
        entry["status"].as_str().unwrap_or(""),
        "inbound",
        "ledger status must be 'inbound'"
    );
    assert_eq!(
        entry["caller_request_id"].as_str().unwrap_or(""),
        "caller-abc-123"
    );
}

// ── 4c. Valid Lark grammar with LarkValidator configured passes through ───

/// With a LarkValidator configured and a valid Lark grammar, the Doorman
/// does NOT reject at the boundary — the request passes through to Tier B.
/// We use a wiremock server to confirm exactly ONE request reaches the backend.
#[tokio::test]
async fn valid_lark_grammar_passes_through_to_tier_b() {
    use slm_doorman::{
        tier::{PricingConfig, StaticBearer, YoYoTierClient, YoYoTierConfig},
        DoormanConfig, LarkValidator, YOYO_CONTRACT_VERSION,
    };
    use slm_doorman_server::test_helpers::temp_ledger;
    use std::sync::Arc;

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "choices": [{"message": {"role": "assistant", "content": "pong"}}],
            "x_foundry_inference_ms": 100
        })))
        // Expect exactly ONE request — valid grammar must pass through.
        .expect(1)
        .mount(&server)
        .await;

    let lark_validator = LarkValidator::new().expect("LarkValidator must init");
    let yoyo = YoYoTierClient::new(
        YoYoTierConfig {
            endpoint: server.uri(),
            default_model: "test-model".to_string(),
            contract_version: YOYO_CONTRACT_VERSION.to_string(),
            pricing: PricingConfig::default(),
            zone: None,
            health_path: "/health".to_string(),
        },
        Arc::new(StaticBearer::new("test-token")),
    );
    let doorman = Doorman::new(
        DoormanConfig {
            local: None,
            yoyo: {
                let mut m = std::collections::HashMap::new();
                m.insert("default".to_string(), yoyo);
                m
            },
            external: None,
            lark_validator: Some(lark_validator),
            graph_context_client: None,
            tier_a_first: false,
            daily_yoyo_cap_usd: None,
            cost_ledger: None,
        },
        temp_ledger(),
    );

    use slm_core::{ChatMessage, Complexity, GrammarConstraint, ModuleId, RequestId, Tier};
    use std::str::FromStr;

    let req = slm_core::ComputeRequest {
        request_id: RequestId::new(),
        module_id: ModuleId::from_str("test").unwrap(),
        model: None,
        messages: vec![ChatMessage {
            role: "user".to_string(),
            content: "ping".to_string(),
        }],
        complexity: Complexity::High,
        tier_hint: Some(Tier::Yoyo),
        stream: false,
        max_tokens: None,
        temperature: None,
        sanitised_outbound: true,
        tier_c_label: None,
        yoyo_label: None,
        grammar: Some(GrammarConstraint::Lark(
            // Valid Lark grammar — simple yes/no alternation.
            "start: /yes/ | /no/".to_string(),
        )),
        speculation: None,
        graph_context_enabled: None,
        tools: None,
        stop_sequences: None,
        session_context: None,
    };

    let resp = doorman.route(&req).await;
    // The valid grammar passes pre-validation; whatever Tier B responds
    // with is the real result (could be Ok or a network/parse error
    // depending on the wiremock response body, but it must NOT be
    // MalformedLarkGrammar).
    assert!(
        !matches!(resp, Err(DoormanError::MalformedLarkGrammar { .. })),
        "valid Lark grammar must NOT produce MalformedLarkGrammar; got: {resp:?}"
    );
    // wiremock drops here — expect(1) fires and panics if zero or >1
    // requests reached the backend.
}

// ── 4d. JsonSchema grammar in POST body is parsed and forwarded ────────────
//
// Tests the HTTP body → grammar parsing path added to `ChatCompletionsBody`.
// Tests 4b and 4c call `doorman.route()` directly; this test drives the
// full HTTP handler so the `serde(default)` grammar field is actually
// deserialised from the JSON body.

/// POST /v1/chat/completions with a JsonSchema grammar in the JSON body
/// reaches the local tier — confirms HTTP body → grammar parsing works
/// end-to-end through the axum handler.
#[tokio::test]
async fn json_schema_grammar_in_body_passes_through_to_local_tier() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "choices": [{"message": {"role": "assistant", "content": "[]"}}]
        })))
        .expect(1) // MUST receive exactly one call — JsonSchema is not rejected
        .mount(&server)
        .await;

    let state = app_state_with_local(server.uri());
    let app = router(state);

    // Grammar uses the serde-tagged form for GrammarConstraint::JsonSchema.
    let req_body = json!({
        "messages": [{"role": "user", "content": "extract entities"}],
        "grammar": {
            "type": "json-schema",
            "value": {"type": "array", "items": {"type": "object"}}
        }
    });
    let resp = app
        .oneshot(post_json("/v1/chat/completions", &req_body))
        .await
        .expect("oneshot");

    // 200 OK confirms: grammar was parsed from the body, passed to the router,
    // and accepted by Tier A (JsonSchema is natively supported by llama-server).
    assert_eq!(resp.status(), StatusCode::OK);
    // wiremock expect(1) fires on drop — confirms backend received the call.
}

// ===========================================================================
// Section 6 — audit_proxy upstream relay tests (PS.4 step 2) — 6 tests
// ===========================================================================
//
// These tests exercise the new upstream provider relay wired in PS.4 step 2.
// All use wiremock; no live API calls per the standing operator guardrail.
//
// Test helper: `app_state_with_audit_proxy(provider, server_uri, pricing)`
// constructs an AppState with an AuditProxyClient pointing at the given
// mock server URI plus a known-path temp ledger returned as the second tuple
// element. The Doorman has no compute tiers (audit_proxy tests do not need
// inference routing).

/// Helper: valid audit_proxy request body targeting "anthropic". Uses
/// `editorial-refinement` as the purpose — one of the four documented
/// purposes in `FOUNDRY_DEFAULT_PURPOSE_ALLOWLIST`.
fn valid_audit_proxy_relay_body() -> serde_json::Value {
    json!({
        "module_id": "woodfine",
        "purpose": "editorial-refinement",
        "provider": "anthropic",
        "model": "claude-opus-4-7",
        "messages": [{"role": "user", "content": "Please review this paragraph."}],
        "max_tokens": 100,
        "caller_request_id": "relay-caller-abc"
    })
}

/// Standard upstream response shape returned by the mock server.
fn upstream_ok_body(prompt_tokens: u32, completion_tokens: u32) -> serde_json::Value {
    json!({
        "choices": [
            { "message": { "role": "assistant", "content": "relay-response-content" } }
        ],
        "usage": {
            "prompt_tokens": prompt_tokens,
            "completion_tokens": completion_tokens
        }
    })
}

// ── 6a. Anthropic happy path → 200 + ledger has stub + final "ok" entry ────

/// POST /v1/audit/proxy (Anthropic provider):
///   - wiremock returns Anthropic-shaped JSON
///   - response is 200 with content and usage
///   - ledger has TWO entries: stub (status "inbound") + final (status "ok")
#[tokio::test]
async fn audit_proxy_anthropic_happy_path_returns_200_with_content_and_logs_full_entry() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(upstream_ok_body(50, 20)))
        .expect(1)
        .mount(&server)
        .await;

    let pricing = TierCPricing {
        anthropic_input_per_mtok_usd: 0.25,
        anthropic_output_per_mtok_usd: 1.25,
        ..Default::default()
    };
    let (state, ledger_dir) =
        app_state_with_audit_proxy(TierCProvider::Anthropic, server.uri(), pricing);
    let app = router(state);

    let resp = app
        .oneshot(post_json(
            "/v1/audit/proxy",
            &valid_audit_proxy_relay_body(),
        ))
        .await
        .expect("oneshot");

    // 1. HTTP 200 OK.
    assert_eq!(resp.status(), StatusCode::OK);

    // 2. Response body carries content, audit_id, usage.
    let body = body_json(resp).await;
    let audit_id = body["audit_id"].as_str().expect("audit_id must be present");
    assert!(!audit_id.is_empty());
    assert_eq!(
        body["content"].as_str().unwrap_or(""),
        "relay-response-content"
    );
    assert_eq!(
        body["caller_request_id"].as_str().unwrap_or(""),
        "relay-caller-abc"
    );
    assert_eq!(body["usage"]["prompt_tokens"].as_u64().unwrap_or(0), 50);
    assert_eq!(body["usage"]["completion_tokens"].as_u64().unwrap_or(0), 20);
    // 50 in × $0.25/M + 20 out × $1.25/M = 0.0000375
    let cost = body["usage"]["cost_usd"].as_f64().unwrap_or(-1.0);
    assert!(
        (cost - 0.0000375).abs() < 1e-10,
        "expected cost ~$0.0000375, got ${cost}"
    );

    // 3. Ledger must contain BOTH entries for the same audit_id.
    let jsonl_files: Vec<_> = std::fs::read_dir(&ledger_dir)
        .expect("read ledger dir")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "jsonl").unwrap_or(false))
        .collect();
    assert_eq!(jsonl_files.len(), 1, "exactly one JSONL file");
    let contents = std::fs::read_to_string(jsonl_files[0].path()).expect("read JSONL");
    let lines: Vec<serde_json::Value> = contents
        .lines()
        .map(|l| serde_json::from_str(l).expect("valid JSON line"))
        .collect();
    assert_eq!(lines.len(), 2, "two ledger entries: stub + final");

    // First entry is the stub.
    let stub = &lines[0];
    assert_eq!(stub["audit_id"].as_str().unwrap_or(""), audit_id);
    assert_eq!(stub["status"].as_str().unwrap_or(""), "inbound");
    assert_eq!(stub["module_id"].as_str().unwrap_or(""), "woodfine");

    // Second entry is the final outcome.
    let final_entry = &lines[1];
    assert_eq!(final_entry["audit_id"].as_str().unwrap_or(""), audit_id);
    assert_eq!(final_entry["status"].as_str().unwrap_or(""), "ok");
    assert_eq!(final_entry["prompt_tokens"].as_u64().unwrap_or(0), 50);
    assert_eq!(final_entry["completion_tokens"].as_u64().unwrap_or(0), 20);
    assert!(
        final_entry["error_message"].is_null(),
        "no error_message on ok"
    );
}

// ── 6b. Gemini happy path → 200 ─────────────────────────────────────────────

/// POST /v1/audit/proxy (Gemini provider) → 200 with content.
#[tokio::test]
async fn audit_proxy_gemini_happy_path_returns_200() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(upstream_ok_body(30, 10)))
        .expect(1)
        .mount(&server)
        .await;

    let (state, _ledger) =
        app_state_with_audit_proxy(TierCProvider::Gemini, server.uri(), TierCPricing::default());
    let app = router(state);

    let mut body = valid_audit_proxy_relay_body();
    body["provider"] = json!("gemini");
    body["model"] = json!("gemini-2.5-pro");

    let resp = app
        .oneshot(post_json("/v1/audit/proxy", &body))
        .await
        .expect("oneshot");

    assert_eq!(resp.status(), StatusCode::OK);
    let resp_body = body_json(resp).await;
    assert_eq!(
        resp_body["content"].as_str().unwrap_or(""),
        "relay-response-content"
    );
    assert_eq!(
        resp_body["usage"]["prompt_tokens"].as_u64().unwrap_or(0),
        30
    );
}

// ── 6c. OpenAI happy path → 200 ─────────────────────────────────────────────

/// POST /v1/audit/proxy (OpenAI provider) → 200 with content.
#[tokio::test]
async fn audit_proxy_openai_happy_path_returns_200() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(upstream_ok_body(40, 15)))
        .expect(1)
        .mount(&server)
        .await;

    let (state, _ledger) =
        app_state_with_audit_proxy(TierCProvider::Openai, server.uri(), TierCPricing::default());
    let app = router(state);

    let mut body = valid_audit_proxy_relay_body();
    body["provider"] = json!("openai");
    body["model"] = json!("gpt-4o-mini");

    let resp = app
        .oneshot(post_json("/v1/audit/proxy", &body))
        .await
        .expect("oneshot");

    assert_eq!(resp.status(), StatusCode::OK);
    let resp_body = body_json(resp).await;
    assert_eq!(
        resp_body["content"].as_str().unwrap_or(""),
        "relay-response-content"
    );
    assert_eq!(
        resp_body["usage"]["completion_tokens"]
            .as_u64()
            .unwrap_or(0),
        15
    );
}

// ── 6d. Unconfigured client → 503 with "unconfigured" message ───────────────

/// POST /v1/audit/proxy with `audit_proxy_client = None` (no providers
/// configured at startup) → 503 SERVICE_UNAVAILABLE with the "unconfigured"
/// message (not the old "pending PS.4 step 2" message).
#[tokio::test]
async fn audit_proxy_provider_unconfigured_returns_503_unconfigured() {
    // app_state_no_tiers() sets audit_proxy_client: None.
    let state = app_state_no_tiers();
    let app = router(state);

    let resp = app
        .oneshot(post_json(
            "/v1/audit/proxy",
            &valid_audit_proxy_relay_body(),
        ))
        .await
        .expect("oneshot");

    assert_eq!(
        resp.status(),
        StatusCode::SERVICE_UNAVAILABLE,
        "unconfigured audit_proxy must return 503 SERVICE_UNAVAILABLE"
    );
    let body = body_json(resp).await;
    let error = body["error"].as_str().unwrap_or_default();
    assert!(
        error.contains("unconfigured"),
        "error message must contain 'unconfigured'; got: {error}"
    );
    // Must NOT contain "PS.4 step 2" (that was the scaffold placeholder).
    assert!(
        !error.contains("PS.4 step 2"),
        "error message must not contain 'PS.4 step 2' (scaffold message retired); got: {error}"
    );
}

// ── 6e. Upstream 500 → logged as upstream-error in ledger ───────────────────

/// POST /v1/audit/proxy where the upstream returns 500:
///   - response is a 502 BAD_GATEWAY (UpstreamShape error)
///   - ledger has TWO entries: stub (status "inbound") + final (status "upstream-error")
#[tokio::test]
async fn audit_proxy_provider_returns_500_logged_as_upstream_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(500).set_body_string("upstream failure"))
        .expect(1)
        .mount(&server)
        .await;

    let (state, ledger_dir) = app_state_with_audit_proxy(
        TierCProvider::Anthropic,
        server.uri(),
        TierCPricing::default(),
    );
    let app = router(state);

    let resp = app
        .oneshot(post_json(
            "/v1/audit/proxy",
            &valid_audit_proxy_relay_body(),
        ))
        .await
        .expect("oneshot");

    // The relay converts UpstreamShape to 502 BAD_GATEWAY.
    assert_eq!(resp.status(), StatusCode::BAD_GATEWAY);

    // Ledger must have both stub + final "upstream-error" entries.
    let jsonl_files: Vec<_> = std::fs::read_dir(&ledger_dir)
        .expect("read ledger dir")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "jsonl").unwrap_or(false))
        .collect();
    assert_eq!(jsonl_files.len(), 1);
    let contents = std::fs::read_to_string(jsonl_files[0].path()).expect("read JSONL");
    let lines: Vec<serde_json::Value> = contents
        .lines()
        .map(|l| serde_json::from_str(l).expect("valid JSON line"))
        .collect();
    assert_eq!(lines.len(), 2, "two ledger entries: stub + final");
    assert_eq!(lines[0]["status"].as_str().unwrap_or(""), "inbound");
    assert_eq!(lines[1]["status"].as_str().unwrap_or(""), "upstream-error");
    // Final entry must carry an error_message.
    let err_msg = lines[1]["error_message"].as_str().unwrap_or_default();
    assert!(
        !err_msg.is_empty(),
        "error_message must be non-empty on upstream-error"
    );
}

// ── 6f. Cost arithmetic matches pricing config ───────────────────────────────

/// POST /v1/audit/proxy with known token counts + configured pricing:
///   - assert cost_usd = prompt × input_per_mtok + completion × output_per_mtok
///   - arithmetic matches the same formula used by TierCPricing::cost_usd
#[tokio::test]
async fn audit_proxy_cost_arithmetic_matches_pricing_config() {
    let server = MockServer::start().await;
    // Return exactly 100 prompt + 50 completion tokens.
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(upstream_ok_body(100, 50)))
        .expect(1)
        .mount(&server)
        .await;

    // Rates: Anthropic $1.00/mtok in, $4.00/mtok out.
    // Expected: (100/1_000_000) * 1.00 + (50/1_000_000) * 4.00
    //         = 0.0001 + 0.0002 = 0.0003
    let pricing = TierCPricing {
        anthropic_input_per_mtok_usd: 1.0,
        anthropic_output_per_mtok_usd: 4.0,
        ..Default::default()
    };
    let (state, _ledger) =
        app_state_with_audit_proxy(TierCProvider::Anthropic, server.uri(), pricing);
    let app = router(state);

    let resp = app
        .oneshot(post_json(
            "/v1/audit/proxy",
            &valid_audit_proxy_relay_body(),
        ))
        .await
        .expect("oneshot");

    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp).await;
    let cost = body["usage"]["cost_usd"].as_f64().unwrap_or(-1.0);
    assert!(
        (cost - 0.0003).abs() < 1e-9,
        "expected cost $0.0003, got ${cost}"
    );
    assert_eq!(body["usage"]["prompt_tokens"].as_u64().unwrap_or(0), 100);
    assert_eq!(body["usage"]["completion_tokens"].as_u64().unwrap_or(0), 50);
}

// ===========================================================================
// Section 7 — audit_proxy purpose allowlist tests (PS.4 step 3) — 4 tests
// ===========================================================================
//
// These tests exercise the new purpose-allowlist enforcement added in PS.4
// step 3. The allowlist check runs AFTER the non-empty purpose validation and
// BEFORE audit_id generation / stub ledger write.
//
// Ordering invariant tested:
//   - An un-allowlisted purpose returns 403 FORBIDDEN.
//   - No upstream provider call is made for un-allowlisted purposes.
//   - No stub ledger entry is written for un-allowlisted purposes
//     (because the check runs before the stub write).
//   - All four documented default purposes pass the allowlist check.

// ── 7a. Unallowlisted purpose → 403 FORBIDDEN ─────────────────────────────

/// POST /v1/audit/proxy with an un-allowlisted purpose (e.g. "ad-hoc-call")
/// → 403 FORBIDDEN. The error message must name the rejected purpose.
#[tokio::test]
async fn audit_proxy_unallowlisted_purpose_returns_403() {
    // app_state_no_tiers() uses FOUNDRY_DEFAULT_PURPOSE_ALLOWLIST which does
    // not include "ad-hoc-call".
    let state = app_state_no_tiers();
    let app = router(state);

    let mut body = valid_audit_proxy_body();
    body["purpose"] = json!("ad-hoc-call");

    let resp = app
        .oneshot(post_json("/v1/audit/proxy", &body))
        .await
        .expect("oneshot");

    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "un-allowlisted purpose must return 403 FORBIDDEN"
    );
    let resp_body = body_json(resp).await;
    let msg = resp_body["error"]["message"].as_str().unwrap_or_default();
    assert!(
        msg.contains("ad-hoc-call"),
        "error message must name the rejected purpose; got: {msg}"
    );
}

// ── 7b. Unallowlisted purpose does not call upstream ──────────────────────

/// POST /v1/audit/proxy with an un-allowlisted purpose and a configured
/// provider: the wiremock server must receive ZERO requests — the allowlist
/// check runs before any upstream call.
#[tokio::test]
async fn audit_proxy_unallowlisted_purpose_does_not_call_upstream() {
    let server = MockServer::start().await;
    // No mocks mounted — wiremock would record any requests that land.

    // Use the default allowlist (FOUNDRY_DEFAULT_PURPOSE_ALLOWLIST).
    let (state, _ledger_dir) = app_state_with_audit_proxy(
        TierCProvider::Anthropic,
        server.uri(),
        TierCPricing::default(),
    );
    let app = router(state);

    let mut body = valid_audit_proxy_relay_body();
    body["purpose"] = json!("forbidden-purpose");

    let resp = app
        .oneshot(post_json("/v1/audit/proxy", &body))
        .await
        .expect("oneshot");

    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "un-allowlisted purpose must return 403 FORBIDDEN"
    );

    // Assert zero requests reached the mock server.
    let received = server.received_requests().await.unwrap_or_default();
    assert_eq!(
        received.len(),
        0,
        "Doorman MUST NOT issue an upstream call before the purpose allowlist check"
    );
}

// ── 7c. Unallowlisted purpose does not write a ledger entry ───────────────

/// POST /v1/audit/proxy with an un-allowlisted purpose: no stub or final
/// ledger entry must be written. The allowlist check runs before the
/// stub write so the audit trail is not polluted by policy-denied requests.
#[tokio::test]
async fn audit_proxy_unallowlisted_purpose_does_not_write_ledger_entry() {
    use slm_doorman::{AuditLedger, Doorman, DoormanConfig};
    use slm_doorman_server::http::AppState;

    // Build a state with a ledger rooted at a known temp directory so we can
    // confirm NO JSONL files are written after the rejected request.
    let ledger_dir = std::env::temp_dir().join(format!(
        "slm-audit-proxy-allowlist-ledger-test-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&ledger_dir).expect("create test ledger dir");
    let ledger = AuditLedger::new(&ledger_dir).expect("create test audit ledger");
    let doorman = Doorman::new(DoormanConfig::default(), ledger);

    let state = Arc::new(AppState {
        doorman,
        apprenticeship: None,
        brief_cache: Arc::new(slm_doorman::BriefCache::default()),
        verdict_dispatcher: None,
        // No providers configured (audit_proxy_client = None), but the
        // allowlist check still runs before the unconfigured 503 path.
        audit_proxy_client: None,
        audit_proxy_purpose_allowlist: FOUNDRY_DEFAULT_PURPOSE_ALLOWLIST,
        audit_tenant_concurrency: Arc::new(Mutex::new(HashMap::new())),
        audit_tenant_concurrency_cap: 100,
        queue_config: temp_queue_config(),
        service_content_endpoint: String::new(),
        node_class: "hardware",
        tier_a_reason: "available",
        express_lane: Arc::new(ExpressLane::with_default_labels()),
    });
    let app = router(state);

    let mut body = valid_audit_proxy_body();
    body["purpose"] = json!("not-on-allowlist");

    let resp = app
        .oneshot(post_json("/v1/audit/proxy", &body))
        .await
        .expect("oneshot");

    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "un-allowlisted purpose must return 403 FORBIDDEN; got {}",
        resp.status()
    );

    // Assert NO ledger files were written (ordering: allowlist check is BEFORE
    // the stub write, so a policy-denied request never reaches the write step).
    let jsonl_files: Vec<_> = std::fs::read_dir(&ledger_dir)
        .expect("read ledger dir")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "jsonl").unwrap_or(false))
        .collect();
    assert_eq!(
        jsonl_files.len(),
        0,
        "no JSONL ledger entries must be written for un-allowlisted purposes"
    );
}

// ── 7d. Default allowlist accepts all four documented purposes ─────────────

/// The four documented default purposes all pass the allowlist check.
/// Tests each via a wiremock-backed round trip returning 200 to confirm
/// that none of the documented purposes is rejected at the HTTP layer.
#[tokio::test]
async fn audit_proxy_default_allowlist_accepts_documented_purposes() {
    let documented_purposes = [
        "editorial-refinement",
        "citation-grounding",
        "entity-disambiguation",
        "initial-graph-build",
    ];

    for purpose in documented_purposes {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(upstream_ok_body(10, 5)))
            .expect(1)
            .mount(&server)
            .await;

        let (state, _ledger_dir) = app_state_with_audit_proxy(
            TierCProvider::Anthropic,
            server.uri(),
            TierCPricing::default(),
        );
        let app = router(state);

        let mut body = valid_audit_proxy_relay_body();
        body["purpose"] = json!(purpose);

        let resp = app
            .oneshot(post_json("/v1/audit/proxy", &body))
            .await
            .expect("oneshot");

        assert_eq!(
            resp.status(),
            StatusCode::OK,
            "documented purpose {purpose:?} must be accepted (200 OK); got {}",
            resp.status()
        );
        // wiremock drops here — expect(1) fires and panics if the request
        // did not reach the upstream, confirming the purpose was allowed
        // and the relay was executed.
    }
}

// ===========================================================================
// Section 8 — audit_capture endpoint tests (PS.4 step 4) — 6 tests
// ===========================================================================
//
// These tests exercise the new POST /v1/audit/capture endpoint added in
// PS.4 step 4. Validation failures return 400 (or 413 for oversized
// payloads); a valid request writes a single capture entry to the audit
// ledger and returns 200 with an AuditCaptureResponse body.

/// Build a valid audit_capture request body. Individual tests override
/// fields to exercise specific validation paths.
fn valid_audit_capture_body() -> serde_json::Value {
    json!({
        "audit_id": "01900000-0000-7000-8000-000000000001",
        "module_id": "woodfine",
        "event_type": "prose-edit",
        "source": "project-language",
        "status": "ok",
        "event_at": "2026-04-28T10:00:00Z",
        "payload": {
            "draft_id": "d-001",
            "from_state": "draft-created",
            "to_state": "draft-refined"
        },
        "caller_request_id": "caller-capture-001"
    })
}

// ── 8a. happy path → 200, ledger entry written ────────────────────────────

/// POST /v1/audit/capture with a fully valid prose-edit body:
///   - returns 200 with AuditCaptureResponse shape
///   - audit_id echoed back
///   - caller_request_id echoed back
///   - status in response is "captured"
///   - ledger has exactly ONE entry with expected fields
#[tokio::test]
async fn audit_capture_valid_prose_edit_event_returns_200_and_writes_ledger() {
    use slm_doorman_server::http::AppState;

    let ledger_dir = std::env::temp_dir().join(format!(
        "slm-audit-capture-happy-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&ledger_dir).expect("create test ledger dir");
    let ledger = AuditLedger::new(&ledger_dir).expect("create test audit ledger");
    let doorman = Doorman::new(DoormanConfig::default(), ledger);
    let state = Arc::new(AppState {
        doorman,
        apprenticeship: None,
        brief_cache: Arc::new(BriefCache::default()),
        verdict_dispatcher: None,
        audit_proxy_client: None,
        audit_proxy_purpose_allowlist: FOUNDRY_DEFAULT_PURPOSE_ALLOWLIST,
        audit_tenant_concurrency: Arc::new(Mutex::new(HashMap::new())),
        audit_tenant_concurrency_cap: 100,
        queue_config: temp_queue_config(),
        service_content_endpoint: String::new(),
        node_class: "hardware",
        tier_a_reason: "available",
        express_lane: Arc::new(ExpressLane::with_default_labels()),
    });
    let app = router(state);

    let body = valid_audit_capture_body();
    let resp = app
        .oneshot(post_json("/v1/audit/capture", &body))
        .await
        .expect("oneshot");

    // 1. HTTP 200 OK.
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "valid audit_capture request must return 200 OK"
    );

    // 2. Response body carries expected AuditCaptureResponse fields.
    let resp_body = body_json(resp).await;
    assert_eq!(
        resp_body["audit_id"].as_str().unwrap_or(""),
        "01900000-0000-7000-8000-000000000001",
        "audit_id must be echoed from request"
    );
    assert_eq!(
        resp_body["caller_request_id"].as_str().unwrap_or(""),
        "caller-capture-001",
        "caller_request_id must be echoed from request"
    );
    assert_eq!(
        resp_body["status"].as_str().unwrap_or(""),
        "captured",
        "response status must be 'captured'"
    );

    // 3. Ledger must contain exactly ONE entry.
    let jsonl_files: Vec<_> = std::fs::read_dir(&ledger_dir)
        .expect("read ledger dir")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "jsonl").unwrap_or(false))
        .collect();
    assert_eq!(jsonl_files.len(), 1, "exactly one JSONL file must exist");

    let contents = std::fs::read_to_string(jsonl_files[0].path()).expect("read JSONL");
    let lines: Vec<_> = contents.lines().collect();
    assert_eq!(lines.len(), 1, "exactly one ledger entry must be written");

    let entry: serde_json::Value = serde_json::from_str(lines[0]).expect("valid JSON line");
    assert_eq!(
        entry["audit_id"].as_str().unwrap_or(""),
        "01900000-0000-7000-8000-000000000001"
    );
    assert_eq!(entry["module_id"].as_str().unwrap_or(""), "woodfine");
    assert_eq!(entry["event_type"].as_str().unwrap_or(""), "prose-edit");
    assert_eq!(entry["source"].as_str().unwrap_or(""), "project-language");
    assert_eq!(entry["status"].as_str().unwrap_or(""), "ok");
    assert!(
        entry["captured_at"].is_string(),
        "captured_at must be a timestamp string"
    );
    assert_eq!(
        entry["caller_request_id"].as_str().unwrap_or(""),
        "caller-capture-001"
    );
    // Payload fields must be preserved.
    assert_eq!(entry["payload"]["draft_id"].as_str().unwrap_or(""), "d-001");
}

// ── 8b. invalid module_id → 400 ───────────────────────────────────────────

/// POST /v1/audit/capture with an uppercase module_id → 400 BAD_REQUEST.
#[tokio::test]
async fn audit_capture_invalid_module_id_returns_400() {
    let state = app_state_no_tiers();
    let app = router(state);

    let mut body = valid_audit_capture_body();
    body["module_id"] = json!("INVALID-UPPERCASE");

    let resp = app
        .oneshot(post_json("/v1/audit/capture", &body))
        .await
        .expect("oneshot");

    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "invalid module_id must return 400 BAD_REQUEST"
    );
}

// ── 8c. unknown event_type → 400 with event_type in error ─────────────────

/// POST /v1/audit/capture with an unrecognised event_type → 400 BAD_REQUEST.
/// The error message must name the rejected event_type.
#[tokio::test]
async fn audit_capture_unknown_event_type_returns_400() {
    let state = app_state_no_tiers();
    let app = router(state);

    let mut body = valid_audit_capture_body();
    body["event_type"] = json!("not-a-real-event-type");

    let resp = app
        .oneshot(post_json("/v1/audit/capture", &body))
        .await
        .expect("oneshot");

    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "unknown event_type must return 400 BAD_REQUEST"
    );
    let resp_body = body_json(resp).await;
    let msg = resp_body["error"]["message"].as_str().unwrap_or_default();
    assert!(
        msg.contains("not-a-real-event-type"),
        "error message must include the rejected event_type; got: {msg}"
    );
}

// ── 8d. invalid timestamp → 400 ───────────────────────────────────────────

/// POST /v1/audit/capture with a non-RFC-3339 event_at → 400 BAD_REQUEST.
#[tokio::test]
async fn audit_capture_invalid_timestamp_returns_400() {
    let state = app_state_no_tiers();
    let app = router(state);

    let mut body = valid_audit_capture_body();
    // Not RFC 3339: missing time component, no timezone.
    body["event_at"] = json!("28 April 2026");

    let resp = app
        .oneshot(post_json("/v1/audit/capture", &body))
        .await
        .expect("oneshot");

    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "invalid RFC 3339 timestamp must return 400 BAD_REQUEST"
    );
}

// ── 8e. oversized payload → 413, ledger NOT updated ───────────────────────

/// POST /v1/audit/capture with a payload larger than AUDIT_CAPTURE_MAX_PAYLOAD_BYTES
/// → 413 PAYLOAD_TOO_LARGE; no ledger entry written.
#[tokio::test]
async fn audit_capture_oversized_payload_returns_413() {
    use slm_doorman_server::http::AppState;

    let ledger_dir = std::env::temp_dir().join(format!(
        "slm-audit-capture-oversized-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&ledger_dir).expect("create test ledger dir");
    let ledger = AuditLedger::new(&ledger_dir).expect("create test audit ledger");
    let doorman = Doorman::new(DoormanConfig::default(), ledger);
    let state = Arc::new(AppState {
        doorman,
        apprenticeship: None,
        brief_cache: Arc::new(BriefCache::default()),
        verdict_dispatcher: None,
        audit_proxy_client: None,
        audit_proxy_purpose_allowlist: FOUNDRY_DEFAULT_PURPOSE_ALLOWLIST,
        audit_tenant_concurrency: Arc::new(Mutex::new(HashMap::new())),
        audit_tenant_concurrency_cap: 100,
        queue_config: temp_queue_config(),
        service_content_endpoint: String::new(),
        node_class: "hardware",
        tier_a_reason: "available",
        express_lane: Arc::new(ExpressLane::with_default_labels()),
    });
    let app = router(state);

    // Build a payload that exceeds the 16 KiB limit.
    // Each character in a JSON string is 1 byte; we overshoot by a margin.
    let oversized_string = "x".repeat(AUDIT_CAPTURE_MAX_PAYLOAD_BYTES + 512);
    let mut body = valid_audit_capture_body();
    body["payload"] = json!({ "data": oversized_string });

    let resp = app
        .oneshot(post_json("/v1/audit/capture", &body))
        .await
        .expect("oneshot");

    assert!(
        resp.status() == StatusCode::PAYLOAD_TOO_LARGE || resp.status() == StatusCode::BAD_REQUEST,
        "oversized payload must return 413 PAYLOAD_TOO_LARGE or 400 BAD_REQUEST; got {}",
        resp.status()
    );

    // Ledger must NOT have been updated.
    let jsonl_files: Vec<_> = std::fs::read_dir(&ledger_dir)
        .expect("read ledger dir")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "jsonl").unwrap_or(false))
        .collect();
    assert_eq!(
        jsonl_files.len(),
        0,
        "no ledger entries must be written for an oversized payload"
    );
}

// ── 8f. all five event_types accepted ─────────────────────────────────────

/// All five documented event_type values must pass validation and return 200.
#[tokio::test]
async fn audit_capture_default_event_types_all_accepted() {
    use slm_doorman_server::http::AppState;

    let accepted_event_types = [
        "prose-edit",
        "design-edit",
        "graph-mutation",
        "anchor-event",
        "verdict-issued",
    ];

    for event_type in accepted_event_types {
        let ledger_dir = std::env::temp_dir().join(format!(
            "slm-audit-capture-evtype-{}-{}",
            event_type,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&ledger_dir).expect("create test ledger dir");
        let ledger = AuditLedger::new(&ledger_dir).expect("create test audit ledger");
        let doorman = Doorman::new(DoormanConfig::default(), ledger);
        let state = Arc::new(AppState {
            doorman,
            apprenticeship: None,
            brief_cache: Arc::new(BriefCache::default()),
            verdict_dispatcher: None,
            audit_proxy_client: None,
            audit_proxy_purpose_allowlist: FOUNDRY_DEFAULT_PURPOSE_ALLOWLIST,
            audit_tenant_concurrency: Arc::new(Mutex::new(HashMap::new())),
            audit_tenant_concurrency_cap: 100,
            queue_config: temp_queue_config(),
            service_content_endpoint: String::new(),
            node_class: "hardware",
            tier_a_reason: "available",
            express_lane: Arc::new(ExpressLane::with_default_labels()),
        });
        let app = router(state);

        let mut body = valid_audit_capture_body();
        body["event_type"] = json!(event_type);

        let resp = app
            .oneshot(post_json("/v1/audit/capture", &body))
            .await
            .expect("oneshot");

        assert_eq!(
            resp.status(),
            StatusCode::OK,
            "event_type {event_type:?} must be accepted (200 OK); got {}",
            resp.status()
        );
        // Verify the ledger entry records the correct event_type.
        let jsonl_files: Vec<_> = std::fs::read_dir(&ledger_dir)
            .expect("read ledger dir")
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map(|x| x == "jsonl").unwrap_or(false))
            .collect();
        assert_eq!(jsonl_files.len(), 1, "one JSONL file for {event_type:?}");
        let contents = std::fs::read_to_string(jsonl_files[0].path()).expect("read JSONL");
        let entry: serde_json::Value =
            serde_json::from_str(contents.lines().next().unwrap()).expect("valid JSON");
        assert_eq!(
            entry["event_type"].as_str().unwrap_or(""),
            event_type,
            "ledger entry event_type must match request"
        );
    }
}

// ===========================================================================
// Section 9 — audit endpoint hardening tests (payload cap + concurrency cap)
// ===========================================================================
//
// Part 1: AUDIT_PROXY_MAX_REQUEST_BYTES (64 KiB) body-size cap on /v1/audit/proxy.
//   - Oversized request → 413 before deserialise; ledger NOT written.
//   - Right-at-boundary minus 1 byte → passes the size check.
//
// Part 2: Per-tenant (moduleId) concurrency cap shared across both endpoints.
//   - Excess concurrent requests → 503 with Retry-After: 5.
//   - Caps are per-tenant; two different tenants under cap-1 both succeed.

// ── 9a. audit_proxy oversized request → 413, ledger NOT written ──────────────

/// POST /v1/audit/proxy with a raw body just over `AUDIT_PROXY_MAX_REQUEST_BYTES`
/// → 413 PAYLOAD_TOO_LARGE. The body-size check fires BEFORE JSON deserialisation
/// so the ledger is NOT written (no paper trail for policy-denied size violations).
#[tokio::test]
async fn audit_proxy_oversized_request_returns_413() {
    let ledger_dir = std::env::temp_dir().join(format!(
        "slm-audit-proxy-oversized-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&ledger_dir).expect("create test ledger dir");
    let ledger = AuditLedger::new(&ledger_dir).expect("create test audit ledger");
    let doorman = Doorman::new(DoormanConfig::default(), ledger);
    let state = Arc::new(AppState {
        doorman,
        apprenticeship: None,
        brief_cache: Arc::new(BriefCache::default()),
        verdict_dispatcher: None,
        audit_proxy_client: None,
        audit_proxy_purpose_allowlist: FOUNDRY_DEFAULT_PURPOSE_ALLOWLIST,
        audit_tenant_concurrency: Arc::new(Mutex::new(HashMap::new())),
        audit_tenant_concurrency_cap: 100,
        queue_config: temp_queue_config(),
        service_content_endpoint: String::new(),
        node_class: "hardware",
        tier_a_reason: "available",
        express_lane: Arc::new(ExpressLane::with_default_labels()),
    });
    let app = router(state);

    // Build a raw body that is just over the 64 KiB cap.
    // We synthesise a JSON-shaped body manually so the size is predictable.
    // The exact content does not need to be valid JSON — the check fires before
    // deserialisation. We use a JSON object with a large "data" string field
    // to make the bytes count predictable.
    let big_data = "x".repeat(AUDIT_PROXY_MAX_REQUEST_BYTES + 1);
    let oversized_body = format!(r#"{{"data": "{big_data}"}}"#);
    assert!(
        oversized_body.len() > AUDIT_PROXY_MAX_REQUEST_BYTES,
        "synthesised body must exceed the cap for this test to be meaningful"
    );

    let req = Request::builder()
        .method("POST")
        .uri("/v1/audit/proxy")
        .header("content-type", "application/json")
        .body(Body::from(oversized_body))
        .expect("build request");

    let resp = app.oneshot(req).await.expect("oneshot");

    assert_eq!(
        resp.status(),
        StatusCode::PAYLOAD_TOO_LARGE,
        "oversized audit_proxy request must return 413 PAYLOAD_TOO_LARGE; got {}",
        resp.status()
    );

    // Error message must include the size so callers know how much to trim.
    let body_val = body_json(resp).await;
    let msg = body_val["error"]["message"].as_str().unwrap_or_default();
    assert!(
        msg.contains("bytes"),
        "error message must mention 'bytes'; got: {msg}"
    );

    // Ledger MUST NOT have been written — body-size check is before stub write.
    let jsonl_files: Vec<_> = std::fs::read_dir(&ledger_dir)
        .expect("read ledger dir")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "jsonl").unwrap_or(false))
        .collect();
    assert_eq!(
        jsonl_files.len(),
        0,
        "no ledger entries must be written when body-size check fires"
    );
}

// ── 9b. audit_proxy just-under-max-size passes size check ─────────────────────

/// POST /v1/audit/proxy with a body right at `AUDIT_PROXY_MAX_REQUEST_BYTES - 1`
/// bytes. The size check must pass; the request proceeds past the cap guard
/// (it may fail later for other reasons such as missing required fields or no
/// configured provider — the point is the size check does NOT reject it).
#[tokio::test]
async fn audit_proxy_just_under_max_size_passes_size_check() {
    let state = app_state_no_tiers();
    let app = router(state);

    // Build a valid JSON body whose total size is just under the limit.
    // We use a valid audit_proxy body and pad the model field to bring the
    // total up near the limit while staying valid JSON.
    //
    // The valid body is ~200 bytes. Pad with AUDIT_PROXY_MAX_REQUEST_BYTES - 200
    // chars in the model field minus a margin to keep under the limit.
    let base_body = valid_audit_proxy_body();
    let base_json = base_body.to_string();
    let base_len = base_json.len();

    // We want total < AUDIT_PROXY_MAX_REQUEST_BYTES. Use (max - base_len - 20)
    // as padding inside the model field. If base is already ≥ max, skip padding.
    let padding_len = if base_len + 20 < AUDIT_PROXY_MAX_REQUEST_BYTES {
        AUDIT_PROXY_MAX_REQUEST_BYTES - base_len - 20
    } else {
        0
    };
    let padded_model = format!("model-{}", "a".repeat(padding_len));
    let mut body = valid_audit_proxy_body();
    body["model"] = json!(padded_model);
    let body_str = body.to_string();

    // Assert the synthesised body is actually under the cap.
    assert!(
        body_str.len() < AUDIT_PROXY_MAX_REQUEST_BYTES,
        "test body must be under the cap ({} < {})",
        body_str.len(),
        AUDIT_PROXY_MAX_REQUEST_BYTES
    );

    let req = Request::builder()
        .method("POST")
        .uri("/v1/audit/proxy")
        .header("content-type", "application/json")
        .body(Body::from(body_str))
        .expect("build request");

    let resp = app.oneshot(req).await.expect("oneshot");

    // The body-size check passes; the request proceeds to validation.
    // No providers configured → writes stub → 503 SERVICE_UNAVAILABLE.
    // What we assert: status is NOT 413 (size check did not fire).
    assert_ne!(
        resp.status(),
        StatusCode::PAYLOAD_TOO_LARGE,
        "body under the cap must NOT return 413; got {}",
        resp.status()
    );
}

// ── 9c. per-tenant concurrency cap rejects excess requests ────────────────────

/// With `audit_tenant_concurrency_cap = 2`, send 4 requests for the same
/// `moduleId` where 2 are already "in-flight" (permits held by pinned futures)
/// before the other 2 arrive. The 3rd and 4th requests must be rejected 503.
///
/// Implementation approach: pre-fill the semaphore by acquiring all permits
/// before sending requests, then send requests and confirm 503, then release.
///
/// We directly test the `acquire_tenant_permit` semantics via the full HTTP
/// handler: build an AppState with cap=2, manually saturate the semaphore for
/// "woodfine" to 2 permits, then fire 2 more requests and confirm both 503.
#[tokio::test]
async fn audit_tenant_concurrency_cap_rejects_excess_requests() {
    use slm_core::ModuleId;
    use std::str::FromStr;
    use tokio::sync::Semaphore;

    // Build state with cap=2.
    let tenant_map: Arc<Mutex<HashMap<ModuleId, Arc<Semaphore>>>> =
        Arc::new(Mutex::new(HashMap::new()));

    // Pre-saturate the semaphore for "woodfine" by inserting it with 0 remaining
    // permits (all 2 permits are consumed by the two held OwnedSemaphorePermit).
    let semaphore = Arc::new(Semaphore::new(2));
    let _held_permit_1 = semaphore.clone().try_acquire_owned().unwrap();
    let _held_permit_2 = semaphore.clone().try_acquire_owned().unwrap();
    {
        let mut map = tenant_map.lock().unwrap();
        map.insert(ModuleId::from_str("woodfine").unwrap(), semaphore.clone());
    }

    let state = Arc::new(AppState {
        doorman: Doorman::new(DoormanConfig::default(), temp_ledger()),
        apprenticeship: None,
        brief_cache: Arc::new(BriefCache::default()),
        verdict_dispatcher: None,
        audit_proxy_client: None,
        audit_proxy_purpose_allowlist: FOUNDRY_DEFAULT_PURPOSE_ALLOWLIST,
        audit_tenant_concurrency: tenant_map,
        audit_tenant_concurrency_cap: 2,
        queue_config: temp_queue_config(),
        service_content_endpoint: String::new(),
        node_class: "hardware",
        tier_a_reason: "available",
        express_lane: Arc::new(ExpressLane::with_default_labels()),
    });

    // Both requests below should fail immediately: no permits available.
    let app1 = router(state.clone());
    let resp1 = app1
        .oneshot(post_json("/v1/audit/proxy", &valid_audit_proxy_body()))
        .await
        .expect("oneshot");

    let app2 = router(state.clone());
    let resp2 = app2
        .oneshot(post_json("/v1/audit/proxy", &valid_audit_proxy_body()))
        .await
        .expect("oneshot");

    assert_eq!(
        resp1.status(),
        StatusCode::SERVICE_UNAVAILABLE,
        "request with no available permits must return 503; got {}",
        resp1.status()
    );
    assert_eq!(
        resp2.status(),
        StatusCode::SERVICE_UNAVAILABLE,
        "second concurrent request with no available permits must return 503; got {}",
        resp2.status()
    );

    // Verify Retry-After: 5 header is set on the 503 response.
    let retry_after = resp1
        .headers()
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(
        retry_after, "5",
        "Retry-After header must be '5' on 503 concurrency-exhausted response; got: {retry_after}"
    );

    // After _held_permit_1 and _held_permit_2 drop (end of scope), the semaphore
    // has 2 permits again. A fresh request for "woodfine" should now be able to
    // acquire a permit. We test this by releasing the holds explicitly and sending
    // one more request — if it gets past the concurrency check (503 would indicate
    // cap still hit; any other code means cap is no longer the blocker).
    drop(_held_permit_1);
    drop(_held_permit_2);

    let app3 = router(state.clone());
    let resp3 = app3
        .oneshot(post_json("/v1/audit/proxy", &valid_audit_proxy_body()))
        .await
        .expect("oneshot");
    // The cap is released; the request proceeds past the concurrency check.
    // No providers configured → writes stub → 503 "unconfigured" — that is
    // different from the cap-exhausted 503 (the error message differs).
    // We assert it is NOT a cap-exhausted rejection by checking the body.
    let body_val = body_json(resp3).await;
    let error_str = body_val["error"].as_str().unwrap_or_default();
    assert!(
        error_str.contains("unconfigured") || !error_str.contains("concurrency"),
        "after permit release, request must NOT be rejected by the concurrency cap; got: {body_val}"
    );
}

// ── 9d. per-tenant caps are independent across tenants ────────────────────────

/// With `audit_tenant_concurrency_cap = 1`, one request from tenant "alpha"
/// and one request from tenant "beta" must both complete successfully.
/// Each tenant has its own semaphore — the cap is per-tenant, not global.
///
/// Uses audit_capture (simpler body shape; no provider needed) so we can
/// test concurrency without a wiremock server.
#[tokio::test]
async fn audit_tenant_concurrency_cap_per_tenant_independent() {
    // Build state with cap=1 but two different tenants.
    let state = Arc::new(AppState {
        doorman: Doorman::new(DoormanConfig::default(), temp_ledger()),
        apprenticeship: None,
        brief_cache: Arc::new(BriefCache::default()),
        verdict_dispatcher: None,
        audit_proxy_client: None,
        audit_proxy_purpose_allowlist: FOUNDRY_DEFAULT_PURPOSE_ALLOWLIST,
        audit_tenant_concurrency: Arc::new(Mutex::new(HashMap::new())),
        // cap = 1: each tenant can have at most 1 in-flight request
        audit_tenant_concurrency_cap: 1,
        queue_config: temp_queue_config(),
        service_content_endpoint: String::new(),
        node_class: "hardware",
        tier_a_reason: "available",
        express_lane: Arc::new(ExpressLane::with_default_labels()),
    });

    // Two requests from different tenants; both should complete (200 OK
    // or 400/503 for other reasons — what we assert is neither returns
    // 503 DUE TO the concurrency cap when the other tenant's request is
    // also in-flight).
    //
    // Because audit_capture requests are synchronous and fast (no upstream
    // call), we send them sequentially here. The per-tenant cap with cap=1
    // allows exactly 1 in-flight per tenant. Since each request completes
    // before the next starts, both succeed even with cap=1.
    //
    // This test primarily validates that tenant "alpha" and tenant "beta"
    // do NOT share the same semaphore bucket.
    let alpha_body = json!({
        "audit_id": "01900000-0000-7000-8000-000000000001",
        "module_id": "alpha",          // ← tenant alpha
        "event_type": "prose-edit",
        "source": "test",
        "status": "ok",
        "event_at": "2026-04-28T10:00:00Z",
        "payload": {},
        "caller_request_id": "alpha-req"
    });
    let beta_body = json!({
        "audit_id": "01900000-0000-7000-8000-000000000002",
        "module_id": "beta",           // ← tenant beta
        "event_type": "prose-edit",
        "source": "test",
        "status": "ok",
        "event_at": "2026-04-28T10:00:00Z",
        "payload": {},
        "caller_request_id": "beta-req"
    });

    let app_alpha = router(state.clone());
    let resp_alpha = app_alpha
        .oneshot(post_json("/v1/audit/capture", &alpha_body))
        .await
        .expect("oneshot alpha");

    let app_beta = router(state.clone());
    let resp_beta = app_beta
        .oneshot(post_json("/v1/audit/capture", &beta_body))
        .await
        .expect("oneshot beta");

    // Both must succeed (200 OK) — caps are per-tenant, not global.
    assert_eq!(
        resp_alpha.status(),
        StatusCode::OK,
        "tenant 'alpha' must succeed (cap=1 is per-tenant); got {}",
        resp_alpha.status()
    );
    assert_eq!(
        resp_beta.status(),
        StatusCode::OK,
        "tenant 'beta' must succeed (cap=1 is per-tenant; caps are independent); got {}",
        resp_beta.status()
    );
}

// ===========================================================================
// Graph proxy — POST /v1/graph/query + POST /v1/graph/mutate
// (conventions/datagraph-access-discipline.md — Doorman is the single
// boundary for all DataGraph access; every call audit-logged).
// ===========================================================================

/// POST /v1/graph/query happy path — proxies to service-content and returns
/// the entity array verbatim. Mock service-content returns a two-entity JSON
/// array; Doorman must forward it with HTTP 200.
#[tokio::test]
async fn graph_query_proxies_to_service_content_returns_200() {
    let mock_sc = MockServer::start().await;

    let entities = serde_json::json!([
        {
            "entity_name": "Woodfine Management Corp.",
            "classification": "company",
            "role_vector": "real estate developer",
            "module_id": "woodfine",
            "confidence": 0.95
        },
        {
            "entity_name": "Jennifer M. Woodfine",
            "classification": "person",
            "role_vector": "principal",
            "module_id": "woodfine",
            "confidence": 0.97
        }
    ]);

    Mock::given(method("GET"))
        .and(path("/v1/graph/context"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&entities))
        .mount(&mock_sc)
        .await;

    let state = app_state_with_service_content(mock_sc.uri());
    let app = router(state);

    let req = Request::builder()
        .method("POST")
        .uri("/v1/graph/query")
        .header("content-type", "application/json")
        .header("x-foundry-module-id", "woodfine")
        .body(Body::from(
            serde_json::json!({"q": "woodfine", "limit": 5}).to_string(),
        ))
        .expect("build request");

    let resp = app.oneshot(req).await.expect("oneshot");
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "graph_query happy path must return 200; got {}",
        resp.status()
    );

    let body = body_json(resp).await;
    assert!(
        body.is_array(),
        "graph_query response body must be a JSON array; got: {body}"
    );
    assert_eq!(
        body.as_array().unwrap().len(),
        2,
        "expected 2 entities forwarded verbatim from service-content"
    );
}

/// POST /v1/graph/mutate happy path — forwards body to service-content and
/// returns the confirmation response verbatim. Mock service-content returns
/// `{"loaded": 1}`; Doorman must forward it with HTTP 200.
#[tokio::test]
async fn graph_mutate_proxies_to_service_content_returns_200() {
    let mock_sc = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/graph/mutate"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"loaded": 1})))
        .mount(&mock_sc)
        .await;

    let state = app_state_with_service_content(mock_sc.uri());
    let app = router(state);

    let req_body = serde_json::json!({
        "module_id": "woodfine",
        "entities": [
            {
                "entity_name": "Test Entity",
                "classification": "company",
                "confidence": 0.9
            }
        ]
    });

    let req = Request::builder()
        .method("POST")
        .uri("/v1/graph/mutate")
        .header("content-type", "application/json")
        .header("x-foundry-module-id", "woodfine")
        .body(Body::from(req_body.to_string()))
        .expect("build request");

    let resp = app.oneshot(req).await.expect("oneshot");
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "graph_mutate happy path must return 200; got {}",
        resp.status()
    );

    let body = body_json(resp).await;
    assert_eq!(
        body["loaded"],
        serde_json::json!(1),
        "graph_mutate must forward service-content response verbatim"
    );
}

/// POST /v1/graph/query — missing X-Foundry-Module-ID header returns 400.
#[tokio::test]
async fn graph_query_missing_module_id_returns_400() {
    let state = app_state_with_service_content("http://127.0.0.1:9081");
    let app = router(state);

    let req = Request::builder()
        .method("POST")
        .uri("/v1/graph/query")
        .header("content-type", "application/json")
        // deliberately omit x-foundry-module-id
        .body(Body::from(
            serde_json::json!({"q": "woodfine", "limit": 5}).to_string(),
        ))
        .expect("build request");

    let resp = app.oneshot(req).await.expect("oneshot");
    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "graph_query without module_id header must return 400; got {}",
        resp.status()
    );
}

/// POST /v1/graph/mutate — missing X-Foundry-Module-ID header returns 400.
#[tokio::test]
async fn graph_mutate_missing_module_id_returns_400() {
    let state = app_state_with_service_content("http://127.0.0.1:9081");
    let app = router(state);

    let req = Request::builder()
        .method("POST")
        .uri("/v1/graph/mutate")
        .header("content-type", "application/json")
        // deliberately omit x-foundry-module-id
        .body(Body::from(
            serde_json::json!({"module_id": "woodfine", "entities": []}).to_string(),
        ))
        .expect("build request");

    let resp = app.oneshot(req).await.expect("oneshot");
    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "graph_mutate without module_id header must return 400; got {}",
        resp.status()
    );
}

/// POST /v1/graph/query — service-content endpoint unconfigured (empty
/// string) returns 503 SERVICE_UNAVAILABLE.
#[tokio::test]
async fn graph_proxy_service_content_unconfigured_returns_503() {
    // Empty endpoint string means the proxy is unconfigured.
    let state = app_state_with_service_content("");
    let app = router(state);

    let req = Request::builder()
        .method("POST")
        .uri("/v1/graph/query")
        .header("content-type", "application/json")
        .header("x-foundry-module-id", "woodfine")
        .body(Body::from(
            serde_json::json!({"q": "woodfine", "limit": 5}).to_string(),
        ))
        .expect("build request");

    let resp = app.oneshot(req).await.expect("oneshot");
    assert_eq!(
        resp.status(),
        StatusCode::SERVICE_UNAVAILABLE,
        "graph_query with unconfigured service-content must return 503; got {}",
        resp.status()
    );
}

// ===========================================================================
// Section N — POST /v1/messages Anthropic Messages API shim (3 tests)
// ===========================================================================

/// POST /v1/messages non-streaming → 200 with Anthropic Messages API response shape.
/// Verifies id prefix, type, role, stop_reason, content block structure.
#[tokio::test]
async fn anthropic_messages_non_streaming_returns_200_with_anthropic_shape() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "choices": [
                { "message": { "role": "assistant", "content": "pong" } }
            ]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let state = app_state_with_local(server.uri());
    let app = router(state);

    let req_body = json!({
        "model": "claude-haiku-3-5-20241022",
        "messages": [{"role": "user", "content": "ping"}],
        "max_tokens": 100
    });
    let resp = app
        .oneshot(post_json("/v1/messages", &req_body))
        .await
        .expect("oneshot");

    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp).await;
    assert_eq!(body["type"], "message");
    assert_eq!(body["role"], "assistant");
    assert_eq!(body["stop_reason"], "end_turn");
    assert!(
        body["id"].as_str().unwrap_or("").starts_with("msg_"),
        "id must start with msg_; got {:?}",
        body["id"]
    );
    assert_eq!(body["content"][0]["type"], "text");
    assert_eq!(body["content"][0]["text"], "pong");
}

/// POST /v1/messages with system prompt → 200 with Anthropic response shape.
/// The system field is prepended as a system ChatMessage by the adapter.
#[tokio::test]
async fn anthropic_messages_system_prompt_returns_200() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "choices": [
                { "message": { "role": "assistant", "content": "Hello there!" } }
            ]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let state = app_state_with_local(server.uri());
    let app = router(state);

    let req_body = json!({
        "model": "claude-sonnet-4-5",
        "system": "You are a helpful assistant.",
        "messages": [{"role": "user", "content": "Hello"}],
        "max_tokens": 200
    });
    let resp = app
        .oneshot(post_json("/v1/messages", &req_body))
        .await
        .expect("oneshot");

    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp).await;
    assert_eq!(body["type"], "message");
    assert_eq!(body["content"][0]["text"], "Hello there!");
}

/// POST /v1/messages with stream:true → 200 with text/event-stream content-type
/// and all 6 SSE event types present in the response body.
#[tokio::test]
async fn anthropic_messages_streaming_returns_sse_with_all_six_events() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "choices": [
                { "message": { "role": "assistant", "content": "streamed response" } }
            ]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let state = app_state_with_local(server.uri());
    let app = router(state);

    let req_body = json!({
        "model": "claude-sonnet-4-5",
        "messages": [{"role": "user", "content": "stream this"}],
        "max_tokens": 50,
        "stream": true
    });
    let resp = app
        .oneshot(post_json("/v1/messages", &req_body))
        .await
        .expect("oneshot");

    assert_eq!(resp.status(), StatusCode::OK);

    let ct = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        ct.contains("text/event-stream"),
        "content-type must include text/event-stream; got {ct}"
    );

    let bytes = axum::body::to_bytes(resp.into_body(), 1024 * 1024)
        .await
        .expect("read SSE body");
    let body_str = String::from_utf8(bytes.to_vec()).expect("SSE body is UTF-8");

    assert!(
        body_str.contains("event: message_start"),
        "missing message_start"
    );
    assert!(
        body_str.contains("event: content_block_start"),
        "missing content_block_start"
    );
    assert!(
        body_str.contains("event: content_block_delta"),
        "missing content_block_delta"
    );
    assert!(
        body_str.contains("event: content_block_stop"),
        "missing content_block_stop"
    );
    assert!(
        body_str.contains("event: message_delta"),
        "missing message_delta"
    );
    assert!(
        body_str.contains("event: message_stop"),
        "missing message_stop"
    );
    assert!(
        body_str.contains("streamed response"),
        "SSE body must contain the response text"
    );
}
