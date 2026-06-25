// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Tier A — local OpenAI-compatible HTTP endpoint.
//!
//! Backed by mistral.rs (long-term Phase-2 runtime per SLM-STACK.md) or
//! llama-server (the Phase-1 prototype runtime per Master's v0.0.9
//! progress note — the runtime that A3 used). Both expose the same
//! OpenAI-compatible wire format, so the client does not branch on which
//! is running.

use std::sync::Arc;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use slm_core::{ChatMessage, ComputeRequest, ComputeResponse, GrammarConstraint, Tier};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tracing::debug;

use crate::error::{DoormanError, Result};

#[derive(Clone, Debug)]
pub struct LocalTierConfig {
    /// Base URL of the local OpenAI-compatible server, e.g.
    /// `http://127.0.0.1:8080`.
    pub endpoint: String,
    /// Default model identifier. Local Tier A runs OLMo 3 7B Q4
    /// (Apache-2.0 + Open Data Commons; see substrate decision).
    pub default_model: String,
}

pub struct LocalTierClient {
    config: LocalTierConfig,
    http: reqwest::Client,
    /// Total concurrent OLMo slots (SLM_LOCAL_CONCURRENT, default 2).
    /// Held by all callers — interactive and background alike. When full,
    /// the caller receives LocalSaturated immediately instead of queuing
    /// inside llama-server for up to 1 800 s.
    total_sem: Option<Arc<Semaphore>>,
    /// Background-only semaphore (SLM_BACKGROUND_CONCURRENT, default 1).
    /// Acquired BEFORE total_sem by extraction fallback and drain dispatch.
    /// Ensures at least one total slot remains free for interactive callers
    /// even when background work is in flight.
    background_sem: Option<Arc<Semaphore>>,
}

impl LocalTierClient {
    pub fn new(config: LocalTierConfig) -> Self {
        // 1800 s (30 min) covers OLMo 7B Q4_K_M CPU inference up to max_tokens=2048
        // at the observed rate of ~2 tok/s (1024 s theoretical max) plus prefill
        // overhead. Observed real-world runs on this hardware: 17–60 minutes.
        // The prior 120 s value caused an infinite retry loop: the drain worker
        // timed out before llama-server finished, Doorman re-queued the brief,
        // and the next attempt immediately timed out again.
        // Without this timeout the drain worker blocks indefinitely.
        Self {
            config,
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(1800))
                // Catch TCP-level hangs independently of the response timeout.
                // Tier A is always localhost, so 10 s is generous.
                .connect_timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("failed to build Tier A HTTP client"),
            total_sem: None,
            background_sem: None,
        }
    }

    /// Attach priority admission-control semaphores (production only).
    /// Called once after `new()` in the server startup path; tests call
    /// `new()` alone and pass `None` implicitly (no cap applied).
    pub fn with_semaphores(mut self, total: Arc<Semaphore>, background: Arc<Semaphore>) -> Self {
        self.total_sem = Some(total);
        self.background_sem = Some(background);
        self
    }

    pub fn endpoint(&self) -> &str {
        &self.config.endpoint
    }

    /// Interactive path: acquires one slot from `total_sem` only.
    /// Returns LocalSaturated immediately when the semaphore is full.
    fn try_acquire_interactive(&self) -> Result<Option<OwnedSemaphorePermit>> {
        match &self.total_sem {
            None => Ok(None),
            Some(sem) => sem
                .clone()
                .try_acquire_owned()
                .map(Some)
                .map_err(|_| DoormanError::LocalSaturated),
        }
    }

    /// Background path: acquires `background_sem` first (caps background
    /// concurrency), then `total_sem` (caps total OLMo load).
    /// If either semaphore is full, returns LocalSaturated immediately.
    /// When `background_sem` succeeds but `total_sem` fails, the background
    /// permit is dropped automatically before returning the error.
    fn try_acquire_background(
        &self,
    ) -> Result<(Option<OwnedSemaphorePermit>, Option<OwnedSemaphorePermit>)> {
        let bg = match &self.background_sem {
            None => None,
            Some(sem) => match sem.clone().try_acquire_owned() {
                Ok(p) => Some(p),
                Err(_) => return Err(DoormanError::LocalSaturated),
            },
        };
        let total = match &self.total_sem {
            None => None,
            Some(sem) => match sem.clone().try_acquire_owned() {
                Ok(p) => Some(p),
                Err(_) => return Err(DoormanError::LocalSaturated),
            },
        };
        Ok((bg, total))
    }

    /// Interactive caller (e.g. `/v1/chat/completions`).
    /// Acquires one total slot; returns LocalSaturated when saturated.
    pub async fn complete(&self, req: &ComputeRequest) -> Result<ComputeResponse> {
        let _permit = self.try_acquire_interactive()?;
        self.complete_inner(req).await
    }

    /// Background caller (extraction fallback, drain dispatch).
    /// Acquires background_sem then total_sem; returns LocalSaturated when
    /// either is full so the caller can back off without queuing in llama-server.
    pub async fn complete_background(&self, req: &ComputeRequest) -> Result<ComputeResponse> {
        let (_bg, _total) = self.try_acquire_background()?;
        self.complete_inner(req).await
    }

    async fn complete_inner(&self, req: &ComputeRequest) -> Result<ComputeResponse> {
        let model = req
            .model
            .clone()
            .unwrap_or_else(|| self.config.default_model.clone());

        // Translate GrammarConstraint → llama-server wire fields.
        // llama-server (llama.cpp HTTP API) accepts:
        //   `grammar`     — GBNF string at the top level of the request body
        //   `json_schema` — JSON Schema object at the top level
        // It does NOT accept Lark grammars (llama-server does not ship
        // llguidance). Lark is rejected here before any network call so the
        // caller can escalate to Tier B (vLLM ≥0.12, which supports Lark via
        // llguidance) or supply a GBNF equivalent. Per v0.1.33 Q1 ratification.
        let (grammar_field, json_schema_field) = match req.grammar.as_ref() {
            None => (None, None),
            Some(GrammarConstraint::Gbnf(s)) => (Some(s.clone()), None),
            Some(GrammarConstraint::JsonSchema(v)) => (None, Some(v.clone())),
            Some(GrammarConstraint::Lark(_)) => {
                return Err(DoormanError::TierAGrammarUnsupported {
                    dialect: "Lark",
                    advice: "escalate to Tier B (Yo-Yo) which supports Lark via llguidance, \
                             or provide a GBNF equivalent for Tier A",
                });
            }
        };

        let body = OpenAiChatRequest {
            model: model.clone(),
            messages: req.messages.clone(),
            stream: req.stream,
            max_tokens: req.max_tokens,
            temperature: req.temperature,
            grammar: grammar_field,
            json_schema: json_schema_field,
            stop: req.stop_sequences.clone(),
            tools: req.tools.as_ref().map(super::anthropic_tools_to_openai),
        };
        let url = format!(
            "{}/v1/chat/completions",
            self.config.endpoint.trim_end_matches('/')
        );
        debug!(target: "slm_doorman::tier::local", %url, %model, "tier-A request");

        let started = Instant::now();
        let resp: OpenAiChatResponse = self
            .http
            .post(&url)
            .json(&body)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let inference_ms = started.elapsed().as_millis() as u64;

        let msg = resp
            .choices
            .into_iter()
            .next()
            .map(|c| c.message)
            .ok_or_else(|| DoormanError::UpstreamShape("no choices in response".into()))?;

        Ok(ComputeResponse {
            request_id: req.request_id,
            tier_used: Tier::Local,
            model,
            content: msg.content.unwrap_or_default(),
            reasoning_content: None,
            inference_ms,
            // Tier A runs on already-paid-for VM compute; per substrate
            // decision the marginal cost is sunk in the VM cost.
            cost_usd: 0.0,
            upstream_version: None,
            tool_calls: msg.tool_calls,
        })
    }
}

#[derive(Serialize)]
struct OpenAiChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "is_false")]
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    /// GBNF grammar string. Top-level llama-server field (NOT inside
    /// `extra_body`). Absent when `None`.
    #[serde(skip_serializing_if = "Option::is_none")]
    grammar: Option<String>,
    /// JSON Schema for structured output. Top-level llama-server field.
    /// Absent when `None`.
    #[serde(skip_serializing_if = "Option::is_none")]
    json_schema: Option<serde_json::Value>,
    /// Stop sequences. Generation halts at the first match. llama-server
    /// accepts this as a top-level `stop` array. Absent when `None`.
    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<Vec<String>>,
    /// OpenAI-format tools array (converted from Anthropic format by
    /// `anthropic_tools_to_openai`). Absent when `None`.
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct OpenAiChatResponse {
    choices: Vec<OpenAiChatChoice>,
}

#[derive(Deserialize)]
struct OpenAiChatChoice {
    message: LocalAssistantMessage,
}

/// Assistant turn from llama-server. Content may be null when the model
/// chose to emit tool_calls instead of text.
#[derive(Deserialize)]
struct LocalAssistantMessage {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<serde_json::Value>,
}

fn is_false(b: &bool) -> bool {
    !*b
}

#[cfg(test)]
mod tests {
    use super::*;
    use slm_core::{
        ChatMessage, Complexity, ComputeRequest, GrammarConstraint, ModuleId, RequestId, Tier,
    };
    use std::str::FromStr;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn req() -> ComputeRequest {
        ComputeRequest {
            request_id: RequestId::new(),
            module_id: ModuleId::from_str("foundry").unwrap(),
            model: Some("OLMo-3-7B-Q4_K_M.gguf".into()),
            messages: vec![ChatMessage {
                role: "user".into(),
                content: "ping".into(),
            }],
            complexity: Complexity::Low,
            tier_hint: Some(Tier::Local),
            stream: false,
            max_tokens: Some(20),
            temperature: Some(0.0),
            sanitised_outbound: true,
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

    fn ok_body() -> serde_json::Value {
        serde_json::json!({
            "choices": [
                { "message": { "role": "assistant", "content": "PONG" } }
            ]
        })
    }

    fn client(server_uri: String) -> LocalTierClient {
        LocalTierClient::new(LocalTierConfig {
            endpoint: server_uri,
            default_model: "OLMo-3-7B-Q4_K_M.gguf".into(),
        })
    }

    /// Happy path — 200 with well-formed choices array. Verify:
    /// - content extracted correctly
    /// - tier_used is Local
    /// - model is echoed from the request
    /// - cost_usd is 0.0 (VM compute is sunk cost per architecture)
    /// - request shape includes model and messages (POST to /v1/chat/completions)
    #[tokio::test]
    async fn happy_path_200_returns_content_and_local_tier() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(ok_body()))
            .expect(1)
            .mount(&server)
            .await;

        let client = client(server.uri());
        let resp = client.complete(&req()).await.expect("happy path 200");
        assert_eq!(resp.tier_used, Tier::Local);
        assert_eq!(resp.content, "PONG");
        assert_eq!(resp.model, "OLMo-3-7B-Q4_K_M.gguf");
        assert_eq!(
            resp.cost_usd, 0.0,
            "Tier A cost is always 0.0 (sunk VM compute)"
        );
        assert!(
            resp.inference_ms < 10_000,
            "sanity: inference_ms should be wall-clock ms"
        );
    }

    /// Default model is used when request carries no model field.
    /// Tier A's default is the configured `LocalTierConfig::default_model`.
    #[tokio::test]
    async fn default_model_used_when_request_omits_model() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(ok_body()))
            .expect(1)
            .mount(&server)
            .await;

        let client = client(server.uri());
        let mut r = req();
        r.model = None; // no model in request — should fall back to config default
        let resp = client.complete(&r).await.expect("default model happy path");
        assert_eq!(resp.model, "OLMo-3-7B-Q4_K_M.gguf");
    }

    /// HTTP 5xx error via `error_for_status()` must surface as
    /// `DoormanError::Upstream` (the `#[from] reqwest::Error` variant).
    /// The Doorman does NOT retry on Tier A — a 500 from the local
    /// llama-server is an operator problem; the router is responsible for
    /// any fallback to Tier B.
    #[tokio::test]
    async fn http_5xx_surfaces_as_upstream_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(500))
            .expect(1)
            .mount(&server)
            .await;

        let client = client(server.uri());
        let err = client
            .complete(&req())
            .await
            .expect_err("500 must surface as error");
        assert!(
            matches!(err, DoormanError::Upstream(_)),
            "expected DoormanError::Upstream for 5xx, got {err:?}"
        );
    }

    /// Empty `choices` array — the server returned 200 but with no
    /// candidates. Must surface `DoormanError::UpstreamShape` naming
    /// the empty-choices case, with no content extracted.
    #[tokio::test]
    async fn empty_choices_surfaces_upstream_shape() {
        let server = MockServer::start().await;
        let empty_body = serde_json::json!({ "choices": [] });
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(empty_body))
            .expect(1)
            .mount(&server)
            .await;

        let client = client(server.uri());
        let err = client
            .complete(&req())
            .await
            .expect_err("empty choices must surface as error");
        match err {
            DoormanError::UpstreamShape(msg) => {
                assert!(
                    msg.contains("no choices"),
                    "UpstreamShape message should mention empty choices, got: {msg:?}"
                );
            }
            other => panic!("expected DoormanError::UpstreamShape, got {other:?}"),
        }
    }

    /// Malformed JSON response body — server returns 200 with an
    /// invalid JSON body. The `resp.json().await?` call returns a
    /// `reqwest::Error` which maps to `DoormanError::Upstream`.
    #[tokio::test]
    async fn malformed_json_body_surfaces_upstream_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(b"not json {".to_vec(), "application/json"),
            )
            .expect(1)
            .mount(&server)
            .await;

        let client = client(server.uri());
        let err = client
            .complete(&req())
            .await
            .expect_err("malformed JSON must surface as error");
        assert!(
            matches!(err, DoormanError::Upstream(_)),
            "expected DoormanError::Upstream for JSON parse failure, got {err:?}"
        );
    }

    // ── Grammar serialisation tests ────────────────────────────────────────

    /// When `grammar` is `None` the upstream body must contain neither
    /// `"grammar"` nor `"json_schema"` keys. Absence is verified both by
    /// parsing the captured body and by checking that the mock received
    /// exactly one request (sanity).
    #[tokio::test]
    async fn grammar_none_omits_all_grammar_fields() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(ok_body()))
            .expect(1)
            .mount(&server)
            .await;

        let client = client(server.uri());
        let mut r = req();
        r.grammar = None;
        client.complete(&r).await.expect("none grammar happy path");

        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1, "expected exactly one upstream request");
        let body: serde_json::Value =
            serde_json::from_slice(&requests[0].body).expect("request body must be valid JSON");
        assert!(
            body.get("grammar").is_none(),
            "body must not contain 'grammar' key when grammar is None"
        );
        assert!(
            body.get("json_schema").is_none(),
            "body must not contain 'json_schema' key when grammar is None"
        );
    }

    /// When `grammar` is `Some(Gbnf(...))` the upstream body must contain
    /// `"grammar": "<gbnf string>"` at the top level (NOT inside
    /// `extra_body`), and must NOT contain `"json_schema"`.
    #[tokio::test]
    async fn grammar_gbnf_serialises_into_top_level_grammar_field() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(ok_body()))
            .expect(1)
            .mount(&server)
            .await;

        let client = client(server.uri());
        let mut r = req();
        let gbnf = r#"root ::= "yes" | "no""#;
        r.grammar = Some(GrammarConstraint::Gbnf(gbnf.to_string()));
        client.complete(&r).await.expect("gbnf grammar happy path");

        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);
        let body: serde_json::Value =
            serde_json::from_slice(&requests[0].body).expect("request body must be valid JSON");
        assert_eq!(
            body.get("grammar").and_then(|v| v.as_str()),
            Some(gbnf),
            "body must contain top-level 'grammar' with the GBNF string"
        );
        assert!(
            body.get("json_schema").is_none(),
            "body must not contain 'json_schema' when grammar is Gbnf"
        );
        // Must NOT be nested inside extra_body (llama-server native field)
        assert!(
            body.get("extra_body").is_none(),
            "Tier A must NOT use extra_body; grammar goes at top level"
        );
    }

    /// When `grammar` is `Some(JsonSchema(...))` the upstream body must
    /// contain `"json_schema": <value>` at the top level and must NOT
    /// contain a `"grammar"` key.
    #[tokio::test]
    async fn grammar_json_schema_serialises_into_top_level_json_schema_field() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(ok_body()))
            .expect(1)
            .mount(&server)
            .await;

        let client = client(server.uri());
        let mut r = req();
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "answer": {"type": "string"}
            },
            "required": ["answer"]
        });
        r.grammar = Some(GrammarConstraint::JsonSchema(schema.clone()));
        client
            .complete(&r)
            .await
            .expect("json_schema grammar happy path");

        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);
        let body: serde_json::Value =
            serde_json::from_slice(&requests[0].body).expect("request body must be valid JSON");
        assert_eq!(
            body.get("json_schema"),
            Some(&schema),
            "body must contain top-level 'json_schema' with the schema value"
        );
        assert!(
            body.get("grammar").is_none(),
            "body must not contain 'grammar' key when grammar is JsonSchema"
        );
        assert!(
            body.get("extra_body").is_none(),
            "Tier A must NOT use extra_body; json_schema goes at top level"
        );
    }

    /// When `grammar` is `Some(Lark(...))` the call must return a typed
    /// `DoormanError::TierAGrammarUnsupported` error BEFORE making any
    /// network call. The wiremock server must have received zero requests.
    #[tokio::test]
    async fn grammar_lark_rejected_before_any_network_call() {
        let server = MockServer::start().await;
        // No mock registered — any request reaching the server would be
        // an unexpected call and cause the test to fail at server drop.

        let client = client(server.uri());
        let mut r = req();
        r.grammar = Some(GrammarConstraint::Lark("start: /[a-z]+/".to_string()));
        let err = client
            .complete(&r)
            .await
            .expect_err("Lark grammar must be rejected");

        assert!(
            matches!(
                err,
                DoormanError::TierAGrammarUnsupported {
                    dialect: "Lark",
                    ..
                }
            ),
            "expected TierAGrammarUnsupported with dialect=Lark, got {err:?}"
        );

        // Critical: no upstream call must have been made.
        let received = server.received_requests().await.unwrap();
        assert!(
            received.is_empty(),
            "Lark rejection must happen before any network call; \
             wiremock received {} request(s)",
            received.len()
        );
    }
}
