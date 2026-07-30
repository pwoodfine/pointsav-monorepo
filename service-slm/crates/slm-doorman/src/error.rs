// SPDX-License-Identifier: Apache-2.0 OR MIT

use slm_core::Tier;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, DoormanError>;

#[derive(Debug, Error)]
pub enum DoormanError {
    #[error("requested tier {0:?} is not configured")]
    TierUnavailable(Tier),

    #[error("tier {tier:?} is not yet implemented (filled in {filled_in_by})")]
    NotImplemented {
        tier: Tier,
        filled_in_by: &'static str,
    },

    #[error(
        "tier-C task label {label:?} is not on the allowlist; \
         see DoormanConfig::external_allowlist"
    )]
    ExternalNotAllowlisted { label: String },

    #[error("upstream HTTP error: {0}")]
    Upstream(#[from] reqwest::Error),

    #[error("audit ledger I/O error: {0}")]
    LedgerIo(#[from] std::io::Error),

    #[error("audit ledger serialisation error: {0}")]
    LedgerSerde(#[from] serde_json::Error),

    #[error("home directory not resolvable; HOME env var is unset")]
    HomeUnset,

    #[error("upstream response could not be parsed as OpenAI-compatible: {0}")]
    UpstreamShape(String),

    #[error(
        "Yo-Yo contract MAJOR-version mismatch: remote returned {remote_status} \
         (Doorman speaks contract {doorman_version}); refusing to retry"
    )]
    ContractMajorMismatch {
        remote_status: u16,
        doorman_version: &'static str,
    },

    #[error("Yo-Yo bearer-token provider returned no token: {0}")]
    BearerToken(String),

    #[error("verdict signature verification failed: {0}")]
    VerifySignature(String),

    #[error("apprenticeship ledger lock or write failed: {0}")]
    LedgerLock(String),

    #[error("apprenticeship corpus write failed at {path}: {reason}")]
    CorpusWrite { path: String, reason: String },

    #[error("verdict body could not be parsed: {0}")]
    VerdictParse(String),

    #[error(
        "verdict POST referenced an unknown (brief_id, attempt_id) — \
             brief cache miss; senior must reissue the brief"
    )]
    BriefCacheMiss,

    /// A verdict arrived for a `brief_id` that has no corpus tuple on
    /// disk (i.e., the shadow brief was never dispatched or was never
    /// captured). Per apprenticeship-substrate.md §7B: verdicts only
    /// promote existing tuples; orphan verdicts are logged and no-op'd.
    /// HTTP 410 — the caller should verify the shadow brief was posted
    /// before attempting to sign a verdict.
    #[error(
        "verdict POST for brief_id {brief_id:?} found no corpus tuple at \
         {corpus_path}; shadow brief may not have been dispatched or \
         captured. Per §7B, verdict signing only promotes existing tuples \
         — no orphan corpus row is created"
    )]
    OrphanVerdictNoCorpusTuple {
        /// The `brief_id` from the verdict frontmatter.
        brief_id: String,
        /// The canonical corpus path that was checked.
        corpus_path: String,
    },

    #[error(
        "Tier A (llama-server) does not accept {dialect} grammars; \
         {advice}"
    )]
    TierAGrammarUnsupported {
        /// The grammar dialect that was rejected, e.g. `"Lark"`.
        dialect: &'static str,
        /// Human-readable advice for the caller.
        advice: &'static str,
    },

    #[error(
        "Tier C (external API) does not accept {dialect} grammars; \
         {advice}"
    )]
    TierCGrammarUnsupported {
        /// The grammar dialect that was rejected, e.g. `"Lark"`, `"GBNF"`,
        /// or `"JsonSchema"`.
        dialect: &'static str,
        /// Human-readable advice for the caller.
        advice: &'static str,
    },

    /// The caller supplied a Lark grammar that failed to compile at the
    /// Doorman boundary (PS.3 step 5). The error message from llguidance
    /// includes line/column context so the caller can fix the grammar
    /// without needing to route to a backend at all.
    #[error("Lark grammar failed pre-validation: {reason}")]
    MalformedLarkGrammar {
        /// Parse-error message from llguidance's Lark compiler, including
        /// line/column context and a snippet of the offending input.
        reason: String,
    },

    /// `POST /v1/audit/proxy` caller supplied an unrecognised provider string
    /// (PS.4). Accepted values: "anthropic", "gemini", "openai". 400 BAD_REQUEST.
    #[error(
        "audit_proxy provider {provider:?} is not recognised; \
         accepted values: anthropic, gemini, openai"
    )]
    AuditProxyInvalidProvider {
        /// The provider string the caller submitted.
        provider: String,
    },

    /// `POST /v1/audit/proxy` targeted a provider that has no configured
    /// endpoint or API key at Doorman startup (PS.4 step 2). This is a
    /// server-side configuration gap, not a caller policy violation —
    /// hence 503 SERVICE_UNAVAILABLE rather than 403.
    #[error(
        "audit_proxy provider {provider:?} is not configured; \
         set SLM_TIER_C_{PROVIDER}_ENDPOINT and SLM_TIER_C_{PROVIDER}_API_KEY \
         to enable this provider",
        PROVIDER = provider.to_ascii_uppercase()
    )]
    AuditProxyProviderUnavailable {
        /// The provider string (e.g. "anthropic", "gemini", "openai").
        provider: String,
    },

    /// `POST /v1/audit/proxy` caller supplied a purpose that is not on the
    /// server's `AuditProxyPurposeAllowlist` (PS.4 step 3). This is a
    /// caller-side policy violation — the purpose is not documented as
    /// permissible on this deployment. 403 FORBIDDEN (same classification
    /// as `ExternalNotAllowlisted`, which mirrors this pattern for Tier C
    /// task labels).
    #[error(
        "audit_proxy purpose {purpose:?} is not on the purpose allowlist; \
         see AuditProxyConfig::purpose_allowlist for documented purposes"
    )]
    AuditProxyPurposeNotAllowlisted {
        /// The purpose string the caller submitted.
        purpose: String,
    },

    /// `POST /v1/audit/capture` caller supplied an event_type that is not one
    /// of the five accepted values (PS.4 step 4). Accepted values:
    /// "prose-edit", "design-edit", "graph-mutation", "anchor-event",
    /// "verdict-issued". 400 BAD_REQUEST.
    #[error(
        "audit_capture event_type {event_type:?} is not recognised; \
         accepted values: prose-edit, design-edit, graph-mutation, anchor-event, verdict-issued"
    )]
    AuditCaptureUnknownEventType {
        /// The event_type string the caller submitted.
        event_type: String,
    },

    /// `POST /v1/audit/capture` caller submitted a payload larger than the
    /// maximum permitted size (`AUDIT_CAPTURE_MAX_PAYLOAD_BYTES`). 413
    /// PAYLOAD_TOO_LARGE. Classified as PolicyDenied — the caller must shrink
    /// the payload before retrying.
    #[error(
        "audit_capture payload is {size_bytes} bytes, exceeding the {max_bytes}-byte limit; \
         reduce the payload size before retrying"
    )]
    AuditCapturePayloadTooLarge {
        /// Serialised payload size in bytes.
        size_bytes: usize,
        /// Maximum permitted size in bytes.
        max_bytes: usize,
    },

    /// `POST /v1/audit/capture` caller supplied an `event_at` timestamp that
    /// does not parse as RFC 3339 / ISO 8601. 400 BAD_REQUEST.
    #[error(
        "audit_capture event_at {value:?} is not a valid RFC 3339 timestamp; \
         use format YYYY-MM-DDTHH:MM:SSZ or YYYY-MM-DDTHH:MM:SS+HH:MM"
    )]
    AuditCaptureInvalidTimestamp {
        /// The raw string the caller submitted.
        value: String,
    },

    /// `POST /v1/audit/proxy` caller submitted a raw request body that exceeds
    /// `AUDIT_PROXY_MAX_REQUEST_BYTES` (64 KiB). Checked BEFORE JSON
    /// deserialisation so an oversized request is rejected early without
    /// allocating heap memory for the payload. 413 PAYLOAD_TOO_LARGE.
    /// Classified as PolicyDenied — the caller must reduce the request size.
    #[error(
        "audit_proxy request is {size_bytes} bytes, exceeding the {max_bytes}-byte limit; \
         reduce the request size before retrying"
    )]
    AuditProxyPayloadTooLarge {
        /// Raw body size in bytes.
        size_bytes: usize,
        /// Maximum permitted size in bytes (`AUDIT_PROXY_MAX_REQUEST_BYTES`).
        max_bytes: usize,
    },

    /// A tenant's (`moduleId`) in-flight request count for the audit endpoints
    /// (`/v1/audit/proxy` and `/v1/audit/capture`) has reached
    /// `SLM_AUDIT_TENANT_CONCURRENCY_CAP` (default 4). The per-tenant
    /// `Semaphore` has no permits available. 503 SERVICE_UNAVAILABLE with
    /// `Retry-After: 5`. Classified as PolicyDenied — the caller may retry
    /// once an in-flight request from the same tenant completes.
    #[error(
        "audit tenant {module_id:?} has reached the concurrency cap of {cap} \
         in-flight requests; retry after in-flight requests complete"
    )]
    AuditTenantConcurrencyExhausted {
        /// The `moduleId` that hit the cap.
        module_id: String,
        /// The configured cap value (`SLM_AUDIT_TENANT_CONCURRENCY_CAP`).
        cap: u32,
    },

    // ── Tier B resilience ────────────────────────────────────────────────
    /// Tier B (Yo-Yo) request timed out. The 180-second outer deadline
    /// (tokio::time::timeout) fired before the response completed — including
    /// any 503 Retry-After cold-start sleep. The router treats this as a
    /// transient upstream failure and may fall back to Tier A.
    #[error("Tier B request timed out (180 s outer deadline exceeded)")]
    TierBTimeout,

    /// Tier B (Yo-Yo) circuit breaker is open. Five consecutive failures
    /// occurred and the 5-minute cooldown has not yet elapsed. No HTTP
    /// request was made. The router falls back to Tier A immediately.
    #[error("Tier B circuit breaker is open; cooling down after consecutive failures")]
    TierBCircuitOpen,

    /// The extraction handler's 120 s deadline fired before any tier returned
    /// a result. Releases the per-tenant semaphore permit so a stuck OLMo call
    /// does not permanently consume a concurrency slot when the client disconnects.
    #[error("extraction deadline (120 s) exceeded; semaphore permit released")]
    RequestTimeout,

    // ── Graph proxy (conventions/datagraph-access-discipline.md) ────────
    /// `POST /v1/graph/query` or `POST /v1/graph/mutate` — caller omitted the
    /// mandatory `X-Foundry-Module-ID` header. 400 BAD_REQUEST.
    #[error(
        "graph proxy requires the X-Foundry-Module-ID header; \
         use 'woodfine' or 'pointsav'"
    )]
    GraphProxyMissingModuleId,

    /// `POST /v1/graph/query` or `POST /v1/graph/mutate` — service-content
    /// is not reachable at `SERVICE_CONTENT_ENDPOINT` or the endpoint is
    /// unconfigured. 503 SERVICE_UNAVAILABLE (transient; caller may retry
    /// once service-content is running).
    #[error(
        "graph proxy: service-content is unreachable or not configured; \
         ensure SERVICE_CONTENT_ENDPOINT is set and service-content is running"
    )]
    GraphProxyServiceUnavailable,

    // ── Brief Queue Substrate (apprenticeship-substrate.md §7C) ─────────
    /// A file-system I/O error occurred while operating on the brief queue
    /// (enqueue, dequeue, lease rename, release, or reap). 500
    /// INTERNAL_SERVER_ERROR.
    #[error("brief queue I/O error at {path}: {reason}")]
    QueueIo {
        /// Filesystem path where the operation failed.
        path: String,
        /// Human-readable description of the failure.
        reason: String,
    },

    /// A brief queue directory lock could not be acquired (another process
    /// or the reaper holds the `.lock` sentinel). The caller should retry
    /// after a short delay. 503 SERVICE_UNAVAILABLE.
    #[error("brief queue lock unavailable at {path}: {reason}")]
    QueueLockFailed {
        /// Path to the `.lock` sentinel file.
        path: String,
        /// Human-readable description (e.g. "EWOULDBLOCK").
        reason: String,
    },

    /// A queue entry file could not be parsed as a valid
    /// `ApprenticeshipBrief` JSONL record. The entry has been moved to the
    /// `queue-poison/` bucket. 400 BAD_REQUEST when surfaced via HTTP;
    /// background worker logs and continues.
    #[error("brief queue entry at {path} is malformed and was moved to queue-poison: {reason}")]
    QueueMalformedBrief {
        /// Path of the malformed brief file (original location).
        path: String,
        /// Parse-error description.
        reason: String,
    },

    /// A flow gate (per-label or global kill switch) is closed. The request
    /// targeting the named label is refused; the caller should retry after
    /// the operator re-opens the gate. 503 SERVICE_UNAVAILABLE with a
    /// `Retry-After` header. The express lane returns this; nothing bypasses
    /// the kill switch.
    #[error("flow gate '{label}' is closed (operator kill switch); retry after it re-opens")]
    FlowGateClosed {
        /// The gate label that was closed ("global", "batch", "express",
        /// "tier-c", or a custom node label).
        label: String,
    },

    /// A priority queue directory operation failed (create, lease, rename,
    /// or read). 500 INTERNAL when surfaced via HTTP; background drain
    /// worker logs and continues to the next item.
    #[error("priority queue I/O error at {path}: {reason}")]
    PriorityQueueIo {
        /// Path to the queue file or directory that failed.
        path: String,
        /// Human-readable description of the failure.
        reason: String,
    },

    /// A GCP Compute Engine REST call (start/stop/status/operation poll) or
    /// the metadata-server token fetch failed. Server-side; the VM lifecycle
    /// monitor logs and retries, and the router falls back to Tier A. 502 when
    /// surfaced via HTTP.
    #[error("GCP Compute API error during {operation}: {reason}")]
    GcpApi {
        /// The operation that failed ("start", "stop", "status",
        /// "wait-operation", "token").
        operation: &'static str,
        /// Human-readable description (status code, body excerpt, or transport
        /// error).
        reason: String,
    },

    // ── Corpus quality gate (learning-loop-master-plan §P1-1.1) ─────────
    /// The corpus gate rejected a shadow tuple or DPO pair before writing
    /// to disk. `reason` contains the human-readable gate failure message
    /// including the specific check that failed (duplicate hash, oversized
    /// diff, Do-Not-Use term, template-echo prefix, or length ratio). Callers
    /// MUST NOT write the JSONL row on this error.
    #[error("corpus gate rejected tuple: {reason}")]
    CorpusGateRejected {
        /// Human-readable description of the gate failure, suitable for
        /// logging and the drain-worker audit trail.
        reason: String,
    },
}
