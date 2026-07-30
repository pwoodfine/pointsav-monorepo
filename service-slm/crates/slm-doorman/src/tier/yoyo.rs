// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Tier B — Yo-Yo cloud burst over the contract at
//! `infrastructure/slm-yoyo/CONTRACT.md`.
//!
//! Implements the **client** side of the Yo-Yo HTTP API:
//!
//! - `POST /v1/chat/completions` with `Authorization: Bearer <token>`
//!   plus the four `X-Foundry-*` headers
//!   (`Request-ID`, `Module-ID`, `Contract-Version`, `Complexity`)
//! - Retry-on-503 honouring `Retry-After` (one retry, capped at 60 s)
//! - Auth refresh on 401 / 403 (one retry against a fresh token)
//! - MAJOR contract mismatch on 410 (no retry; refuse loudly)
//! - Capture of `X-Foundry-Inference-Ms` and `X-Foundry-Yoyo-Version`
//!   response headers for the audit ledger
//!
//! Resilience stack (added 2026-05-06):
//! - 60 s reqwest socket timeout + 90 s tokio outer deadline
//! - Three-state circuit breaker (Closed → Open → HalfOpen → Closed)
//! - Background health probe: polls /health every 30 s; marks
//!   `health_up` false after 3 consecutive failures
//! - Both guards checked before every dispatch; fast path is two atomic loads
//!
//! Per operator direction relayed via Master 2026-04-26: this code is
//! mock-tested only — no `tofu apply`, no live HTTP, no real
//! bearer-token consumption against any provider. Live Yo-Yo
//! deployments are a separate Master-scope decision with explicit
//! cost-cap configuration.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use slm_core::{ChatMessage, ComputeRequest, ComputeResponse, GrammarConstraint, Tier};
use tracing::{debug, info, warn};

use crate::error::{DoormanError, Result};
use crate::tier::circuit_breaker::CircuitBreaker;

// ── Timeouts ─────────────────────────────────────────────────────────────────
/// Per-socket read timeout on the reqwest client. Covers slow body reads and
/// stalled TCP connections. Combined with the outer tokio deadline below.
/// Raised from 60 s to 180 s: Think-class models (OLMo-3 32B Think) spend
/// ~500 tokens on reasoning blocks before emitting the answer. At 8-9 tok/s
/// that takes 55-60 s just for thinking, exceeding the old 60 s limit.
const SOCKET_TIMEOUT: Duration = Duration::from_secs(180);

/// Hard outer deadline wrapping the entire complete() call, including any
/// 503 Retry-After sleep. Fires DoormanError::TierBTimeout to caller.
/// Raised from 90 s to 300 s to accommodate 180 s socket timeout + one retry.
const OUTER_DEADLINE: Duration = Duration::from_secs(300);

// ── Health probe ──────────────────────────────────────────────────────────────
/// How often the health probe polls /health.
const HEALTH_PROBE_INTERVAL: Duration = Duration::from_secs(30);
/// How many consecutive failures before health_up is set false.
const HEALTH_FAILURE_THRESHOLD: u32 = 3;
/// Timeout for each /health probe request (short — we don't want to block).
const HEALTH_PROBE_TIMEOUT: Duration = Duration::from_secs(2);

/// Bearer-token source for Tier B requests. Real implementations
/// (GCP Workload Identity, RunPod / Modal API key from Secret
/// Manager, customer mTLS / shared secret) implement this trait;
/// `StaticBearer` covers tests and dev-loop usage.
#[async_trait]
pub trait BearerTokenProvider: Send + Sync + std::fmt::Debug {
    /// Returns the current bearer token. Implementations may cache
    /// and refresh proactively.
    async fn token(&self) -> Result<String>;

    /// Forces a token refresh after a 401 / 403 response. Returns
    /// the freshly obtained token.
    async fn refresh(&self) -> Result<String>;
}

/// Static bearer token. Used by the Doorman binary when
/// `SLM_YOYO_BEARER` env var is set, and by the unit tests below.
#[derive(Clone, Debug)]
pub struct StaticBearer {
    token: String,
}

impl StaticBearer {
    pub fn new(token: impl Into<String>) -> Self {
        Self {
            token: token.into(),
        }
    }
}

#[async_trait]
impl BearerTokenProvider for StaticBearer {
    async fn token(&self) -> Result<String> {
        Ok(self.token.clone())
    }

    async fn refresh(&self) -> Result<String> {
        Ok(self.token.clone())
    }
}

/// GCP Compute Engine metadata-service identity token bearer.
/// Fetches a fresh token on every call — identity tokens expire after 1 h,
/// fetching on each request keeps them valid without manual rotation.
/// Enable via `SLM_YOYO_GCP_AUTH=true` when the Tier B endpoint is Cloud Run.
#[derive(Debug)]
pub struct MetadataBearer {
    audience: String,
    client: reqwest::Client,
}

impl MetadataBearer {
    pub fn new(audience: impl Into<String>) -> Self {
        Self {
            audience: audience.into(),
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .unwrap_or_default(),
        }
    }
}

#[async_trait]
impl BearerTokenProvider for MetadataBearer {
    async fn token(&self) -> Result<String> {
        let url = format!(
            "http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/identity?audience={}",
            self.audience
        );
        let token = self
            .client
            .get(&url)
            .header("Metadata-Flavor", "Google")
            .send()
            .await
            .map_err(|e| {
                crate::error::DoormanError::UpstreamShape(format!(
                    "metadata identity fetch failed: {e}"
                ))
            })?
            .text()
            .await
            .map_err(|e| {
                crate::error::DoormanError::UpstreamShape(format!(
                    "metadata identity read failed: {e}"
                ))
            })?;
        Ok(token.trim().to_string())
    }

    async fn refresh(&self) -> Result<String> {
        self.token().await
    }
}

/// Per-provider pricing for Tier B cost computation. CONTRACT.md does
/// not carry a cost field on the wire; the Doorman computes cost
/// deterministically as `(hourly_usd / 3_600_000) × inference_ms`
/// from operator-supplied configuration. Defaults are zero — meaning
/// "unknown / dev" — and the audit-ledger entry records `cost_usd:
/// 0.0` until the operator wires real rates per their cloud provider.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PricingConfig {
    /// Hourly USD rate for the configured Yo-Yo provider (e.g.
    /// `0.84` for GCP Cloud Run GPU L4, `0.34` for RunPod L4, per
    /// `~/Foundry/conventions/llm-substrate-decision.md` §"Three
    /// compute tiers"). Multiplied by inference-ms to produce the
    /// per-call cost recorded in the audit ledger.
    pub yoyo_hourly_usd: f64,
}

impl PricingConfig {
    /// Compute Tier B cost in USD from inference time in milliseconds.
    /// Returns 0.0 when `yoyo_hourly_usd` is zero (the dev default —
    /// "unknown" rather than mis-attributed).
    pub fn yoyo_cost_usd(&self, inference_ms: u64) -> f64 {
        const MS_PER_HOUR: f64 = 3_600_000.0;
        (self.yoyo_hourly_usd / MS_PER_HOUR) * inference_ms as f64
    }
}

#[derive(Clone, Debug)]
pub struct YoYoTierConfig {
    /// Base URL of the Yo-Yo node (e.g. `https://yoyo-foundry.run.app`).
    pub endpoint: String,
    /// Default model identifier. Yo-Yo runs `Olmo-3-1125-32B-Think`
    /// (canonical Allen AI name; see substrate decision).
    pub default_model: String,
    /// Contract version this client speaks. Sent in
    /// `X-Foundry-Contract-Version` per CONTRACT.md.
    pub contract_version: String,
    /// Per-provider pricing for Tier B cost computation. Empty default
    /// means cost_usd is 0.0 (community-tier / dev mode).
    pub pricing: PricingConfig,
    /// GCP zone where the Yo-Yo VM runs (e.g. "europe-west4-a").
    /// Read from `SLM_YOYO_GCP_ZONE` at construction; surfaced in /readyz.
    pub zone: Option<String>,
    /// Path polled by the background health probe (default: `/health`).
    /// Override via `SLM_YOYO_HEALTH_PATH` when the Tier B backend uses a
    /// different liveness endpoint (e.g. `/` for Ollama).
    pub health_path: String,
}

impl Default for YoYoTierConfig {
    fn default() -> Self {
        Self {
            endpoint: String::new(),
            default_model: "Olmo-3-1125-32B-Think".to_string(),
            contract_version: crate::YOYO_CONTRACT_VERSION.to_string(),
            pricing: PricingConfig::default(),
            zone: None,
            health_path: "/health".to_string(),
        }
    }
}

pub struct YoYoTierClient {
    config: YoYoTierConfig,
    http: reqwest::Client,
    bearer: Arc<dyn BearerTokenProvider>,
    /// True when the background health probe last saw /health return 200.
    /// Three consecutive failures flip this to false; one recovery resets.
    /// Checked by the router before dispatching — single atomic load.
    pub health_up: Arc<AtomicBool>,
    /// Unix epoch seconds at which health_up last transitioned to false.
    /// Zero means health is currently up (or was never down).
    /// Updated atomically by the health probe; read by health_down_secs().
    pub health_down_since_secs: Arc<AtomicU64>,
    /// Three-state circuit breaker driven by actual request outcomes.
    /// Shared with no other task; updated only by complete() return path.
    pub circuit: Arc<CircuitBreaker>,
    /// GCP zone for this node (from `SLM_YOYO_GCP_ZONE`). Surfaced in /readyz.
    pub zone: Option<String>,
}

impl YoYoTierClient {
    pub fn new(config: YoYoTierConfig, bearer: Arc<dyn BearerTokenProvider>) -> Self {
        let health_up = Arc::new(AtomicBool::new(true));
        let health_down_since_secs = Arc::new(AtomicU64::new(0));
        let circuit = Arc::new(CircuitBreaker::new());

        // Spawn background health probe if a tokio runtime is available.
        // guard with try_current() so unit tests that call new() outside
        // a runtime don't panic.
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            let health_up_clone = Arc::clone(&health_up);
            let health_down_clone = Arc::clone(&health_down_since_secs);
            let endpoint = config.endpoint.clone();
            let health_path = config.health_path.clone();
            handle.spawn(run_health_probe(
                endpoint,
                health_path,
                health_up_clone,
                health_down_clone,
                bearer.clone(),
            ));
        }

        let zone = config.zone.clone();
        Self {
            config,
            http: reqwest::Client::builder()
                .timeout(SOCKET_TIMEOUT)
                .danger_accept_invalid_certs(true)
                .build()
                .unwrap_or_default(),
            bearer,
            health_up,
            health_down_since_secs,
            circuit,
            zone,
        }
    }

    /// Returns seconds since the health probe last transitioned to down,
    /// or `None` when health is currently up. Independent of circuit state —
    /// useful for observing probe failures before they trip the circuit breaker.
    pub fn health_down_secs(&self) -> Option<u64> {
        let since = self.health_down_since_secs.load(Ordering::Relaxed);
        if since == 0 {
            None
        } else {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            Some(now.saturating_sub(since))
        }
    }

    pub fn endpoint(&self) -> &str {
        &self.config.endpoint
    }

    pub fn contract_version(&self) -> &str {
        &self.config.contract_version
    }

    /// Returns true if both the health probe and circuit breaker allow a request.
    pub fn allow_request(&self) -> bool {
        self.health_up.load(Ordering::Relaxed) && self.circuit.allow_request()
    }

    /// Route one request to Tier B with full resilience stack:
    /// circuit breaker check → 90 s outer deadline → 60 s socket timeout
    /// → 503 cold-start retry → 401/403 auth refresh.
    #[tracing::instrument(
        skip(self, req),
        fields(
            tier = "yoyo",
            model = tracing::field::Empty,
            request_id = %req.request_id,
            latency_ms = tracing::field::Empty,
            prompt_tokens = tracing::field::Empty,
            completion_tokens = tracing::field::Empty,
            cold_start = false,
            circuit_open = false,
        )
    )]
    pub async fn complete(&self, req: &ComputeRequest) -> Result<ComputeResponse> {
        // Circuit breaker fast path — no HTTP if the breaker is open.
        if !self.circuit.allow_request() {
            tracing::Span::current().record("circuit_open", true);
            return Err(DoormanError::TierBCircuitOpen);
        }

        let started = Instant::now();
        let span = tracing::Span::current();

        let result = tokio::time::timeout(OUTER_DEADLINE, self.inner_complete(req)).await;

        let elapsed_ms = started.elapsed().as_millis() as u64;
        span.record("latency_ms", elapsed_ms);

        match result {
            Ok(Ok(resp)) => {
                self.circuit.record_success();
                span.record("model", resp.model.as_str());
                Ok(resp)
            }
            Ok(Err(e)) => {
                self.circuit.record_failure();
                // reqwest issue #2839: when our socket timeout fires mid-stream,
                // reqwest reports "error decoding response body" rather than a
                // timeout error. Reclassify so callers apply timeout backoff.
                if let DoormanError::Upstream(ref re) = e {
                    if re.is_decode() {
                        warn!(
                            target: "slm_doorman::tier::yoyo",
                            latency_ms = elapsed_ms,
                            "Tier B: body-decode error reclassified as TierBTimeout (reqwest #2839)"
                        );
                        return Err(DoormanError::TierBTimeout);
                    }
                }
                Err(e)
            }
            Err(_elapsed) => {
                self.circuit.record_failure();
                warn!(
                    target: "slm_doorman::tier::yoyo",
                    latency_ms = elapsed_ms,
                    "Tier B: outer 300 s deadline exceeded"
                );
                Err(DoormanError::TierBTimeout)
            }
        }
    }

    async fn inner_complete(&self, req: &ComputeRequest) -> Result<ComputeResponse> {
        let model = req
            .model
            .clone()
            .unwrap_or_else(|| self.config.default_model.clone());

        // Translate GrammarConstraint → llama.cpp top-level structured-output fields.
        // The deployed Tier B server is llama.cpp (llama-server), which reads the
        // constraint from TOP-LEVEL request fields, NOT vLLM's
        // `extra_body.structured_outputs` (llama.cpp silently ignores extra_body →
        // unconstrained output). Confirmed empirically 2026-06-01: a top-level
        // `json_schema` field forces schema-conformant JSON; `extra_body` does nothing.
        //   JsonSchema → top-level `json_schema`: <schema object>
        //   Lark/GBNF  → top-level `grammar`: "<GBNF string>"
        // See BRIEF-slm-substrate-master.md §2.8.
        let mut json_schema: Option<serde_json::Value> = None;
        let mut grammar: Option<String> = None;
        match req.grammar.as_ref() {
            None => {}
            Some(GrammarConstraint::JsonSchema(v)) => json_schema = Some(v.clone()),
            Some(GrammarConstraint::Lark(s)) | Some(GrammarConstraint::Gbnf(s)) => {
                grammar = Some(s.clone())
            }
        }

        // Workaround for llama.cpp #20345: grammar constraints are silently ignored
        // when thinking is active. Suppress thinking on tool-enabled requests so the
        // model emits valid JSON. llama.cpp accepts a per-request `reasoning_budget`.
        let extra_body: Option<serde_json::Value> = if req.tools.is_some() {
            Some(serde_json::json!({ "reasoning_budget": 0 }))
        } else {
            None
        };

        let body = OpenAiChatRequest {
            model: model.clone(),
            messages: req.messages.clone(),
            stream: req.stream,
            max_tokens: req.max_tokens,
            temperature: req.temperature,
            json_schema,
            grammar,
            extra_body,
            tools: req.tools.as_ref().map(super::anthropic_tools_to_openai),
        };
        let url = format!(
            "{}/v1/chat/completions",
            self.config.endpoint.trim_end_matches('/')
        );
        debug!(target: "slm_doorman::tier::yoyo", %url, %model, "tier-B request");

        let started = Instant::now();
        let resp = self.send_with_retries(&url, &body, req).await?;

        // Capture Foundry response metadata before consuming the body.
        let inference_ms = resp
            .headers()
            .get("x-foundry-inference-ms")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or_else(|| started.elapsed().as_millis() as u64);
        let upstream_version = resp
            .headers()
            .get("x-foundry-yoyo-version")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        let body: OpenAiChatResponse = resp.json().await?;
        let msg = body
            .choices
            .into_iter()
            .next()
            .map(|c| c.message)
            .ok_or_else(|| DoormanError::UpstreamShape("no choices in response".into()))?;

        Ok(ComputeResponse {
            request_id: req.request_id,
            tier_used: Tier::Yoyo,
            model,
            content: msg.content.unwrap_or_default(),
            reasoning_content: msg.reasoning_content,
            inference_ms,
            cost_usd: self.config.pricing.yoyo_cost_usd(inference_ms),
            upstream_version,
            tool_calls: msg.tool_calls,
        })
    }

    /// Send one request with up to one retry. Retry policy:
    /// - 200..=299: success
    /// - 503 + Retry-After: sleep up to min(Retry-After, 60s) then retry once
    /// - 401 / 403: refresh token, retry once with fresh token
    /// - 410: contract MAJOR mismatch — refuse, no retry
    /// - other: surface as `UpstreamShape`
    ///
    /// Note: the 60 s Retry-After cap is intentional — the 90 s outer
    /// deadline in `complete()` bounds the worst-case total latency including
    /// the sleep, so the cap keeps the sleep well within budget.
    async fn send_with_retries(
        &self,
        url: &str,
        body: &OpenAiChatRequest,
        req: &ComputeRequest,
    ) -> Result<reqwest::Response> {
        let token = self.bearer.token().await?;
        let resp = self.send_once(url, body, req, &token).await?;
        let status = resp.status().as_u16();
        match status {
            200..=299 => Ok(resp),
            503 => {
                let retry_after = resp
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(1)
                    .min(60);
                warn!(
                    target: "slm_doorman::tier::yoyo",
                    retry_after_s = retry_after,
                    "503 cold start; sleeping then retrying once"
                );
                tracing::Span::current().record("cold_start", true);
                tokio::time::sleep(Duration::from_secs(retry_after)).await;
                let resp2 = self.send_once(url, body, req, &token).await?;
                if resp2.status().is_success() {
                    Ok(resp2)
                } else {
                    Err(DoormanError::UpstreamShape(format!(
                        "retry after 503 returned {}",
                        resp2.status()
                    )))
                }
            }
            401 | 403 => {
                warn!(
                    target: "slm_doorman::tier::yoyo",
                    status,
                    "auth failure; refreshing token and retrying once"
                );
                let new_token = self.bearer.refresh().await?;
                let resp2 = self.send_once(url, body, req, &new_token).await?;
                if resp2.status().is_success() {
                    Ok(resp2)
                } else {
                    Err(DoormanError::UpstreamShape(format!(
                        "retry after auth-refresh returned {}",
                        resp2.status()
                    )))
                }
            }
            410 => Err(DoormanError::ContractMajorMismatch {
                remote_status: 410,
                doorman_version: crate::YOYO_CONTRACT_VERSION,
            }),
            _ => {
                let body_preview = resp
                    .text()
                    .await
                    .unwrap_or_else(|e| format!("<body read failed: {e}>"));
                Err(DoormanError::UpstreamShape(format!(
                    "unexpected status {status}: {}",
                    body_preview.chars().take(200).collect::<String>()
                )))
            }
        }
    }

    /// Single authenticated POST. `send_once` is the sole point where
    /// the bearer token is injected — all retry paths funnel through here.
    async fn send_once(
        &self,
        url: &str,
        body: &OpenAiChatRequest,
        req: &ComputeRequest,
        token: &str,
    ) -> Result<reqwest::Response> {
        let resp = self
            .http
            .post(url)
            .bearer_auth(token)
            .header("X-Foundry-Request-ID", req.request_id.to_string())
            .header("X-Foundry-Module-ID", req.module_id.as_str())
            .header("X-Foundry-Contract-Version", &self.config.contract_version)
            .header("X-Foundry-Complexity", req.complexity.as_str())
            .json(body)
            .send()
            .await?;
        Ok(resp)
    }
}

/// Background health probe task. Spawned once per `YoYoTierClient::new()`.
/// Polls `<endpoint><health_path>` every 30 s with a 2 s timeout.
/// Three consecutive failures set `health_up` to false; one recovery resets.
/// `health_path` defaults to `/health` (llama-server); set to `/` for Ollama.
/// If bearer produces a token it is sent as `Authorization: Bearer <token>`.
async fn run_health_probe(
    endpoint: String,
    health_path: String,
    health_up: Arc<AtomicBool>,
    health_down_since_secs: Arc<AtomicU64>,
    bearer: Arc<dyn BearerTokenProvider>,
) {
    let http = reqwest::Client::builder()
        .timeout(HEALTH_PROBE_TIMEOUT)
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap_or_default();

    let mut consecutive_failures: u32 = 0;
    let url = format!("{}{}", endpoint.trim_end_matches('/'), health_path);

    info!(
        target: "slm_doorman::tier::yoyo",
        %url,
        interval_s = HEALTH_PROBE_INTERVAL.as_secs(),
        "health probe started"
    );

    loop {
        tokio::time::sleep(HEALTH_PROBE_INTERVAL).await;

        let mut req = http.get(&url);
        if let Ok(token) = bearer.token().await {
            if !token.is_empty() {
                req = req.bearer_auth(&token);
            }
        }
        let ok = req
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false);

        if ok {
            if consecutive_failures > 0 {
                info!(
                    target: "slm_doorman::tier::yoyo",
                    prev_failures = consecutive_failures,
                    "health probe: Tier B recovered"
                );
            }
            consecutive_failures = 0;
            health_down_since_secs.store(0, Ordering::Relaxed);
            health_up.store(true, Ordering::Relaxed);
        } else {
            consecutive_failures += 1;
            if consecutive_failures >= HEALTH_FAILURE_THRESHOLD {
                warn!(
                    target: "slm_doorman::tier::yoyo",
                    consecutive_failures,
                    "health probe: Tier B marked unavailable"
                );
                // Record the timestamp only on the first transition to down
                // (when health_down_since_secs is still 0).
                health_down_since_secs.compare_exchange(
                    0,
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ).ok();
                health_up.store(false, Ordering::Relaxed);
            } else {
                debug!(
                    target: "slm_doorman::tier::yoyo",
                    consecutive_failures,
                    "health probe: transient failure (not yet marking unavailable)"
                );
            }
        }
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
    /// llama.cpp top-level JSON-schema constraint. When present, llama-server
    /// converts it to a GBNF grammar and forces schema-conformant output.
    /// This is what actually constrains the deployed Tier B server (llama.cpp);
    /// `extra_body.structured_outputs` (vLLM format) is ignored by it.
    #[serde(skip_serializing_if = "Option::is_none")]
    json_schema: Option<serde_json::Value>,
    /// llama.cpp top-level GBNF grammar string (Lark/GBNF constraints).
    #[serde(skip_serializing_if = "Option::is_none")]
    grammar: Option<String>,
    /// vLLM-only structured-output / reasoning envelope. Retained for the tool
    /// turn `reasoning_budget` workaround; llama.cpp ignores unknown keys here.
    #[serde(skip_serializing_if = "Option::is_none")]
    extra_body: Option<serde_json::Value>,
    /// Anthropic-format tools array. Passed through when the caller
    /// requested tool-enabled inference. Absent when `None`.
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct OpenAiChatResponse {
    choices: Vec<OpenAiChatChoice>,
}

#[derive(Deserialize)]
struct OpenAiChatChoice {
    message: OpenAiAssistantMessage,
}

/// Upstream assistant turn — adds `reasoning_content` for `--reasoning-format deepseek`.
/// When present, `content` is already clean JSON and reasoning tokens are isolated here.
/// Content may be null when the model chose to emit tool_calls instead of text.
#[derive(Deserialize)]
struct OpenAiAssistantMessage {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    reasoning_content: Option<String>,
    #[serde(default)]
    tool_calls: Option<serde_json::Value>,
}

fn is_false(b: &bool) -> bool {
    !*b
}

#[cfg(test)]
mod tests {
    use super::*;
    use slm_core::{ChatMessage, ComputeRequest, ModuleId, RequestId};
    use std::str::FromStr;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, Request, ResponseTemplate};

    fn req() -> ComputeRequest {
        ComputeRequest {
            request_id: RequestId::new(),
            module_id: ModuleId::from_str("foundry").unwrap(),
            model: Some("Olmo-3-1125-32B-Think".into()),
            messages: vec![ChatMessage {
                role: "user".into(),
                content: "ping".into(),
            }],
            complexity: slm_core::Complexity::High,
            tier_hint: Some(Tier::Yoyo),
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

    fn client(server_uri: String) -> YoYoTierClient {
        client_with_pricing(server_uri, PricingConfig::default())
    }

    fn client_with_pricing(server_uri: String, pricing: PricingConfig) -> YoYoTierClient {
        YoYoTierClient::new(
            YoYoTierConfig {
                endpoint: server_uri,
                default_model: "Olmo-3-1125-32B-Think".into(),
                contract_version: crate::YOYO_CONTRACT_VERSION.into(),
                pricing,
                zone: None,
                health_path: "/health".to_string(),
            },
            Arc::new(StaticBearer::new("test-token-v1")),
        )
    }

    /// Happy path — 200 with the four required X-Foundry-* request
    /// headers and a content string in the response.
    #[tokio::test]
    async fn happy_path_200_round_trips_with_headers() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .and(header("authorization", "Bearer test-token-v1"))
            .and(header("x-foundry-module-id", "foundry"))
            .and(header(
                "x-foundry-contract-version",
                crate::YOYO_CONTRACT_VERSION,
            ))
            .and(header("x-foundry-complexity", "high"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("x-foundry-yoyo-version", "mistralrs:0.8")
                    .insert_header("x-foundry-inference-ms", "412")
                    .set_body_json(ok_body()),
            )
            .expect(1)
            .mount(&server)
            .await;

        let client = client(server.uri());
        let resp = client.complete(&req()).await.expect("happy path 200");
        assert_eq!(resp.tier_used, Tier::Yoyo);
        assert_eq!(resp.content, "PONG");
        assert_eq!(resp.inference_ms, 412);
        assert_eq!(resp.upstream_version.as_deref(), Some("mistralrs:0.8"));
    }

    /// 503 + Retry-After cold-start: client sleeps then retries once;
    /// the second response succeeds.
    #[tokio::test]
    async fn cold_start_503_retries_once_then_succeeds() {
        let server = MockServer::start().await;
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(move |_req: &Request| {
                let n = counter_clone.fetch_add(1, Ordering::SeqCst);
                if n == 0 {
                    ResponseTemplate::new(503).insert_header("retry-after", "1")
                } else {
                    ResponseTemplate::new(200)
                        .insert_header("x-foundry-inference-ms", "200")
                        .set_body_json(ok_body())
                }
            })
            .expect(2)
            .mount(&server)
            .await;

        let client = client(server.uri());
        let resp = client
            .complete(&req())
            .await
            .expect("retry after 503 should succeed");
        assert_eq!(resp.content, "PONG");
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    /// 401 unauthorised: client refreshes the bearer token and retries
    /// once with the fresh token.
    #[tokio::test]
    async fn auth_failure_401_refreshes_and_retries() {
        #[derive(Debug)]
        struct FlippingBearer {
            v1: String,
            v2: String,
            refreshed: AtomicUsize,
        }
        #[async_trait]
        impl BearerTokenProvider for FlippingBearer {
            async fn token(&self) -> Result<String> {
                Ok(self.v1.clone())
            }
            async fn refresh(&self) -> Result<String> {
                self.refreshed.fetch_add(1, Ordering::SeqCst);
                Ok(self.v2.clone())
            }
        }

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .and(header("authorization", "Bearer stale-token"))
            .respond_with(ResponseTemplate::new(401))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .and(header("authorization", "Bearer fresh-token"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("x-foundry-inference-ms", "300")
                    .set_body_json(ok_body()),
            )
            .expect(1)
            .mount(&server)
            .await;

        let bearer = Arc::new(FlippingBearer {
            v1: "stale-token".into(),
            v2: "fresh-token".into(),
            refreshed: AtomicUsize::new(0),
        });
        let client = YoYoTierClient::new(
            YoYoTierConfig {
                endpoint: server.uri(),
                default_model: "Olmo-3-1125-32B-Think".into(),
                contract_version: crate::YOYO_CONTRACT_VERSION.into(),
                pricing: PricingConfig::default(),
                zone: None,
                health_path: "/health".to_string(),
            },
            bearer.clone(),
        );

        let resp = client
            .complete(&req())
            .await
            .expect("retry after 401 should succeed");
        assert_eq!(resp.content, "PONG");
        assert_eq!(bearer.refreshed.load(Ordering::SeqCst), 1);
    }

    /// PricingConfig produces non-zero cost_usd for a configured rate
    /// and a measurable inference-ms; verifies the cost arithmetic
    /// `(hourly_usd / 3_600_000) × inference_ms`.
    #[tokio::test]
    async fn pricing_config_computes_non_zero_cost_for_configured_rate() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("x-foundry-inference-ms", "3600000")
                    .set_body_json(ok_body()),
            )
            .expect(1)
            .mount(&server)
            .await;

        let pricing = PricingConfig {
            yoyo_hourly_usd: 0.84,
        };
        let client = client_with_pricing(server.uri(), pricing.clone());
        let resp = client.complete(&req()).await.expect("happy path 200");
        assert!(
            (resp.cost_usd - 0.84).abs() < 1e-9,
            "expected $0.84, got ${}",
            resp.cost_usd
        );

        assert_eq!(pricing.yoyo_cost_usd(0), 0.0);
        assert!((pricing.yoyo_cost_usd(1_800_000) - 0.42).abs() < 1e-9);
    }

    /// Default PricingConfig (operator hasn't configured a rate) keeps
    /// cost_usd at 0.0 — accurate as "unknown" rather than mis-attributed.
    #[tokio::test]
    async fn pricing_config_default_keeps_cost_zero() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("x-foundry-inference-ms", "9999999")
                    .set_body_json(ok_body()),
            )
            .expect(1)
            .mount(&server)
            .await;

        let client = client(server.uri());
        let resp = client.complete(&req()).await.expect("happy path 200");
        assert_eq!(resp.cost_usd, 0.0);
    }

    // ---- Grammar serialisation tests ----

    #[tokio::test]
    async fn grammar_lark_serialises_into_grammar_field() {
        use std::sync::Mutex;

        let captured: Arc<Mutex<Option<serde_json::Value>>> = Arc::new(Mutex::new(None));
        let captured_clone = captured.clone();

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(move |req: &Request| {
                let body: serde_json::Value = serde_json::from_slice(&req.body).unwrap_or_default();
                *captured_clone.lock().unwrap() = Some(body);
                ResponseTemplate::new(200)
                    .insert_header("x-foundry-inference-ms", "100")
                    .set_body_json(ok_body())
            })
            .expect(1)
            .mount(&server)
            .await;

        let client = client(server.uri());
        let mut r = req();
        r.grammar = Some(slm_core::GrammarConstraint::Lark(
            "start: /[a-z]+/".to_string(),
        ));
        client.complete(&r).await.expect("lark grammar happy path");

        let body = captured.lock().unwrap().clone().expect("body captured");
        // llama.cpp top-level grammar field (not vLLM extra_body).
        assert_eq!(body["grammar"], serde_json::json!("start: /[a-z]+/"));
        assert!(
            body.get("json_schema").is_none(),
            "json_schema absent for Lark"
        );
        assert!(
            body.get("extra_body").is_none(),
            "no extra_body without tools"
        );
    }

    #[tokio::test]
    async fn grammar_gbnf_serialises_into_grammar_field() {
        use std::sync::Mutex;

        let captured: Arc<Mutex<Option<serde_json::Value>>> = Arc::new(Mutex::new(None));
        let captured_clone = captured.clone();

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(move |req: &Request| {
                let body: serde_json::Value = serde_json::from_slice(&req.body).unwrap_or_default();
                *captured_clone.lock().unwrap() = Some(body);
                ResponseTemplate::new(200)
                    .insert_header("x-foundry-inference-ms", "100")
                    .set_body_json(ok_body())
            })
            .expect(1)
            .mount(&server)
            .await;

        let client = client(server.uri());
        let mut r = req();
        r.grammar = Some(slm_core::GrammarConstraint::Gbnf(
            r#"root ::= "yes" | "no""#.to_string(),
        ));
        client.complete(&r).await.expect("gbnf grammar happy path");

        let body = captured.lock().unwrap().clone().expect("body captured");
        // llama.cpp top-level grammar field (not vLLM extra_body).
        assert_eq!(
            body["grammar"],
            serde_json::json!(r#"root ::= "yes" | "no""#)
        );
        assert!(
            body.get("json_schema").is_none(),
            "json_schema absent for GBNF"
        );
        assert!(
            body.get("extra_body").is_none(),
            "no extra_body without tools"
        );
    }

    #[tokio::test]
    async fn grammar_json_schema_serialises_into_top_level_json_schema_field() {
        use std::sync::Mutex;

        let captured: Arc<Mutex<Option<serde_json::Value>>> = Arc::new(Mutex::new(None));
        let captured_clone = captured.clone();

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(move |req: &Request| {
                let body: serde_json::Value = serde_json::from_slice(&req.body).unwrap_or_default();
                *captured_clone.lock().unwrap() = Some(body);
                ResponseTemplate::new(200)
                    .insert_header("x-foundry-inference-ms", "100")
                    .set_body_json(ok_body())
            })
            .expect(1)
            .mount(&server)
            .await;

        let schema = serde_json::json!({
            "type": "object",
            "properties": { "result": { "type": "string" } },
            "required": ["result"]
        });
        let client = client(server.uri());
        let mut r = req();
        r.grammar = Some(slm_core::GrammarConstraint::JsonSchema(schema.clone()));
        client
            .complete(&r)
            .await
            .expect("json_schema grammar happy path");

        let body = captured.lock().unwrap().clone().expect("body captured");
        // llama.cpp top-level json_schema field (not vLLM extra_body) — this is what
        // actually constrains the deployed Tier B server. Confirmed live 2026-06-01.
        assert_eq!(body["json_schema"], schema);
        assert!(
            body.get("grammar").is_none(),
            "grammar absent for JsonSchema"
        );
        assert!(
            body.get("extra_body").is_none(),
            "no extra_body without tools"
        );
    }

    #[tokio::test]
    async fn grammar_none_omits_grammar_and_response_format() {
        use std::sync::Mutex;

        let captured: Arc<Mutex<Option<serde_json::Value>>> = Arc::new(Mutex::new(None));
        let captured_clone = captured.clone();

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(move |req: &Request| {
                let body: serde_json::Value = serde_json::from_slice(&req.body).unwrap_or_default();
                *captured_clone.lock().unwrap() = Some(body);
                ResponseTemplate::new(200)
                    .insert_header("x-foundry-inference-ms", "100")
                    .set_body_json(ok_body())
            })
            .expect(1)
            .mount(&server)
            .await;

        let client = client(server.uri());
        client
            .complete(&req())
            .await
            .expect("none grammar happy path");

        let body = captured.lock().unwrap().clone().expect("body captured");
        assert!(
            body.get("extra_body").is_none(),
            "extra_body must be absent when GrammarConstraint is None"
        );
    }

    // ---- BearerTokenProvider failure-path tests ----

    #[tokio::test]
    async fn bearer_token_provider_error_surfaces_typed_error_with_zero_network_calls() {
        #[derive(Debug)]
        struct FailingBearer;
        #[async_trait]
        impl BearerTokenProvider for FailingBearer {
            async fn token(&self) -> Result<String> {
                Err(DoormanError::BearerToken(
                    "identity provider offline".into(),
                ))
            }
            async fn refresh(&self) -> Result<String> {
                Err(DoormanError::BearerToken(
                    "identity provider offline".into(),
                ))
            }
        }

        let server = MockServer::start().await;

        let client = YoYoTierClient::new(
            YoYoTierConfig {
                endpoint: server.uri(),
                default_model: "Olmo-3-1125-32B-Think".into(),
                contract_version: crate::YOYO_CONTRACT_VERSION.into(),
                pricing: PricingConfig::default(),
                zone: None,
                health_path: "/health".to_string(),
            },
            Arc::new(FailingBearer),
        );

        let err = client
            .complete(&req())
            .await
            .expect_err("failing bearer must return error");
        match err {
            DoormanError::BearerToken(msg) => {
                assert!(
                    msg.contains("offline"),
                    "error message should preserve provider detail: {msg}"
                );
            }
            other => panic!("expected BearerToken error, got {other:?}"),
        }
        assert_eq!(server.received_requests().await.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn bearer_empty_string_causes_auth_failure_path() {
        #[derive(Debug)]
        struct EmptyBearer;
        #[async_trait]
        impl BearerTokenProvider for EmptyBearer {
            async fn token(&self) -> Result<String> {
                Ok(String::new())
            }
            async fn refresh(&self) -> Result<String> {
                Ok(String::new())
            }
        }

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(401))
            .expect(2)
            .mount(&server)
            .await;

        let client = YoYoTierClient::new(
            YoYoTierConfig {
                endpoint: server.uri(),
                default_model: "Olmo-3-1125-32B-Think".into(),
                contract_version: crate::YOYO_CONTRACT_VERSION.into(),
                pricing: PricingConfig::default(),
                zone: None,
                health_path: "/health".to_string(),
            },
            Arc::new(EmptyBearer),
        );

        let err = client
            .complete(&req())
            .await
            .expect_err("empty bearer leading to repeated 401 must return error");
        match err {
            DoormanError::UpstreamShape(msg) => {
                assert!(
                    msg.contains("401"),
                    "UpstreamShape message should mention the status: {msg}"
                );
            }
            other => panic!("expected UpstreamShape from failed auth retry, got {other:?}"),
        }
    }

    /// 410 MAJOR contract mismatch: client must NOT retry and must
    /// surface `ContractMajorMismatch`.
    #[tokio::test]
    async fn contract_410_returns_major_mismatch_no_retry() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(410))
            .expect(1)
            .mount(&server)
            .await;

        let client = client(server.uri());
        let err = client
            .complete(&req())
            .await
            .expect_err("410 must surface as error");
        match err {
            DoormanError::ContractMajorMismatch {
                remote_status,
                doorman_version,
            } => {
                assert_eq!(remote_status, 410);
                assert_eq!(doorman_version, crate::YOYO_CONTRACT_VERSION);
            }
            other => panic!("expected ContractMajorMismatch, got {other:?}"),
        }
    }

    /// Circuit breaker opens after FAILURE_THRESHOLD consecutive failures
    /// and subsequent complete() returns TierBCircuitOpen immediately.
    #[tokio::test]
    async fn circuit_breaker_opens_after_consecutive_failures() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let client = client(server.uri());

        // Drive failures until circuit opens (FAILURE_THRESHOLD = 5)
        for _ in 0..5 {
            let _ = client.complete(&req()).await;
        }

        // Next call should be rejected by circuit breaker, not by HTTP
        let err = client.complete(&req()).await.expect_err("circuit open");
        assert!(
            matches!(err, DoormanError::TierBCircuitOpen),
            "expected TierBCircuitOpen, got {err:?}"
        );
    }

    /// health_up starts true; can be manually flipped and back.
    #[test]
    fn health_up_atomic_default_true() {
        let server_uri = "http://127.0.0.1:1".to_string(); // unreachable; no probe spawned in sync test
        let client = YoYoTierClient::new(
            YoYoTierConfig {
                endpoint: server_uri,
                ..YoYoTierConfig::default()
            },
            Arc::new(StaticBearer::new("tok")),
        );
        assert!(client.health_up.load(Ordering::Relaxed));
        client.health_up.store(false, Ordering::Relaxed);
        assert!(!client.health_up.load(Ordering::Relaxed));
    }
}
