use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use serde_json::{Value, json};
use uuid::Uuid;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("[ERROR] Invalid execution. Usage: spatial-ledger <TOTEBOX_ROOT>");
        std::process::exit(1);
    }

    let totebox_root = &args[1];
    
    // Strict Routing Boundaries
    let queue_dir = format!("{}/service-people/discovery-queue", totebox_root);
    let substrate_dir = format!("{}/service-people/substrate", totebox_root);
    let ledger_path = format!("{}/ledger_personnel.jsonl", substrate_dir);

    fs::create_dir_all(&queue_dir).unwrap();
    fs::create_dir_all(&substrate_dir).unwrap();

    let mut processed = 0;

    if let Ok(entries) = fs::read_dir(&queue_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().unwrap_or_default() == "json" {
                let content = fs::read_to_string(&path).unwrap_or_default();
                
                if let Ok(payload) = serde_json::from_str::<Value>(&content) {
                    let sender = payload["spatial_vectors"]["sender"].as_str().unwrap_or("UNKNOWN").to_lowercase();
                    let subject = payload["spatial_vectors"]["subject"].as_str().unwrap_or("NO SUBJECT");
                    let tx_id = payload["transaction_id"].as_str().unwrap_or("UNKNOWN");
                    let ts_received = payload["timestamp_received"].as_str().unwrap_or("UNKNOWN");

                    // 1. FORGE THE DETERMINISTIC VECTOR (UUIDv5)
                    let hash = Uuid::new_v5(&Uuid::NAMESPACE_DNS, sender.as_bytes()).to_string();
                    let sov_id = format!("RAW-{}", &hash[..8].to_uppercase());

                    // 2. ASSEMBLE THE LEDGER RECORD
                    let ledger_entry = json!({
                        "sovereign_id": sov_id,
                        "identity_anchor": sender,
                        "latest_subject": subject,
                        "transaction_id": tx_id,
                        "timestamp_received": ts_received,
                        "status": "LOGGED"
                    });

                    // 3. APPEND TO IMMUTABLE SUBSTRATE
                    let mut file = OpenOptions::new().create(true).append(true).open(&ledger_path).unwrap();
                    writeln!(file, "{}", serde_json::to_string(&ledger_entry).unwrap()).unwrap();

                    println!("  -> [LEDGER] Identity Mapped: {} -> {}", sender, sov_id);
                    
                    // 4. ERADICATE THE TRANSIENT QUEUE FILE
                    fs::remove_file(&path).unwrap();
                    processed += 1;
                }
            }
        }
    }
    
    if processed > 0 {
        println!("[SUCCESS] Extracted {} Cryptographic Vectors to Spatial Ledger.", processed);
    }
}
