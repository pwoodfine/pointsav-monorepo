use dotenv::from_path;
use reqwest::Client;
use serde_json::Value;
use std::env;
use std::fs;
use uuid::Uuid;

// THE PHYSICAL LAWS OF THE INGRESS DIODE
const BATCH_SIZE: usize = 3;
const SPOOL_DIR: &str = "/opt/woodfine/cluster-totebox-personnel-1/service-email/maildir/new";
const AUTH_ENV: &str = "/opt/woodfine/cluster-totebox-personnel-1/service-email/auth-credentials.env";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("========================================================");
    println!(" 💥 INGRESS DIODE ACTIVE: MICRO-BATCHING (MAX {})", BATCH_SIZE);
    println!("========================================================");

    // 1. Load WORM Credentials
    if let Err(_) = from_path(AUTH_ENV) {
        println!("[WARNING] Target environment file not found at {}. Falling back to system env.", AUTH_ENV);
    }
    
    let tenant = env::var("AZURE_TENANT_ID").expect("[FATAL] Missing AZURE_TENANT_ID");
    let client_id = env::var("AZURE_CLIENT_ID").expect("[FATAL] Missing AZURE_CLIENT_ID");
    let secret = env::var("AZURE_CLIENT_SECRET").expect("[FATAL] Missing AZURE_CLIENT_SECRET");
    let user = env::var("EXCHANGE_TARGET_USER").expect("[FATAL] Missing EXCHANGE_TARGET_USER");

    let client = Client::new();
    let token = get_token(&client, &tenant, &client_id, &secret).await?;

    let target_folder_names = vec!["totebox-ingress", "OpenStack", "PostgresSQL"];
    
    // 2. Dynamically resolve Folder IDs to prevent hardcoded drift
    let active_folders = discover_folders(&client, &token, &user, &target_folder_names).await;

    if active_folders.is_empty() {
        println!("[SYSTEM] No target folders located. Terminating cycle.");
        return Ok(());
    }

    // Ensure the spool directory physically exists
    fs::create_dir_all(SPOOL_DIR)?;
    let mut grand_total = 0;

    // 3. Execute the Micro-Batch Sweep sequentially to protect 1GB RAM limits
    for (folder_name, folder_id) in active_folders {
        println!("\n[SYSTEM] Sweeping Folder: {}", folder_name);
        
        let url = format!("https://graph.microsoft.com/v1.0/users/{}/mailFolders/{}/messages?$top={}&$select=id,subject", user, folder_id, BATCH_SIZE);
        let res = client.get(&url).bearer_auth(&token).send().await?;
        
        let msg_data: Value = res.json().await?;
        let messages = match msg_data["value"].as_array() {
            Some(arr) if !arr.is_empty() => arr,
            _ => {
                println!("  -> [STATUS] Folder is mathematically empty. Bypassing...");
                continue;
            }
        };

        println!("  -> Lock acquired on {} assets. Initiating extraction...", messages.len());

        for msg in messages {
            let msg_id = msg["id"].as_str().unwrap().to_string();
            let subject = msg["subject"].as_str().unwrap_or("No Subject");
            
            // Generate a unique UUID for the payload to prevent collisions
            let tx_id = Uuid::new_v4().to_string().chars().take(8).collect::<String>().to_uppercase();
            let local_file = format!("{}/VAULT_INGRESS_{}_{}.eml", SPOOL_DIR, folder_name, tx_id);
            
            // Step A: Download the raw .eml MIME stream
            let download_url = format!("https://graph.microsoft.com/v1.0/users/{}/messages/{}/$value", user, msg_id);
            if let Ok(response) = client.get(&download_url).bearer_auth(&token).send().await {
                if let Ok(bytes) = response.bytes().await {
                    fs::write(&local_file, bytes)?;
                    println!("     [+] Vaulted: {} (Subj: {})", tx_id, subject);
                    
                    // Step B: Idempotent Hard Delete from Microsoft Cloud
                    let delete_url = format!("https://graph.microsoft.com/v1.0/users/{}/messages/{}", user, msg_id);
                    if client.delete(&delete_url).bearer_auth(&token).send().await.is_ok() {
                        println!("     [-] Purged from MSFT Cloud.");
                        grand_total += 1;
                    }
                }
            }
        }
    }

    println!("\n========================================================");
    println!("[SUCCESS] INGRESS CYCLE COMPLETE. Total Assets Extracted: {}", grand_total);
    Ok(())
}

async fn get_token(client: &Client, tenant: &str, client_id: &str, secret: &str) -> Result<String, Box<dyn std::error::Error>> {
    let url = format!("https://login.microsoftonline.com/{}/oauth2/v2.0/token", tenant);
    let params = [
        ("client_id", client_id),
        ("scope", "https://graph.microsoft.com/.default"),
        ("client_secret", secret),
        ("grant_type", "client_credentials"),
    ];
    let res = client.post(&url).form(&params).send().await?;
    let json: Value = res.json().await?;
    Ok(json["access_token"].as_str().unwrap().to_string())
}

async fn discover_folders(client: &Client, token: &str, user: &str, targets: &[&str]) -> Vec<(String, String)> {
    let mut found = Vec::new();
    
    // Check root folders
    let url = format!("https://graph.microsoft.com/v1.0/users/{}/mailFolders?$top=100", user);
    if let Ok(res) = client.get(&url).bearer_auth(token).send().await {
        if let Ok(data) = res.json::<Value>().await {
            if let Some(arr) = data["value"].as_array() {
                for f in arr {
                    if let (Some(name), Some(id)) = (f["displayName"].as_str(), f["id"].as_str()) {
                        if targets.contains(&name) {
                            found.push((name.to_string(), id.to_string()));
                        }
                    }
                }
            }
        }
    }
    found
}
