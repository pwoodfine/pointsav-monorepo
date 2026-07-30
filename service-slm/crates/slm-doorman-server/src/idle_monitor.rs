// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Yo-Yo idle monitor (B5) — replaces yoyo-manual/yoyo-idle-check.sh.
//!
//! Polls the Yo-Yo VM `/metrics` endpoint every 5 minutes for an active-request
//! counter. When the VM has been idle (zero active requests) longer than
//! `SLM_YOYO_IDLE_MINUTES` (default 30), sends a GCP `instances.stop` request
//! via the Compute Engine API using the workspace Service Account ADC token from
//! the GCE metadata server.
//!
//! The monitor is spawned as a background tokio task in `main.rs` only when all
//! four GCP env vars are set (`SLM_YOYO_GCP_PROJECT`, `SLM_YOYO_GCP_ZONE`,
//! `SLM_YOYO_GCP_INSTANCE`, and `SLM_YOYO_ENDPOINT`). Absent any of these,
//! `IdleMonitorConfig::from_env()` returns `None` and no task is spawned.
//!
//! Env vars:
//!   SLM_YOYO_ENDPOINT        Yo-Yo base URL (also consumed by Tier B client)
//!   SLM_YOYO_IDLE_MINUTES    idle threshold in minutes; default 30
//!   SLM_YOYO_METRICS_KEY     Prometheus metric name for active-request count;
//!                             default: llama_active_slots_total (llama-server);
//!                             set to vllm:num_requests_running for vLLM
//!   SLM_YOYO_GCP_PROJECT     GCP project id (e.g. pointsav-public)
//!   SLM_YOYO_GCP_ZONE        GCP zone (e.g. us-west1-b)
//!   SLM_YOYO_GCP_INSTANCE    GCP instance name (e.g. yoyo-tier-b-1)

use std::time::{Duration, Instant};

use tracing::{info, warn};

const POLL_INTERVAL: Duration = Duration::from_secs(300); // 5 minutes
const HTTP_TIMEOUT: Duration = Duration::from_secs(10);
const GCP_METADATA_TOKEN_URL: &str =
    "http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token";

#[derive(Clone, Debug)]
pub struct IdleMonitorConfig {
    pub yoyo_endpoint: String,
    pub yoyo_bearer: Option<String>,
    pub idle_threshold: Duration,
    pub metrics_key: String,
    pub gcp_project: String,
    pub gcp_zone: String,
    pub gcp_instance: String,
}

impl IdleMonitorConfig {
    /// Constructs config from env vars. Returns `None` if any required var is absent.
    pub fn from_env() -> Option<Self> {
        let yoyo_endpoint = std::env::var("SLM_YOYO_ENDPOINT").ok()?;
        if yoyo_endpoint.is_empty() {
            return None;
        }
        let yoyo_bearer = std::env::var("SLM_YOYO_BEARER")
            .ok()
            .filter(|s| !s.is_empty());
        let idle_minutes: u64 = std::env::var("SLM_YOYO_IDLE_MINUTES")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(30);
        let metrics_key = std::env::var("SLM_YOYO_METRICS_KEY")
            .unwrap_or_else(|_| "llama_active_slots_total".to_string());
        let gcp_project = std::env::var("SLM_YOYO_GCP_PROJECT").ok()?;
        let gcp_zone = std::env::var("SLM_YOYO_GCP_ZONE").ok()?;
        let gcp_instance = std::env::var("SLM_YOYO_GCP_INSTANCE").ok()?;

        Some(Self {
            yoyo_endpoint,
            yoyo_bearer,
            idle_threshold: Duration::from_secs(idle_minutes * 60),
            metrics_key,
            gcp_project,
            gcp_zone,
            gcp_instance,
        })
    }
}

/// Run the idle monitor loop. Call via `tokio::spawn(run_idle_monitor(config))`.
pub async fn run_idle_monitor(config: IdleMonitorConfig) {
    let client = reqwest::Client::builder()
        .timeout(HTTP_TIMEOUT)
        .build()
        .unwrap_or_default();

    // Start the idle clock at task-spawn time so a cold VM doesn't get stopped
    // before its first request within the threshold window.
    let mut last_active = Instant::now();
    // Guard against sending repeated stop requests for the same idle period.
    let mut stop_sent = false;

    info!(
        target: "slm_doorman::idle_monitor",
        endpoint = %config.yoyo_endpoint,
        idle_threshold_secs = config.idle_threshold.as_secs(),
        gcp_instance = %config.gcp_instance,
        "Yo-Yo idle monitor started"
    );

    loop {
        tokio::time::sleep(POLL_INTERVAL).await;

        match poll_active_slots(
            &client,
            &config.yoyo_endpoint,
            config.yoyo_bearer.as_deref(),
            &config.metrics_key,
        )
        .await
        {
            Some(0) => {
                // Metrics reachable, zero active slots — VM is up but idle.
                // Let the idle clock run; fire stop if threshold exceeded.
                let idle_secs = last_active.elapsed().as_secs();
                if !stop_sent && last_active.elapsed() >= config.idle_threshold {
                    warn!(
                        target: "slm_doorman::idle_monitor",
                        idle_secs,
                        project = %config.gcp_project,
                        zone = %config.gcp_zone,
                        instance = %config.gcp_instance,
                        "Yo-Yo idle threshold reached; sending GCP stop request"
                    );
                    match stop_gcp_instance(&client, &config).await {
                        Ok(()) => {
                            info!(
                                target: "slm_doorman::idle_monitor",
                                instance = %config.gcp_instance,
                                "GCP stop request accepted"
                            );
                            stop_sent = true;
                        }
                        Err(reason) => {
                            warn!(
                                target: "slm_doorman::idle_monitor",
                                %reason,
                                "GCP stop request failed; will retry next cycle"
                            );
                        }
                    }
                }
            }
            Some(n) => {
                // VM is serving (n > 0) — reset idle clock.
                last_active = Instant::now();
                stop_sent = false;
                info!(
                    target: "slm_doorman::idle_monitor",
                    active_slots = n,
                    "Yo-Yo busy; idle clock reset"
                );
            }
            None => {
                // Metrics unreachable — VM is booting, stopped, or vLLM not yet
                // loaded. Reset the idle clock so we don't stop a VM that was never
                // serving. The nightly GCP instance schedule is the hard-stop
                // fallback when the VM is unreachable.
                last_active = Instant::now();
                stop_sent = false;
            }
        }
    }
}

/// Poll the Yo-Yo `/metrics` endpoint and extract the active-request counter
/// named by `metrics_key`. Returns `None` on network error, non-200 response,
/// or missing metric.
async fn poll_active_slots(
    client: &reqwest::Client,
    endpoint: &str,
    bearer: Option<&str>,
    metrics_key: &str,
) -> Option<u64> {
    let url = format!("{}/metrics", endpoint.trim_end_matches('/'));
    let mut req = client.get(&url);
    if let Some(token) = bearer {
        req = req.bearer_auth(token);
    }
    let resp = req.send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let text = resp.text().await.ok()?;
    for line in text.lines() {
        if line.starts_with(metrics_key) && !line.starts_with('#') {
            if let Some(val) = line.split_once(' ').map(|x| x.1) {
                return val.trim().parse::<f64>().ok().map(|f| f as u64);
            }
        }
    }
    None
}

/// Fetch an ADC bearer token from the GCE metadata server.
async fn fetch_gcp_adc_token(client: &reqwest::Client) -> Result<String, String> {
    #[derive(serde::Deserialize)]
    struct TokenResp {
        access_token: String,
    }
    let resp = client
        .get(GCP_METADATA_TOKEN_URL)
        .header("Metadata-Flavor", "Google")
        .send()
        .await
        .map_err(|e| format!("metadata server unreachable: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("metadata server HTTP {}", resp.status()));
    }
    let t: TokenResp = resp
        .json()
        .await
        .map_err(|e| format!("token JSON parse failed: {e}"))?;
    Ok(t.access_token)
}

/// POST `instances.stop` to the GCP Compute Engine API.
async fn stop_gcp_instance(
    client: &reqwest::Client,
    config: &IdleMonitorConfig,
) -> Result<(), String> {
    let token = fetch_gcp_adc_token(client).await?;
    let url = format!(
        "https://compute.googleapis.com/compute/v1/projects/{}/zones/{}/instances/{}/stop",
        config.gcp_project, config.gcp_zone, config.gcp_instance
    );
    // GCP Compute Engine API requires Content-Length: 0 on empty-body POSTs.
    // reqwest may use chunked transfer encoding for .body(""), which the API
    // rejects with HTTP 411. Setting the header explicitly is the safe path.
    let resp = client
        .post(&url)
        .bearer_auth(&token)
        .header(reqwest::header::CONTENT_LENGTH, "0")
        .body(reqwest::Body::from(""))
        .send()
        .await
        .map_err(|e| format!("GCP API request failed: {e}"))?;
    if resp.status().is_success() {
        Ok(())
    } else {
        Err(format!(
            "GCP instances.stop returned HTTP {}",
            resp.status()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    // Serialize tests that mutate process-global env vars.
    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    fn env_lock() -> &'static Mutex<()> {
        ENV_LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn from_env_returns_none_without_gcp_vars() {
        let _g = env_lock().lock().unwrap();
        // All GCP env vars unset in test environment — should return None.
        // (SLM_YOYO_ENDPOINT may or may not be set; we rely on GCP vars being absent.)
        std::env::remove_var("SLM_YOYO_GCP_PROJECT");
        std::env::remove_var("SLM_YOYO_GCP_ZONE");
        std::env::remove_var("SLM_YOYO_GCP_INSTANCE");
        // Even if endpoint is set, missing GCP vars → None
        std::env::set_var("SLM_YOYO_ENDPOINT", "http://127.0.0.1:8080");
        let result = IdleMonitorConfig::from_env();
        assert!(result.is_none());
        std::env::remove_var("SLM_YOYO_ENDPOINT");
    }

    #[test]
    fn from_env_builds_config_with_all_vars() {
        let _g = env_lock().lock().unwrap();
        std::env::set_var("SLM_YOYO_ENDPOINT", "http://1.2.3.4:8080");
        std::env::set_var("SLM_YOYO_IDLE_MINUTES", "45");
        std::env::set_var("SLM_YOYO_GCP_PROJECT", "my-project");
        std::env::set_var("SLM_YOYO_GCP_ZONE", "us-west1-a");
        std::env::set_var("SLM_YOYO_GCP_INSTANCE", "yoyo-tier-b-1");
        std::env::remove_var("SLM_YOYO_METRICS_KEY");
        let cfg = IdleMonitorConfig::from_env().expect("should build config");
        assert_eq!(cfg.idle_threshold, Duration::from_secs(45 * 60));
        assert_eq!(cfg.gcp_project, "my-project");
        assert_eq!(cfg.metrics_key, "llama_active_slots_total");
        std::env::remove_var("SLM_YOYO_ENDPOINT");
        std::env::remove_var("SLM_YOYO_IDLE_MINUTES");
        std::env::remove_var("SLM_YOYO_GCP_PROJECT");
        std::env::remove_var("SLM_YOYO_GCP_ZONE");
        std::env::remove_var("SLM_YOYO_GCP_INSTANCE");
    }

    #[test]
    fn from_env_builds_config_with_custom_metrics_key() {
        let _g = env_lock().lock().unwrap();
        std::env::set_var("SLM_YOYO_ENDPOINT", "http://1.2.3.4:9443");
        std::env::set_var("SLM_YOYO_GCP_PROJECT", "pointsav-public");
        std::env::set_var("SLM_YOYO_GCP_ZONE", "us-west1-b");
        std::env::set_var("SLM_YOYO_GCP_INSTANCE", "yoyo-tier-b-1");
        std::env::set_var("SLM_YOYO_METRICS_KEY", "vllm:num_requests_running");
        let cfg = IdleMonitorConfig::from_env().expect("should build config");
        assert_eq!(cfg.metrics_key, "vllm:num_requests_running");
        std::env::remove_var("SLM_YOYO_ENDPOINT");
        std::env::remove_var("SLM_YOYO_GCP_PROJECT");
        std::env::remove_var("SLM_YOYO_GCP_ZONE");
        std::env::remove_var("SLM_YOYO_GCP_INSTANCE");
        std::env::remove_var("SLM_YOYO_METRICS_KEY");
    }

    #[test]
    fn poll_active_slots_parses_prometheus_line() {
        // Simulate the parse logic directly — no network call needed.
        let metrics_text = "# HELP llama_active_slots_total Active slots\n\
                            # TYPE llama_active_slots_total gauge\n\
                            llama_active_slots_total 3\n";
        let mut result: Option<u64> = None;
        for line in metrics_text.lines() {
            if line.starts_with("llama_active_slots_total") && !line.starts_with('#') {
                if let Some(val) = line.split_once(' ').map(|x| x.1) {
                    result = val.trim().parse::<f64>().ok().map(|f| f as u64);
                }
            }
        }
        assert_eq!(result, Some(3));
    }
}
