// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `service-email` daemon — Ring 1 Communications Ledger ingest.
//!
//! Polls an Exchange mailbox via EWS SOAP, fetches unread messages as
//! raw MIME, writes them to a local maildir vault (transition sink),
//! and marks each message as read.
//!
//! Auth pattern: AZURE_ACCESS_TOKEN consumed from env (token acquired
//! out-of-process). No inline OAuth handshake. Per operator decision
//! 2026-04-25 and service-email-egress-ews/ reference implementation.
//!
//! Required env vars:
//!   AZURE_ACCESS_TOKEN    — pre-acquired bearer token for Exchange
//!   EXCHANGE_TARGET_USER  — target mailbox SMTP address
//!
//! Optional env vars:
//!   EWS_ENDPOINT          — EWS URL (default: Exchange Online)
//!   TOTEBOX_ARCHIVE_PATH  — maildir root (default: /assets/personnel-maildir)

mod auth;
mod ews_client;
mod maildir;

use auth::EwsCredentials;
use ews_client::EwsClient;
use maildir::MaildirVault;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("SYSTEM EVENT: Initializing service-email daemon (EWS SOAP mode).");

    let creds = EwsCredentials::from_env().unwrap_or_else(|e| {
        eprintln!("FATAL: {e}");
        std::process::exit(1);
    });

    let archive_path = std::env::var("TOTEBOX_ARCHIVE_PATH")
        .unwrap_or_else(|_| "/assets/personnel-maildir".to_string());

    let vault = MaildirVault::init(&archive_path).unwrap_or_else(|e| {
        eprintln!("FATAL: maildir init failed at {archive_path}: {e}");
        std::process::exit(1);
    });

    println!("SYSTEM EVENT: Maildir archive verified at {archive_path}");
    println!(
        "SYSTEM EVENT: Entering polling loop. Target: {}",
        creds.target_user
    );

    loop {
        let client = EwsClient::new(
            creds.access_token.clone(),
            creds.ews_endpoint.clone(),
            creds.target_user.clone(),
        );

        match client.find_unread_ids().await {
            Err(e) => eprintln!("SYSTEM ERROR: find_unread_ids: {e}"),
            Ok(ids) if ids.is_empty() => {
                println!("SYSTEM EVENT: No unread messages. Sleeping.");
            }
            Ok(ids) => {
                println!("SYSTEM EVENT: {} unread message(s) found.", ids.len());
                let mut ingested = 0usize;
                for id in &ids {
                    match client.get_mime(id).await {
                        Err(e) => eprintln!("SYSTEM ERROR: get_mime({id}): {e}"),
                        Ok(mime_bytes) => {
                            let payload = String::from_utf8_lossy(&mime_bytes).into_owned();
                            match vault.write_payload(&payload) {
                                Err(e) => eprintln!("SYSTEM ERROR: vault write({id}): {e}"),
                                Ok(_) => {
                                    if let Err(e) = client.mark_read(id).await {
                                        eprintln!("SYSTEM ERROR: mark_read({id}): {e}");
                                    }
                                    ingested += 1;
                                }
                            }
                        }
                    }
                    // Anti-throttle pause between per-message EWS calls.
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
                println!(
                    "SYSTEM EVENT: Ingested {ingested}/{} messages.",
                    ids.len()
                );
            }
        }

        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}
