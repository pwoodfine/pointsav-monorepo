use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub const PROTOCOLS: &[(&str, &str)] = &[
    ("prose-architecture", "PROSE — ARCHITECTURE"),
    ("prose-guide", "PROSE — GUIDE (operational runbook)"),
    ("prose-memo", "PROSE — MEMO"),
    ("prose-readme", "PROSE — README"),
    ("prose-topic", "PROSE — TOPIC (content-wiki)"),
    ("comms-chat", "COMMS — chat"),
    ("comms-email", "COMMS — email"),
    ("comms-ticket-comment", "COMMS — ticket comment"),
    ("translate-en-es", "TRANSLATE — EN → ES"),
];

pub const DEFAULT_PROTOCOL_IDX: usize = 4; // prose-topic

#[derive(Serialize)]
struct ProofreadRequest {
    text: String,
    protocol: String,
    tenant: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ProofreadResponse {
    pub improved_text: String,
    #[serde(default)]
    pub diff: Vec<serde_json::Value>,
    #[serde(default)]
    pub explanations: Vec<serde_json::Value>,
    pub tier_used: Option<String>,
    pub audit_ledger_id: Option<String>,
    pub request_id: String,
    pub protocol: String,
    pub template_display_name: Option<String>,
    #[serde(default)]
    pub degraded: Vec<String>,
}

#[derive(Serialize)]
struct VerdictRequest {
    request_id: String,
    draft_id: String,
    tenant: String,
    verdict: String,
}

pub fn submit_proofread(
    text: &str,
    protocol: &str,
    tenant: &str,
    endpoint: &str,
) -> Result<ProofreadResponse> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(300))
        .build()?;
    let req = ProofreadRequest {
        text: text.to_string(),
        protocol: protocol.to_string(),
        tenant: tenant.to_string(),
    };
    let url = format!("{}/v1/proofread", endpoint.trim_end_matches('/'));
    let resp = client.post(&url).json(&req).send()?;
    if !resp.status().is_success() {
        let body = resp.text().unwrap_or_default();
        bail!("service-proofreader: {}", body);
    }
    Ok(resp.json::<ProofreadResponse>()?)
}

pub fn post_verdict(request_id: &str, tenant: &str, verdict: &str, endpoint: &str) -> Result<()> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;
    let req = VerdictRequest {
        request_id: request_id.to_string(),
        draft_id: request_id.to_string(),
        tenant: tenant.to_string(),
        verdict: verdict.to_string(),
    };
    let url = format!("{}/v1/verdict", endpoint.trim_end_matches('/'));
    let _ = client.post(&url).json(&req).send()?.text();
    Ok(())
}
