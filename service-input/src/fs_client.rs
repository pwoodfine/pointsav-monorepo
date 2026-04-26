// SPDX-License-Identifier: Apache-2.0 OR MIT

//! HTTP client for submitting parsed documents to service-fs's WORM ledger.
//!
//! Wire format: JSON over HTTP, POST /v1/append (the same surface
//! service-fs exposes at its REST boundary). Blocking I/O via ureq
//! — no async needed at Ring 1 boundary-ingest throughput.
//!
//! When service-fs's MCP-server interface replaces the JSON-over-HTTP
//! surface (per worm-ledger-design.md §5 step 5), the call inside
//! `submit` swaps to MCP-client semantics with no change to the
//! `FsClient` API.
//!
//! ADR-07 compliant — no AI inference, deterministic.

use crate::ParsedDocument;

/// Error type for `FsClient` operations.
#[derive(Debug)]
pub enum FsClientError {
    /// Serialization of the `ParsedDocument` to JSON failed.
    Serialization(String),
    /// HTTP transport error (connection refused, timeout, I/O error).
    Transport(String),
    /// service-fs returned a non-2xx HTTP status.
    StatusError { status: u16 },
    /// The response body could not be deserialized.
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

/// Thin blocking HTTP client for writing parsed documents into service-fs.
///
/// Construct once at daemon startup with the `service-fs` base URL and
/// the per-tenant `moduleId` (matches `FS_MODULE_ID` on the server).
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

    /// POST the parsed document to `POST /v1/append`. Returns the
    /// assigned ledger cursor (monotonically increasing; starts at 1).
    ///
    /// Wire body: `{ "payload_id": doc.source_id, "payload": doc }`.
    /// Header: `X-Foundry-Module-ID: <module_id>` (per-tenant boundary
    /// enforcement on the server side per Doctrine §IV.b).
    pub fn submit(&self, doc: &ParsedDocument) -> Result<u64, FsClientError> {
        let payload = serde_json::to_value(doc)
            .map_err(|e| FsClientError::Serialization(e.to_string()))?;

        let body = serde_json::json!({
            "payload_id": doc.source_id,
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
    use crate::{Format, ParsedDocument};
    use std::sync::Arc;

    /// Spin up a real service-fs axum server on an ephemeral port,
    /// submit a ParsedDocument, and assert the returned cursor is >= 1.
    ///
    /// This is an integration test at the service-input/service-fs
    /// boundary: it exercises the full HTTP wire path (ureq → axum →
    /// LedgerBackend → JSON response) without mocks.
    #[test]
    fn submit_appends_to_service_fs_and_returns_cursor_ge_1() {
        use service_fs::ledger::InMemoryLedger;

        // Bind on port 0: OS assigns a free ephemeral port immediately.
        // We do this in std (sync) so we can read the port before
        // entering any async context.
        let std_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = std_listener.local_addr().unwrap().port();
        let module_id = "test-input-tenant";

        // Spawn a background thread that runs the axum server. Using a
        // dedicated tokio runtime per thread avoids nested-runtime issues
        // (ureq is blocking; the test thread is synchronous).
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(async move {
                std_listener.set_nonblocking(true).unwrap();
                let listener = tokio::net::TcpListener::from_std(std_listener).unwrap();
                let state = Arc::new(service_fs::AppState {
                    module_id: module_id.to_string(),
                    ledger: Box::new(
                        InMemoryLedger::open(
                            std::env::temp_dir().join("svc-input-test-ledger"),
                            module_id,
                        )
                        .unwrap(),
                    ),
                    audit_ledger: Box::new(
                        InMemoryLedger::open(
                            std::env::temp_dir().join("svc-input-test-audit"),
                            "audit-log",
                        )
                        .unwrap(),
                    ),
                });
                axum::serve(listener, service_fs::router(state))
                    .await
                    .unwrap();
            });
        });

        // Brief pause to let the server reach its accept loop.
        std::thread::sleep(std::time::Duration::from_millis(50));

        let client = FsClient::new(
            format!("http://127.0.0.1:{port}"),
            module_id.to_string(),
        );

        let doc = ParsedDocument {
            format: Format::Markdown,
            source_id: "test-doc-1".to_string(),
            text: "# Hello\n\nBody text.".to_string(),
            metadata: serde_json::json!({ "headings": ["Hello"], "parser": "test" }),
        };

        let cursor = client.submit(&doc).expect("submit must succeed");
        assert!(cursor >= 1, "cursor must be >= 1, got {cursor}");
    }
}
