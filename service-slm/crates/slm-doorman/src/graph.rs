// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Graph context client — queries `service-content`'s Ring 2 knowledge graph
//! to inject per-tenant entity context ahead of Doorman inference calls.
//!
//! The client is intentionally fault-tolerant: a missing or unavailable
//! `service-content` instance is logged as a warning but never causes an
//! inference call to fail. The Doorman proceeds without graph context rather
//! than surfacing an error to the caller.
//!
//! # Circuit breaker
//!
//! After `GRAPH_CIRCUIT_THRESHOLD` consecutive failures the client opens a
//! circuit for `GRAPH_CIRCUIT_OPEN_SECS` seconds. During this window every
//! call returns `None` immediately without making an HTTP request — preventing
//! a degraded `service-content` from adding a 5-second timeout stall to every
//! OLMo dispatch. After the window expires the client probes once; on success
//! the circuit resets.
//!
//! Architectural background: `service-slm/ARCHITECTURE.md` Ring 2 (Knowledge &
//! Processing). Wire protocol: `GET /v1/graph/context?q=<query>&module_id=<mid>&limit=<n>`.

use reqwest::Client;
use serde::Deserialize;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::{debug, warn};

/// Consecutive failures before the circuit opens.
const GRAPH_CIRCUIT_THRESHOLD: u32 = 3;
/// Seconds the circuit stays open before the next probe attempt.
const GRAPH_CIRCUIT_OPEN_SECS: u64 = 120;

/// One entity row returned by the `service-content` graph API.
#[derive(Debug, Deserialize)]
struct GraphEntityRow {
    entity_name: String,
    classification: String,
    role_vector: Option<String>,
    location_vector: Option<String>,
    contact_vector: Option<String>,
}

/// HTTP client for `service-content`'s `GET /v1/graph/context` endpoint.
///
/// Construct once at Doorman startup and share as `Option<GraphContextClient>`
/// in `DoormanConfig`. Thread-safe: `reqwest::Client` is `Clone + Send + Sync`;
/// the circuit-breaker fields use atomics (`AtomicU32`, `AtomicU64`) so no
/// mutex is needed.
pub struct GraphContextClient {
    /// Base URL of the `service-content` HTTP server, e.g.
    /// `http://127.0.0.1:9081`. No trailing slash.
    endpoint: String,
    http: Client,
    /// Number of consecutive failures since the last successful call.
    consecutive_failures: AtomicU32,
    /// Unix epoch seconds at which the circuit reopens for a probe.
    /// Zero means the circuit is closed (healthy).
    circuit_open_until_secs: AtomicU64,
}

impl GraphContextClient {
    /// Construct a client targeting the given `endpoint`.
    ///
    /// The underlying `reqwest::Client` is built with a 5-second timeout so
    /// a slow or unresponsive `service-content` instance never stalls an
    /// inference call for more than 5 seconds. The circuit breaker starts
    /// closed (healthy).
    pub fn new(endpoint: String) -> Self {
        Self {
            endpoint,
            http: Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .expect("failed to build graph HTTP client"),
            consecutive_failures: AtomicU32::new(0),
            circuit_open_until_secs: AtomicU64::new(0),
        }
    }

    /// Query `service-content` for entities matching `query` under `module_id`.
    ///
    /// Returns a formatted multi-line context string when entities are found,
    /// or `None` when:
    /// - the circuit is open (service-content previously failed repeatedly)
    /// - `service-content` is unavailable (non-fatal — logged at WARN, returns `None`)
    /// - the endpoint returns a non-2xx status
    /// - the response body cannot be parsed as a JSON array
    /// - no entities match the query
    ///
    /// `limit` is capped at 10 to bound context injection size.
    pub async fn fetch_context(
        &self,
        module_id: &str,
        query: &str,
        limit: usize,
    ) -> Option<String> {
        let now_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        // Circuit breaker check — skip HTTP when service-content is known-down.
        let open_until = self.circuit_open_until_secs.load(Ordering::Relaxed);
        if open_until > 0 {
            if now_secs < open_until {
                debug!(
                    target: "slm_doorman::graph",
                    secs_remaining = open_until - now_secs,
                    "graph context circuit open — skipping service-content request"
                );
                return None;
            }
            // Timeout expired — allow one probe attempt; reset the open flag.
            self.circuit_open_until_secs.store(0, Ordering::Relaxed);
        }

        let limit = limit.min(10);
        let url = format!("{}/v1/graph/context", self.endpoint);

        let resp = self
            .http
            .get(&url)
            .query(&[
                ("q", query),
                ("module_id", module_id),
                ("limit", &limit.to_string()),
            ])
            .send()
            .await;

        match resp {
            Err(e) => {
                warn!(
                    target: "slm_doorman::graph",
                    error = %e,
                    "service-content graph unavailable"
                );
                self.record_failure(now_secs);
                None
            }
            Ok(r) if !r.status().is_success() => {
                warn!(
                    target: "slm_doorman::graph",
                    status = %r.status(),
                    "service-content graph returned non-2xx status"
                );
                self.record_failure(now_secs);
                None
            }
            Ok(r) => {
                let entities: Vec<GraphEntityRow> = match r.json().await {
                    Ok(v) => v,
                    Err(e) => {
                        warn!(
                            target: "slm_doorman::graph",
                            error = %e,
                            "graph response parse error"
                        );
                        self.record_failure(now_secs);
                        return None;
                    }
                };
                // Success — reset the circuit breaker.
                self.consecutive_failures.store(0, Ordering::Relaxed);
                self.circuit_open_until_secs.store(0, Ordering::Relaxed);

                if entities.is_empty() {
                    return None;
                }
                let ctx = entities
                    .iter()
                    .map(|e| {
                        let mut parts = vec![format!("{} ({})", e.entity_name, e.classification)];
                        if let Some(r) = &e.role_vector {
                            parts.push(format!("role: {}", r));
                        }
                        if let Some(l) = &e.location_vector {
                            parts.push(format!("location: {}", l));
                        }
                        if let Some(c) = &e.contact_vector {
                            parts.push(format!("contact: {}", c));
                        }
                        parts.join("; ")
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                Some(ctx)
            }
        }
    }

    /// Record one failure. Opens the circuit after `GRAPH_CIRCUIT_THRESHOLD`
    /// consecutive failures.
    fn record_failure(&self, now_secs: u64) {
        let failures = self.consecutive_failures.fetch_add(1, Ordering::Relaxed) + 1;
        if failures >= GRAPH_CIRCUIT_THRESHOLD {
            let open_until = now_secs + GRAPH_CIRCUIT_OPEN_SECS;
            self.circuit_open_until_secs
                .store(open_until, Ordering::Relaxed);
            warn!(
                target: "slm_doorman::graph",
                consecutive_failures = failures,
                open_secs = GRAPH_CIRCUIT_OPEN_SECS,
                "graph context circuit opened — skipping service-content for {GRAPH_CIRCUIT_OPEN_SECS}s"
            );
        }
    }

    /// Test-only: construct a client with pre-set circuit state.
    #[cfg(test)]
    fn new_with_state(endpoint: String, failures: u32, open_until_secs: u64) -> Self {
        let client = Self::new(endpoint);
        client
            .consecutive_failures
            .store(failures, Ordering::Relaxed);
        client
            .circuit_open_until_secs
            .store(open_until_secs, Ordering::Relaxed);
        client
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// When service-content returns one entity with all vector fields populated,
    /// `fetch_context` must return a `Some(String)` that contains all expected
    /// fields formatted correctly.
    #[tokio::test]
    async fn fetch_context_returns_formatted_string_when_entities_found() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/graph/context"))
            .and(query_param("q", "john"))
            .and(query_param("module_id", "woodfine"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {
                    "entity_name": "John Smith",
                    "classification": "Person",
                    "role_vector": "property manager",
                    "location_vector": null,
                    "contact_vector": "john@example.com",
                    "module_id": "woodfine",
                    "confidence": 0.95
                }
            ])))
            .mount(&server)
            .await;

        let client = GraphContextClient::new(server.uri());
        let result = client.fetch_context("woodfine", "john", 5).await;
        assert!(result.is_some(), "expected Some context string, got None");
        let ctx = result.unwrap();
        assert!(
            ctx.contains("John Smith"),
            "context must contain entity name; got: {ctx}"
        );
        assert!(
            ctx.contains("Person"),
            "context must contain classification; got: {ctx}"
        );
        assert!(
            ctx.contains("property manager"),
            "context must contain role_vector; got: {ctx}"
        );
        assert!(
            ctx.contains("john@example.com"),
            "context must contain contact_vector; got: {ctx}"
        );
    }

    /// When service-content returns an empty array, `fetch_context` must return
    /// `None` (no entities matched — no context to inject).
    #[tokio::test]
    async fn fetch_context_returns_none_when_no_entities() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/graph/context"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
            .mount(&server)
            .await;

        let client = GraphContextClient::new(server.uri());
        let result = client.fetch_context("woodfine", "unknown", 5).await;
        assert!(
            result.is_none(),
            "empty entity list must produce None, got Some"
        );
    }

    /// When the `service-content` instance is unreachable, `fetch_context` must
    /// return `None` without panicking. The Doorman must be able to proceed
    /// without graph context.
    #[tokio::test]
    async fn fetch_context_returns_none_when_service_unavailable() {
        // Use a port nothing is listening on — connection will be refused.
        let client = GraphContextClient::new("http://127.0.0.1:19999".to_string());
        let result = client.fetch_context("woodfine", "test", 5).await;
        assert!(
            result.is_none(),
            "unavailable service must produce None (non-fatal); got Some"
        );
    }

    // ── Circuit breaker ───────────────────────────────────────────────────────

    /// After GRAPH_CIRCUIT_THRESHOLD consecutive non-2xx responses the circuit
    /// opens. The next call returns None immediately without hitting the mock
    /// (verified via wiremock request count).
    #[tokio::test]
    async fn circuit_opens_after_threshold_failures() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/graph/context"))
            .respond_with(ResponseTemplate::new(500))
            .expect(GRAPH_CIRCUIT_THRESHOLD as u64) // exactly threshold calls reach the server
            .mount(&server)
            .await;

        let client = GraphContextClient::new(server.uri());

        // Drive failures up to the threshold — circuit opens on the last one.
        for _ in 0..GRAPH_CIRCUIT_THRESHOLD {
            let r = client.fetch_context("m", "q", 5).await;
            assert!(r.is_none());
        }

        // Circuit is now open — this call must NOT reach the mock.
        let r = client.fetch_context("m", "q", 5).await;
        assert!(r.is_none(), "open circuit must return None immediately");

        // wiremock verifies the expect(N) on drop — if the mock received >N
        // requests the test would panic there.
        server.verify().await;
    }

    /// After two failures (below threshold) a success resets the counter.
    /// Subsequent calls go through to the mock — the circuit never opened.
    #[tokio::test]
    async fn circuit_resets_after_success() {
        let server = MockServer::start().await;

        // First two calls: 500 (failures, but below threshold of 3)
        Mock::given(method("GET"))
            .and(path("/v1/graph/context"))
            .respond_with(ResponseTemplate::new(500))
            .up_to_n_times(2)
            .mount(&server)
            .await;

        // Third call: 200 with one entity — success resets the counter.
        Mock::given(method("GET"))
            .and(path("/v1/graph/context"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {"entity_name": "Alice", "classification": "Person",
                 "role_vector": null, "location_vector": null, "contact_vector": null}
            ])))
            .mount(&server)
            .await;

        let client = GraphContextClient::new(server.uri());

        // Two failures — counter at 2, circuit still closed.
        for _ in 0..2 {
            assert!(client.fetch_context("m", "q", 5).await.is_none());
        }
        assert_eq!(
            client.consecutive_failures.load(Ordering::Relaxed),
            2,
            "counter must be 2 after two failures"
        );

        // Success — resets counter to 0.
        let r = client.fetch_context("m", "q", 5).await;
        assert!(r.is_some(), "third call (200) must return Some");
        assert_eq!(
            client.consecutive_failures.load(Ordering::Relaxed),
            0,
            "counter must reset to 0 after success"
        );
        assert_eq!(
            client.circuit_open_until_secs.load(Ordering::Relaxed),
            0,
            "circuit_open_until must be 0 after success"
        );
    }

    /// When the circuit's open window has expired (open_until is in the past),
    /// the next call probes the server. On success the circuit resets.
    #[tokio::test]
    async fn circuit_recovers_after_timeout() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/graph/context"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {"entity_name": "Bob", "classification": "Person",
                 "role_vector": null, "location_vector": null, "contact_vector": null}
            ])))
            .expect(1) // exactly one call must reach the server
            .mount(&server)
            .await;

        // Construct a client with the circuit "open" but timestamp 1 (epoch+1s = ancient).
        let client = GraphContextClient::new_with_state(server.uri(), GRAPH_CIRCUIT_THRESHOLD, 1);

        // The circuit window has expired — must probe and succeed.
        let r = client.fetch_context("m", "q", 5).await;
        assert!(
            r.is_some(),
            "expired circuit must allow probe and return Some on success"
        );
        assert_eq!(
            client.consecutive_failures.load(Ordering::Relaxed),
            0,
            "success after probe must reset the failure counter"
        );
        assert_eq!(
            client.circuit_open_until_secs.load(Ordering::Relaxed),
            0,
            "success after probe must reset circuit_open_until to 0"
        );

        server.verify().await;
    }
}
