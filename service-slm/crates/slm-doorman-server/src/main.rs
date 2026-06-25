// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `slm-doorman-server` — HTTP entry point for the service-slm Doorman.
//!
//! B1 scope: bind axum, mount /healthz, /readyz, /v1/contract, and a
//! POST /v1/chat/completions stub that forwards through `Doorman::route`.
//! Tier B (Yo-Yo) wiring lands in B2; Tier C (External) in B4.
//!
//! Environment configuration:
//!   SLM_BIND_ADDR             default 127.0.0.1:9080
//!   SLM_LOCAL_ENDPOINT        default http://127.0.0.1:8080  (Tier A)
//!   SLM_LOCAL_MODEL           default olmo-3-7b-instruct
//!   SLM_YOYO_ENDPOINT         optional; absent = no Yo-Yo (community-tier mode)
//!   SLM_YOYO_MODEL            default Olmo-3-1125-32B-Think
//!   SLM_YOYO_BEARER           static bearer token used by Tier B (B2);
//!   SLM_YOYO_HEALTH_PATH      health probe path (default /health; use / for Ollama)
//!   SLM_YOYO_GCP_AUTH         if "true", use GCP metadata identity tokens instead of
//!                              SLM_YOYO_BEARER (required for Cloud Run endpoints)
//!                             real deployments swap StaticBearer for a
//!                             provider-specific BearerTokenProvider impl
//!   SLM_YOYO_HOURLY_USD       per-provider hourly USD rate used to
//!                             compute Tier B cost_usd in the audit
//!                             ledger; default 0.0 (unknown/dev).
//!                             Example: 0.84 for GCP L4, 0.34 for RunPod L4
//!   SLM_APPRENTICESHIP_ENABLED  AS-2..AS-4 endpoints (POST /v1/brief,
//!                             /v1/verdict, /v1/shadow). Default off.
//!                             Set to `true` or `1` to enable.
//!   FOUNDRY_ROOT              workspace root used by the apprenticeship
//!                             dispatcher to resolve scope.files paths
//!                             and read citations.yaml; default
//!                             /srv/foundry.
//!   SLM_BRIEF_TIER_B_THRESHOLD_CHARS
//!                             char-budget proxy for Tier-B routing on
//!                             /v1/brief; default 8000 (~2000 tokens).
//!   FOUNDRY_ALLOWED_SIGNERS   path to allowed_signers used by AS-3
//!                             ssh-keygen -Y verify; default
//!                             ${FOUNDRY_ROOT}/identity/allowed_signers.
//!   FOUNDRY_DOCTRINE_VERSION  doctrine version embedded in apprenticeship
//!                             corpus tuples; default 0.0.7.
//!   FOUNDRY_TENANT            tenant tag on corpus tuples; default pointsav.
//!   SLM_AUDIT_DIR             directory for the append-only JSONL audit ledger.
//!                             If unset, defaults to $HOME/.service-slm/audit/.
//!                             The directory is created on startup if absent.
//!                             A creation failure is non-fatal: the server logs
//!                             a warning and falls back to the default location.
//!   SLM_LARK_VALIDATION_ENABLED  pre-validate Lark grammars at the Doorman
//!                             boundary using llguidance (PS.3 step 5).
//!                             Default true. Set to `false` or `0` to disable.
//!   SERVICE_CONTENT_ENDPOINT  service-content graph API base URL
//!                             (e.g. http://127.0.0.1:9081). When absent
//!                             the Doorman proceeds without graph context.
//!   SLM_AUDIT_TENANT_CONCURRENCY_CAP
//!                             maximum number of concurrent in-flight audit
//!                             requests per tenant (moduleId) across BOTH
//!                             /v1/audit/proxy and /v1/audit/capture. Excess
//!                             requests → 503 SERVICE_UNAVAILABLE with
//!                             Retry-After: 5. Default 4.
//!   SLM_ORCHESTRATION_ENDPOINT  base URL of the app-orchestration-slm chassis
//!                             (e.g. http://10.0.0.1:9180). When set, the
//!                             Doorman POSTs its identity to the chassis on
//!                             startup (non-blocking). Absent = standalone mode.
//!   SLM_MODULE_ID             flat module identifier for chassis registration
//!                             (e.g. "project-jennifer"). Required when
//!                             SLM_ORCHESTRATION_ENDPOINT is set.
//!   SLM_ARCHIVE_ID            archive name for chassis registration
//!                             (e.g. "cluster-totebox-jennifer").
//!   SLM_TIER_B_SUBSCRIBED     "true" or "1" if this archive has a paid Tier B
//!                             subscription via the chassis. Default false.
//!   RUST_LOG                  default slm_doorman=info,slm_doorman_server=info
//!
//! Per `conventions/three-ring-architecture.md` the Doorman boots fine
//! with no Yo-Yo configured (Optional Intelligence). B5 verifies this
//! end-to-end.

use slm_doorman_server::http;
use slm_doorman_server::idle_monitor::IdleMonitorConfig;
use slm_doorman_server::queue::{
    dequeue_shadow, ensure_dirs, reap_expired_leases, release_shadow, QueueConfig, ReleaseOutcome,
};

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Context;
use slm_doorman::tier::{
    BearerTokenProvider, ExternalTierClient, ExternalTierConfig, LocalTierClient, LocalTierConfig,
    MetadataBearer, PricingConfig, StaticBearer, TierCPricing, TierCProvider, YoYoTierClient,
    YoYoTierConfig, FOUNDRY_DEFAULT_ALLOWLIST,
};
use slm_doorman::{
    ApprenticeshipConfig, AuditLedger, AuditProxyClient, AuditProxyConfig, BriefCache, Doorman,
    DoormanConfig, ExpressLane, GraphContextClient, LarkValidator, PromotionLedger,
    SshKeygenVerifier, VerdictDispatcher, VerdictVerifier, FOUNDRY_DEFAULT_PURPOSE_ALLOWLIST,
};
use slm_doorman::express_lane::DEFAULT_BATCH_SLOTS;
use tracing::{info, warn};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    let bind_addr: SocketAddr = std::env::var("SLM_BIND_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:9080".to_string())
        .parse()
        .context("SLM_BIND_ADDR must be a socket address")?;

    let doorman = build_doorman()?;
    let apprenticeship = build_apprenticeship_config();
    let brief_cache = Arc::new(BriefCache::default());
    let verdict_dispatcher = match apprenticeship.as_ref() {
        Some(cfg) => Some(build_verdict_dispatcher(cfg, brief_cache.clone())?),
        None => None,
    };
    let audit_proxy_client = build_audit_proxy_client();

    // SLM_AUDIT_TENANT_CONCURRENCY_CAP — maximum in-flight audit requests per
    // tenant across both /v1/audit/proxy and /v1/audit/capture. Default 4.
    let audit_tenant_concurrency_cap: u32 = std::env::var("SLM_AUDIT_TENANT_CONCURRENCY_CAP")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(4);

    // Brief Queue Substrate (§7C) — build QueueConfig before constructing
    // AppState so both the handler and the drain worker share the same config.
    let queue_cfg = QueueConfig::from_env();

    // Graph proxy — reuse the SERVICE_CONTENT_ENDPOINT already consumed by
    // GraphContextClient above. Default to 127.0.0.1:9081 if unset so the
    // proxy is available in community-tier deployments without extra config.
    let service_content_endpoint = std::env::var("SERVICE_CONTENT_ENDPOINT")
        .unwrap_or_else(|_| http::DEFAULT_SERVICE_CONTENT_ENDPOINT.to_string());

    // SLM_BATCH_SLOTS — concurrency limit for /v1/chat/completions and /v1/messages.
    // Defaults to DEFAULT_BATCH_SLOTS (2). Returns 429 when all slots are in use.
    // Set to a higher value on nodes with more VRAM headroom (e.g. 4 on L4/24GB).
    let batch_slots: usize = std::env::var("SLM_BATCH_SLOTS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_BATCH_SLOTS);
    let mut express_caps = HashMap::new();
    express_caps.insert("batch".to_string(), batch_slots);
    let express_lane = Arc::new(ExpressLane::new(express_caps));
    info!(batch_slots, "express lane initialised");

    // Node class: env-var override or default "hardware".
    // "micro" = $7/mo e2-micro fleet; "hardware" = workspace VM; "cloud" = GCE GPU node.
    let node_class: &'static str = match std::env::var("SLM_NODE_CLASS").as_deref() {
        Ok("micro") => "micro",
        Ok("cloud") => "cloud",
        _ => "hardware",
    };

    // Derive Tier A availability reason for /readyz diagnostics.
    let force_broker = std::env::var("SLM_FORCE_BROKER_MODE")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false);
    let tier_a_reason: &'static str = if force_broker {
        "force-broker-mode"
    } else if node_class == "micro" {
        "micro-node-class"
    } else if doorman.has_local() {
        "available"
    } else {
        "no-local-tier"
    };

    let state = Arc::new(http::AppState {
        doorman,
        apprenticeship,
        brief_cache,
        verdict_dispatcher,
        audit_proxy_client,
        // PS.4 step 3 — purpose allowlist. Default: four documented purposes.
        // Operator overrides by replacing with a custom const via deployment
        // env config (compile-time extension per doctrine).
        audit_proxy_purpose_allowlist: FOUNDRY_DEFAULT_PURPOSE_ALLOWLIST,
        // Per-tenant concurrency semaphore map — lazily populated on first
        // request from each tenant.
        audit_tenant_concurrency: Arc::new(Mutex::new(HashMap::new())),
        audit_tenant_concurrency_cap,
        // Brief Queue Substrate (§7C) — shadow_handler enqueues here;
        // drain worker reads from the same config.
        queue_config: Arc::new(queue_cfg.clone()),
        // Graph proxy — base URL for service-content (datagraph-access-discipline).
        service_content_endpoint,
        node_class,
        tier_a_reason,
        express_lane,
    });

    info!(
        version = slm_doorman::DOORMAN_VERSION,
        %bind_addr,
        has_local = state.doorman.has_local(),
        has_yoyo = state.doorman.has_yoyo(),
        has_external = state.doorman.has_external(),
        apprenticeship_enabled = state.apprenticeship.is_some(),
        audit_proxy_enabled = state.audit_proxy_client.is_some(),
        "service-slm Doorman starting"
    );

    // ── Brief Queue Substrate (apprenticeship-substrate.md §7C) ─────────
    //
    // Spawn two background tokio tasks:
    //   1. `queue_drain_worker` — polls queue/ at configurable interval and
    //      dispatches briefs to the apprentice via dispatch_shadow.
    //   2. `queue_reaper`       — reclaims expired leases from queue-in-flight/
    //      so crashed workers' briefs are retried.
    //
    // Both tasks run regardless of SLM_APPRENTICESHIP_ENABLED.  If
    // apprenticeship is disabled the drain worker finds no briefs in the queue
    // (capture-edit.py also checks the flag before writing) and exits each
    // poll cycle immediately.  This keeps the queue infrastructure live and
    // ready for the flag to be enabled without a restart.
    //
    // Env vars:
    //   SLM_QUEUE_DRAIN_INTERVAL_SEC   drain poll interval; default 30s
    //   SLM_QUEUE_LEASE_EXPIRY_SEC     lease age before reaper reclaims; default 2100s
    //   SLM_DRAIN_MAX_RETRIES          retries before a brief is poisoned; default 5
    {
        // Ensure queue directories exist at startup so the background tasks
        // can scan them immediately.  A creation failure is non-fatal (we log
        // and continue); the tasks will retry on each cycle.
        if let Err(e) = ensure_dirs(&queue_cfg) {
            tracing::warn!(error = %e, "brief queue directory bootstrap failed; retrying lazily");
        }

        // ── Drain worker ─────────────────────────────────────────────────
        let drain_interval_secs: u64 = std::env::var("SLM_QUEUE_DRAIN_INTERVAL_SEC")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(30);
        let drain_interval = Duration::from_secs(drain_interval_secs);

        // Maximum times a brief is retried before being moved to queue-poison/.
        // A brief that always fails (scope-resolution error, unreachable files,
        // etc.) would otherwise retry indefinitely and block the serial drain.
        let max_retries: u32 = std::env::var("SLM_DRAIN_MAX_RETRIES")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(5);

        // Sprint 3C: hold queue when all Tier B nodes have been circuit-open
        // for longer than this threshold. Briefs stay in queue/ until circuit
        // closes. Env var: SLM_HOLD_THRESHOLD_SECS (default 3600 = 1 h).
        // When all Tier B nodes are circuit-open or health-probe-down for longer
        // than this threshold, the drain worker holds the queue (no dispatch).
        // Not bypassed by SLM_TIER_A_FIRST — see Sprint 3C hold below.
        let hold_threshold_secs: u64 = std::env::var("SLM_HOLD_THRESHOLD_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(3600);
        // Read for consistency; the drain worker no longer uses this value
        // directly (Sprint 3C hold and yoyo_node_ready guard replaced the
        // old tier_a_first bypass). Doorman config reads it again at startup.
        let _tier_a_first: bool = std::env::var("SLM_TIER_A_FIRST")
            .ok()
            .map(|v| v.eq_ignore_ascii_case("true") || v == "1")
            .unwrap_or(false);

        // SLM_DRAIN_PAUSED: hard, unconditional pause of shadow-brief dispatch.
        // When set, the drain worker skips every cycle WITHOUT dequeuing —
        // briefs stay untouched in queue/ and the inference tier is never hit.
        // Decoupled from SLM_TIER_A_FIRST (which bypasses the Sprint 3C hold)
        // and from SLM_APPRENTICESHIP_ENABLED (which 404s the /v1/shadow capture
        // endpoint). This lets the operator stop wasteful CPU drain while keeping
        // capture writing new briefs to queue/ for later GPU processing.
        // Read once at startup; restart Doorman to change. See
        // BRIEF-slm-learning-loop.md §10.2.
        let drain_paused: bool = std::env::var("SLM_DRAIN_PAUSED")
            .ok()
            .map(|v| v.eq_ignore_ascii_case("true") || v == "1")
            .unwrap_or(false);

        // Payload size gate: briefs exceeding this byte limit are poisoned without
        // dispatch. Prevents oversized payloads (large diffs, injected data) from
        // wedging OLMo or consuming unbounded tokens. Default 16 KiB.
        // Env var: SLM_QUEUE_MAX_PAYLOAD_BYTES
        let max_payload_bytes: usize = std::env::var("SLM_QUEUE_MAX_PAYLOAD_BYTES")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(16 * 1024);

        // Number of concurrent drain workers. The queue uses file-level locking
        // (QueueLockFailed → 2s back-off) so N workers race safely — each grabs
        // a different lease file. With Tier B GPU, 4 workers × 227s/item ≈ 18h
        // to drain 1,128 items; 1 worker ≈ 71h. Default 1 for safe rollout;
        // set SLM_DRAIN_CONCURRENCY=4 in systemd override once Tier B is stable.
        let drain_concurrency: usize = std::env::var("SLM_DRAIN_CONCURRENCY")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(1)
            .max(1);

        for drain_worker_num in 0..drain_concurrency {
        let drain_cfg = queue_cfg.clone();
        let drain_doorman_arc = Arc::clone(&state);

        tokio::spawn(async move {
            // Worker identifier — PID + worker number makes lease filenames
            // unique across Doorman restarts and concurrent workers.
            let worker_id = format!("drain-{}-{}", std::process::id(), drain_worker_num);
            info!(
                %worker_id,
                drain_interval_secs,
                "brief queue drain worker started"
            );

            if drain_paused {
                info!(
                    %worker_id,
                    "drain worker: SLM_DRAIN_PAUSED=true — dispatch suspended; \
                     capture continues writing to queue/ for later GPU processing"
                );
            }

            loop {
                // SLM_DRAIN_PAUSED: unconditional pause — never dequeue, never
                // dispatch. Briefs accumulate untouched in queue/. The reaper
                // still runs (separate task) to reclaim any stale in-flight
                // leases. Highest-priority skip — checked before the Sprint 3C
                // hold and before any dequeue.
                if drain_paused {
                    tokio::time::sleep(drain_interval).await;
                    continue;
                }

                // Sprint 3C (+ health_up extension): when all configured Tier B
                // nodes have been circuit-open longer than the hold threshold,
                // or when all Tier B health probes are currently failing, skip
                // this drain cycle. Briefs accumulate in queue/ until Tier B
                // recovers. Health probe failures do not trip the circuit
                // breaker directly (only dispatch failures do), so the
                // original circuit-only predicate would miss the health-down
                // case. SLM_TIER_A_FIRST does NOT bypass this hold: draining
                // shadow briefs through Tier A when Tier B is offline starves
                // entity extraction (same OLMo 7B slot). Hold regardless of
                // tier routing preference.
                let tier_b = drain_doorman_arc.doorman.tier_b_status();
                if !tier_b.is_empty()
                    && tier_b.values().all(|info| {
                        // Either the circuit has been open long enough, or
                        // health probes are failing (circuit may still be
                        // "closed" if no dispatch failures have occurred yet).
                        let effectively_down = info.circuit == "open" || !info.health_up;
                        let long_enough = info
                            .opened_for_secs
                            .map(|s| s >= hold_threshold_secs)
                            // No circuit timer means circuit is closed but health
                            // is down — treat as long enough (3 consecutive probe
                            // failures already confirm the node is offline).
                            .unwrap_or(!info.health_up);
                        effectively_down && long_enough
                    })
                {
                    info!(
                        hold_threshold_secs,
                        "drain worker: all Tier B nodes offline (circuit or health) — holding queue"
                    );
                    tokio::time::sleep(drain_interval).await;
                    continue;
                }

                // Drain-target guard: hold if the specific "trainer" node is
                // circuit-open or health-probe-down. The Sprint 3C hold above
                // handles the all-nodes-down case; this guard handles the
                // targeted-node case independently of the global tier_a_first
                // setting. Checked before dequeue so no lease is acquired during
                // the hold — the brief stays untouched in queue/.
                //
                // Does not close the 5-failure startup window (allow_request()
                // is optimistic until the circuit opens), but closes the
                // steady-state gap where select_tier() would fall to Tier A
                // after the circuit has opened.
                if !drain_doorman_arc.doorman.yoyo_node_ready("trainer") {
                    tracing::debug!(
                        target: "slm_doorman_server",
                        %worker_id,
                        "drain worker: trainer node not ready (circuit-open or health-down) \
                         — holding queue to protect Tier A inference slots"
                    );
                    tokio::time::sleep(drain_interval).await;
                    continue;
                }

                match dequeue_shadow(&drain_cfg, &worker_id) {
                    Ok(None) => {
                        // Queue empty; sleep and poll again.
                        tokio::time::sleep(drain_interval).await;
                    }
                    Ok(Some(leased)) => {
                        let brief_id = leased.entry.brief.brief_id.clone();

                        // Payload size gate: poison oversized briefs immediately so
                        // they never reach OLMo or the dispatch path.
                        let payload_size = serde_json::to_vec(&leased.entry)
                            .map(|v| v.len())
                            .unwrap_or(usize::MAX);
                        if payload_size > max_payload_bytes {
                            tracing::warn!(
                                brief_id = %brief_id,
                                size_bytes = payload_size,
                                max_bytes = max_payload_bytes,
                                "drain worker: oversized payload — poisoning without dispatch"
                            );
                            if let Err(e) =
                                release_shadow(&drain_cfg, &leased, ReleaseOutcome::Poison)
                            {
                                tracing::warn!(
                                    brief_id = %brief_id,
                                    error = %e,
                                    "drain worker: release_shadow failed for oversized entry"
                                );
                            }
                            continue;
                        }

                        // P0 guard: skip briefs with an empty actual_diff. These
                        // carry no ground-truth reference, so dispatching them to
                        // OLMo yields a hallucinated diff with nothing to compare
                        // against — pure wasted CPU. Worse, OLMo can run away on
                        // such out-of-distribution prompts and block the whole
                        // drain queue for the full max_tokens budget. Move straight
                        // to done without ever touching the inference tier. The
                        // decision lives in `drain::classify_shadow_brief` so it is
                        // unit-testable (drain.rs + drain_worker_integration test).
                        if matches!(
                            slm_doorman_server::drain::classify_shadow_brief(&leased.entry),
                            slm_doorman_server::drain::DrainDecision::Skip
                        ) {
                            tracing::warn!(
                                brief_id = %brief_id,
                                task_type = %leased.entry.brief.task_type,
                                "drain worker: skipping empty-diff brief (no actual_diff captured); \
                                 marking done without OLMo dispatch"
                            );
                            if let Err(e) =
                                release_shadow(&drain_cfg, &leased, ReleaseOutcome::Done)
                            {
                                tracing::warn!(
                                    brief_id = %brief_id,
                                    error = %e,
                                    "drain worker: release_shadow failed for empty-diff brief"
                                );
                            }
                            continue;
                        }

                        info!(
                            brief_id = %brief_id,
                            task_type = %leased.entry.brief.task_type,
                            "drain worker: dispatching queued shadow brief"
                        );

                        // Only dispatch if apprenticeship is enabled.
                        let outcome = if let Some(cfg) = drain_doorman_arc.apprenticeship.as_ref() {
                            use slm_doorman::ApprenticeshipDispatcher;
                            // The drain queue exists specifically to use Tier B for
                            // enrichment when it is available. Override tier_a_first
                            // (which is normally true to protect real-time paths from
                            // accruing GPU charges) and pin the yoyo node to "trainer"
                            // so briefs reach the L4 GPU rather than any offline default
                            // node. The hold-check above already ensures Tier B is healthy
                            // before this dispatcher is created.
                            let mut drain_cfg = cfg.clone();
                            drain_cfg.tier_a_first = false;
                            // All drain items go to Tier B regardless of body size:
                            // the queue exists for Tier B enrichment, and brief bodies
                            // are 100–400 chars while the default threshold is 8,000 —
                            // without this override nothing ever routes to Yoyo.
                            drain_cfg.brief_tier_b_threshold_chars = 0;
                            drain_cfg.yoyo_dispatch_label = Some("trainer".to_string());
                            let dispatcher = ApprenticeshipDispatcher::with_cache(
                                &drain_doorman_arc.doorman,
                                drain_cfg,
                                Arc::clone(&drain_doorman_arc.brief_cache),
                            );
                            // Pass the actual_diff from the queue entry so the
                            // corpus tuple carries the senior's real committed diff
                            // (per §7B capture-on-completion semantics).
                            // 1860 s safety-net: the Tier A HTTP client timeout is
                            // 1800 s; this wrapper fires 60 s later to catch any
                            // path that bypasses the client timeout. With the
                            // empty-diff guard above and the Tier A max_tokens=512
                            // cap, worst-case Tier A dispatch is ~4 min, so this
                            // timeout should never fire in practice.
                            let dispatch_result = tokio::time::timeout(
                                std::time::Duration::from_secs(1860),
                                dispatcher.dispatch_shadow(
                                    &leased.entry.brief,
                                    &leased.entry.actual_diff,
                                ),
                            )
                            .await;
                            match dispatch_result {
                                Err(_elapsed) => {
                                    tracing::warn!(
                                        brief_id = %brief_id,
                                        "drain worker: dispatch timed out after 1860s — \
                                         brief will be re-queued by reaper"
                                    );
                                    ReleaseOutcome::Retry
                                }
                                Ok(Ok(_)) => {
                                    info!(brief_id = %brief_id, "drain worker: shadow dispatch ok");
                                    ReleaseOutcome::Done
                                }
                                Ok(Err(e)) => {
                                    tracing::warn!(
                                        brief_id = %brief_id,
                                        error = %e,
                                        "drain worker: shadow dispatch failed; retry"
                                    );
                                    // Check for malformed-brief class errors that should
                                    // not be retried — move to poison instead.
                                    if matches!(
                                        e,
                                        slm_doorman::DoormanError::QueueMalformedBrief { .. }
                                    ) {
                                        ReleaseOutcome::Poison
                                    } else {
                                        ReleaseOutcome::Retry
                                    }
                                }
                            }
                        } else {
                            // Apprenticeship disabled — re-queue the brief for when
                            // the operator enables the flag without restarting.
                            tracing::debug!(
                                brief_id = %brief_id,
                                "drain worker: apprenticeship disabled; re-queuing brief"
                            );
                            ReleaseOutcome::Retry
                        };

                        // Retry counter: escalate Retry → Poison once a brief
                        // has been retried too many times. Prevents a single
                        // persistently-failing brief from blocking the serial
                        // drain queue indefinitely.
                        let outcome = if outcome == ReleaseOutcome::Retry {
                            let attempts = slm_doorman_server::queue::bump_attempts(
                                &drain_cfg, &brief_id,
                            )
                            .unwrap_or_else(|e| {
                                tracing::warn!(
                                    brief_id = %brief_id,
                                    error = %e,
                                    "drain worker: attempts counter I/O error; treating as 1"
                                );
                                1
                            });
                            if attempts >= max_retries {
                                tracing::warn!(
                                    brief_id = %brief_id,
                                    attempts,
                                    max_retries,
                                    "drain worker: max retries reached — poisoning brief"
                                );
                                ReleaseOutcome::Poison
                            } else {
                                tracing::info!(
                                    brief_id = %brief_id,
                                    attempts,
                                    max_retries,
                                    "drain worker: brief retry {attempts}/{max_retries}"
                                );
                                ReleaseOutcome::Retry
                            }
                        } else {
                            outcome
                        };

                        // Clear the attempts sidecar on terminal outcomes so
                        // stale counters do not accumulate in queue-attempts/.
                        if matches!(outcome, ReleaseOutcome::Done | ReleaseOutcome::Poison) {
                            slm_doorman_server::queue::clear_attempts(&drain_cfg, &brief_id);
                        }

                        if let Err(e) = release_shadow(&drain_cfg, &leased, outcome) {
                            tracing::warn!(
                                brief_id = %brief_id,
                                error = %e,
                                "drain worker: release_shadow failed"
                            );
                        }

                        // Back off after a transient failure so we don't spin
                        // tight-looping on briefs when the inference tier is
                        // unavailable (circuit open, Yo-Yo offline, etc.).
                        if outcome == ReleaseOutcome::Retry {
                            tokio::time::sleep(drain_interval).await;
                        }
                        // Do NOT sleep on Done/Poison — drain the queue as fast
                        // as the apprentice tier allows when it IS available.
                    }
                    Err(slm_doorman::DoormanError::QueueLockFailed { .. }) => {
                        // Another worker (or the reaper) holds the lock.  Back off
                        // and retry after a short interval.
                        tokio::time::sleep(Duration::from_secs(2)).await;
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "drain worker: dequeue error; sleeping");
                        tokio::time::sleep(drain_interval).await;
                    }
                }
            }
        }); // end tokio::spawn
        } // end for drain_worker_num in 0..drain_concurrency

        // ── Reaper task ───────────────────────────────────────────────────
        let reap_interval = Duration::from_secs(60);
        // 2100 s = dispatch timeout (1860 s) + 240 s buffer. Must be > the
        // dispatch timeout or the reaper reclaims in-flight leases mid-dispatch,
        // producing spurious retries. Was 300 s (too short).
        let lease_expiry_secs: u64 = std::env::var("SLM_QUEUE_LEASE_EXPIRY_SEC")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(2_100);
        let lease_expiry = Duration::from_secs(lease_expiry_secs);

        let reap_cfg = queue_cfg.clone();

        tokio::spawn(async move {
            info!(
                lease_expiry_secs,
                reap_interval_secs = reap_interval.as_secs(),
                "brief queue reaper started"
            );
            loop {
                tokio::time::sleep(reap_interval).await;
                match reap_expired_leases(&reap_cfg, lease_expiry) {
                    Ok(0) => {} // nothing to do
                    Ok(n) => info!(reclaimed = n, "reaper: reclaimed expired leases"),
                    Err(e) => tracing::warn!(error = %e, "reaper: reap_expired_leases failed"),
                }
            }
        });
    }
    // ────────────────────────────────────────────────────────────────────

    // ── Yo-Yo idle monitor (B5) ─────────────────────────────────────────
    //
    // Polls llama-server /metrics every 5 min. After SLM_YOYO_IDLE_MINUTES
    // (default 30) of zero active slots, sends a GCP instances.stop request
    // via the workspace SA ADC token from the GCE metadata server.
    // Requires all four GCP env vars — absent any, the monitor does not start.
    if let Some(idle_cfg) = IdleMonitorConfig::from_env() {
        info!(
            idle_threshold_secs = idle_cfg.idle_threshold.as_secs(),
            gcp_instance = %idle_cfg.gcp_instance,
            "Yo-Yo idle monitor enabled"
        );
        tokio::spawn(slm_doorman_server::idle_monitor::run_idle_monitor(idle_cfg));
    }
    // ────────────────────────────────────────────────────────────────────

    // ── Chassis self-registration (app-orchestration-slm) ────────────────
    //
    // When SLM_ORCHESTRATION_ENDPOINT is set, POST our identity to the
    // chassis on startup so it can include us in GET /v1/fleet.
    // Non-blocking — a registration failure never prevents the Doorman
    // from serving local requests.
    //
    // Env vars:
    //   SLM_ORCHESTRATION_ENDPOINT  chassis base URL (e.g. http://10.0.0.1:9180)
    //   SLM_MODULE_ID               flat module identifier (e.g. "project-jennifer")
    //   SLM_ARCHIVE_ID              archive name (e.g. "cluster-totebox-jennifer")
    //   SLM_TIER_B_SUBSCRIBED       "true" if this archive has a paid Tier B
    //                               subscription; default false
    if let Ok(chassis_endpoint) = std::env::var("SLM_ORCHESTRATION_ENDPOINT") {
        let module_id = std::env::var("SLM_MODULE_ID").unwrap_or_default();
        let archive_id = std::env::var("SLM_ARCHIVE_ID").unwrap_or_default();
        let tier_b_subscribed = std::env::var("SLM_TIER_B_SUBSCRIBED")
            .map(|v| v.eq_ignore_ascii_case("true") || v == "1")
            .unwrap_or(false);
        info!(
            %chassis_endpoint,
            %module_id,
            %archive_id,
            tier_b_subscribed,
            "registering with orchestration chassis"
        );
        tokio::spawn(async move {
            let body = serde_json::json!({
                "module_id": module_id,
                "archive_id": archive_id,
                "doorman_endpoint": "",
                "tier_b_subscribed": tier_b_subscribed
            });
            let url = format!("{chassis_endpoint}/v1/discovery/register");
            match reqwest::Client::new().post(&url).json(&body).send().await {
                Ok(resp) if resp.status().is_success() => {
                    tracing::info!(%url, "chassis registration succeeded");
                }
                Ok(resp) => {
                    tracing::warn!(%url, status = %resp.status(), "chassis registration rejected");
                }
                Err(e) => {
                    tracing::warn!(%url, error = %e, "chassis registration failed; continuing");
                }
            }
        });
    }
    // ────────────────────────────────────────────────────────────────────

    let app = http::router(state);
    let listener = tokio::net::TcpListener::bind(bind_addr)
        .await
        .with_context(|| format!("failed to bind {bind_addr}"))?;
    axum::serve(listener, app)
        .await
        .context("axum serve loop exited")?;
    Ok(())
}

fn build_doorman() -> anyhow::Result<Doorman> {
    let force_broker = std::env::var("SLM_FORCE_BROKER_MODE")
        .map(|v| matches!(v.trim(), "true" | "1"))
        .unwrap_or(false);
    let tier_a_first = std::env::var("SLM_TIER_A_FIRST")
        .map(|v| matches!(v.trim(), "true" | "1"))
        .unwrap_or(false);

    if force_broker && tier_a_first {
        anyhow::bail!(
            "SLM_FORCE_BROKER_MODE=true and SLM_TIER_A_FIRST=true are mutually exclusive. \
             FORCE_BROKER_MODE disables Tier A entirely; TIER_A_FIRST makes it the primary. \
             Set at most one of these flags."
        );
    }

    if tier_a_first {
        info!("SLM_TIER_A_FIRST=true: Tier A is the confident primary; Tier B used only when explicitly hinted and circuit closed");
    }

    // ── Admission control semaphores ─────────────────────────────────────────
    // SLM_LOCAL_CONCURRENT (default 2): total OLMo slots across all callers.
    // SLM_BACKGROUND_CONCURRENT (default 1): cap for extraction + drain only;
    //   ensures at least one slot is always free for interactive callers.
    // SLM_TIER_B_CONCURRENT (default 4): GPU handles more concurrency than CPU.
    let local_concurrent = std::env::var("SLM_LOCAL_CONCURRENT")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(2);
    let background_concurrent = std::env::var("SLM_BACKGROUND_CONCURRENT")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(1);
    let tier_b_concurrent = std::env::var("SLM_TIER_B_CONCURRENT")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(4);
    info!(
        local_concurrent,
        background_concurrent,
        tier_b_concurrent,
        "admission control semaphores initialised"
    );
    let total_sem = Arc::new(tokio::sync::Semaphore::new(local_concurrent));
    let background_sem = Arc::new(tokio::sync::Semaphore::new(background_concurrent));
    let tier_b_sem = Arc::new(tokio::sync::Semaphore::new(tier_b_concurrent));

    let local = if force_broker {
        info!("SLM_FORCE_BROKER_MODE=true: Tier A disabled; all inference routes to Yo-Yo");
        None
    } else {
        Some(
            LocalTierClient::new(LocalTierConfig {
                endpoint: std::env::var("SLM_LOCAL_ENDPOINT")
                    .unwrap_or_else(|_| "http://127.0.0.1:8080".to_string()),
                default_model: std::env::var("SLM_LOCAL_MODEL")
                    .unwrap_or_else(|_| "olmo-3-7b-instruct".to_string()),
            })
            .with_semaphores(Arc::clone(&total_sem), Arc::clone(&background_sem)),
        )
    };

    let mut yoyo = std::collections::HashMap::new();

    // 1. Check for legacy SLM_YOYO_ENDPOINT (mapped to "default")
    if let Some(client) = build_yoyo_client(
        "SLM_YOYO_ENDPOINT",
        "SLM_YOYO_MODEL",
        "SLM_YOYO_BEARER",
        "SLM_YOYO_HOURLY_USD",
    )
    .map(|c| c.with_concurrency_sem(Arc::clone(&tier_b_sem)))
    {
        yoyo.insert("default".to_string(), client);
    }

    // 2. Check for specialized Multi-Yo-Yo endpoints (Leapfrog 2030)
    if let Some(client) = build_yoyo_client(
        "SLM_YOYO_TRAINER_ENDPOINT",
        "SLM_YOYO_TRAINER_MODEL",
        "SLM_YOYO_TRAINER_BEARER",
        "SLM_YOYO_TRAINER_HOURLY_USD",
    )
    .map(|c| c.with_concurrency_sem(Arc::clone(&tier_b_sem)))
    {
        info!("Yo-Yo 'trainer' node configured");
        yoyo.insert("trainer".to_string(), client);
    }

    if let Some(client) = build_yoyo_client(
        "SLM_YOYO_GRAPH_ENDPOINT",
        "SLM_YOYO_GRAPH_MODEL",
        "SLM_YOYO_GRAPH_BEARER",
        "SLM_YOYO_GRAPH_HOURLY_USD",
    )
    .map(|c| c.with_concurrency_sem(Arc::clone(&tier_b_sem)))
    {
        info!("Yo-Yo 'graph' node configured");
        yoyo.insert("graph".to_string(), client);
    }

    let external = build_external_tier_client();

    // PS.3 step 5 — Lark grammar pre-validation.
    // Enabled by default; set SLM_LARK_VALIDATION_ENABLED=false to disable
    // (e.g., if the llguidance init overhead is undesirable in a test
    // environment that never submits Lark grammars).
    let lark_validator = {
        let enabled = std::env::var("SLM_LARK_VALIDATION_ENABLED")
            .map(|v| !matches!(v.trim(), "false" | "0"))
            .unwrap_or(true);
        if enabled {
            match LarkValidator::new() {
                Ok(v) => {
                    info!("Lark grammar pre-validation enabled (PS.3 step 5)");
                    Some(v)
                }
                Err(e) => {
                    // Validation init failure is non-fatal — the Doorman
                    // starts without it and logs a warning.
                    tracing::warn!("LarkValidator init failed (Lark pre-validation disabled): {e}");
                    None
                }
            }
        } else {
            info!("Lark grammar pre-validation disabled (SLM_LARK_VALIDATION_ENABLED=false)");
            None
        }
    };

    // Resolve the audit ledger directory.  SLM_AUDIT_DIR takes precedence;
    // fall back to the $HOME/.service-slm/audit/ default on any error.
    let ledger = match std::env::var_os("SLM_AUDIT_DIR") {
        Some(path) if !path.is_empty() => {
            let dir = std::path::PathBuf::from(&path);
            match std::fs::create_dir_all(&dir) {
                Ok(()) => match AuditLedger::new(&dir) {
                    Ok(l) => {
                        info!(audit_dir = %dir.display(), "audit ledger directory (SLM_AUDIT_DIR)");
                        l
                    }
                    Err(e) => {
                        tracing::warn!(
                            audit_dir = %dir.display(),
                            error = %e,
                            "SLM_AUDIT_DIR unusable; falling back to default"
                        );
                        AuditLedger::default_for_user()
                            .context("failed to open fallback audit ledger; ensure HOME is set")?
                    }
                },
                Err(e) => {
                    tracing::warn!(
                        audit_dir = %dir.display(),
                        error = %e,
                        "SLM_AUDIT_DIR create_dir_all failed; falling back to default"
                    );
                    AuditLedger::default_for_user()
                        .context("failed to open fallback audit ledger; ensure HOME is set")?
                }
            }
        }
        _ => {
            let l = AuditLedger::default_for_user()
                .context("failed to open audit ledger; ensure HOME is set")?;
            info!(audit_dir = %l.base_dir().display(), "audit ledger directory (default)");
            l
        }
    };

    // Graph context (service-content Ring 2 — Brief E).
    // When SERVICE_CONTENT_ENDPOINT is set, the Doorman queries the
    // service-content graph before each inference call and injects matching
    // entity rows as a system message. Non-fatal if absent.
    let graph_context_client = std::env::var("SERVICE_CONTENT_ENDPOINT").ok().map(|ep| {
        info!("Graph context enabled; service-content endpoint: {}", ep);
        GraphContextClient::new(ep)
    });

    // Daily Tier B spend cap (P3-3.5-followup). Non-fatal if unavailable.
    let foundry_root = std::env::var_os("FOUNDRY_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("/srv/foundry"));
    let cost_ledger_dir = foundry_root.join("data").join("cost-ledger");
    let cost_ledger = match std::fs::create_dir_all(&cost_ledger_dir)
        .and_then(|_| slm_doorman::cost_ledger::CostLedger::new(&cost_ledger_dir))
    {
        Ok(cl) => {
            info!(dir = %cost_ledger_dir.display(), "cost ledger initialised");
            Some(std::sync::Arc::new(cl))
        }
        Err(e) => {
            tracing::warn!(error = %e, "cost ledger unavailable — no spend tracking or cap enforcement");
            None
        }
    };
    let daily_yoyo_cap_usd = std::env::var("SLM_YOYO_DAILY_CAP_USD")
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
        .filter(|&v| v > 0.0);
    if let Some(cap) = daily_yoyo_cap_usd {
        info!(
            cap_usd = cap,
            "daily Tier B spend cap configured (SLM_YOYO_DAILY_CAP_USD)"
        );
    }

    Ok(Doorman::new(
        DoormanConfig {
            local,
            yoyo,
            external,
            lark_validator,
            graph_context_client,
            tier_a_first,
            daily_yoyo_cap_usd,
            cost_ledger,
        },
        ledger,
    ))
}

fn build_yoyo_client(
    env_endpoint: &str,
    env_model: &str,
    env_bearer: &str,
    env_hourly: &str,
) -> Option<YoYoTierClient> {
    match std::env::var(env_endpoint) {
        Ok(endpoint) if !endpoint.is_empty() => {
            let use_gcp_auth = std::env::var("SLM_YOYO_GCP_AUTH")
                .map(|v| v.eq_ignore_ascii_case("true"))
                .unwrap_or(false);
            if use_gcp_auth && std::env::var("SLM_YOYO_GCP_ZONE").is_err() {
                warn!("SLM_YOYO_GCP_AUTH=true but SLM_YOYO_GCP_ZONE is unset; /readyz zone field will be empty");
            }
            let bearer: Arc<dyn BearerTokenProvider> = if use_gcp_auth {
                Arc::new(MetadataBearer::new(&endpoint))
            } else {
                let bearer_token = std::env::var(env_bearer).unwrap_or_default();
                Arc::new(StaticBearer::new(bearer_token))
            };
            let yoyo_hourly_usd = std::env::var(env_hourly)
                .ok()
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0);
            let health_path =
                std::env::var("SLM_YOYO_HEALTH_PATH").unwrap_or_else(|_| "/health".to_string());
            if !health_path.starts_with('/') {
                eprintln!(
                    "[FATAL] SLM_YOYO_HEALTH_PATH must start with '/' (got {:?})",
                    health_path
                );
                std::process::exit(1);
            }
            Some(YoYoTierClient::new(
                YoYoTierConfig {
                    endpoint,
                    default_model: std::env::var(env_model)
                        .unwrap_or_else(|_| "Olmo-3-1125-32B-Think".to_string()),
                    contract_version: slm_doorman::YOYO_CONTRACT_VERSION.to_string(),
                    pricing: PricingConfig { yoyo_hourly_usd },
                    zone: std::env::var("SLM_YOYO_GCP_ZONE").ok(),
                    health_path,
                },
                bearer,
            ))
        }
        _ => None,
    }
}

/// Build the Tier C (external API) client from env vars. Returns `None`
/// if no provider endpoints are configured — operator cost guardrail
/// ensures no Tier C dispatch happens unless explicitly enabled.
///
/// Env var format per provider:
///   SLM_TIER_C_ANTHROPIC_ENDPOINT      base URL (e.g., https://api.anthropic.com)
///   SLM_TIER_C_ANTHROPIC_API_KEY       API key (can be empty in dev/mock mode)
///   SLM_TIER_C_ANTHROPIC_INPUT_PER_MTOK_USD    pricing (default 0.0)
///   SLM_TIER_C_ANTHROPIC_OUTPUT_PER_MTOK_USD   pricing (default 0.0)
/// Same pattern for GEMINI and OPENAI.
fn build_external_tier_client() -> Option<ExternalTierClient> {
    let mut endpoints = std::collections::HashMap::new();
    let mut api_keys = std::collections::HashMap::new();
    let mut pricing = TierCPricing::default();

    // Anthropic
    if let Ok(endpoint) = std::env::var("SLM_TIER_C_ANTHROPIC_ENDPOINT") {
        if !endpoint.is_empty() {
            endpoints.insert(TierCProvider::Anthropic, endpoint);
            api_keys.insert(
                TierCProvider::Anthropic,
                std::env::var("SLM_TIER_C_ANTHROPIC_API_KEY").unwrap_or_default(),
            );
            pricing.anthropic_input_per_mtok_usd =
                std::env::var("SLM_TIER_C_ANTHROPIC_INPUT_PER_MTOK_USD")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0.0);
            pricing.anthropic_output_per_mtok_usd =
                std::env::var("SLM_TIER_C_ANTHROPIC_OUTPUT_PER_MTOK_USD")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0.0);
        }
    }

    // Gemini
    if let Ok(endpoint) = std::env::var("SLM_TIER_C_GEMINI_ENDPOINT") {
        if !endpoint.is_empty() {
            endpoints.insert(TierCProvider::Gemini, endpoint);
            api_keys.insert(
                TierCProvider::Gemini,
                std::env::var("SLM_TIER_C_GEMINI_API_KEY").unwrap_or_default(),
            );
            pricing.gemini_input_per_mtok_usd =
                std::env::var("SLM_TIER_C_GEMINI_INPUT_PER_MTOK_USD")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0.0);
            pricing.gemini_output_per_mtok_usd =
                std::env::var("SLM_TIER_C_GEMINI_OUTPUT_PER_MTOK_USD")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0.0);
        }
    }

    // OpenAI
    if let Ok(endpoint) = std::env::var("SLM_TIER_C_OPENAI_ENDPOINT") {
        if !endpoint.is_empty() {
            endpoints.insert(TierCProvider::Openai, endpoint);
            api_keys.insert(
                TierCProvider::Openai,
                std::env::var("SLM_TIER_C_OPENAI_API_KEY").unwrap_or_default(),
            );
            pricing.openai_input_per_mtok_usd =
                std::env::var("SLM_TIER_C_OPENAI_INPUT_PER_MTOK_USD")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0.0);
            pricing.openai_output_per_mtok_usd =
                std::env::var("SLM_TIER_C_OPENAI_OUTPUT_PER_MTOK_USD")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0.0);
        }
    }

    // Only build the client if at least one provider is configured.
    if endpoints.is_empty() {
        return None;
    }

    let config = ExternalTierConfig {
        allowlist: FOUNDRY_DEFAULT_ALLOWLIST,
        provider_endpoints: endpoints,
        provider_api_keys: api_keys,
        pricing,
    };

    Some(ExternalTierClient::new(config))
}

/// Build the audit proxy client from env vars. Reuses the same
/// `SLM_TIER_C_*` namespace as `build_external_tier_client()` — the
/// audit_proxy relay and the Tier C compute routing share provider
/// config so operators only need one set of env vars.
///
/// Returns `None` if no providers are configured. An absent client causes
/// `POST /v1/audit/proxy` to return 503 with a clear "unconfigured" message.
fn build_audit_proxy_client() -> Option<AuditProxyClient> {
    let mut endpoints = std::collections::HashMap::new();
    let mut api_keys = std::collections::HashMap::new();
    let mut pricing = TierCPricing::default();

    // Anthropic
    if let Ok(endpoint) = std::env::var("SLM_TIER_C_ANTHROPIC_ENDPOINT") {
        if !endpoint.is_empty() {
            endpoints.insert(TierCProvider::Anthropic, endpoint);
            api_keys.insert(
                TierCProvider::Anthropic,
                std::env::var("SLM_TIER_C_ANTHROPIC_API_KEY").unwrap_or_default(),
            );
            pricing.anthropic_input_per_mtok_usd =
                std::env::var("SLM_TIER_C_ANTHROPIC_INPUT_PER_MTOK_USD")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0.0);
            pricing.anthropic_output_per_mtok_usd =
                std::env::var("SLM_TIER_C_ANTHROPIC_OUTPUT_PER_MTOK_USD")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0.0);
        }
    }

    // Gemini
    if let Ok(endpoint) = std::env::var("SLM_TIER_C_GEMINI_ENDPOINT") {
        if !endpoint.is_empty() {
            endpoints.insert(TierCProvider::Gemini, endpoint);
            api_keys.insert(
                TierCProvider::Gemini,
                std::env::var("SLM_TIER_C_GEMINI_API_KEY").unwrap_or_default(),
            );
            pricing.gemini_input_per_mtok_usd =
                std::env::var("SLM_TIER_C_GEMINI_INPUT_PER_MTOK_USD")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0.0);
            pricing.gemini_output_per_mtok_usd =
                std::env::var("SLM_TIER_C_GEMINI_OUTPUT_PER_MTOK_USD")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0.0);
        }
    }

    // OpenAI
    if let Ok(endpoint) = std::env::var("SLM_TIER_C_OPENAI_ENDPOINT") {
        if !endpoint.is_empty() {
            endpoints.insert(TierCProvider::Openai, endpoint);
            api_keys.insert(
                TierCProvider::Openai,
                std::env::var("SLM_TIER_C_OPENAI_API_KEY").unwrap_or_default(),
            );
            pricing.openai_input_per_mtok_usd =
                std::env::var("SLM_TIER_C_OPENAI_INPUT_PER_MTOK_USD")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0.0);
            pricing.openai_output_per_mtok_usd =
                std::env::var("SLM_TIER_C_OPENAI_OUTPUT_PER_MTOK_USD")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0.0);
        }
    }

    if endpoints.is_empty() {
        return None;
    }

    Some(AuditProxyClient::new(AuditProxyConfig {
        provider_endpoints: endpoints,
        provider_api_keys: api_keys,
        pricing,
        // PS.4 step 3 — default to the four documented purposes.
        purpose_allowlist: FOUNDRY_DEFAULT_PURPOSE_ALLOWLIST,
    }))
}

/// Build the apprenticeship config when `SLM_APPRENTICESHIP_ENABLED=true`.
/// Default off — existing deployments keep their existing behaviour
/// (the three apprenticeship endpoints return 404). Per design-pass Q9
/// + Master's brief.
fn build_apprenticeship_config() -> Option<ApprenticeshipConfig> {
    let enabled = std::env::var("SLM_APPRENTICESHIP_ENABLED")
        .ok()
        .map(|s| s.eq_ignore_ascii_case("true") || s == "1")
        .unwrap_or(false);
    if !enabled {
        return None;
    }
    Some(ApprenticeshipConfig::from_env())
}

/// Build the AS-3 verdict dispatcher: shells out to `ssh-keygen -Y
/// verify` against `${FOUNDRY_ROOT}/identity/allowed_signers` (or
/// `FOUNDRY_ALLOWED_SIGNERS` override per design-pass Q1) and writes
/// corpus tuples + ledger events under `${FOUNDRY_ROOT}/data/`.
fn build_verdict_dispatcher(
    cfg: &ApprenticeshipConfig,
    cache: Arc<BriefCache>,
) -> anyhow::Result<VerdictDispatcher> {
    let allowed_signers = std::env::var_os("FOUNDRY_ALLOWED_SIGNERS")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| cfg.foundry_root.join("identity").join("allowed_signers"));
    let verifier: Arc<dyn VerdictVerifier> = Arc::new(SshKeygenVerifier::new(allowed_signers));
    let ledger_dir = cfg.foundry_root.join("data").join("apprenticeship");
    let ledger = PromotionLedger::new(ledger_dir).context("create promotion ledger dir")?;
    let doctrine_version =
        std::env::var("FOUNDRY_DOCTRINE_VERSION").unwrap_or_else(|_| "0.0.7".to_string());
    let tenant = std::env::var("FOUNDRY_TENANT").unwrap_or_else(|_| "pointsav".to_string());
    Ok(VerdictDispatcher {
        verifier,
        cache,
        ledger,
        corpus_root: cfg.foundry_root.clone(),
        doctrine_version,
        tenant,
    })
}

fn init_tracing() {
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("slm_doorman=info,slm_doorman_server=info,axum=warn"));
    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer())
        .init();
}
