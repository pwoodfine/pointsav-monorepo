// SPDX-License-Identifier: Apache-2.0 OR MIT

//! HTTP client for appending email records to service-fs's WORM ledger.
//!
//! Wire format: JSON over HTTP, POST /v1/append (the same surface
//! service-fs exposes at its REST boundary). Blocking I/O via ureq —
//! no async needed at Ring 1 boundary-ingest throughput.
//!
//! ADR-07 compliant — no AI inference, deterministic.

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use chrono::Utc;
use serde::Serialize;

// ── Payload type ─────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct EmailRecord {
    message_id: String,
    raw_eml_b64: String,
    ingested_at: String,
}

// ── Error type ───────────────────────────────────────────────────────

#[derive(Debug)]
pub enum FsClientError {
    Serialization(String),
    Transport(String),
    StatusError { status: u16 },
    ResponseParse(String),
}

impl std::fmt::Display for FsClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FsClientError::Serialization(msg) => write!(f, "serialization error: {msg}"),
            FsClientError::Transport(msg) => write!(f, "transport error: {msg}"),
            FsClientError::StatusError { status } => {
                write!(f, "service-fs returned HTTP {status}")
            }
            FsClientError::ResponseParse(msg) => write!(f, "response parse error: {msg}"),
        }
    }
}

impl std::error::Error for FsClientError {}

// ── Client ───────────────────────────────────────────────────────────

/// Thin blocking HTTP client for writing email records into service-fs.
///
/// Construct once at daemon startup with the service-fs base URL and
/// the per-tenant moduleId (must match FS_MODULE_ID on the server).
#[derive(Clone)]
pub struct FsClient {
    base_url: String,
    module_id: String,
}

impl FsClient {
    pub fn new(base_url: impl Into<String>, module_id: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            module_id: module_id.into(),
        }
    }

    /// Append one email record to service-fs. Returns the assigned ledger cursor.
    ///
    /// Wire body: `{ "payload_id": message_id, "payload": EmailRecord }`.
    /// Header: `X-Foundry-Module-ID: <module_id>` (per-tenant boundary,
    /// Doctrine §IV.b).
    pub fn append_email(
        &self,
        message_id: &str,
        raw_eml: &[u8],
    ) -> Result<u64, FsClientError> {
        let record = EmailRecord {
            message_id: message_id.to_string(),
            raw_eml_b64: BASE64.encode(raw_eml),
            ingested_at: Utc::now().to_rfc3339(),
        };

        let payload = serde_json::to_value(&record)
            .map_err(|e| FsClientError::Serialization(e.to_string()))?;

        let body = serde_json::json!({
            "payload_id": message_id,
            "payload": payload,
        });

        let url = format!("{}/v1/append", self.base_url);

        let mut response = ureq::post(&url)
            .header("Content-Type", "application/json")
            .header("X-Foundry-Module-ID", &self.module_id)
            .send_json(&body)
            .map_err(|e| match e {
                ureq::Error::StatusCode(status) => FsClientError::StatusError { status },
                other => FsClientError::Transport(other.to_string()),
            })?;

        let resp_json: serde_json::Value = response
            .body_mut()
            .read_json()
            .map_err(|e| FsClientError::ResponseParse(e.to_string()))?;

        resp_json["cursor"].as_u64().ok_or_else(|| {
            FsClientError::ResponseParse(format!(
                "missing or non-u64 'cursor' in response: {resp_json}"
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fs_client_constructs_correctly() {
        let client = FsClient::new("http://127.0.0.1:9100", "email-module");
        assert_eq!(client.base_url, "http://127.0.0.1:9100");
        assert_eq!(client.module_id, "email-module");
    }

    #[test]
    fn fs_client_is_clone() {
        let client = FsClient::new("http://127.0.0.1:9100", "m1");
        let client2 = client.clone();
        assert_eq!(client.base_url, client2.base_url);
    }

    #[test]
    fn email_record_serializes_all_fields() {
        let record = EmailRecord {
            message_id: "AAABBB".to_string(),
            raw_eml_b64: BASE64.encode(b"From: a@b.com\r\n"),
            ingested_at: "2026-05-20T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&record).unwrap();
        assert!(json.contains("\"message_id\""));
        assert!(json.contains("\"raw_eml_b64\""));
        assert!(json.contains("\"ingested_at\""));
    }
}
