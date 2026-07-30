use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use uuid::Uuid;

#[derive(Serialize)]
struct AppendBody {
    payload_id: String,
    payload: serde_json::Value,
}

#[derive(Deserialize, Debug)]
pub struct AppendResponse {
    pub payload_id: Option<String>,
    pub module_id: Option<String>,
    pub ledger_root: Option<String>,
}

pub struct IngestResult {
    pub payload_id: String,
    pub ledger_root: Option<String>,
    pub warning: Option<String>,
}

pub fn submit(path: &str, username: &str, tenant: &str, endpoint: &str) -> Result<IngestResult> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;

    let payload_id = Uuid::new_v4().to_string();
    let body = AppendBody {
        payload_id: payload_id.clone(),
        payload: serde_json::json!({
            "path": path,
            "submitted_by": username,
            "tenant": tenant,
            "source": "app-console-input",
        }),
    };

    let url = format!("{}/v1/append", endpoint.trim_end_matches('/'));
    let resp = client
        .post(&url)
        .header("X-Foundry-Module-ID", tenant)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()?;

    if resp.status().is_success() {
        let parsed: AppendResponse = resp.json().unwrap_or(AppendResponse {
            payload_id: Some(payload_id.clone()),
            module_id: None,
            ledger_root: None,
        });
        Ok(IngestResult {
            payload_id: parsed.payload_id.unwrap_or(payload_id),
            ledger_root: parsed.ledger_root,
            warning: None,
        })
    } else {
        let body_text = resp.text().unwrap_or_default();
        Ok(IngestResult {
            payload_id: payload_id.clone(),
            ledger_root: None,
            warning: Some(format!("service-input: {}", body_text)),
        })
    }
}
