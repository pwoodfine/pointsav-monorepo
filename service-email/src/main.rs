mod maildir;
mod graph_client;
mod auth;

use maildir::MaildirVault;
use graph_client::GraphBridge;
use std::env;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("SYSTEM EVENT: Initializing Sovereign Email Bridge daemon (Anti-Throttling Mode).");

    let archive_path = env::var("TOTEBOX_ARCHIVE_PATH")
        .unwrap_or_else(|_| "/assets/personnel-maildir".to_string());
    let target_user = env::var("EXCHANGE_TARGET_USER")
        .expect("FATAL: EXCHANGE_TARGET_USER missing.");
    
    let tenant_id = env::var("AZURE_TENANT_ID").expect("FATAL: AZURE_TENANT_ID missing.");
    let client_id = env::var("AZURE_CLIENT_ID").expect("FATAL: AZURE_CLIENT_ID missing.");
    let secret = env::var("AZURE_CLIENT_SECRET").expect("FATAL: AZURE_CLIENT_SECRET missing.");

    let vault = MaildirVault::init(&archive_path)?;
    println!("SYSTEM EVENT: Totebox archive verified at {}", archive_path);
    println!("SYSTEM EVENT: Entering persistent polling loop. Target: {}", target_user);

    loop {
        match auth::negotiate_token(&tenant_id, &client_id, &secret).await {
            Ok(token) => {
                let bridge = GraphBridge::new(token);
                
                let mut current_url = format!(
                    "https://graph.microsoft.com/v1.0/users/{}/messages?$filter=isRead eq false&$top=500",
                    target_user
                );

                loop {
                    match bridge.fetch_url(&current_url).await {
                        Ok(messages) => {
                            if let Some(msg_array) = messages["value"].as_array() {
                                if !msg_array.is_empty() {
                                    let mut mutation_count = 0;
                                    for msg in msg_array {
                                        let raw_json = msg.to_string();
                                        
                                        if let Ok(_) = vault.write_payload(&raw_json) {
                                            if let Some(msg_id) = msg["id"].as_str() {
                                                match bridge.mutate_state(&target_user, msg_id).await {
                                                    Ok(_) => {
                                                        mutation_count += 1;
                                                    }
                                                    Err(e) => {
                                                        eprintln!("SYSTEM ERROR: Failed to mutate state for {} - REASON: {}", msg_id, e);
                                                    }
                                                }
                                                // ANTI-THROTTLING INJECTION: 50ms physical sleep between mutations
                                                tokio::time::sleep(Duration::from_millis(50)).await;
                                            }
                                        }
                                    }
                                    println!("SYSTEM EVENT: Extracted and mutated {} payloads to local archive.", mutation_count);
                                }
                            }
                            
                            if let Some(next_link) = messages["@odata.nextLink"].as_str() {
                                println!("SYSTEM EVENT: Network pagination detected. Bypassing throttle to recursively extract next batch.");
                                current_url = next_link.to_string();
                            } else {
                                break; 
                            }
                        }
                        Err(e) => {
                            eprintln!("SYSTEM ERROR: Graph Bridge extraction failed: {}", e);
                            break; 
                        }
                    }
                }
            }
            Err(e) => eprintln!("SYSTEM ERROR: OAuth2 Negotiation Failed: {}", e),
        }
        
        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}
