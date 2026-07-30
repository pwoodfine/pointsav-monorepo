use reqwest::Client;
use serde_json::Value;
use std::collections::VecDeque;
use std::env;
use std::fs;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("SYSTEM EVENT: Initializing OMNISCIENT Template Rescue Protocol (SAFE MODE)...");

    let tenant_id = env::var("AZURE_TENANT_ID").expect("FATAL: AZURE_TENANT_ID missing.");
    let client_id = env::var("AZURE_CLIENT_ID").expect("FATAL: AZURE_CLIENT_ID missing.");
    let secret = env::var("AZURE_CLIENT_SECRET").expect("FATAL: AZURE_CLIENT_SECRET missing.");
    let target_user = env::var("EXCHANGE_TARGET_USER").expect("FATAL: EXCHANGE_TARGET_USER missing.");

    let vault_dir = "/assets/template-vault";
    fs::create_dir_all(vault_dir).unwrap_or_else(|_| ());

    let client = Client::new();

    // 1. Negotiate Token
    let token_url = format!("https://login.microsoftonline.com/{}/oauth2/v2.0/token", tenant_id);
    let params = [
        ("client_id", client_id.as_str()),
        ("scope", "https://graph.microsoft.com/.default"),
        ("client_secret", secret.as_str()),
        ("grant_type", "client_credentials"),
    ];
    
    let res = client.post(&token_url).form(&params).send().await?.json::<Value>().await?;
    let access_token = res["access_token"].as_str().expect("FATAL: Failed to secure access token.");

    // 2. Queue for Infinite Breadth-First Traversal
    let mut folder_queue = VecDeque::new();
    folder_queue.push_back(format!("https://graph.microsoft.com/v1.0/users/{}/mailFolders?$top=100", target_user));

    let mut folders_processed = 0;

    while let Some(mut current_url) = folder_queue.pop_front() {
        loop {
            let folders_res = client.get(&current_url).bearer_auth(access_token).send().await?.json::<Value>().await?;
            
            if let Some(folders) = folders_res["value"].as_array() {
                for folder in folders {
                    let name = folder["displayName"].as_str().unwrap_or("");
                    let folder_id = folder["id"].as_str().unwrap();
                    
                    // Push child folders to the queue to ensure zero blind spots
                    folder_queue.push_back(format!("https://graph.microsoft.com/v1.0/users/{}/mailFolders/{}/childFolders?$top=100", target_user, folder_id));

                    if name.starts_with("Templates") {
                        folders_processed += 1;
                        println!("SYSTEM EVENT: Locked onto Target Folder [{}]: {}", folders_processed, name);
                        
                        // 3. Message Extraction with Pagination
                        let mut msgs_url = format!("https://graph.microsoft.com/v1.0/users/{}/mailFolders/{}/messages?$top=100", target_user, folder_id);
                        
                        loop {
                            let msgs_res = client.get(&msgs_url).bearer_auth(access_token).send().await?.json::<Value>().await?;
                            
                            if let Some(messages) = msgs_res["value"].as_array() {
                                for msg in messages {
                                    let subject = msg["subject"].as_str().unwrap_or("UNTITLED_TEMPLATE");
                                    let body = msg["bodyPreview"].as_str().unwrap_or("");
                                    
                                    let safe_subject = subject.replace(|c: char| !c.is_alphanumeric(), "_");
                                    // Prefix with folder count to prevent filename collisions across different folders
                                    let file_path = format!("{}/{}_{}.txt", vault_dir, folders_processed, safe_subject);
                                    
                                    let payload = format!("TEMPLATE_CATEGORY: {}\nSUBJECT: {}\n---\n{}", name, subject, body);
                                    match fs::write(&file_path, payload) {
                                        Ok(_) => println!("  -> Extracted: {}", subject),
                                        Err(e) => println!("  -> SYSTEM ERROR: Failed to write {}: {}", subject, e),
                                    }
                                }
                            }

                            // Handle Message Pagination (If a folder has >100 templates)
                            if let Some(next_msg_link) = msgs_res["@odata.nextLink"].as_str() {
                                msgs_url = next_msg_link.to_string();
                            } else {
                                break;
                            }
                        }
                    }
                }
            }
            
            // Handle Folder Pagination (If user has >100 folders at this level)
            if let Some(next_link) = folders_res["@odata.nextLink"].as_str() {
                current_url = next_link.to_string();
            } else {
                break;
            }
        }
    }

    println!("SYSTEM EVENT: Omniscient Safe Template Rescue Complete. Total Template Folders Processed: {}", folders_processed);
    Ok(())
}
