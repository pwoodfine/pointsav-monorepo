use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::time::Duration;
use reqwest::Client;
use tokio::time::sleep;
use chrono::Utc;

fn log_fmt(level: &str, message: &str) -> String {
    format!("[{}] [{}] {}", Utc::now().to_rfc3339(), level, message)
}

fn count_physical_files(vault_path: &str) -> std::io::Result<usize> {
    let mut count = 0;
    for dir in ["new", "cur"].iter() {
        let target = format!("{}/{}", vault_path, dir);
        if let Ok(entries) = fs::read_dir(&target) {
            for entry in entries.flatten() {
                if let Ok(file_type) = entry.file_type() {
                    if file_type.is_file() {
                        count += 1;
                    }
                }
            }
        }
    }
    Ok(count)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", log_fmt("INIT", "PointSav Destructive Pruning Daemon (High-Fidelity Mode)"));

    let target_user = env::var("EXCHANGE_TARGET_USER").expect("FATAL: EXCHANGE_TARGET_USER missing.");
    let access_token = env::var("AZURE_ACCESS_TOKEN").expect("FATAL: AZURE_ACCESS_TOKEN missing.");
    let vault_path = env::var("TOTEBOX_VAULT_PATH").expect("FATAL: TOTEBOX_VAULT_PATH missing.");
    
    let roster_path = "../data-ledgers/personnel_roster.csv";
    let prune_ledger_path = "../data-ledgers/prune_ledger.log";

    println!("{}", log_fmt("AUDIT", "Initiating Phase 3 Parity Audit..."));

    let physical_count = match count_physical_files(&vault_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}", log_fmt("FATAL", &format!("Cannot access external vault at {}. Error: {}", vault_path, e)));
            std::process::exit(1);
        }
    };
    println!("{}", log_fmt("AUDIT", &format!("Physical payload count on 1.0 TB drive: {}", physical_count)));

    let file = File::open(roster_path).expect("FATAL ERROR: Cannot find personnel_roster.csv ledger.");
    let reader = BufReader::new(file);
    let mut roster_lines: Vec<String> = reader.lines().filter_map(Result::ok).collect();
    
    if roster_lines.is_empty() {
        eprintln!("{}", log_fmt("FATAL", "Ledger is empty."));
        std::process::exit(1);
    }
    
    roster_lines.remove(0); // Strip CSV Header
    let ledger_count = roster_lines.len();
    println!("{}", log_fmt("AUDIT", &format!("Ledger record count (personnel_roster.csv): {}", ledger_count)));

    // THE HARD GATE
    if physical_count != ledger_count {
        eprintln!("\n=======================================================");
        eprintln!("{}", log_fmt("SECURITY", "CRITICAL PARITY FAILURE!"));
        eprintln!("Physical files ({}) DO NOT MATCH Ledger records ({}).", physical_count, ledger_count);
        eprintln!("SYSTEM HALTED. No destruction commands will be issued.");
        eprintln!("=======================================================\n");
        std::process::exit(1);
    }

    println!("{}", log_fmt("VERIFIED", "Volume Parity Achieved. Data integrity confirmed."));

    // THE KINETIC LOCK (User Authorization)
    println!("\n-------------------------------------------------------");
    println!("WARNING: YOU ARE ABOUT TO PERMANENTLY DESTROY {} CLOUD ASSETS.", ledger_count);
    println!("This action cannot be undone by PointSav or Woodfine Management.");
    println!("To authorize destruction, type the exact number of assets below:");
    println!("-------------------------------------------------------");
    print!("AUTHORIZATION CODE (Type {}): ", ledger_count);
    io::stdout().flush().unwrap();

    let mut auth_input = String::new();
    io::stdin().read_line(&mut auth_input).unwrap();
    
    if auth_input.trim() != ledger_count.to_string() {
        eprintln!("{}", log_fmt("ABORT", "Authorization failed. System powering down."));
        std::process::exit(1);
    }

    println!("{}", log_fmt("EXEC", "Authorization accepted. Engaging MSFT Graph API..."));

    let client = Client::new();
    let mut log_file = OpenOptions::new().create(true).write(true).append(true).open(prune_ledger_path)?;
    let mut deleted_count = 0;
    
    for line in roster_lines {
        let columns: Vec<&str> = line.split(',').collect();
        if columns.len() > 2 {
            let message_id = columns[2];
            let delete_url = format!("https://graph.microsoft.com/v1.0/users/{}/messages/{}", target_user, message_id);
            
            // Loop with internal retry logic
            let mut attempt = 0;
            loop {
                attempt += 1;
                match client.delete(&delete_url).bearer_auth(&access_token).send().await {
                    Ok(res) if res.status().is_success() || res.status().as_u16() == 404 => {
                        deleted_count += 1;
                        let msg = format!("Pruned Asset {}/{} | ID: {}", deleted_count, ledger_count, message_id);
                        println!("{}", log_fmt("SUCCESS", &msg));
                        writeln!(log_file, "{}", log_fmt("SUCCESS", &msg))?;
                        break; // Exit retry loop
                    },
                    Ok(res) if res.status().as_u16() == 429 => {
                        println!("{}", log_fmt("THROTTLE", "MSFT Rate Limit hit. Cooling down for 5 seconds..."));
                        sleep(Duration::from_secs(5)).await;
                    },
                    Ok(res) => {
                        let msg = format!("Failed to prune ID {}. MSFT Status: {}", message_id, res.status());
                        eprintln!("{}", log_fmt("WARN", &msg));
                        writeln!(log_file, "{}", log_fmt("WARN", &msg))?;
                        break; // Move to next asset
                    },
                    Err(e) => {
                        if attempt >= 3 {
                            let msg = format!("Network dropped on ID {}. Max retries exceeded. Skipping.", message_id);
                            eprintln!("{}", log_fmt("ERROR", &msg));
                            writeln!(log_file, "{}", log_fmt("ERROR", &msg))?;
                            break;
                        }
                        println!("{}", log_fmt("RETRY", "Network error. Retrying..."));
                        sleep(Duration::from_secs(2)).await;
                    }
                }
            }
            sleep(Duration::from_millis(150)).await; // Strict Anti-Throttling pace
        }
    }

    println!("\n=======================================================");
    println!("{}", log_fmt("COMPLETE", &format!("PRUNING CYCLE FINISHED. {} ASSETS DESTROYED.", deleted_count)));
    println!("=======================================================\n");
    Ok(())
}
