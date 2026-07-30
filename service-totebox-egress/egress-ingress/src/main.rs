mod maildir;
mod state_manager;

use std::env;
use std::time::Duration;
use reqwest::Client;
use serde_json::Value;
use tokio::signal;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("SYSTEM EVENT: Initializing PointSav Ingress Daemon (Dynamic Tier 3).");

    let target_user = env::var("EXCHANGE_TARGET_USER").expect("FATAL: EXCHANGE_TARGET_USER missing.");
    let access_token = env::var("AZURE_ACCESS_TOKEN").expect("FATAL: AZURE_ACCESS_TOKEN missing.");
    
    // Dynamic Routing & Gating
    let staging_path = env::var("TOTEBOX_VAULT_PATH").expect("FATAL: TOTEBOX_VAULT_PATH missing.");
    let batch_limit_bytes: u64 = env::var("BATCH_LIMIT_BYTES")
        .unwrap_or_else(|_| "3000000000".to_string())
        .parse()
        .expect("FATAL: BATCH_LIMIT_BYTES must be a valid number.");

    println!("SYSTEM EVENT: Verifying physical I/O integrity at {}...", staging_path);
    let vault = match maildir::MaildirVault::init(&staging_path) {
        Ok(v) => {
            println!("SYSTEM EVENT: I/O Verification Passed. Limit set to {} bytes.", batch_limit_bytes);
            v
        },
        Err(e) => {
            eprintln!("FATAL: Physical I/O failure on staging directory: {}", e);
            std::process::exit(1);
        }
    };

    let client = Client::new();
    
    let mut current_url = match state_manager::load_checkpoint() {
        Some(saved_link) => {
            println!("SYSTEM EVENT: Valid Checkpoint found. Resuming extraction sequence...");
            saved_link
        },
        None => {
            println!("SYSTEM EVENT: No checkpoint found. Initiating clean extraction sequence on IN-PLACE ARCHIVE...");
            format!("https://graph.microsoft.com/v1.0/users/{}/mailFolders/ArchiveMsgFolderRoot/messages?$select=id&$top=250", target_user)
        }
    };

    let mut session_bytes: u64 = 0;

    println!("SYSTEM EVENT: Engaging Cloud Bridge to MSFT Archive. Press Ctrl+C for graceful shutdown.");

    loop {
        tokio::select! {
            _ = async {
                loop {
                    if session_bytes >= batch_limit_bytes {
                        println!("SYSTEM ALERT: {} Byte Batch Limit Reached. Safely halting for physical egress.", batch_limit_bytes);
                        std::process::exit(0);
                    }

                    match client.get(&current_url).bearer_auth(&access_token).send().await {
                        Ok(res) if res.status().is_success() => {
                            if let Ok(body) = res.json::<Value>().await {
                                if let Some(messages) = body["value"].as_array() {
                                    for msg in messages {
                                        if let Some(id) = msg["id"].as_str() {
                                            let mime_url = format!("https://graph.microsoft.com/v1.0/users/{}/messages/{}/$value", target_user, id);
                                            match client.get(&mime_url).bearer_auth(&access_token).send().await {
                                                Ok(mime_res) if mime_res.status().is_success() => {
                                                    if let Ok(raw_bytes) = mime_res.text().await {
                                                        match vault.write_payload(&raw_bytes) {
                                                            Ok(bytes_written) => {
                                                                session_bytes += bytes_written;
                                                                println!("SUCCESS: Extracted {} bytes. Session total: {} bytes.", bytes_written, session_bytes);
                                                            },
                                                            Err(e) => {
                                                                eprintln!("CRITICAL I/O ERROR: Failed to write to disk. Error: {}", e);
                                                                std::process::exit(1);
                                                            }
                                                        }
                                                    }
                                                },
                                                _ => eprintln!("WARNING: Failed to fetch MIME for Archive ID: {}", id),
                                            }
                                            sleep(Duration::from_millis(150)).await;
                                        }
                                    }
                                }

                                if let Some(next_link) = body["@odata.nextLink"].as_str() {
                                    current_url = next_link.to_string();
                                    if let Err(e) = state_manager::save_checkpoint(&current_url) {
                                        eprintln!("CRITICAL STATE ERROR: Failed to write checkpoint: {}. Halting.", e);
                                        std::process::exit(1);
                                    }
                                } else {
                                    println!("SYSTEM EVENT: Archive extraction complete.");
                                    let _ = std::fs::remove_file("../data-ledgers/checkpoint.json");
                                    std::process::exit(0);
                                }
                            }
                        },
                        Ok(res) => {
                            eprintln!("NETWORK WARNING: Graph API Status: {}. Retrying...", res.status());
                            sleep(Duration::from_secs(10)).await;
                        },
                        Err(e) => {
                            eprintln!("NETWORK FAILURE: Connection dropped. Retrying... Error: {}", e);
                            sleep(Duration::from_secs(10)).await;
                        }
                    }
                }
            } => {},

            _ = signal::ctrl_c() => {
                println!("\nSYSTEM EVENT: Operator termination command (SIGINT) received.");
                if let Err(e) = state_manager::save_checkpoint(&current_url) {
                    eprintln!("WARNING: Failed to save final checkpoint during shutdown: {}", e);
                } else {
                    println!("SYSTEM EVENT: State successfully preserved.");
                }
                std::process::exit(0);
            },
        }
    }
}
