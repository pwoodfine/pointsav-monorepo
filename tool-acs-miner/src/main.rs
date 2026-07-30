use chrono::Utc;
use regex::Regex;
use serde::Serialize;
use std::collections::HashSet;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use uuid::Uuid;

#[derive(Serialize)]
struct Anchor {
    target_uuid: String,
    anchor_source: String,
    timestamp: String,
}

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

    // 1. Establish Substrate Boundaries
    let substrate_dir = format!("{}/service-people/substrate", totebox_root);
    fs::create_dir_all(&substrate_dir).unwrap();

    let anchors_path = format!("{}/anchors.jsonl", substrate_dir);
    let claims_path = format!("{}/claims.jsonl", substrate_dir);

    let mut anchors_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&anchors_path)
        .unwrap();
    let mut claims_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&claims_path)
        .unwrap();

    // 2. Read the Raw Linguistic Body
    let mut content = String::new();
    if fs::File::open(input_path)
        .unwrap()
        .read_to_string(&mut content)
        .is_err()
    {
        std::process::exit(0);
    }

    let source_name = Path::new(input_path)
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();

    // 3. The Infinite Net (Multi-Pass Regex Extraction)
    let email_regex = Regex::new(r"(?i)\b[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}\b").unwrap();
    let phone_regex =
        Regex::new(r"(?i)\b(\+?\d{1,3}[-.\s]?)?\(?\d{3}\)?[-.\s]?\d{3}[-.\s]?\d{4}\b").unwrap();
    let entity_regex = Regex::new(r"\b[A-Z][a-z]+(?:\s+[A-Z][a-z]+){1,3}\b").unwrap(); // 2-4 Capitalized words

    let mut unique_emails = HashSet::new();
    let mut unique_phones = HashSet::new();
    let mut unique_entities = HashSet::new();

    for cap in email_regex.captures_iter(&content) {
        unique_emails.insert(cap[0].to_lowercase());
    }
    for cap in phone_regex.captures_iter(&content) {
        unique_phones.insert(cap[0].to_string());
    }
    for cap in entity_regex.captures_iter(&content) {
        unique_entities.insert(cap[0].to_string());
    }

    let timestamp = Utc::now().to_rfc3339();
    let mut total_mass = 0;

    // 4. Forge Ledgers and Compile Entity Bundle
    let mut entity_bundle_text = String::new();
    entity_bundle_text.push_str("--- EXTRACTED IDENTITY MASS ---\n");

    let mut write_claim = |attr: &str, val: &str, score: f32| {
        let target_uuid = Uuid::new_v5(&Uuid::NAMESPACE_DNS, val.as_bytes()).to_string();

        let anchor = Anchor {
            target_uuid: target_uuid.clone(),
            anchor_source: val.to_string(),
            timestamp: timestamp.clone(),
        };
        let claim = Claim {
            claim_id: Uuid::new_v4().to_string(),
            target_uuid,
            attribute: attr.to_string(),
            value: val.to_string(),
            confidence_score: score,
            source_id: source_name.clone(),
            timestamp: timestamp.clone(),
        };

        writeln!(anchors_file, "{}", serde_json::to_string(&anchor).unwrap()).unwrap();
        writeln!(claims_file, "{}", serde_json::to_string(&claim).unwrap()).unwrap();
        total_mass += 1;
    };

    if !unique_emails.is_empty() {
        entity_bundle_text.push_str("\n[EMAILS]:\n");
        for e in &unique_emails {
            write_claim("email", e, 1.0);
            entity_bundle_text.push_str(&format!("- {}\n", e));
        }
    }

    if !unique_phones.is_empty() {
        entity_bundle_text.push_str("\n[PHONES]:\n");
        for p in &unique_phones {
            write_claim("phone", p, 0.9);
            entity_bundle_text.push_str(&format!("- {}\n", p));
        }
    }

    if !unique_entities.is_empty() {
        entity_bundle_text.push_str("\n[PROPER NOUNS / ENTITIES]:\n");
        for ent in &unique_entities {
            write_claim("proper_noun", ent, 0.6);
            entity_bundle_text.push_str(&format!("- {}\n", ent));
        }
    }

    // 5. Output the Bundle for the SLM Gardener
    let bundle_out_path = format!(
        "{}/service-slm/transient-queues/{}_identities.txt",
        totebox_root, source_name
    );
    if total_mass > 0 {
        fs::write(&bundle_out_path, entity_bundle_text).unwrap();
        println!(
            "[SUCCESS] ACS Engine extracted {} Identity Signals. Bundle staged for SLM.",
            total_mass
        );
    } else {
        println!("[SYSTEM] Zero Identity Mass detected in {}.", source_name);
    }
}
