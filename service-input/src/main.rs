// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `service-input` daemon — Ring 1 generic document ingest.
//!
//! Reads env vars at startup, constructs the parser Dispatcher and
//! FsClient, then serves the MCP-server interface at `POST /mcp`.
//!
//! Required env vars:
//!   INPUT_MODULE_ID   — per-tenant moduleId (must match FS_MODULE_ID
//!                       on the service-fs instance this process writes to).
//!   INPUT_FS_URL      — service-fs base URL (e.g., http://127.0.0.1:9100).
//!
//! Optional env vars:
//!   INPUT_BIND_ADDR   — address:port to listen on (default 0.0.0.0:9200).

use std::sync::Arc;

use tracing::info;

use service_input::http::{router, AppState};
use service_input::{Dispatcher, DocxParser, FsClient, MarkdownParser, PdfParser, XlsxParser};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "service_input=info".parse().unwrap()),
        )
        .init();

    let module_id = std::env::var("INPUT_MODULE_ID")
        .expect("INPUT_MODULE_ID is required");
    let fs_url = std::env::var("INPUT_FS_URL")
        .expect("INPUT_FS_URL is required");
    let bind_addr = std::env::var("INPUT_BIND_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:9200".to_string());

    let dispatcher = Dispatcher::new()
        .with_pdf(Box::new(PdfParser))
        .with_markdown(Box::new(MarkdownParser))
        .with_docx(Box::new(DocxParser))
        .with_xlsx(Box::new(XlsxParser));

    let fs_client = FsClient::new(fs_url, module_id.clone());

    let state = Arc::new(AppState {
        module_id: module_id.clone(),
        dispatcher,
        fs_client,
    });

    let app = router(state);

    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .unwrap_or_else(|e| panic!("failed to bind {bind_addr}: {e}"));

    info!(module_id, bind_addr, "service-input starting");

    axum::serve(listener, app)
        .await
        .expect("axum server error");
}
