use std::time::Duration;

use anyhow::Result;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct PendingRequest {
    pub request_id: String,
    pub code: String,
    pub username: String,
    pub tenant: String,
    pub created_at: String,
    // Absent until the pairing API returns the field; None means "not yet provided by server".
    pub fingerprint: Option<String>,
}

#[derive(Deserialize)]
struct PendingResponse {
    pending: Vec<PendingRequest>,
}

fn client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("http client")
}

pub fn fetch_pending(base_url: &str) -> Result<Vec<PendingRequest>> {
    let url = format!("{}/v1/pair/pending", base_url.trim_end_matches('/'));
    let resp: PendingResponse = client().get(&url).send()?.error_for_status()?.json()?;
    Ok(resp.pending)
}

pub fn approve(base_url: &str, code: &str) -> Result<()> {
    let url = format!("{}/v1/pair/approve", base_url.trim_end_matches('/'));
    client()
        .post(&url)
        .json(&serde_json::json!({ "code": code }))
        .send()?
        .error_for_status()?;
    Ok(())
}

pub fn deny(base_url: &str, code: &str) -> Result<()> {
    let url = format!("{}/v1/pair/deny", base_url.trim_end_matches('/'));
    client()
        .post(&url)
        .json(&serde_json::json!({ "code": code }))
        .send()?
        .error_for_status()?;
    Ok(())
}
