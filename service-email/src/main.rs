// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `service-email` daemon — Ring 1 Communications Ledger ingest.
//!
//! Two concurrent workloads in one process:
//!
//! 1. **MCP HTTP server** (POST /mcp) — accepts `email.ingest` tool
//!    calls from Ring 2 clients, writes raw EML into service-fs via
//!    FsClient.
//!
//! 2. **EWS polling daemon** — polls an Exchange mailbox via EWS SOAP,
//!    fetches unread messages as raw MIME, writes each to service-fs via
//!    FsClient, and marks the message as read.
//!
//! Auth pattern: AZURE_ACCESS_TOKEN consumed from env (token acquired
//! out-of-process). No inline OAuth handshake. Per operator decision
//! 2026-04-25 and service-email-egress-ews/ reference implementation.
//!
//! Required env vars:
//!   AZURE_ACCESS_TOKEN    — pre-acquired bearer token for Exchange
//!   EXCHANGE_TARGET_USER  — target mailbox SMTP address
//!   EMAIL_MODULE_ID       — per-tenant moduleId (must match FS_MODULE_ID
//!                           on the service-fs instance)
//!   EMAIL_FS_URL          — service-fs base URL (e.g. http://127.0.0.1:9100)
//!
//! Optional env vars:
//!   EWS_ENDPOINT          — EWS URL (default: Exchange Online)
//!   EMAIL_BIND_ADDR       — MCP server bind address (default: 127.0.0.1:9200)

mod auth;
mod ews_client;
mod fs_client;
mod http;
mod maildir;
mod mcp;

use auth::EwsCredentials;
use ews_client::EwsClient;
use fs_client::FsClient;
use http::AppState;
use std::sync::Arc;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "service_email=info".parse().unwrap()),
        )
        .init();

    let creds = EwsCredentials::from_env().unwrap_or_else(|e| {
        eprintln!("FATAL: {e}");
        std::process::exit(1);
    });

    let module_id = std::env::var("EMAIL_MODULE_ID")
        .expect("EMAIL_MODULE_ID is required");
    let fs_url = std::env::var("EMAIL_FS_URL")
        .expect("EMAIL_FS_URL is required");
    let bind_addr = std::env::var("EMAIL_BIND_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:9200".to_string());

    let fs_client = FsClient::new(&fs_url, &module_id);
    let fs_client_daemon = fs_client.clone();

    let state = Arc::new(AppState {
        module_id: module_id.clone(),
        fs_client,
    });

    let app = http::router(state);
    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .unwrap_or_else(|e| panic!("failed to bind {bind_addr}: {e}"));

    tracing::info!(module_id, bind_addr, "service-email MCP server starting");

    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("axum serve error");
    });

    tracing::info!(
        target_user = creds.target_user,
        "service-email EWS polling daemon starting"
    );

    loop {
        let client = EwsClient::new(
            creds.access_token.clone(),
            creds.ews_endpoint.clone(),
            creds.target_user.clone(),
        );

        match client.find_unread_ids().await {
            Err(e) => tracing::error!("find_unread_ids: {e}"),
            Ok(ids) if ids.is_empty() => {
                tracing::debug!("no unread messages; sleeping");
            }
            Ok(ids) => {
                tracing::info!("{} unread message(s) found", ids.len());
                let mut ingested = 0usize;
                for id in &ids {
                    match client.get_mime(id).await {
                        Err(e) => tracing::error!("get_mime({id}): {e}"),
                        Ok(mime_bytes) => {
                            match fs_client_daemon.append_email(id, &mime_bytes) {
                                Err(e) => tracing::error!("fs append({id}): {e}"),
                                Ok(cursor) => {
                                    tracing::debug!("appended {id} at cursor {cursor}");
                                    if let Err(e) = client.mark_read(id).await {
                                        tracing::error!("mark_read({id}): {e}");
                                    }
                                    ingested += 1;
                                }
                            }
                        }
                    }
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
                tracing::info!("ingested {ingested}/{} messages", ids.len());
            }
        }

        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}
