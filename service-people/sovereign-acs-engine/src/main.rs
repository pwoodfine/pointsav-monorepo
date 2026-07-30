use chrono::Utc;
use regex::Regex;
use serde::Serialize;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use uuid::Uuid;

// The Immutable Anchor
#[derive(Serialize)]
struct Anchor {
    target_uuid: String,
    anchor_source: String,
    timestamp: String,
}

// The Append-Only Observation
#[derive(Serialize)]
struct Claim {
    claim_id: String,
    target_uuid: String,
    attribute: String,
    value: String,
    confidence_score: f32,
    source_id: String,
    timestamp: String,
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("[ERROR] Usage: sovereign-acs-engine <TEXT_FILE_PATH> <TOTEBOX_ROOT>");
        std::process::exit(1);
    }

    let input_path = &args[1];
    let totebox_root = &args[2];

    // 1. Map the Substrate Directories
    let substrate_dir = format!("{}/service-people/substrate", totebox_root);
    if let Err(e) = fs::create_dir_all(&substrate_dir) {
        eprintln!("[ERROR] Failed to forge substrate boundary: {}", e);
        std::process::exit(1);
    }

    let anchors_path = format!("{}/anchors.jsonl", substrate_dir);
    let claims_path = format!("{}/claims.jsonl", substrate_dir);

    // 2. Open the Immutable Ledgers in Append Mode
    let mut anchors_file = OpenOptions::new().create(true).append(true).open(&anchors_path).unwrap();
    let mut claims_file = OpenOptions::new().create(true).append(true).open(&claims_path).unwrap();

    // 3. Ingest the Raw Linguistic Body
    let mut content = String::new();
    if let Err(_) = fs::File::open(input_path).unwrap().read_to_string(&mut content) {
        eprintln!("[WARNING] Unable to read payload text. Halting extraction.");
        std::process::exit(0);
    }

    let source_name = Path::new(input_path).file_name().unwrap().to_str().unwrap().to_string();

    // 4. The Infinite Gravity Sweep (Email Topology)
    let email_regex = Regex::new(r"(?i)[a-z0-9._%+-]+@[a-z0-9.-]+\.[a-z]{2,}").unwrap();
    let mut signals_found = 0;

    for cap in email_regex.captures_iter(&content) {
        let raw_email = cap[0].to_lowercase();
        
        // Deterministic Identity Resolution (UUIDv5)
        let target_uuid = Uuid::new_v5(&Uuid::NAMESPACE_DNS, raw_email.as_bytes()).to_string();
        let timestamp = Utc::now().to_rfc3339();

        // Forge the Anchor
        let anchor = Anchor {
            target_uuid: target_uuid.clone(),
            anchor_source: raw_email.clone(),
            timestamp: timestamp.clone(),
        };

        // Forge the Claim
        let claim = Claim {
            claim_id: Uuid::new_v4().to_string(),
            target_uuid: target_uuid.clone(),
            attribute: "email".to_string(),
            value: raw_email,
            confidence_score: 1.0, // System-verified regex extraction
            source_id: source_name.clone(),
            timestamp: timestamp.clone(),
        };

        // Serialize to JSON-Lines
        let anchor_json = serde_json::to_string(&anchor).unwrap();
        let claim_json = serde_json::to_string(&claim).unwrap();

        // Write sequentially to the Substrate
        writeln!(anchors_file, "{}", anchor_json).unwrap();
        writeln!(claims_file, "{}", claim_json).unwrap();
        
        signals_found += 1;
    }

    println!("[SUCCESS] ACS Engine swept {}. Identity Mass Extracted: {}", source_name, signals_found);
}
