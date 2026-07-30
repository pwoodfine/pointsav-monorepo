use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DoormanHealth {
    #[serde(default)]
    pub ai_available: bool,
    #[serde(default)]
    pub tier_b_circuit_state: String,
    #[serde(default)]
    pub entity_count: Option<u64>,
    #[serde(default)]
    pub active_tier: Option<String>,
    // P2-B: structured status fields
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub tier_a: bool,
    #[serde(default)]
    pub tier_a_reason: Option<String>,
    #[serde(default)]
    pub node_class: Option<String>,
    // Sprint 5B: brief queue snapshot (from /readyz)
    #[serde(default)]
    pub queue_pending: Option<u64>,
    #[serde(default)]
    pub queue_done: Option<u64>,
    #[serde(default)]
    pub queue_poison: Option<u64>,
}

/// Queue depth across all apprenticeship directories (from /v1/status/queue).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QueueStatus {
    #[serde(default)]
    pub pending: u64,
    #[serde(default)]
    pub in_flight: u64,
    #[serde(default)]
    pub paused: u64,
    #[serde(default)]
    pub done: u64,
    #[serde(default)]
    pub poison: u64,
    #[serde(default)]
    pub quarantine: u64,
}

/// Daily cost rollup (from /v1/status/cost).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CostStatus {
    #[serde(default)]
    pub ledger_available: bool,
    #[serde(default)]
    pub daily_usd: f64,
    #[serde(default)]
    pub local_usd: f64,
    #[serde(default)]
    pub yoyo_usd: f64,
    #[serde(default)]
    pub ext_usd: f64,
    #[serde(default)]
    pub vm_hours_usd: f64,
    #[serde(default)]
    pub request_count: usize,
}

/// llama-server throughput (from /v1/status/tier-a).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TierAStatus {
    #[serde(default)]
    pub reachable: bool,
    #[serde(default)]
    pub tok_per_s: Option<f64>,
    #[serde(default)]
    pub requests_processing: Option<u64>,
    #[serde(default)]
    pub prompt_tokens_total: Option<u64>,
}

/// Yo-Yo fleet state (from /v1/status/yoyo).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct YoyoStatus {
    #[serde(default)]
    pub has_yoyo: bool,
    #[serde(default)]
    pub nodes: HashMap<String, serde_json::Value>,
}

fn blocking_client() -> anyhow::Result<reqwest::blocking::Client> {
    Ok(reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()?)
}

pub fn fetch_readyz(endpoint: &str) -> anyhow::Result<DoormanHealth> {
    let client = blocking_client()?;
    let url = format!("{}/readyz", endpoint);
    let resp = client.get(&url).send()?;
    let status = resp.status();
    if status.is_success() || status == reqwest::StatusCode::SERVICE_UNAVAILABLE {
        Ok(resp.json::<DoormanHealth>()?)
    } else {
        anyhow::bail!("HTTP {}", status)
    }
}

pub fn fetch_queue(endpoint: &str) -> anyhow::Result<QueueStatus> {
    let client = blocking_client()?;
    let url = format!("{}/v1/status/queue", endpoint);
    Ok(client.get(&url).send()?.json::<QueueStatus>()?)
}

pub fn fetch_cost(endpoint: &str) -> anyhow::Result<CostStatus> {
    let client = blocking_client()?;
    let url = format!("{}/v1/status/cost", endpoint);
    Ok(client.get(&url).send()?.json::<CostStatus>()?)
}

pub fn fetch_tier_a(endpoint: &str) -> anyhow::Result<TierAStatus> {
    let client = blocking_client()?;
    let url = format!("{}/v1/status/tier-a", endpoint);
    Ok(client.get(&url).send()?.json::<TierAStatus>()?)
}

pub fn fetch_yoyo(endpoint: &str) -> anyhow::Result<YoyoStatus> {
    let client = blocking_client()?;
    let url = format!("{}/v1/status/yoyo", endpoint);
    Ok(client.get(&url).send()?.json::<YoyoStatus>()?)
}
