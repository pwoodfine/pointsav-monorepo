// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{Deserialize, Serialize};
use crate::person::Person;

#[derive(Debug, Clone)]
pub struct FsClient {
    base_url: String,
    module_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AppendRequest {
    pub payload_id: String,
    pub payload: Person,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AppendResponse {
    pub cursor: u64,
}

#[derive(Debug)]
pub enum FsClientError {
    Serialization(String),
    Transport(String),
    StatusError(u32, String),
    ResponseParse(String),
}

impl std::fmt::Display for FsClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FsClientError::Serialization(e) => write!(f, "Serialization error: {}", e),
            FsClientError::Transport(e) => write!(f, "Transport error: {}", e),
            FsClientError::StatusError(code, body) => {
                write!(f, "Status error {}: {}", code, body)
            }
            FsClientError::ResponseParse(e) => write!(f, "Response parse error: {}", e),
        }
    }
}

impl std::error::Error for FsClientError {}

impl FsClient {
    pub fn new(base_url: impl Into<String>, module_id: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            module_id: module_id.into(),
        }
    }

    pub fn append(&self, person: &Person) -> Result<u64, FsClientError> {
        let url = format!("{}/v1/append", self.base_url);
        let body = serde_json::json!({
            "payload_id": person.id.to_string(),
            "payload": person
        });

        let mut response = ureq::post(&url)
            .header("Content-Type", "application/json")
            .header("X-Foundry-Module-ID", &self.module_id)
            .send_json(&body)
            .map_err(|e| match e {
                ureq::Error::StatusCode(status) => FsClientError::StatusError(status as u32, "server error".to_string()),
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
    use chrono::Utc;
    use uuid::Uuid;

    #[test]
    fn append_serializes_person_correctly() {
        let person = Person::new("Test User", "test@example.com");
        let request = AppendRequest {
            payload_id: person.id.to_string(),
            payload: person.clone(),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"payload_id\""));
        assert!(json.contains("\"payload\""));
        assert!(json.contains("test@example.com"));
    }

    #[test]
    fn fs_client_constructs_correctly() {
        let client = FsClient::new("http://127.0.0.1:9100", "test-module");
        assert_eq!(client.base_url, "http://127.0.0.1:9100");
        assert_eq!(client.module_id, "test-module");
    }
}
