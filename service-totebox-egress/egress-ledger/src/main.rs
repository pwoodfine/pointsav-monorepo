use std::env;
use std::fs::OpenOptions;
use std::io::Write;
use reqwest::Client;
use serde_json::Value;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("SYSTEM EVENT: Initializing PointSav Metadata Ledger (Phase 1A - Archive Target).");

    let target_user = env::var("EXCHANGE_TARGET_USER").expect("FATAL: EXCHANGE_TARGET_USER missing.");
    let access_token = env::var("AZURE_ACCESS_TOKEN").expect("FATAL: AZURE_ACCESS_TOKEN missing.");
    let ledger_path = "../data-ledgers/mailbox_manifest.txt";

    let client = Client::new();
    
    // Engineering Logic: Route explicitly into the Archive's child folders
    let mut current_url = format!(
        "https://graph.microsoft.com/v1.0/users/{}/mailFolders/ArchiveMsgFolderRoot/childFolders?$top=250",
        target_user
    );

    let mut file = OpenOptions::new().create(true).write(true).truncate(true).open(ledger_path)?;
    writeln!(file, "--- WOODFINE MANAGEMENT CORP: IN-PLACE ARCHIVE MANIFEST ---")?;
    writeln!(file, "TARGET: {}", target_user)?;
    writeln!(file, "-----------------------------------------------------------")?;

    println!("SYSTEM EVENT: Traversing remote archive folder tree...");

    loop {
        let res = client.get(&current_url).bearer_auth(&access_token).send().await?;
        if !res.status().is_success() {
            eprintln!("SYSTEM ERROR: Failed to fetch archive folders: {}", res.status());
            break;
        }

        let body = res.json::<Value>().await?;
        if let Some(folders) = body["value"].as_array() {
            for folder in folders {
                let name = folder["displayName"].as_str().unwrap_or("UNKNOWN");
                let item_count = folder["totalItemCount"].as_u64().unwrap_or(0);
                let unread_count = folder["unreadItemCount"].as_u64().unwrap_or(0);
                
                let log_entry = format!("ARCHIVE FOLDER: {:<30} | TOTAL ITEMS: {:<6} | UNREAD: {}", name, item_count, unread_count);
                println!("{}", log_entry);
                writeln!(file, "{}", log_entry)?;
            }
        }

        if let Some(next_link) = body["@odata.nextLink"].as_str() {
            current_url = next_link.to_string();
        } else {
            break;
        }
    }

    println!("SYSTEM EVENT: Archive Ledger written to {}", ledger_path);
    Ok(())
}
