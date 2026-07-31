// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Shared types for service-slm.
//!
//! `slm-core` holds only types and small value-objects. It has no async
//! runtime, no HTTP client, no I/O. Crates that route, log, or serve HTTP
//! depend on this crate; nothing in this crate depends on them.

pub mod apprenticeship;
pub mod error;
pub mod mesh;
pub mod module_id;
pub mod request_id;
pub mod tier;

pub use apprenticeship::{
    ApprenticeshipAttempt, ApprenticeshipBrief, ApprenticeshipVerdict, BriefScope, SeniorRole,
    VerdictOutcome, APPRENTICE_ESCALATE_THRESHOLD, DEFAULT_BRIEF_TIER_B_THRESHOLD_CHARS,
    VERDICT_BATCH_NAMESPACE, VERDICT_NAMESPACE,
};
pub use error::{CoreError, Result};
pub use mesh::{EnergySource, EnvironmentMetadata, NodeDescriptor, NodeId};
pub use module_id::ModuleId;
pub use request_id::RequestId;
pub use tier::{Complexity, SpeculationRequest, Tier};

use serde::{Deserialize, Serialize};

/// One inbound message in OpenAI chat-completions shape.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

/// Decode-time grammar constraint that the caller wants the Doorman to
/// enforce on the backend's output.
///
/// Wire shape (adjacent-tagged, matching OpenAI `function_call` / `tool_choice`
/// conventions):
///
/// ```json
/// {"type": "lark",        "value": "start: ..."}
/// {"type": "gbnf",        "value": "root ::= ..."}
/// {"type": "json-schema", "value": {"type": "object", ...}}
/// ```
///
/// Tier translation policy:
/// - **Tier A** (llama-server): `gbnf` and `json-schema` accepted natively;
///   `lark` rejected (llama-server HTTP API does not accept Lark grammars).
/// - **Tier B** (vLLM ≥0.12): all three variants forwarded via
///   `extra_body.structured_outputs.{grammar,json_schema}`.
/// - **Tier C** (external API): all variants rejected — no arbitrary grammar
///   support across vendors. Steps 2-5 (PS.3) wire the per-tier logic.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "kebab-case")]
pub enum GrammarConstraint {
    /// Lark EBNF grammar string.
    Lark(String),
    /// GBNF grammar string (llama.cpp native format).
    Gbnf(String),
    /// JSON Schema object. Version not pinned at the type level; schema
    /// validity is the backend's responsibility.
    JsonSchema(serde_json::Value),
}

/// Metadata attached by the calling Totebox session. Populated by
/// `bin/edit-via-doorman.sh` from `FOUNDRY_*` env vars exported at session
/// start (AGENT.md §3b). Doorman uses this for audit-ledger tagging and
/// Yo-Yo routing fallback; it is stripped before any Tier C (external API)
/// call.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionContext {
    pub archive_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archive_domain: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tetrad_summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_task: Option<String>,
}

/// Request crossing the Doorman boundary.
///
/// `request_id` and `module_id` are mandatory — they tag every audit-ledger
/// entry and every multi-tenant routing decision. `tier_hint` is advisory;
/// the router may override based on budget caps and request shape.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ComputeRequest {
    pub request_id: RequestId,
    pub module_id: ModuleId,
    pub model: Option<String>,
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub complexity: Complexity,
    #[serde(default)]
    pub tier_hint: Option<Tier>,
    #[serde(default)]
    pub stream: bool,
    #[serde(default)]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub temperature: Option<f32>,
    /// True if the caller has already sanitised identifiers out of the
    /// payload per the Doorman Protocol (`ARCHITECTURE.md` §1).
    #[serde(default)]
    pub sanitised_outbound: bool,
    /// Required to dispatch to Tier C (External API). The Doorman
    /// refuses any request hinted at Tier C without an allowlisted
    /// label per `~/Foundry/conventions/llm-substrate-decision.md`.
    /// Optional for Tier A and Tier B.
    #[serde(default)]
    pub tier_c_label: Option<String>,
    /// Optional identifier for Multi-Yo-Yo routing. When multiple Tier B
    /// endpoints are configured, this selects which Yo-Yo instance to use.
    #[serde(default)]
    pub yoyo_label: Option<String>,
    /// Optional decode-time grammar constraint. When present the Doorman
    /// translates the constraint into the backend-specific wire format for
    /// the selected tier. Absent from most requests; omitted from the
    /// serialised form when `None` so existing callers are unaffected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grammar: Option<GrammarConstraint>,
    /// Optional speculative decoding configuration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speculation: Option<SpeculationRequest>,
    /// When `Some(false)`, graph-context injection is skipped for this request.
    /// Defaults to `None` (injection enabled when service-content is configured).
    /// The Anthropic Messages shim sets this to `Some(false)` to prevent
    /// DataGraph entity rows from bloating Claude Code prompts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub graph_context_enabled: Option<bool>,
    /// Anthropic-format `tools` array passed through to the backend tier.
    /// Present only when the Anthropic Messages shim receives a tools-enabled
    /// request. Absent from most requests; omitted from the serialised form
    /// when `None` so existing callers are unaffected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<serde_json::Value>,
    /// Optional stop sequences. When present, generation halts at the first
    /// match. Translated to the backend's native stop-sequence field by each
    /// tier client. Absent from most requests; omitted from serialised form
    /// when `None` so existing callers are unaffected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
    /// Calling Totebox archive identity. Populated by `bin/edit-via-doorman.sh`
    /// from `FOUNDRY_*` env vars; absent from all existing requests. Stripped
    /// before Tier C (external API) calls. See `SessionContext`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_context: Option<SessionContext>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal `ComputeRequest` with only mandatory fields.
    fn minimal_request() -> ComputeRequest {
        use std::str::FromStr;
        ComputeRequest {
            request_id: RequestId::new(),
            module_id: ModuleId::from_str("test-module").unwrap(),
            model: None,
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: "hello".to_string(),
            }],
            complexity: Complexity::default(),
            tier_hint: None,
            stream: false,
            max_tokens: None,
            temperature: None,
            sanitised_outbound: false,
            tier_c_label: None,
            yoyo_label: None,
            grammar: None,
            speculation: None,
            graph_context_enabled: None,
            tools: None,
            stop_sequences: None,
            session_context: None,
        }
    }

    #[test]
    fn compute_request_serde_round_trip_no_grammar() {
        let req = minimal_request();
        let json = serde_json::to_string(&req).unwrap();
        // grammar field must be absent when None (skip_serializing_if)
        assert!(
            !json.contains("grammar"),
            "serialised form must not contain 'grammar' key when grammar is None; got: {json}"
        );
        let req2: ComputeRequest = serde_json::from_str(&json).unwrap();
        assert!(req2.grammar.is_none());
    }

    #[test]
    fn compute_request_serde_round_trip_lark() {
        let grammar_str = r#"start: /[a-z]+/ NEWLINE?"#;
        let mut req = minimal_request();
        req.grammar = Some(GrammarConstraint::Lark(grammar_str.to_string()));

        let json = serde_json::to_string(&req).unwrap();
        // Wire shape must contain the lark type discriminant.
        assert!(
            json.contains(r#""type":"lark""#),
            "expected lark type discriminant; got: {json}"
        );
        // The grammar string value must survive in the serialised form.
        assert!(
            json.contains(grammar_str),
            "grammar string must survive serialisation; got: {json}"
        );

        // Round-trip: deserialise back and check equality.
        let req2: ComputeRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(
            req2.grammar,
            Some(GrammarConstraint::Lark(grammar_str.to_string()))
        );
    }

    #[test]
    fn compute_request_serde_round_trip_gbnf() {
        let grammar_str = r#"root ::= "yes" | "no""#;
        let mut req = minimal_request();
        req.grammar = Some(GrammarConstraint::Gbnf(grammar_str.to_string()));

        let json = serde_json::to_string(&req).unwrap();
        assert!(
            json.contains(r#""type":"gbnf""#),
            "expected gbnf type discriminant; got: {json}"
        );

        let req2: ComputeRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(
            req2.grammar,
            Some(GrammarConstraint::Gbnf(grammar_str.to_string()))
        );
    }

    #[test]
    fn compute_request_serde_round_trip_json_schema() {
        let schema: serde_json::Value = serde_json::json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"},
                "age":  {"type": "integer"}
            },
            "required": ["name"]
        });
        let mut req = minimal_request();
        req.grammar = Some(GrammarConstraint::JsonSchema(schema.clone()));

        let json = serde_json::to_string(&req).unwrap();
        assert!(
            json.contains(r#""type":"json-schema""#),
            "expected json-schema type discriminant; got: {json}"
        );

        let req2: ComputeRequest = serde_json::from_str(&json).unwrap();
        match req2.grammar {
            Some(GrammarConstraint::JsonSchema(v)) => {
                assert_eq!(v, schema, "JSON Schema value must be preserved exactly");
            }
            other => panic!("expected JsonSchema variant, got: {other:?}"),
        }
    }

    #[test]
    fn compute_request_default_grammar_is_none() {
        // Construct a request without setting grammar; verify the field is None.
        let req = minimal_request();
        assert!(
            req.grammar.is_none(),
            "grammar must default to None when not set"
        );

        // Also verify that deserialising a JSON object that lacks the grammar
        // key produces grammar: None (serde default attribute).
        let json_without_grammar = serde_json::json!({
            "request_id": req.request_id,
            "module_id": req.module_id,
            "messages": [{"role": "user", "content": "test"}],
        })
        .to_string();
        let req2: ComputeRequest = serde_json::from_str(&json_without_grammar).unwrap();
        assert!(
            req2.grammar.is_none(),
            "grammar must default to None when absent from JSON"
        );
    }

    #[test]
    fn session_context_absent_when_none() {
        let req = minimal_request();
        let json = serde_json::to_string(&req).unwrap();
        assert!(
            !json.contains("session_context"),
            "session_context must not appear in serialised form when None; got: {json}"
        );
        let req2: ComputeRequest = serde_json::from_str(&json).unwrap();
        assert!(req2.session_context.is_none());
    }

    #[test]
    fn session_context_round_trip() {
        let mut req = minimal_request();
        req.session_context = Some(SessionContext {
            archive_name: "project-intelligence".to_string(),
            archive_domain: Some("intelligence".to_string()),
            tetrad_summary: None,
            current_task: Some("feat(service-slm): SessionContext".to_string()),
        });
        let json = serde_json::to_string(&req).unwrap();
        assert!(
            json.contains("project-intelligence"),
            "archive_name must appear in serialised form; got: {json}"
        );
        assert!(
            !json.contains("tetrad_summary"),
            "tetrad_summary must be absent when None; got: {json}"
        );
        let req2: ComputeRequest = serde_json::from_str(&json).unwrap();
        let sc = req2
            .session_context
            .expect("session_context must survive round-trip");
        assert_eq!(sc.archive_name, "project-intelligence");
        assert_eq!(sc.archive_domain.as_deref(), Some("intelligence"));
        assert!(sc.tetrad_summary.is_none());
    }

    #[test]
    fn session_context_backward_compat_missing_field() {
        // A JSON payload without session_context (existing callers) must
        // deserialise successfully with session_context: None.
        let json = r#"{"request_id":"00000000-0000-0000-0000-000000000000","module_id":"test-module","messages":[{"role":"user","content":"hi"}]}"#;
        let req: ComputeRequest = serde_json::from_str(json).unwrap();
        assert!(req.session_context.is_none());
    }
}

// ---------------------------------------------------------------------------
// audit_proxy wire shapes (PS.4)
// ---------------------------------------------------------------------------

/// Request body for `POST /v1/audit/proxy`.
///
/// The caller (e.g., project-language editorial gateway) holds no provider
/// API keys. It submits a structured request to the Doorman; the Doorman
/// authenticates with the provider, captures the full request + response +
/// cost into the audit ledger, and returns the response.
///
/// PS.4 step 2 wires the upstream provider call; step 1 (this commit)
/// scaffolds validation + ledger stub + 503 placeholder response.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuditProxyRequest {
    /// Tenant identifier (e.g., "pointsav", "woodfine"). Validated as
    /// [`ModuleId`] — only `[a-z0-9-]`, 1..=64 chars.
    pub module_id: String,
    /// Audit purpose label — must match an entry in the Doorman's
    /// audit-purpose allowlist (PS.4 step 2 enforces the allowlist;
    /// step 1 only requires non-empty).
    pub purpose: String,
    /// Provider identifier. Accepted values: "anthropic", "gemini", "openai".
    pub provider: String,
    /// Model identifier on the provider (e.g., "claude-opus-4-7",
    /// "gemini-2.5-pro"). No `provider:` prefix required here — the
    /// provider field is already explicit.
    pub model: String,
    /// OpenAI-compatible chat-completion messages. Must be non-empty.
    pub messages: Vec<ChatMessage>,
    /// Optional sampling parameters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// Caller's request correlation ID for cross-system tracing. Doorman
    /// echoes it back in the response and records it in the audit ledger.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caller_request_id: Option<String>,
}

/// Response body for `POST /v1/audit/proxy` (step 1: scaffold / stub).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuditProxyResponse {
    /// Doorman-generated UUIDv7 audit-ledger entry ID. Present even when
    /// the upstream call is pending (stub phase), so paper trails exist
    /// for attempted proxy calls.
    pub audit_id: String,
    /// Echoed from the request's `caller_request_id` field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caller_request_id: Option<String>,
    /// Response content (provider's reply). Empty / placeholder in step 1.
    pub content: String,
    /// Token / cost accounting.
    pub usage: AuditUsage,
}

/// Token usage and cost breakdown for an audit proxy call.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AuditUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub cost_usd: f64,
}

// ---------------------------------------------------------------------------
// audit_capture wire shapes (PS.4 step 4)
// ---------------------------------------------------------------------------

/// Request body for `POST /v1/audit/capture`.
///
/// The inverse direction of `audit_proxy`: cross-cluster callers push audit
/// events to the Doorman for work they did LOCALLY without going through the
/// Doorman. The Doorman validates, writes to the central audit ledger, and
/// returns 200.
///
/// Used by:
///   - project-data anchor-emitter (event_type: "anchor-event")
///   - project-language editorial gateway (event_type: "prose-edit")
///   - Any Ring 1/2/3 producer that does work the audit ledger should record
///     but that did not route through the Doorman.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuditCaptureRequest {
    /// Caller-generated UUID (any version). Caller is the source of truth for
    /// its own audit_id (the work happened locally; Doorman is downstream).
    pub audit_id: String,
    /// Tenant identifier; validated as [`ModuleId`] — only `[a-z0-9-]`,
    /// 1..=64 chars.
    pub module_id: String,
    /// Event taxonomy discriminator. Accepted values:
    /// "prose-edit" | "design-edit" | "graph-mutation" | "anchor-event" |
    /// "verdict-issued".
    pub event_type: String,
    /// Caller's component / cluster identifier for traceability (e.g.
    /// "project-language", "project-data:anchor-emitter"). Free-form; must
    /// be non-empty.
    pub source: String,
    /// Status of the work the caller did. Same vocabulary as audit_proxy final
    /// entries: "ok" | "policy-denied" | "upstream-error" | other. Free-form;
    /// must be non-empty.
    pub status: String,
    /// ISO 8601 / RFC 3339 timestamp of the event (caller's clock).
    pub event_at: String,
    /// Event-specific payload (untyped JSON object). Future steps may
    /// validate per-event-type schemas; step 4 accepts any JSON value.
    pub payload: serde_json::Value,
    /// Optional caller request correlation ID for cross-system tracing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caller_request_id: Option<String>,
}

/// Response body for `POST /v1/audit/capture`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuditCaptureResponse {
    /// Echoed from the request — confirms the Doorman accepted and wrote.
    pub audit_id: String,
    /// Echoed from the request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caller_request_id: Option<String>,
    /// "captured" on success.
    pub status: String,
}

/// Response returned through the Doorman.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ComputeResponse {
    pub request_id: RequestId,
    pub tier_used: Tier,
    pub model: String,
    pub content: String,
    /// Think-model reasoning tokens when upstream uses `--reasoning-format deepseek`.
    /// Present means `content` is already clean JSON; absent means `content` may
    /// contain raw `<think>…</think>` blocks that callers must strip.
    #[serde(default)]
    pub reasoning_content: Option<String>,
    pub inference_ms: u64,
    pub cost_usd: f64,
    /// Yo-Yo or external-API implementation version, opaque string.
    #[serde(default)]
    pub upstream_version: Option<String>,
    /// OpenAI-format `tool_calls` array from the backend when the model
    /// chose to invoke a tool. The Anthropic Messages shim converts these
    /// into Anthropic `tool_use` content blocks in the SSE response.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// Extraction wire shapes (POST /v1/extract)
// ---------------------------------------------------------------------------

/// Input body for `POST /v1/extract`.
///
/// SYS-ADR-07 boundary: `text` must be unstructured prose only. The `schema`
/// field constrains the OUTPUT shape from the inference model; structured graph
/// facts must never be injected into the AI prompt verbatim.
///
/// `#[serde(deny_unknown_fields)]` locks the contract at the boundary — unknown
/// fields return 400 before any inference call, enforcing the ADR-07 scope.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtractionRequest {
    /// Unstructured prose to extract entities from.
    pub text: String,
    /// JSON Schema constraining the OUTPUT array from the inference model.
    pub schema: serde_json::Value,
    /// Tenant identifier — validated as [`ModuleId`].
    pub module_id: String,
    /// Per-request inference timeout in seconds (default 180).
    #[serde(default = "default_extraction_timeout")]
    pub timeout_secs: u64,
}

fn default_extraction_timeout() -> u64 {
    180
}

/// Output body for `POST /v1/extract`.
///
/// Always HTTP 200. When `deferred: true`, `entities` is empty and
/// `defer_reason` describes why inference was skipped.
#[derive(Debug, Serialize)]
pub struct ExtractionResponse {
    /// Extracted entity array. Empty (`[]`) when `deferred: true`.
    pub entities: Vec<serde_json::Value>,
    /// `"yoyo_trainer"` on success, `"deferred"` when unavailable.
    pub tier_used: String,
    /// Model identifier reported by the backend, or `"none"` when deferred.
    pub model: String,
    /// `true` when entities were successfully extracted and parsed.
    pub extraction_ok: bool,
    /// `true` when the request was deferred (Yo-Yo unavailable).
    pub deferred: bool,
    /// Present when `deferred: true`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub defer_reason: Option<DeferReason>,
}

/// Why an extraction request was deferred rather than executed.
#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DeferReason {
    /// The `"trainer"` Yo-Yo label is not configured on this Doorman.
    YoyoLabelUnconfigured,
    /// The Yo-Yo circuit breaker is open after consecutive failures.
    YoyoCircuitOpen,
    /// A transient upstream error from Tier B (timeout or 5xx before fallback).
    YoyoTransient,
    /// Tier B was unavailable and Tier A (local OLMo) also returned an error.
    TierAFailed,
    /// Tier A returned output that could not be parsed as a JSON entity array.
    ParseError,
    /// The extraction handler timed out waiting for inference to complete.
    Timeout,
    /// Both Tier B and Tier A reported unavailable; no tier could serve the request.
    AllTiersUnavailable,
    /// Tier A (local OLMo) has all concurrent inference slots occupied.
    /// The caller should back off and retry; exponential backoff recommended.
    OoseSaturated,
}
