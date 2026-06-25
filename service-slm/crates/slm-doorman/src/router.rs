// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Doorman — three-tier router skeleton.
//!
//! Routes one `ComputeRequest` to a configured tier and writes one audit
//! entry per call. Per the Optional Intelligence principle
//! (`conventions/three-ring-architecture.md`) every tier is optional;
//! the Doorman boots and serves /healthz with no tiers configured at
//! all. A request to an unconfigured tier returns
//! `DoormanError::TierUnavailable`, which a caller (Ring 2 service or
//! the inbound HTTP server) MAY translate to a "fall back to
//! deterministic processing" decision.

use chrono::Utc;
use slm_core::{Complexity, ComputeRequest, ComputeResponse, GrammarConstraint, Tier};
use tracing::{debug, info, warn};

use crate::cost_ledger::{CostLedger, CostRow};
use crate::error::{DoormanError, Result};
use crate::grammar_validation::LarkValidator;
use crate::graph::GraphContextClient;
use crate::ledger::{AuditEntry, AuditLedger, CompletionStatus, ENTRY_TYPE_CHAT_COMPLETION};
use crate::mesh::MeshRegistry;
use crate::tier::{ExternalTierClient, LocalTierClient, YoYoTierClient};

use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;

/// Runtime observability snapshot for a single Tier B node.
/// Serialized into `GET /readyz` under the `tier_b` key.
#[derive(Debug, Serialize)]
pub struct TierBInfo {
    pub configured: bool,
    pub health_up: bool,
    /// Seconds since the health probe last marked this node unavailable.
    /// `null` when health is currently up. Independent of circuit state —
    /// non-zero here means probes are failing even if `circuit` shows "closed"
    /// (circuit only trips on actual inference failures, not probe failures).
    pub health_down_secs: Option<u64>,
    pub circuit: &'static str,
    pub opened_for_secs: Option<u64>,
    /// Why the circuit opened. Derived from health_up + circuit state.
    /// "health-probe-failures" when probes fail; "request-failures" when
    /// dispatch errors trigger the circuit; null when circuit is closed.
    pub reason: Option<&'static str>,
    /// GCP zone where this Yo-Yo node runs. Populated from `SLM_YOYO_GCP_ZONE`.
    pub zone: Option<String>,
}

#[derive(Default)]
pub struct DoormanConfig {
    pub local: Option<LocalTierClient>,
    pub yoyo: HashMap<String, YoYoTierClient>,
    pub external: Option<ExternalTierClient>,
    /// Optional Lark grammar pre-validator (PS.3 step 5). When `Some`,
    /// the router validates any Lark grammar before dispatching to Tier B.
    /// `None` disables the validation step (opt-out for callers that do not
    /// have the llguidance init overhead or for testing). Set via
    /// `LarkValidator::new()` at Doorman startup.
    pub lark_validator: Option<LarkValidator>,
    /// Optional graph context client (Brief E). When `Some`, the router
    /// queries `service-content`'s `/v1/graph/context` endpoint before
    /// dispatching to any tier, injecting matching entity rows as a
    /// `[ENTITY CONTEXT]` system message prepended to the message list.
    /// Non-fatal: if the client is absent or the query returns no results,
    /// the request proceeds without context injection.
    /// Set via `SERVICE_CONTENT_ENDPOINT` env var at Doorman startup.
    pub graph_context_client: Option<GraphContextClient>,
    /// When `true`, Tier A is the confident primary regardless of request
    /// complexity. The router only escalates to Tier B when the caller
    /// explicitly hints `Tier::Yoyo` AND the relevant node circuit is closed
    /// AND health probe is up. Set via `SLM_TIER_A_FIRST=true`. Mutually
    /// exclusive with `SLM_FORCE_BROKER_MODE=true`.
    pub tier_a_first: bool,
    /// Optional per-day Tier B spend cap in USD. When `Some(cap)` and the
    /// cost ledger reports today's total (per-request + VM-hours) ≥ cap,
    /// all Tier B dispatches are refused and fall back to Tier A.
    /// Set via `SLM_YOYO_DAILY_CAP_USD`. Absent = no cap.
    pub daily_yoyo_cap_usd: Option<f64>,
    /// Cost ledger used for per-request writes and daily-cap checks.
    /// `None` disables both (no billing records, no cap enforcement).
    /// Injected via `DoormanConfig`; the server wires it from FOUNDRY_ROOT.
    pub cost_ledger: Option<Arc<CostLedger>>,
}

pub struct Orchestrator {
    pub registry: Box<dyn MeshRegistry>,
}

pub struct Doorman {
    local: Option<LocalTierClient>,
    yoyo: HashMap<String, YoYoTierClient>,
    external: Option<ExternalTierClient>,
    ledger: AuditLedger,
    lark_validator: Option<LarkValidator>,
    graph_context_client: Option<GraphContextClient>,
    pub orchestrator: Option<Orchestrator>,
    tier_a_first: bool,
    daily_yoyo_cap_usd: Option<f64>,
    cost_ledger: Option<Arc<CostLedger>>,
}

impl Doorman {
    pub fn new(config: DoormanConfig, ledger: AuditLedger) -> Self {
        Self {
            local: config.local,
            yoyo: config.yoyo,
            external: config.external,
            ledger,
            lark_validator: config.lark_validator,
            graph_context_client: config.graph_context_client,
            orchestrator: None,
            tier_a_first: config.tier_a_first,
            daily_yoyo_cap_usd: config.daily_yoyo_cap_usd,
            cost_ledger: config.cost_ledger,
        }
    }

    pub async fn route_async(&self, req: &ComputeRequest) -> Result<ComputeResponse> {
        if let Some(ref orch) = self.orchestrator {
            info!(target: "slm_doorman::router", request_id = %req.request_id, "dispatching via orchestrator");

            if let Some(node) = orch.registry.select_optimal(req).await {
                info!(target: "slm_doorman::router", node_id = %node.id.0, "selected node");
                // Here we would dispatch to node.endpoint
                // For Phase 1, we continue to delegate to existing dispatch path
                // but node-aware dispatch logic goes here in future steps.
            }
        }
        self.route(req).await
    }

    pub fn has_local(&self) -> bool {
        self.local.is_some()
    }

    pub fn has_yoyo(&self) -> bool {
        !self.yoyo.is_empty()
    }

    pub fn has_external(&self) -> bool {
        self.external.is_some()
    }

    /// Returns runtime state for each configured Tier B node.
    /// Exposes circuit breaker state and health probe outcome for use in /readyz.
    pub fn tier_b_status(&self) -> HashMap<String, TierBInfo> {
        use std::sync::atomic::Ordering;
        self.yoyo
            .iter()
            .map(|(label, client)| {
                let health_up = client.health_up.load(Ordering::Relaxed);
                let circuit = client.circuit.state_label();
                let reason = match circuit {
                    "open" if !health_up => Some("health-probe-failures"),
                    "open" => Some("request-failures"),
                    _ => None,
                };
                let info = TierBInfo {
                    configured: true,
                    health_up,
                    health_down_secs: client.health_down_secs(),
                    circuit,
                    opened_for_secs: client.circuit.opened_for_secs(),
                    reason,
                    zone: client.zone.clone(),
                };
                (label.clone(), info)
            })
            .collect()
    }

    /// Returns true when the named Yoyo node has its circuit closed and its
    /// health probe up — i.e. it is currently accepting requests.
    /// Used by the drain worker to gate shadow brief dispatch before dequeuing,
    /// preventing Tier A fallback from saturating OLMo when the GPU node is
    /// unavailable (STOCKOUT, circuit open, or health probe failure).
    pub fn yoyo_node_ready(&self, label: &str) -> bool {
        self.yoyo.get(label).map(|c| c.allow_request()).unwrap_or(false)
    }

    pub fn ledger(&self) -> &AuditLedger {
        &self.ledger
    }

    /// Pick a tier from the request and dispatch. The caller's `tier_hint`
    /// is honoured when the named tier is configured; otherwise the
    /// router maps `complexity` to a default tier and probes for the
    /// best configured option.
    ///
    /// When a `graph_context_client` is configured, the router queries
    /// `service-content` for entity context matching the last user message
    /// (first 200 chars) before dispatching. Matching entities are prepended
    /// as a `[ENTITY CONTEXT]` system message. The query is non-fatal: if
    /// service-content is unavailable or returns no results, the request
    /// proceeds without modification.
    pub async fn route(&self, req: &ComputeRequest) -> Result<ComputeResponse> {
        let target = self.select_tier(req)?;

        // Graph context injection (Brief E).
        //
        // When service-content is configured and the request contains a user
        // message, fetch matching entity rows and prepend them as a system
        // message. The owned `effective_req` must be declared before the
        // conditional block so its lifetime covers the dispatch call below.
        let effective_req: ComputeRequest;
        let req_ref: &ComputeRequest = if req.graph_context_enabled == Some(false) {
            req
        } else if let Some(ref gc) = self.graph_context_client {
            // Extract the last user message for entity lookup.
            // service-content does substring matching on entity_name, so we must
            // query individual tokens rather than the full sentence — a sentence
            // like "List the Woodfine companies" never substring-matches any entity
            // name, but "Woodfine" does. Try each word ≥4 chars (skip stop-words)
            // and return the first query that produces entity results.
            let user_text = req
                .messages
                .iter()
                .rev()
                .find(|m| m.role == "user")
                .map(|m| m.content.chars().take(200).collect::<String>())
                .unwrap_or_default();

            let mut seen = std::collections::HashSet::new();
            let candidates: Vec<String> = user_text
                .split(|c: char| !c.is_alphanumeric())
                .filter(|w| w.len() >= 4)
                .filter(|w| seen.insert(w.to_lowercase()))
                .map(|w| w.to_string())
                .collect();

            let mut ctx_opt: Option<String> = None;
            for candidate in candidates.iter().take(6) {
                if let Some(ctx) = gc.fetch_context(req.module_id.as_str(), candidate, 5).await {
                    ctx_opt = Some(ctx);
                    break;
                }
            }

            if let Some(ctx) = ctx_opt {
                let entity_count = ctx.lines().count();
                let mut cloned = req.clone();
                cloned.messages.insert(
                    0,
                    slm_core::ChatMessage {
                        role: "system".to_string(),
                        content: format!("[ENTITY CONTEXT]\n{}", ctx),
                    },
                );
                effective_req = cloned;
                info!(
                    target: "slm_doorman::graph",
                    module_id = %req.module_id,
                    entity_count,
                    "graph context injected"
                );
                &effective_req
            } else {
                debug!(
                    target: "slm_doorman::graph",
                    module_id = %req.module_id,
                    "graph context: no entities returned — skipping injection"
                );
                req
            }
        } else {
            req
        };

        let result = self.dispatch(target, req_ref).await;
        self.write_audit(req, target, &result);
        result
    }

    fn select_tier(&self, req: &ComputeRequest) -> Result<Tier> {
        // SLM_TIER_A_FIRST mode: Tier A is the confident primary. Escalate to
        // Tier B only when the caller explicitly hints Yoyo AND the circuit is
        // closed AND the health probe is up. All other requests go to Tier A.
        if self.tier_a_first {
            let yoyo_hint = req.tier_hint == Some(Tier::Yoyo);
            if yoyo_hint {
                let label = req
                    .yoyo_label
                    .as_deref()
                    .or_else(|| {
                        req.session_context
                            .as_ref()
                            .and_then(|sc| sc.archive_domain.as_deref())
                            .filter(|domain| self.yoyo.contains_key(*domain))
                    })
                    .unwrap_or("default");
                let yoyo_ready = self
                    .yoyo
                    .get(label)
                    .map(|c| c.allow_request())
                    .unwrap_or(false);
                if yoyo_ready {
                    return self.confirm_tier_with_req(Tier::Yoyo, req);
                }
            }
            return self.confirm_tier_with_req(Tier::Local, req);
        }

        if let Some(hint) = req.tier_hint {
            return self.confirm_tier_with_req(hint, req);
        }
        // Default policy: low / medium → local, high → yoyo if configured
        // else local. Tier C is never a default — callers must hint it
        // explicitly and the label-allowlist check runs in `dispatch`.
        let preferred = match req.complexity {
            Complexity::Low | Complexity::Medium => Tier::Local,
            Complexity::High => {
                if !self.yoyo.is_empty() {
                    Tier::Yoyo
                } else {
                    Tier::Local
                }
            }
        };
        self.confirm_tier_with_req(preferred, req)
    }

    /// Returns `true` when today's Tier B spend (per-request + VM-hours) has
    /// reached or exceeded the configured `daily_yoyo_cap_usd`. Always
    /// returns `false` when no cap is configured or the cost ledger is absent.
    fn daily_cap_exceeded(&self) -> bool {
        let Some(cap) = self.daily_yoyo_cap_usd else {
            return false;
        };
        let Some(ref ledger) = self.cost_ledger else {
            return false;
        };
        let today = Utc::now().format("%Y-%m-%d").to_string();
        let Ok(rollup) = ledger.daily_rollup(&today) else {
            return false;
        };
        let total = rollup.total_cost_usd + rollup.vm_hours_cost_usd;
        if total >= cap {
            warn!(
                target: "slm_doorman::router",
                daily_spend_usd = total,
                cap_usd = cap,
                "daily Tier B spend cap reached — refusing Tier B dispatch, falling back to Tier A"
            );
            return true;
        }
        false
    }

    fn confirm_tier_with_req(&self, tier: Tier, req: &ComputeRequest) -> Result<Tier> {
        // Daily spend cap: intercept Tier B before circuit check. When the cap
        // is exceeded, silently downgrade to Tier A so the request still completes.
        let tier = if tier == Tier::Yoyo && self.daily_cap_exceeded() {
            Tier::Local
        } else {
            tier
        };
        let configured = match tier {
            Tier::Local => self.local.is_some(),
            Tier::Yoyo => {
                if let Some(ref label) = req.yoyo_label {
                    self.yoyo.contains_key(label)
                } else {
                    !self.yoyo.is_empty()
                }
            }
            Tier::External => self.external.is_some(),
        };
        if configured {
            Ok(tier)
        } else {
            warn!(
                target: "slm_doorman::router",
                ?tier,
                yoyo_label = ?req.yoyo_label,
                "tier not configured — community-tier mode may be active"
            );
            Err(DoormanError::TierUnavailable(tier))
        }
    }

    async fn dispatch(&self, tier: Tier, req: &ComputeRequest) -> Result<ComputeResponse> {
        info!(
            target: "slm_doorman::router",
            request_id = %req.request_id,
            module_id = %req.module_id,
            tier = tier.as_str(),
            "dispatching"
        );

        // PS.3 step 5 — pre-validate Lark grammar before Tier B dispatch.
        //
        // Tier B (vLLM ≥0.12) accepts Lark grammars. A malformed Lark string
        // relayed to vLLM produces an opaque upstream 400/500 with no useful
        // error message. We validate at the boundary so callers get a typed
        // 400 MalformedLarkGrammar with parse-error location instead.
        //
        // Only runs when:
        //   (a) a LarkValidator is configured (lark_validator.is_some()), AND
        //   (b) the selected tier is Tier B (Yoyo), AND
        //   (c) the request carries GrammarConstraint::Lark.
        //
        // Tier A rejects Lark in its own grammar-policy check (steps 2-4).
        // Tier C rejects all grammars (steps 2-4). Neither path reaches here
        // with a Lark grammar under normal routing. The guard on Tier::Yoyo
        // is defensive; it makes the semantics explicit rather than relying on
        // routing invariants.
        if tier == Tier::Yoyo {
            if let (Some(validator), Some(GrammarConstraint::Lark(lark_src))) =
                (&self.lark_validator, &req.grammar)
            {
                if let Err(reason) = validator.validate(lark_src) {
                    return Err(DoormanError::MalformedLarkGrammar { reason });
                }
            }
        }

        match tier {
            Tier::Local => {
                self.local
                    .as_ref()
                    .ok_or(DoormanError::TierUnavailable(Tier::Local))?
                    .complete(req)
                    .await
            }
            Tier::Yoyo => {
                let client = if let Some(ref label) = req.yoyo_label {
                    self.yoyo.get(label).ok_or_else(|| {
                        warn!(target: "slm_doorman::router", label, "requested Yo-Yo label not configured");
                        DoormanError::TierUnavailable(Tier::Yoyo)
                    })?
                } else {
                    // Default to the first configured Yo-Yo if no label provided.
                    // If multiple are configured and no label is provided, the
                    // behavior is non-deterministic (HashMap order) but safe.
                    // Operator should always use labels for Multi-Yo-Yo.
                    self.yoyo
                        .values()
                        .next()
                        .ok_or_else(|| DoormanError::TierUnavailable(Tier::Yoyo))?
                };

                // B4: fast-path health + circuit check before making any HTTP call.
                if !client.allow_request() {
                    warn!(
                        target: "slm_doorman::router",
                        request_id = %req.request_id,
                        "Tier B unavailable (health probe down or circuit open); falling back to Tier A"
                    );
                    return self.try_local_fallback(req).await;
                }

                match client.complete(req).await {
                    Ok(resp) => Ok(resp),
                    Err(e) if is_transient_tier_b_failure(&e) => {
                        warn!(
                            target: "slm_doorman::router",
                            request_id = %req.request_id,
                            reason = %e,
                            "Tier B transient failure; falling back to Tier A"
                        );
                        self.try_local_fallback(req).await
                    }
                    Err(e) => Err(e),
                }
            }
            Tier::External => {
                // Strip session_context before forwarding to external Tier C.
                // The field is internal Foundry metadata and must not leave the
                // Totebox boundary.
                let req_for_tier_c = ComputeRequest {
                    session_context: None,
                    ..req.clone()
                };
                self.external
                    .as_ref()
                    .ok_or(DoormanError::TierUnavailable(Tier::External))?
                    .complete(&req_for_tier_c)
                    .await
            }
        }
    }

    fn write_audit(&self, req: &ComputeRequest, tier: Tier, result: &Result<ComputeResponse>) {
        let entry = match result {
            Ok(resp) => AuditEntry {
                entry_type: ENTRY_TYPE_CHAT_COMPLETION.to_string(),
                timestamp_utc: Utc::now(),
                request_id: req.request_id,
                module_id: req.module_id.clone(),
                tier: resp.tier_used,
                model: resp.model.clone(),
                inference_ms: resp.inference_ms,
                cost_usd: resp.cost_usd,
                sanitised_outbound: req.sanitised_outbound,
                completion_status: CompletionStatus::Ok,
                error_message: None,
                archive_name: req
                    .session_context
                    .as_ref()
                    .map(|sc| sc.archive_name.clone()),
            },
            Err(e) => AuditEntry {
                entry_type: ENTRY_TYPE_CHAT_COMPLETION.to_string(),
                timestamp_utc: Utc::now(),
                request_id: req.request_id,
                module_id: req.module_id.clone(),
                tier,
                model: req.model.clone().unwrap_or_default(),
                inference_ms: 0,
                cost_usd: 0.0,
                sanitised_outbound: req.sanitised_outbound,
                completion_status: classify_error(e),
                error_message: Some(e.to_string()),
                archive_name: req
                    .session_context
                    .as_ref()
                    .map(|sc| sc.archive_name.clone()),
            },
        };
        if let Err(write_err) = self.ledger.append(&entry) {
            // Audit failure must never silently drop. Surface to logs;
            // upstream observability picks it up.
            warn!(
                target: "slm_doorman::ledger",
                error = %write_err,
                request_id = %req.request_id,
                "failed to append audit entry"
            );
        }

        // Write to cost ledger for successful Tier B responses (P3-3.5-followup).
        // Non-fatal: cost tracking must never block the response path.
        if let (Some(ref cost_ledger), Ok(resp)) = (&self.cost_ledger, result) {
            if resp.tier_used == Tier::Yoyo {
                let row = CostRow {
                    ts: entry
                        .timestamp_utc
                        .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                    request_id: req.request_id.to_string(),
                    tier: "yoyo".to_string(),
                    model: resp.model.clone(),
                    cost_usd: resp.cost_usd,
                    inference_ms: resp.inference_ms,
                    adapter_version: None,
                };
                if let Err(e) = cost_ledger.append(&row) {
                    warn!(
                        target: "slm_doorman::cost_ledger",
                        error = %e,
                        request_id = %req.request_id,
                        "cost ledger write failed (non-fatal)"
                    );
                }
            }
        }
    }
}

impl Doorman {
    /// Route a request to a specific named Yo-Yo backend without Tier A fallback.
    ///
    /// Unlike `route()`, this method does NOT fall back to Tier A on
    /// circuit-open or transient failure. Returns
    /// `Err(DoormanError::TierUnavailable(Tier::Yoyo))` immediately so the
    /// caller can defer the request rather than routing to an inappropriate
    /// backend.
    ///
    /// Used by `POST /v1/extract`: entity extraction requires the "trainer"
    /// Yo-Yo node (OLMo 3 32B-Think). OLMo 7B (Tier A) cannot produce
    /// structured JSON arrays reliably and must never serve as a fallback for
    /// extraction.
    pub async fn route_yoyo_only(
        &self,
        req: &ComputeRequest,
        label: &str,
    ) -> Result<ComputeResponse> {
        let client = self.yoyo.get(label).ok_or_else(|| {
            warn!(
                target: "slm_doorman::router",
                label,
                "route_yoyo_only: Yo-Yo label not configured"
            );
            DoormanError::TierUnavailable(Tier::Yoyo)
        })?;

        if !client.allow_request() {
            warn!(
                target: "slm_doorman::router",
                request_id = %req.request_id,
                label,
                "route_yoyo_only: circuit not allowing request (open or health-probe down)"
            );
            return Err(DoormanError::TierUnavailable(Tier::Yoyo));
        }

        // PS.3: validate Lark grammar at boundary if validator configured.
        // Extraction uses JsonSchema; this branch is defensive only.
        if let (Some(validator), Some(GrammarConstraint::Lark(lark_src))) =
            (&self.lark_validator, &req.grammar)
        {
            if let Err(reason) = validator.validate(lark_src) {
                return Err(DoormanError::MalformedLarkGrammar { reason });
            }
        }

        client.complete(req).await
    }
}

impl Doorman {
    async fn try_local_fallback(&self, req: &ComputeRequest) -> Result<ComputeResponse> {
        match &self.local {
            Some(local) => {
                info!(
                    target: "slm_doorman::router",
                    request_id = %req.request_id,
                    "Tier A fallback active"
                );
                local.complete(req).await
            }
            None => Err(DoormanError::TierUnavailable(Tier::Local)),
        }
    }

    /// Route a background request (extraction fallback, drain dispatch) directly
    /// to Tier A using the background slot. Returns LocalSaturated immediately when
    /// either background_sem or total_sem is full — no queuing inside llama-server.
    /// Skips graph-context injection (the extraction handler pre-builds its request).
    pub async fn route_local_background(&self, req: &ComputeRequest) -> Result<ComputeResponse> {
        match &self.local {
            Some(local) => local.complete_background(req).await,
            None => Err(DoormanError::TierUnavailable(Tier::Local)),
        }
    }
}

fn is_transient_tier_b_failure(e: &DoormanError) -> bool {
    matches!(
        e,
        DoormanError::TierBTimeout
            | DoormanError::TierBCircuitOpen
            | DoormanError::Upstream(_)
            | DoormanError::UpstreamShape(_)
    )
}

fn classify_error(e: &DoormanError) -> CompletionStatus {
    match e {
        DoormanError::TierUnavailable(_) | DoormanError::NotImplemented { .. } => {
            CompletionStatus::TierUnavailable
        }
        DoormanError::ExternalNotAllowlisted { .. }
        | DoormanError::VerifySignature(_)
        // Caller submitted a grammar dialect unsupported on the selected tier.
        // Both Tier A (e.g. Lark) and Tier C (any grammar) violations are
        // classified as PolicyDenied — the request violated the per-tier
        // input policy.
        | DoormanError::TierAGrammarUnsupported { .. }
        | DoormanError::TierCGrammarUnsupported { .. }
        // Caller submitted a syntactically malformed Lark grammar. The Doorman
        // rejected it at the boundary (PS.3 step 5) before any upstream call.
        // Classified as PolicyDenied: the error is entirely on the caller's side.
        | DoormanError::MalformedLarkGrammar { .. }
        // Caller supplied an unrecognised provider string to audit_proxy (PS.4).
        // The request violated input policy — classified as PolicyDenied.
        | DoormanError::AuditProxyInvalidProvider { .. }
        // Caller supplied an un-allowlisted purpose to audit_proxy (PS.4 step 3).
        // The request violated input policy — classified as PolicyDenied.
        | DoormanError::AuditProxyPurposeNotAllowlisted { .. }
        // audit_capture caller-side policy violations (PS.4 step 4): unknown
        // event_type, oversized payload, or unparseable timestamp. All are
        // caller errors — classified as PolicyDenied.
        | DoormanError::AuditCaptureUnknownEventType { .. }
        | DoormanError::AuditCapturePayloadTooLarge { .. }
        | DoormanError::AuditCaptureInvalidTimestamp { .. }
        // audit_proxy oversized request body — caller must reduce the request
        // size. Classified as PolicyDenied (same classification as the
        // audit_capture payload cap variant above).
        | DoormanError::AuditProxyPayloadTooLarge { .. }
        // Per-tenant (moduleId) concurrency cap hit on the audit endpoints.
        // The caller may retry; classified as PolicyDenied because the
        // Doorman is enforcing a per-tenant resource policy.
        | DoormanError::AuditTenantConcurrencyExhausted { .. } => {
            CompletionStatus::PolicyDenied
        }
        // The audit_proxy targeted a provider that is not configured at startup
        // (PS.4 step 2). Server-side configuration gap — classified as UpstreamError
        // (not PolicyDenied; the caller did nothing wrong). HTTP 503.
        DoormanError::AuditProxyProviderUnavailable { .. } => CompletionStatus::UpstreamError,
        // Tier B resilience errors — the circuit or deadline fired. Classified
        // as UpstreamError so the audit ledger reflects upstream unavailability.
        // In the normal routing path these are caught by is_transient_tier_b_failure
        // and trigger Tier A fallback; they reach classify_error only when
        // local is also unavailable.
        DoormanError::TierBTimeout | DoormanError::TierBCircuitOpen => {
            CompletionStatus::UpstreamError
        }
        DoormanError::Upstream(_)
        | DoormanError::UpstreamShape(_)
        | DoormanError::ContractMajorMismatch { .. }
        | DoormanError::BearerToken(_) => CompletionStatus::UpstreamError,
        DoormanError::LedgerIo(_)
        | DoormanError::LedgerSerde(_)
        | DoormanError::HomeUnset
        | DoormanError::LedgerLock(_)
        | DoormanError::CorpusWrite { .. }
        | DoormanError::VerdictParse(_)
        | DoormanError::BriefCacheMiss => CompletionStatus::UpstreamError,
        // Orphan verdict: no shadow corpus tuple exists for this brief_id.
        // Per §7B, no corpus row is created. Classified as PolicyDenied
        // — the caller should verify the shadow brief was dispatched
        // before signing a verdict. HTTP 410 in the HTTP layer.
        DoormanError::OrphanVerdictNoCorpusTuple { .. } => CompletionStatus::PolicyDenied,
        // Brief Queue Substrate (apprenticeship-substrate.md §7C):
        //   QueueIo       — file-system I/O failure; UpstreamError (server-side)
        //   QueueLockFailed — transient lock contention; UpstreamError (server-side)
        //   QueueMalformedBrief — caller-side malformed content; PolicyDenied
        DoormanError::QueueIo { .. } | DoormanError::QueueLockFailed { .. } => {
            CompletionStatus::UpstreamError
        }
        DoormanError::QueueMalformedBrief { .. } => CompletionStatus::PolicyDenied,
        // Graph proxy errors: caller-side for missing header (PolicyDenied),
        // server-side for unreachable service-content (UpstreamError).
        DoormanError::GraphProxyMissingModuleId => CompletionStatus::PolicyDenied,
        DoormanError::GraphProxyServiceUnavailable => CompletionStatus::UpstreamError,
        // Flow gate closed (operator kill switch): the operator deliberately
        // refused this tier. Classified as PolicyDenied — the request was
        // rejected by an explicit policy decision, not an upstream fault.
        // HTTP 503 with Retry-After in the HTTP layer.
        DoormanError::FlowGateClosed { .. } => CompletionStatus::PolicyDenied,
        // Priority queue I/O failure: server-side file-system fault.
        DoormanError::PriorityQueueIo { .. } => CompletionStatus::UpstreamError,
        // GCP Compute API failure (VM start/stop/status): upstream provider
        // fault; the lifecycle monitor retries and the router falls to Tier A.
        DoormanError::GcpApi { .. } => CompletionStatus::UpstreamError,
        // Corpus quality gate rejected a shadow tuple or DPO pair (length ratio,
        // template echo, Do-Not-Use term, etc.). Content-level rejection — the
        // caller's submitted content does not meet corpus quality requirements.
        // Classified as PolicyDenied (same as malformed brief / purpose deny).
        DoormanError::CorpusGateRejected { .. } => CompletionStatus::PolicyDenied,
        // Extract handler 120 s deadline. The inference slot is released; the
        // caller should retry or accept the deferred response. UpstreamError
        // because the extraction timed out due to OLMo saturation.
        DoormanError::RequestTimeout => CompletionStatus::UpstreamError,
        // Tier A admission control: all slots occupied. The request was
        // fast-failed before queuing in llama-server. PolicyDenied — the
        // Doorman is enforcing a server-side concurrency cap.
        DoormanError::LocalSaturated => CompletionStatus::PolicyDenied,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slm_core::{ChatMessage, ModuleId, RequestId};
    use std::str::FromStr;

    fn req(
        complexity: Complexity,
        hint: Option<Tier>,
        yoyo_label: Option<String>,
    ) -> ComputeRequest {
        ComputeRequest {
            request_id: RequestId::new(),
            module_id: ModuleId::from_str("foundry").unwrap(),
            model: None,
            messages: vec![ChatMessage {
                role: "user".into(),
                content: "ping".into(),
            }],
            complexity,
            tier_hint: hint,
            stream: false,
            max_tokens: None,
            temperature: None,
            sanitised_outbound: true,
            tier_c_label: None,
            yoyo_label,
            grammar: None,
            speculation: None,
            graph_context_enabled: None,
            tools: None,
            stop_sequences: None,
            session_context: None,
        }
    }

    fn ledger() -> AuditLedger {
        let dir = std::env::temp_dir().join(format!(
            "slm-doorman-router-test-{}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        AuditLedger::new(dir).unwrap()
    }

    #[tokio::test]
    async fn unconfigured_router_refuses_with_tier_unavailable() {
        let doorman = Doorman::new(DoormanConfig::default(), ledger());
        let result = doorman.route(&req(Complexity::Medium, None, None)).await;
        match result {
            Err(DoormanError::TierUnavailable(Tier::Local)) => {}
            other => panic!("expected TierUnavailable(Local), got {other:?}"),
        }
    }

    #[test]
    fn high_complexity_prefers_yoyo_when_configured() {
        // Pure selection logic — no network. We construct a Doorman with
        // a Yo-Yo config that points at a bogus endpoint; select_tier
        // does not hit the network.
        let yoyo = YoYoTierClient::new(
            crate::tier::YoYoTierConfig {
                endpoint: "http://invalid.example".into(),
                default_model: "Olmo-3-1125-32B-Think".into(),
                contract_version: crate::YOYO_CONTRACT_VERSION.into(),
                pricing: crate::tier::PricingConfig::default(),
                zone: None,
                health_path: "/health".to_string(),
            },
            std::sync::Arc::new(crate::tier::StaticBearer::new("unused-in-selection-test")),
        );
        let mut yoyo_map = HashMap::new();
        yoyo_map.insert("default".to_string(), yoyo);
        let doorman = Doorman::new(
            DoormanConfig {
                local: None,
                yoyo: yoyo_map,
                external: None,
                lark_validator: None,
                graph_context_client: None,
                tier_a_first: false,
                daily_yoyo_cap_usd: None,
                cost_ledger: None,
            },
            ledger(),
        );
        let picked = doorman
            .select_tier(&req(Complexity::High, None, None))
            .expect("should pick yoyo");
        assert_eq!(picked, Tier::Yoyo);
    }

    fn make_yoyo(zone: Option<String>) -> YoYoTierClient {
        YoYoTierClient::new(
            crate::tier::YoYoTierConfig {
                endpoint: "http://invalid.example".into(),
                default_model: "Olmo-3-1125-32B-Think".into(),
                contract_version: crate::YOYO_CONTRACT_VERSION.into(),
                pricing: crate::tier::PricingConfig::default(),
                zone,
                health_path: "/health".to_string(),
            },
            std::sync::Arc::new(crate::tier::StaticBearer::new("test")),
        )
    }

    #[test]
    fn tier_b_status_reason_health_probe_failures() {
        use std::sync::atomic::Ordering;
        let yoyo = make_yoyo(Some("europe-west4-a".into()));
        // Simulate health probe failure + circuit open.
        yoyo.health_up.store(false, Ordering::Relaxed);
        // FAILURE_THRESHOLD is 5 — need 5 consecutive failures to open circuit
        for _ in 0..5 {
            yoyo.circuit.record_failure();
        }

        let mut map = HashMap::new();
        map.insert("default".to_string(), yoyo);
        let doorman = Doorman::new(
            DoormanConfig {
                yoyo: map,
                ..Default::default()
            },
            ledger(),
        );
        let status = doorman.tier_b_status();
        let info = status.get("default").unwrap();
        assert_eq!(info.reason, Some("health-probe-failures"));
        assert_eq!(info.zone.as_deref(), Some("europe-west4-a"));
    }

    #[test]
    fn tier_b_status_reason_request_failures() {
        use std::sync::atomic::Ordering;
        let yoyo = make_yoyo(None);
        // health probe still up, but circuit tripped by request failures.
        yoyo.health_up.store(true, Ordering::Relaxed);
        // FAILURE_THRESHOLD is 5 — need 5 consecutive failures to open circuit
        for _ in 0..5 {
            yoyo.circuit.record_failure();
        }

        let mut map = HashMap::new();
        map.insert("default".to_string(), yoyo);
        let doorman = Doorman::new(
            DoormanConfig {
                yoyo: map,
                ..Default::default()
            },
            ledger(),
        );
        let status = doorman.tier_b_status();
        let info = status.get("default").unwrap();
        assert_eq!(info.reason, Some("request-failures"));
        assert!(info.zone.is_none());
    }

    #[test]
    fn tier_b_status_no_reason_when_closed() {
        let yoyo = make_yoyo(Some("us-central1-a".into()));
        let mut map = HashMap::new();
        map.insert("default".to_string(), yoyo);
        let doorman = Doorman::new(
            DoormanConfig {
                yoyo: map,
                ..Default::default()
            },
            ledger(),
        );
        let status = doorman.tier_b_status();
        let info = status.get("default").unwrap();
        assert_eq!(info.circuit, "closed");
        assert!(info.reason.is_none());
        assert_eq!(info.zone.as_deref(), Some("us-central1-a"));
    }

    /// When today's spend in the cost ledger meets or exceeds the daily cap,
    /// `select_tier` must redirect a High-complexity request to Tier A even
    /// when the Tier B circuit is closed and endpoints are configured.
    #[test]
    fn daily_cap_exceeded_blocks_tier_b_falls_back_to_local() {
        let cost_dir = std::env::temp_dir().join(format!(
            "slm-cap-test-exceeded-{}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        let cost_ledger = Arc::new(CostLedger::new(&cost_dir).unwrap());

        // Write a row dated today that puts spend over the $20 cap.
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        cost_ledger
            .append(&CostRow {
                ts: format!("{}T00:00:00Z", today),
                request_id: "test-req-1".to_string(),
                tier: "yoyo".to_string(),
                model: "olmo3".to_string(),
                cost_usd: 21.00,
                inference_ms: 72_000,
                adapter_version: None,
            })
            .unwrap();

        let mut yoyo_map = HashMap::new();
        yoyo_map.insert("default".to_string(), make_yoyo(None));
        let doorman = Doorman::new(
            DoormanConfig {
                local: None,
                yoyo: yoyo_map,
                external: None,
                lark_validator: None,
                graph_context_client: None,
                tier_a_first: false,
                daily_yoyo_cap_usd: Some(20.0),
                cost_ledger: Some(cost_ledger),
            },
            ledger(),
        );
        // Tier A is not configured (local: None), so if the cap logic is
        // working it will redirect to Local — which is then TierUnavailable
        // because no local client exists. That's the expected signal.
        let result = doorman.select_tier(&req(Complexity::High, None, None));
        match result {
            // local is None, so confirm_tier_with_req(Local) → TierUnavailable(Local)
            Err(DoormanError::TierUnavailable(Tier::Local)) => {}
            other => panic!("expected TierUnavailable(Local) after cap redirect, got {other:?}"),
        }
    }

    /// When today's spend is below the daily cap, Tier B is selected normally.
    #[test]
    fn daily_cap_not_exceeded_allows_tier_b() {
        let cost_dir = std::env::temp_dir().join(format!(
            "slm-cap-test-ok-{}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        let cost_ledger = Arc::new(CostLedger::new(&cost_dir).unwrap());

        // Write a row that puts spend well below the $20 cap.
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        cost_ledger
            .append(&CostRow {
                ts: format!("{}T00:00:00Z", today),
                request_id: "test-req-2".to_string(),
                tier: "yoyo".to_string(),
                model: "olmo3".to_string(),
                cost_usd: 1.50,
                inference_ms: 72_000,
                adapter_version: None,
            })
            .unwrap();

        let mut yoyo_map = HashMap::new();
        yoyo_map.insert("default".to_string(), make_yoyo(None));
        let doorman = Doorman::new(
            DoormanConfig {
                local: None,
                yoyo: yoyo_map,
                external: None,
                lark_validator: None,
                graph_context_client: None,
                tier_a_first: false,
                daily_yoyo_cap_usd: Some(20.0),
                cost_ledger: Some(cost_ledger),
            },
            ledger(),
        );
        let picked = doorman
            .select_tier(&req(Complexity::High, None, None))
            .expect("cap not exceeded — should pick yoyo");
        assert_eq!(picked, Tier::Yoyo);
    }
}
