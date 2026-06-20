// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Library surface for `slm-doorman-server`.
//!
//! Exposes `AppState` and `router` so integration tests under `tests/`
//! can build a real axum `Router` with injected tier configs and exercise
//! the full HTTP → Doorman → HTTP response path without starting a live
//! TCP listener.
//!
//! The `main.rs` binary target uses this library via `slm_doorman_server::http`.

/// Drain-worker decision logic (empty-diff skip guard), extracted for unit tests.
pub mod drain;
pub mod http;
pub mod idle_monitor;
/// Brief Queue Substrate (apprenticeship-substrate.md §7C).
///
/// File-backed durable queue that decouples brief acceptance from
/// apprentice execution, providing tolerance for Tier A CPU latency and
/// Yo-Yo idle-shutdown preemption. See `queue.rs` module-level docs for
/// the full design.
pub mod queue;

/// Test helpers — factory functions shared across integration test files.
///
/// Only compiled for `#[cfg(test)]` consumers; the module itself is always
/// present at the crate boundary so `tests/` crates can import it regardless
/// of the feature set.
pub mod test_helpers {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    use slm_doorman::tier::{
        ExternalTierClient, LocalTierClient, LocalTierConfig, StaticBearer, TierCPricing,
        TierCProvider, YoYoTierClient, YoYoTierConfig,
    };
    use slm_doorman::{
        AuditLedger, AuditProxyClient, AuditProxyConfig, AuditProxyPurposeAllowlist, BriefCache,
        Doorman, DoormanConfig, ExpressLane, PromotionLedger, VerdictDispatcher, VerdictVerifier,
        FOUNDRY_DEFAULT_PURPOSE_ALLOWLIST,
    };
    use tokio::sync::Semaphore;

    use crate::http::AppState;
    use crate::queue::QueueConfig;

    /// Default per-tenant concurrency cap used in test helpers.
    ///
    /// Set to 100 so the cap never interferes with tests that are not
    /// specifically testing the concurrency limit. Tests that exercise the
    /// cap explicitly use a low value (e.g. 1 or 2) via
    /// `app_state_with_audit_proxy_capped`.
    const TEST_AUDIT_CONCURRENCY_CAP: u32 = 100;

    /// Build an `Arc<Mutex<HashMap>>` for `audit_tenant_concurrency` with no
    /// pre-populated entries (lazy-init; tenants are added on first request).
    fn empty_concurrency_map() -> Arc<Mutex<HashMap<slm_core::ModuleId, Arc<Semaphore>>>> {
        Arc::new(Mutex::new(HashMap::new()))
    }

    /// Construct a `QueueConfig` pointing at a unique temporary directory.
    /// Each call returns a distinct path so parallel tests do not race on the
    /// queue lock sentinel or queue files.
    pub fn temp_queue_config() -> Arc<QueueConfig> {
        let dir = std::env::temp_dir().join(format!(
            "slm-doorman-queue-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        Arc::new(QueueConfig::with_base_dir(dir))
    }

    /// Construct a temporary `AuditLedger` under `$TMPDIR`.
    /// Each call returns a unique directory so parallel tests do not race.
    pub fn temp_ledger() -> AuditLedger {
        let dir = std::env::temp_dir().join(format!(
            "slm-doorman-http-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        AuditLedger::new(dir).expect("temp audit ledger")
    }

    /// Construct a temporary `PromotionLedger` under `$TMPDIR`.
    pub fn temp_promotion_ledger() -> PromotionLedger {
        let dir = std::env::temp_dir().join(format!(
            "slm-doorman-promo-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        PromotionLedger::new(dir).expect("temp promotion ledger")
    }

    /// Build an `AppState` with no tiers configured and apprenticeship disabled.
    /// Use when you need a Doorman that refuses every chat-completions call with
    /// `TierUnavailable`.
    pub fn app_state_no_tiers() -> Arc<AppState> {
        let doorman = Doorman::new(DoormanConfig::default(), temp_ledger());
        Arc::new(AppState {
            doorman,
            apprenticeship: None,
            brief_cache: Arc::new(BriefCache::default()),
            verdict_dispatcher: None,
            audit_proxy_client: None,
            audit_proxy_purpose_allowlist: FOUNDRY_DEFAULT_PURPOSE_ALLOWLIST,
            audit_tenant_concurrency: empty_concurrency_map(),
            audit_tenant_concurrency_cap: TEST_AUDIT_CONCURRENCY_CAP,
            queue_config: temp_queue_config(),
            service_content_endpoint: String::new(),
            node_class: "hardware",
            tier_a_reason: "available",
            express_lane: Arc::new(ExpressLane::with_default_labels()),
        })
    }

    /// Build an `AppState` backed by a local tier that hits the given
    /// `local_endpoint`. Apprenticeship disabled.
    pub fn app_state_with_local(local_endpoint: impl Into<String>) -> Arc<AppState> {
        let local = LocalTierClient::new(LocalTierConfig {
            endpoint: local_endpoint.into(),
            default_model: "test-model".to_string(),
        });
        let doorman = Doorman::new(
            DoormanConfig {
                local: Some(local),
                yoyo: std::collections::HashMap::new(),
                external: None,
                lark_validator: None,
                graph_context_client: None,
                tier_a_first: false,
                daily_yoyo_cap_usd: None,
                cost_ledger: None,
            },
            temp_ledger(),
        );
        Arc::new(AppState {
            doorman,
            apprenticeship: None,
            brief_cache: Arc::new(BriefCache::default()),
            verdict_dispatcher: None,
            audit_proxy_client: None,
            audit_proxy_purpose_allowlist: FOUNDRY_DEFAULT_PURPOSE_ALLOWLIST,
            audit_tenant_concurrency: empty_concurrency_map(),
            audit_tenant_concurrency_cap: TEST_AUDIT_CONCURRENCY_CAP,
            queue_config: temp_queue_config(),
            service_content_endpoint: String::new(),
            node_class: "hardware",
            tier_a_reason: "available",
            express_lane: Arc::new(ExpressLane::with_default_labels()),
        })
    }

    /// Build an `AppState` backed by a Yo-Yo tier pointing at `yoyo_endpoint`.
    /// The `_gateway_token` parameter is accepted for call-site compatibility but
    /// is not yet wired into AppState (gateway auth not yet implemented).
    pub fn app_state_with_yoyo(
        yoyo_endpoint: impl Into<String>,
        _gateway_token: Option<String>,
    ) -> Arc<AppState> {
        let yoyo = YoYoTierClient::new(
            YoYoTierConfig {
                endpoint: yoyo_endpoint.into(),
                default_model: "Olmo-3-1125-32B-Think".to_string(),
                contract_version: slm_doorman::YOYO_CONTRACT_VERSION.to_string(),
                pricing: Default::default(),
                zone: None,
                health_path: "/health".to_string(),
            },
            std::sync::Arc::new(StaticBearer::new("test-bearer-token")),
        );
        let mut yoyo_map = std::collections::HashMap::new();
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
            temp_ledger(),
        );
        Arc::new(AppState {
            doorman,
            apprenticeship: None,
            brief_cache: Arc::new(BriefCache::default()),
            verdict_dispatcher: None,
            audit_proxy_client: None,
            audit_proxy_purpose_allowlist: FOUNDRY_DEFAULT_PURPOSE_ALLOWLIST,
            audit_tenant_concurrency: empty_concurrency_map(),
            audit_tenant_concurrency_cap: TEST_AUDIT_CONCURRENCY_CAP,
            queue_config: temp_queue_config(),
            service_content_endpoint: String::new(),
            node_class: "hardware",
            tier_a_reason: "available",
            express_lane: Arc::new(ExpressLane::with_default_labels()),
        })
    }

    /// Build an `AppState` with an external tier client configured.
    /// The external client uses `FOUNDRY_DEFAULT_ALLOWLIST` so requests
    /// without a valid label are refused before any network call.
    /// Apprenticeship disabled.
    pub fn app_state_with_external(external: ExternalTierClient) -> Arc<AppState> {
        let doorman = Doorman::new(
            DoormanConfig {
                local: None,
                yoyo: std::collections::HashMap::new(),
                external: Some(external),
                lark_validator: None,
                graph_context_client: None,
                tier_a_first: false,
                daily_yoyo_cap_usd: None,
                cost_ledger: None,
            },
            temp_ledger(),
        );
        Arc::new(AppState {
            doorman,
            apprenticeship: None,
            brief_cache: Arc::new(BriefCache::default()),
            verdict_dispatcher: None,
            audit_proxy_client: None,
            audit_proxy_purpose_allowlist: FOUNDRY_DEFAULT_PURPOSE_ALLOWLIST,
            audit_tenant_concurrency: empty_concurrency_map(),
            audit_tenant_concurrency_cap: TEST_AUDIT_CONCURRENCY_CAP,
            queue_config: temp_queue_config(),
            service_content_endpoint: String::new(),
            node_class: "hardware",
            tier_a_reason: "available",
            express_lane: Arc::new(ExpressLane::with_default_labels()),
        })
    }

    /// Build an `AppState` with apprenticeship enabled and a custom
    /// `VerdictVerifier` injected. The local tier is absent (apprenticeship
    /// tests do not need inference routing). The corpus root is `$TMPDIR`.
    pub fn app_state_with_apprenticeship(verifier: Arc<dyn VerdictVerifier>) -> Arc<AppState> {
        use slm_doorman::ApprenticeshipConfig;
        use std::path::PathBuf;

        let tmp = std::env::temp_dir();
        let foundry_root: PathBuf = tmp.join(format!(
            "slm-doorman-foundry-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&foundry_root).expect("create test foundry root");

        let cfg = ApprenticeshipConfig {
            foundry_root: foundry_root.clone(),
            citations_path: foundry_root.join("citations.yaml"),
            brief_tier_b_threshold_chars: 8000,
            doctrine_version: "0.0.1".to_string(),
            tenant: "test-tenant".to_string(),
            tier_a_first: false,
            yoyo_dispatch_label: None,
        };

        let brief_cache = Arc::new(BriefCache::default());
        let ledger = temp_promotion_ledger();

        let verdict_dispatcher = VerdictDispatcher {
            verifier,
            cache: brief_cache.clone(),
            ledger,
            corpus_root: foundry_root,
            doctrine_version: "0.0.1".to_string(),
            tenant: "test-tenant".to_string(),
        };

        let doorman = Doorman::new(DoormanConfig::default(), temp_ledger());

        Arc::new(AppState {
            doorman,
            apprenticeship: Some(cfg),
            brief_cache,
            verdict_dispatcher: Some(verdict_dispatcher),
            audit_proxy_client: None,
            audit_proxy_purpose_allowlist: FOUNDRY_DEFAULT_PURPOSE_ALLOWLIST,
            audit_tenant_concurrency: empty_concurrency_map(),
            audit_tenant_concurrency_cap: TEST_AUDIT_CONCURRENCY_CAP,
            queue_config: temp_queue_config(),
            service_content_endpoint: String::new(),
            node_class: "hardware",
            tier_a_reason: "available",
            express_lane: Arc::new(ExpressLane::with_default_labels()),
        })
    }

    /// Build an `AppState` with an `AuditProxyClient` pointing at the given
    /// mock server URI. The Doorman has no compute tiers (audit_proxy tests
    /// do not need inference routing).
    ///
    /// Returns `(state, ledger_dir)` where `ledger_dir` is the base directory
    /// of the test audit ledger so callers can inspect written JSONL files.
    /// `AuditLedger` is not `Clone`, so we return the path instead of the
    /// ledger object itself.
    ///
    /// The concurrency cap is set to `TEST_AUDIT_CONCURRENCY_CAP` (100) so
    /// it does not interfere with tests that are not testing the cap. Use
    /// `app_state_with_audit_proxy_capped` to inject a low cap for concurrency
    /// limit tests.
    pub fn app_state_with_audit_proxy(
        provider: TierCProvider,
        server_uri: impl Into<String>,
        pricing: TierCPricing,
    ) -> (Arc<AppState>, std::path::PathBuf) {
        app_state_with_audit_proxy_capped(provider, server_uri, pricing, TEST_AUDIT_CONCURRENCY_CAP)
    }

    /// Build an `AppState` with an `AuditProxyClient` and a custom per-tenant
    /// concurrency cap. Used by tests that exercise
    /// `AuditTenantConcurrencyExhausted`.
    ///
    /// Returns `(state, ledger_dir)`.
    pub fn app_state_with_audit_proxy_capped(
        provider: TierCProvider,
        server_uri: impl Into<String>,
        pricing: TierCPricing,
        concurrency_cap: u32,
    ) -> (Arc<AppState>, std::path::PathBuf) {
        let ledger_dir = std::env::temp_dir().join(format!(
            "slm-audit-proxy-helper-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&ledger_dir).expect("create test audit ledger dir");
        let ledger = AuditLedger::new(&ledger_dir).expect("create test audit ledger");

        let mut endpoints = HashMap::new();
        endpoints.insert(provider, server_uri.into());
        let mut keys = HashMap::new();
        keys.insert(provider, "sk-test-DO-NOT-USE-LIVE".to_string());
        let audit_config = AuditProxyConfig {
            provider_endpoints: endpoints,
            provider_api_keys: keys,
            pricing,
            purpose_allowlist: FOUNDRY_DEFAULT_PURPOSE_ALLOWLIST,
        };
        let audit_client = AuditProxyClient::new(audit_config);

        let doorman = Doorman::new(DoormanConfig::default(), ledger);
        let state = Arc::new(AppState {
            doorman,
            apprenticeship: None,
            brief_cache: Arc::new(BriefCache::default()),
            verdict_dispatcher: None,
            audit_proxy_client: Some(audit_client),
            audit_proxy_purpose_allowlist: FOUNDRY_DEFAULT_PURPOSE_ALLOWLIST,
            audit_tenant_concurrency: empty_concurrency_map(),
            audit_tenant_concurrency_cap: concurrency_cap,
            queue_config: temp_queue_config(),
            service_content_endpoint: String::new(),
            node_class: "hardware",
            tier_a_reason: "available",
            express_lane: Arc::new(ExpressLane::with_default_labels()),
        });
        (state, ledger_dir)
    }

    /// Build an `AppState` with an `AuditProxyClient` and a custom
    /// `AuditProxyPurposeAllowlist`. Used by PS.4 step 3 tests that need
    /// to inject a specific allowlist (e.g., with a documented purpose or
    /// with an empty allowlist for fail-closed testing).
    pub fn app_state_with_audit_proxy_and_allowlist(
        provider: TierCProvider,
        server_uri: impl Into<String>,
        pricing: TierCPricing,
        purpose_allowlist: AuditProxyPurposeAllowlist,
    ) -> (Arc<AppState>, std::path::PathBuf) {
        let ledger_dir = std::env::temp_dir().join(format!(
            "slm-audit-proxy-allowlist-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&ledger_dir).expect("create test audit ledger dir");
        let ledger = AuditLedger::new(&ledger_dir).expect("create test audit ledger");

        let mut endpoints = HashMap::new();
        endpoints.insert(provider, server_uri.into());
        let mut keys = HashMap::new();
        keys.insert(provider, "sk-test-DO-NOT-USE-LIVE".to_string());
        let audit_config = AuditProxyConfig {
            provider_endpoints: endpoints,
            provider_api_keys: keys,
            pricing,
            purpose_allowlist: FOUNDRY_DEFAULT_PURPOSE_ALLOWLIST,
        };
        let audit_client = AuditProxyClient::new(audit_config);

        let doorman = Doorman::new(DoormanConfig::default(), ledger);
        let state = Arc::new(AppState {
            doorman,
            apprenticeship: None,
            brief_cache: Arc::new(BriefCache::default()),
            verdict_dispatcher: None,
            audit_proxy_client: Some(audit_client),
            audit_proxy_purpose_allowlist: purpose_allowlist,
            audit_tenant_concurrency: empty_concurrency_map(),
            audit_tenant_concurrency_cap: TEST_AUDIT_CONCURRENCY_CAP,
            queue_config: temp_queue_config(),
            service_content_endpoint: String::new(),
            node_class: "hardware",
            tier_a_reason: "available",
            express_lane: Arc::new(ExpressLane::with_default_labels()),
        });
        (state, ledger_dir)
    }

    /// Build an `AppState` with a service-content endpoint configured.
    /// Used by graph proxy tests that need a Doorman pointing at a mock
    /// service-content server.
    pub fn app_state_with_service_content(
        service_content_endpoint: impl Into<String>,
    ) -> Arc<AppState> {
        let doorman = Doorman::new(DoormanConfig::default(), temp_ledger());
        Arc::new(AppState {
            doorman,
            apprenticeship: None,
            brief_cache: Arc::new(BriefCache::default()),
            verdict_dispatcher: None,
            audit_proxy_client: None,
            audit_proxy_purpose_allowlist: FOUNDRY_DEFAULT_PURPOSE_ALLOWLIST,
            audit_tenant_concurrency: empty_concurrency_map(),
            audit_tenant_concurrency_cap: TEST_AUDIT_CONCURRENCY_CAP,
            queue_config: temp_queue_config(),
            service_content_endpoint: service_content_endpoint.into(),
            node_class: "hardware",
            tier_a_reason: "available",
            express_lane: Arc::new(ExpressLane::with_default_labels()),
        })
    }
}
