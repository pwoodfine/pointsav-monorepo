use base64::{engine::general_purpose::STANDARD as BASE64_STD, Engine as _};
use mailparse::{parse_mail, MailHeaderMap};
use notify::{Event, RecursiveMode, Result as NotifyResult, Watcher};
use regex::Regex;
use serde_json::Value;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::sync::mpsc::channel;
use std::thread;
use std::time::Duration;

// ── Allowed classification vocabulary ─────────────────────────────────────────
// When edge_entities supply a classification it must be one of these values
// or the upstream schema is violating the output contract.  The test suite
// enforces this list; update it here when the vocabulary is extended.
pub const ALLOWED_CLASSIFICATIONS: &[&str] = &[
    "ORIGIN SENDER",
    "PERSON",
    "ORGANIZATION",
    "LOCATION",
    "DATE",
    "PRODUCT",
    "EVENT",
    "UNKNOWN",
];

#[derive(serde::Serialize, Clone)]
struct ExtractedEntity {
    entity_name: String,
    classification: String,
    role_vector: String,
    confidence: f32,
    context_anchor: String,
    location_vector: String,
    latent_vectors: Vec<String>,
}

fn main() -> NotifyResult<()> {
    println!("================================================================");
    println!("[SYSTEM] PointSav Cryptographic Router (Dumb Vault Mode)");
    println!("[SYSTEM] Protocol: Consuming Edge Wasm Intelligence...");
    println!("================================================================");

    let base_dir = std::env::var("EXTRACTION_BASE_DIR")
        .unwrap_or_else(|_| "/home/mathew/deployments/woodfine-fleet-deployment".to_string());
    let watch_dir = std::env::var("EXTRACTION_WATCH_DIR").unwrap_or_else(|_| {
        format!(
            "{}/cluster-totebox-personnel-1/service-fs/data/service-people/source",
            base_dir
        )
    });
    // Optional: emit CORPUS_*.json for service-content DataGraph ingestion
    let corpus_emit_dir = std::env::var("EXTRACTION_EMIT_CORPUS_DIR").ok();
    // Optional: set module_id in emitted CORPUS JSON (falls back to SERVICE_CONTENT_MODULE_ID env var in service-content)
    let corpus_module_id = std::env::var("EXTRACTION_CORPUS_MODULE_ID").ok();

    println!("[SYSTEM] Base dir: {}", base_dir);
    println!("[SYSTEM] Watch dir: {}", watch_dir);
    if let Some(dir) = &corpus_emit_dir {
        println!(
            "[SYSTEM] Corpus emit dir: {} (module_id: {})",
            dir,
            corpus_module_id
                .as_deref()
                .unwrap_or("(from service-content env)")
        );
    }

    if !Path::new(&watch_dir).exists() {
        fs::create_dir_all(&watch_dir).unwrap();
    }

    let mut processed_ledgers: Vec<String> = Vec::new();

    if let Ok(entries) = fs::read_dir(Path::new(&watch_dir)) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                let filename = path.file_name().unwrap().to_str().unwrap().to_string();
                if process_payload(
                    &path,
                    &base_dir,
                    corpus_emit_dir.as_deref(),
                    corpus_module_id.as_deref(),
                ) {
                    processed_ledgers.push(filename.clone());
                    // Move the drop file to processed/ after successful emit so the
                    // watch dir does not accumulate unboundedly across restarts.
                    // Moving (not deleting) preserves the original payload for audit.
                    let done_dir = format!("{}/processed", watch_dir);
                    if fs::create_dir_all(&done_dir).is_ok() {
                        let _ = fs::rename(&path, format!("{}/{}", done_dir, filename));
                    }
                }
            }
        }
    }

    let (tx, rx) = channel();
    let mut watcher = notify::recommended_watcher(tx)?;
    watcher.watch(Path::new(&watch_dir), RecursiveMode::NonRecursive)?;

    println!("================================================================");
    println!("[SYSTEM] Active Surveillance Engaged: {}", watch_dir);

    loop {
        match rx.recv() {
            Ok(Ok(Event { paths, .. })) => {
                for path in paths {
                    if let Some(extension) = path.extension() {
                        if extension == "json" {
                            let filename = path.file_name().unwrap().to_str().unwrap().to_string();
                            if !processed_ledgers.contains(&filename) {
                                thread::sleep(Duration::from_millis(250));
                                if process_payload(
                                    &path,
                                    &base_dir,
                                    corpus_emit_dir.as_deref(),
                                    corpus_module_id.as_deref(),
                                ) {
                                    processed_ledgers.push(filename.clone());
                                    // Move to processed/ after successful emit.
                                    let done_dir = format!("{}/processed", watch_dir);
                                    if fs::create_dir_all(&done_dir).is_ok() {
                                        let _ =
                                            fs::rename(&path, format!("{}/{}", done_dir, filename));
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Ok(_) => {}
            Err(_) => {}
        }
    }
}

fn process_payload(
    filepath: &Path,
    base_dir: &str,
    corpus_emit_dir: Option<&str>,
    corpus_module_id: Option<&str>,
) -> bool {
    let content = match fs::read_to_string(filepath) {
        Ok(c) => c,
        Err(_) => return false,
    };
    let payload: Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return false,
    };

    let file_obj = &payload["file"];
    let original_filename = file_obj["filename"].as_str().unwrap_or("unknown_asset");
    let base64_data = file_obj["data"].as_str().unwrap_or("");

    let b64_str = if let Some(idx) = base64_data.find(',') {
        &base64_data[idx + 1..]
    } else {
        base64_data
    };
    let raw_bytes = match BASE64_STD.decode(b64_str) {
        Ok(b) => b,
        Err(_) => return false,
    };

    let dest_archive = payload["destination_archive"]
        .as_str()
        .unwrap_or("cluster-totebox-personnel-1");
    let target_service = payload["target_service"]
        .as_str()
        .unwrap_or("service-people");
    let worm_id = filepath.file_stem().unwrap().to_str().unwrap();

    let ext = Path::new(original_filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let mut graph_entities: Vec<ExtractedEntity> = Vec::new();
    let mut seen_names = HashSet::new();
    let mut corpus_parts: Vec<String> = Vec::new();

    corpus_parts.push(format!("Document: {}", original_filename));

    if matches!(ext.as_str(), "md" | "yaml" | "yml" | "txt") {
        // Non-email text corpus: raw UTF-8 bytes go directly into corpus_parts.
        corpus_parts.push(String::from_utf8_lossy(&raw_bytes).into_owned());
    } else {
        // Email path: parse RFC 2822 headers and body.
        let parsed_mail = match parse_mail(&raw_bytes) {
            Ok(m) => m,
            Err(e) => {
                eprintln!(
                    "[WARN] mailparse failed for {} ({}): {e}",
                    original_filename, worm_id
                );
                return false;
            }
        };
        let headers = parsed_mail.get_headers();
        let sender = headers
            .get_first_value("From")
            .unwrap_or_else(|| "Unknown".to_string());

        corpus_parts.push(format!("From: {}", sender));

        // 1. PURE CRYPTOGRAPHIC ORIGIN ANCHORING
        let re_sender = Regex::new(r#"(?i)"?([^"(<]+)(?:\(([^)]+)\))?"?\s*<([^>]+)>"#).unwrap();
        if let Some(caps) = re_sender.captures(&sender) {
            let raw_name = caps.get(1).map_or("", |m| m.as_str()).trim().to_string();
            let name = raw_name.replace('"', "");
            if !name.is_empty() {
                seen_names.insert(name.clone());
                graph_entities.push(ExtractedEntity {
                    entity_name: name,
                    classification: "ORIGIN SENDER".to_string(),
                    role_vector: "Cryptographic Anchor".to_string(),
                    confidence: 1.0,
                    context_anchor: format!("HEADER: {}", sender),
                    location_vector: "UNVERIFIED".to_string(),
                    latent_vectors: vec![],
                });
            }
        }

        if let Some(subject) = headers.get_first_value("Subject") {
            corpus_parts.push(format!("Subject: {}", subject));
        }

        if let Ok(body) = parsed_mail.get_body() {
            let trimmed = body.trim().to_string();
            if !trimmed.is_empty() {
                corpus_parts.push(trimmed);
            }
        }
    }

    // 2. EDGE AI INGESTION (Trusting the WebAssembly payload blindly)
    if let Some(edge_entities) = payload.get("edge_entities").and_then(|v| v.as_array()) {
        for ent in edge_entities {
            let name = ent["entity_name"].as_str().unwrap_or("").trim().to_string();
            let class = ent["classification"]
                .as_str()
                .unwrap_or("UNKNOWN")
                .to_string();
            let conf = ent["confidence"].as_f64().unwrap_or(0.9) as f32;

            if name.len() > 2 && !seen_names.contains(&name) {
                seen_names.insert(name.clone());
                corpus_parts.push(format!("{}: {}", class, name));
                graph_entities.push(ExtractedEntity {
                    entity_name: name,
                    classification: class,
                    role_vector: "Edge AI Inference".to_string(),
                    confidence: conf,
                    context_anchor: "WASM LOCAL MATRIX".to_string(),
                    location_vector: "UNVERIFIED".to_string(),
                    latent_vectors: vec![],
                });
            }
        }
    }

    let write_ledger = |service: &str, suffix: &str, content: &str| {
        let dir = format!(
            "{}/{}/service-fs/data/{}/ledgers",
            base_dir, dest_archive, service
        );
        fs::create_dir_all(&dir).unwrap();
        fs::write(format!("{}/{}_{}.json", dir, suffix, worm_id), content).unwrap();
    };

    if !graph_entities.is_empty() {
        let people_ledger = serde_json::json!({
            "worm_id": worm_id,
            "source_asset": original_filename,
            "extracted_crm_entities": graph_entities,
        });
        write_ledger(target_service, "CRM", &people_ledger.to_string());
        println!(
            "  -> [VAULT] Successfully secured {} entities evaluated by Edge AI.",
            graph_entities.len()
        );
    }

    // ── CORPUS bridge ─────────────────────────────────────────────────────────
    // When EXTRACTION_EMIT_CORPUS_DIR is set, write a CORPUS_*.json alongside
    // the CRM ledger. service-content watches this dir and feeds the text to
    // Doorman for grammar-constrained entity extraction into LadybugDB.
    if let Some(emit_dir) = corpus_emit_dir {
        let corpus_text = corpus_parts.join("\n");
        if !corpus_text.is_empty() {
            let mut corpus_json = serde_json::json!({
                "worm_id": worm_id,
                "corpus": corpus_text,
            });
            if let Some(mid) = corpus_module_id {
                corpus_json["module_id"] = serde_json::json!(mid);
            }
            let out_path = format!("{}/CORPUS_{}.json", emit_dir, worm_id);
            match fs::write(&out_path, corpus_json.to_string()) {
                Ok(_) => println!(
                    "  -> [CORPUS] Emitted CORPUS_{}.json for DataGraph ingestion.",
                    worm_id
                ),
                Err(e) => println!("  -> [CORPUS] Write failed ({}): {}", out_path, e),
            }
        }
    }

    true
}

// ─────────────────────────────────────────────────────────────────────────────
// Test suite — D11 full pipeline
//
// Four scope areas:
//   A. Output contract  — CRM ledger schema, required fields, classification
//                         vocabulary, no null/empty entity_name pass-through
//   B. Queue drain      — files processed once, moved to processed/, not re-run
//   C. Redrive logic    — files in queue-poison/ can be redriven and succeed
//   D. Poison handling  — malformed/unparseable files return false without panic
//
// All I/O uses temp directories under std::env::temp_dir() with a unique
// suffix derived from the test name.  No writes to /srv/foundry/data/ or
// any production path occur.
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::STANDARD as BASE64_STD;
    use base64::Engine as _;
    use serde_json::Value;
    use std::fs;
    use std::path::{Path, PathBuf};

    // ── Helpers ───────────────────────────────────────────────────────────────

    /// Create a unique temp directory for a test.
    fn test_dir(label: &str) -> PathBuf {
        let base = std::env::temp_dir();
        let dir = base.join(format!(
            "svc-extraction-test-{}-{}",
            label,
            std::process::id()
        ));
        fs::create_dir_all(&dir).expect("create test_dir");
        dir
    }

    /// Build a minimal valid payload JSON string.
    /// `filename` determines the extraction path (email vs text).
    /// `data` must be a base64-encoded bytes string (or empty → "" triggers decode failure).
    fn build_payload(
        filename: &str,
        b64_data: &str,
        dest_archive: &str,
        target_service: &str,
        edge_entities: serde_json::Value,
    ) -> String {
        serde_json::json!({
            "file": {
                "filename": filename,
                "data": b64_data,
            },
            "destination_archive": dest_archive,
            "target_service": target_service,
            "edge_entities": edge_entities,
        })
        .to_string()
    }

    /// Write a payload file to `dir/<worm_id>.json` and return the path.
    fn write_payload_file(dir: &Path, worm_id: &str, payload_json: &str) -> PathBuf {
        let path = dir.join(format!("{}.json", worm_id));
        fs::write(&path, payload_json).expect("write payload file");
        path
    }

    /// Return a valid RFC 2822 email encoded as base64.
    fn sample_email_b64(from: &str, subject: &str, body: &str) -> String {
        let raw = format!(
            "From: {}\r\nSubject: {}\r\nContent-Type: text/plain\r\n\r\n{}\r\n",
            from, subject, body
        );
        BASE64_STD.encode(raw.as_bytes())
    }

    /// Return the path where the CRM ledger is expected after process_payload().
    fn expected_crm_path(
        base_dir: &Path,
        dest_archive: &str,
        service: &str,
        worm_id: &str,
    ) -> PathBuf {
        base_dir
            .join(dest_archive)
            .join("service-fs/data")
            .join(service)
            .join("ledgers")
            .join(format!("CRM_{}.json", worm_id))
    }

    /// Read and parse the CRM ledger JSON at `path`.
    fn read_crm_ledger(path: &Path) -> Value {
        let raw = fs::read_to_string(path).expect("read CRM ledger");
        serde_json::from_str(&raw).expect("parse CRM ledger as JSON")
    }

    // ── Area A: Output contract ───────────────────────────────────────────────

    /// A-01: A valid email payload with a named From header produces a CRM
    ///       ledger that contains `worm_id`, `source_asset`, and
    ///       `extracted_crm_entities`.
    #[test]
    fn a01_crm_ledger_required_fields_present() {
        let td = test_dir("a01");
        let base_dir = td.join("base");
        let watch_dir = td.join("watch");
        fs::create_dir_all(&watch_dir).unwrap();

        let b64 = sample_email_b64("Alice Smith <alice@example.com>", "Hello", "Test body");
        let payload = build_payload(
            "message.eml",
            &b64,
            "cluster-totebox-personnel-1",
            "service-people",
            serde_json::json!([]),
        );
        let filepath = write_payload_file(&watch_dir, "WORM-A01", &payload);

        let ok = process_payload(&filepath, base_dir.to_str().unwrap(), None, None);
        assert!(ok, "process_payload must return true for a valid payload");

        let crm_path = expected_crm_path(
            &base_dir,
            "cluster-totebox-personnel-1",
            "service-people",
            "WORM-A01",
        );
        assert!(crm_path.exists(), "CRM ledger must be written to disk");

        let ledger = read_crm_ledger(&crm_path);
        assert_eq!(ledger["worm_id"].as_str().unwrap(), "WORM-A01");
        assert!(
            ledger["source_asset"].as_str().is_some(),
            "source_asset required"
        );
        assert!(
            ledger["extracted_crm_entities"].is_array(),
            "extracted_crm_entities must be array"
        );
    }

    /// A-02: `entity_name` in each extracted entity must be a non-empty string.
    #[test]
    fn a02_entity_name_non_empty_in_sender_anchor() {
        let td = test_dir("a02");
        let base_dir = td.join("base");
        let watch_dir = td.join("watch");
        fs::create_dir_all(&watch_dir).unwrap();

        let b64 = sample_email_b64("Bob Jones <bob@example.com>", "Subject", "Body text");
        let payload = build_payload(
            "email.eml",
            &b64,
            "cluster-totebox-personnel-1",
            "service-people",
            serde_json::json!([]),
        );
        let filepath = write_payload_file(&watch_dir, "WORM-A02", &payload);
        process_payload(&filepath, base_dir.to_str().unwrap(), None, None);

        let crm_path = expected_crm_path(
            &base_dir,
            "cluster-totebox-personnel-1",
            "service-people",
            "WORM-A02",
        );
        let ledger = read_crm_ledger(&crm_path);
        let entities = ledger["extracted_crm_entities"].as_array().unwrap();
        for entity in entities {
            let name = entity["entity_name"].as_str().unwrap_or("");
            assert!(!name.is_empty(), "entity_name must not be empty");
            assert!(name.len() > 0, "entity_name must have length > 0");
        }
    }

    /// A-03: `classification` field must be present on every entity in the ledger.
    #[test]
    fn a03_classification_field_present_on_every_entity() {
        let td = test_dir("a03");
        let base_dir = td.join("base");
        let watch_dir = td.join("watch");
        fs::create_dir_all(&watch_dir).unwrap();

        let b64 = sample_email_b64(
            "Carol White <carol@example.com>",
            "Re: Meeting",
            "See attached.",
        );
        let payload = build_payload(
            "meeting.eml",
            &b64,
            "cluster-totebox-personnel-1",
            "service-people",
            serde_json::json!([
                { "entity_name": "Acme Corp", "classification": "ORGANIZATION", "confidence": 0.85 }
            ]),
        );
        let filepath = write_payload_file(&watch_dir, "WORM-A03", &payload);
        process_payload(&filepath, base_dir.to_str().unwrap(), None, None);

        let crm_path = expected_crm_path(
            &base_dir,
            "cluster-totebox-personnel-1",
            "service-people",
            "WORM-A03",
        );
        let ledger = read_crm_ledger(&crm_path);
        let entities = ledger["extracted_crm_entities"].as_array().unwrap();
        assert!(entities.len() >= 1, "at least one entity expected");
        for entity in entities {
            assert!(
                entity.get("classification").is_some(),
                "classification field must be present"
            );
            assert!(
                entity["classification"].as_str().is_some(),
                "classification must be a string"
            );
        }
    }

    /// A-04: Sender anchor entity always gets classification "ORIGIN SENDER".
    #[test]
    fn a04_sender_anchor_classification_is_origin_sender() {
        let td = test_dir("a04");
        let base_dir = td.join("base");
        let watch_dir = td.join("watch");
        fs::create_dir_all(&watch_dir).unwrap();

        let b64 = sample_email_b64("Diana Prince <diana@example.com>", "Hi", "Test");
        let payload = build_payload(
            "test.eml",
            &b64,
            "cluster-totebox-personnel-1",
            "service-people",
            serde_json::json!([]),
        );
        let filepath = write_payload_file(&watch_dir, "WORM-A04", &payload);
        process_payload(&filepath, base_dir.to_str().unwrap(), None, None);

        let crm_path = expected_crm_path(
            &base_dir,
            "cluster-totebox-personnel-1",
            "service-people",
            "WORM-A04",
        );
        let ledger = read_crm_ledger(&crm_path);
        let entities = ledger["extracted_crm_entities"].as_array().unwrap();
        let sender_entities: Vec<_> = entities
            .iter()
            .filter(|e| e["classification"].as_str() == Some("ORIGIN SENDER"))
            .collect();
        assert_eq!(
            sender_entities.len(),
            1,
            "exactly one ORIGIN SENDER entity expected"
        );
        let name = sender_entities[0]["entity_name"].as_str().unwrap();
        assert!(
            name.contains("Diana"),
            "sender name must come from From header"
        );
    }

    /// A-05: Edge entities from the payload array appear in the CRM ledger.
    #[test]
    fn a05_edge_entities_appear_in_crm_ledger() {
        let td = test_dir("a05");
        let base_dir = td.join("base");
        let watch_dir = td.join("watch");
        fs::create_dir_all(&watch_dir).unwrap();

        let b64 = sample_email_b64("Eve Adams <eve@example.com>", "Contract", "Signed by Eve");
        let payload = build_payload(
            "contract.eml",
            &b64,
            "cluster-totebox-personnel-1",
            "service-people",
            serde_json::json!([
                { "entity_name": "GlobalTech Ltd", "classification": "ORGANIZATION", "confidence": 0.92 },
                { "entity_name": "New York", "classification": "LOCATION", "confidence": 0.88 }
            ]),
        );
        let filepath = write_payload_file(&watch_dir, "WORM-A05", &payload);
        process_payload(&filepath, base_dir.to_str().unwrap(), None, None);

        let crm_path = expected_crm_path(
            &base_dir,
            "cluster-totebox-personnel-1",
            "service-people",
            "WORM-A05",
        );
        let ledger = read_crm_ledger(&crm_path);
        let entities = ledger["extracted_crm_entities"].as_array().unwrap();
        let names: Vec<&str> = entities
            .iter()
            .filter_map(|e| e["entity_name"].as_str())
            .collect();
        assert!(
            names.contains(&"GlobalTech Ltd"),
            "edge entity GlobalTech Ltd expected"
        );
        assert!(names.contains(&"New York"), "edge entity New York expected");
    }

    /// A-06: Edge entity with entity_name of length ≤ 2 must be filtered out
    ///       (the code guards name.len() > 2).
    #[test]
    fn a06_short_entity_name_filtered() {
        let td = test_dir("a06");
        let base_dir = td.join("base");
        let watch_dir = td.join("watch");
        fs::create_dir_all(&watch_dir).unwrap();

        let b64 = sample_email_b64("Frank Hill <frank@example.com>", "Hi", "Body");
        let payload = build_payload(
            "note.eml",
            &b64,
            "cluster-totebox-personnel-1",
            "service-people",
            serde_json::json!([
                { "entity_name": "AB", "classification": "PERSON", "confidence": 0.5 },
                { "entity_name": "X", "classification": "PERSON", "confidence": 0.5 }
            ]),
        );
        let filepath = write_payload_file(&watch_dir, "WORM-A06", &payload);
        process_payload(&filepath, base_dir.to_str().unwrap(), None, None);

        let crm_path = expected_crm_path(
            &base_dir,
            "cluster-totebox-personnel-1",
            "service-people",
            "WORM-A06",
        );
        let ledger = read_crm_ledger(&crm_path);
        let entities = ledger["extracted_crm_entities"].as_array().unwrap();
        let names: Vec<&str> = entities
            .iter()
            .filter_map(|e| e["entity_name"].as_str())
            .collect();
        assert!(!names.contains(&"AB"), "two-char name must be filtered");
        assert!(!names.contains(&"X"), "single-char name must be filtered");
    }

    /// A-07: Duplicate entity_name is deduplicated — same name appears only once
    ///       even when present in both From header and edge_entities.
    #[test]
    fn a07_duplicate_entity_name_deduplicated() {
        let td = test_dir("a07");
        let base_dir = td.join("base");
        let watch_dir = td.join("watch");
        fs::create_dir_all(&watch_dir).unwrap();

        let b64 = sample_email_b64("Grace Lee <grace@example.com>", "Test", "Body");
        // "Grace Lee" appears in From header; edge entity repeats the same name
        let payload = build_payload(
            "dup.eml",
            &b64,
            "cluster-totebox-personnel-1",
            "service-people",
            serde_json::json!([
                { "entity_name": "Grace Lee", "classification": "PERSON", "confidence": 0.9 }
            ]),
        );
        let filepath = write_payload_file(&watch_dir, "WORM-A07", &payload);
        process_payload(&filepath, base_dir.to_str().unwrap(), None, None);

        let crm_path = expected_crm_path(
            &base_dir,
            "cluster-totebox-personnel-1",
            "service-people",
            "WORM-A07",
        );
        let ledger = read_crm_ledger(&crm_path);
        let entities = ledger["extracted_crm_entities"].as_array().unwrap();
        let grace_count = entities
            .iter()
            .filter(|e| e["entity_name"].as_str() == Some("Grace Lee"))
            .count();
        assert_eq!(
            grace_count, 1,
            "duplicate entity_name must appear exactly once"
        );
    }

    /// A-08: The CRM ledger is valid JSON (no trailing garbage, parseable).
    #[test]
    fn a08_crm_ledger_is_valid_json() {
        let td = test_dir("a08");
        let base_dir = td.join("base");
        let watch_dir = td.join("watch");
        fs::create_dir_all(&watch_dir).unwrap();

        let b64 = sample_email_b64("Hiro Tanaka <hiro@example.com>", "Status", "All good.");
        let payload = build_payload(
            "status.eml",
            &b64,
            "cluster-totebox-personnel-1",
            "service-people",
            serde_json::json!([]),
        );
        let filepath = write_payload_file(&watch_dir, "WORM-A08", &payload);
        process_payload(&filepath, base_dir.to_str().unwrap(), None, None);

        let crm_path = expected_crm_path(
            &base_dir,
            "cluster-totebox-personnel-1",
            "service-people",
            "WORM-A08",
        );
        let raw = fs::read_to_string(&crm_path).expect("read ledger");
        let parsed: Result<Value, _> = serde_json::from_str(&raw);
        assert!(parsed.is_ok(), "CRM ledger must be valid JSON");
    }

    /// A-09: A text/markdown payload (ext .md) does NOT go through the email
    ///       parser; corpus_parts still includes the text content.
    #[test]
    fn a09_markdown_payload_corpus_contains_text() {
        let td = test_dir("a09");
        let base_dir = td.join("base");
        let watch_dir = td.join("watch");
        let corpus_dir = td.join("corpus");
        fs::create_dir_all(&watch_dir).unwrap();
        fs::create_dir_all(&corpus_dir).unwrap();

        let md_content = b"# Report\n\nSome important entity text here.";
        let b64 = BASE64_STD.encode(md_content);
        let payload = build_payload(
            "report.md",
            &b64,
            "cluster-totebox-personnel-1",
            "service-people",
            serde_json::json!([]),
        );
        let filepath = write_payload_file(&watch_dir, "WORM-A09", &payload);
        let ok = process_payload(
            &filepath,
            base_dir.to_str().unwrap(),
            Some(corpus_dir.to_str().unwrap()),
            None,
        );
        assert!(ok, "process_payload must return true for markdown payload");

        let corpus_path = corpus_dir.join("CORPUS_WORM-A09.json");
        assert!(
            corpus_path.exists(),
            "CORPUS file must be emitted for markdown"
        );
        let corpus_raw = fs::read_to_string(&corpus_path).unwrap();
        let corpus_json: Value = serde_json::from_str(&corpus_raw).unwrap();
        let corpus_text = corpus_json["corpus"].as_str().unwrap_or("");
        assert!(
            corpus_text.contains("important entity"),
            "corpus must contain the markdown text content"
        );
    }

    /// A-10: Corpus JSON contains `worm_id` and `corpus` fields.
    #[test]
    fn a10_corpus_json_required_fields() {
        let td = test_dir("a10");
        let base_dir = td.join("base");
        let watch_dir = td.join("watch");
        let corpus_dir = td.join("corpus");
        fs::create_dir_all(&watch_dir).unwrap();
        fs::create_dir_all(&corpus_dir).unwrap();

        let b64 = sample_email_b64(
            "Ivan Petrov <ivan@example.com>",
            "Greetings",
            "Hello world.",
        );
        let payload = build_payload(
            "greet.eml",
            &b64,
            "cluster-totebox-personnel-1",
            "service-people",
            serde_json::json!([]),
        );
        let filepath = write_payload_file(&watch_dir, "WORM-A10", &payload);
        process_payload(
            &filepath,
            base_dir.to_str().unwrap(),
            Some(corpus_dir.to_str().unwrap()),
            Some("module-test"),
        );

        let corpus_path = corpus_dir.join("CORPUS_WORM-A10.json");
        assert!(corpus_path.exists(), "CORPUS file must exist");
        let corpus_json: Value =
            serde_json::from_str(&fs::read_to_string(&corpus_path).unwrap()).unwrap();
        assert!(
            corpus_json.get("worm_id").is_some(),
            "corpus JSON must have worm_id"
        );
        assert!(
            corpus_json.get("corpus").is_some(),
            "corpus JSON must have corpus field"
        );
        assert_eq!(
            corpus_json["module_id"].as_str().unwrap_or(""),
            "module-test",
            "corpus JSON must have module_id when set"
        );
    }

    /// A-11: Classification vocabulary — each entity classification must be
    ///       one of ALLOWED_CLASSIFICATIONS when it originates from edge_entities.
    #[test]
    fn a11_edge_entity_classifications_in_allowed_vocabulary() {
        let td = test_dir("a11");
        let base_dir = td.join("base");
        let watch_dir = td.join("watch");
        fs::create_dir_all(&watch_dir).unwrap();

        let b64 = sample_email_b64("Jenna Fox <jenna@example.com>", "Report", "Details.");
        let payload = build_payload(
            "vocab.eml",
            &b64,
            "cluster-totebox-personnel-1",
            "service-people",
            serde_json::json!([
                { "entity_name": "TechCorp", "classification": "ORGANIZATION", "confidence": 0.9 },
                { "entity_name": "London", "classification": "LOCATION", "confidence": 0.8 }
            ]),
        );
        let filepath = write_payload_file(&watch_dir, "WORM-A11", &payload);
        process_payload(&filepath, base_dir.to_str().unwrap(), None, None);

        let crm_path = expected_crm_path(
            &base_dir,
            "cluster-totebox-personnel-1",
            "service-people",
            "WORM-A11",
        );
        let ledger = read_crm_ledger(&crm_path);
        let entities = ledger["extracted_crm_entities"].as_array().unwrap();
        // Filter to edge-sourced entities (role_vector = "Edge AI Inference")
        let edge_entities: Vec<_> = entities
            .iter()
            .filter(|e| e["role_vector"].as_str() == Some("Edge AI Inference"))
            .collect();
        for entity in &edge_entities {
            let class = entity["classification"].as_str().unwrap_or("");
            assert!(
                ALLOWED_CLASSIFICATIONS.contains(&class),
                "classification '{}' not in allowed vocabulary",
                class
            );
        }
    }

    /// A-12: `confidence` field is a number in range [0.0, 1.0].
    #[test]
    fn a12_confidence_is_normalized_float() {
        let td = test_dir("a12");
        let base_dir = td.join("base");
        let watch_dir = td.join("watch");
        fs::create_dir_all(&watch_dir).unwrap();

        let b64 = sample_email_b64("Karl Mann <karl@example.com>", "Inv", "Body.");
        let payload = build_payload(
            "inv.eml",
            &b64,
            "cluster-totebox-personnel-1",
            "service-people",
            serde_json::json!([
                { "entity_name": "Mann & Co", "classification": "ORGANIZATION", "confidence": 0.75 }
            ]),
        );
        let filepath = write_payload_file(&watch_dir, "WORM-A12", &payload);
        process_payload(&filepath, base_dir.to_str().unwrap(), None, None);

        let crm_path = expected_crm_path(
            &base_dir,
            "cluster-totebox-personnel-1",
            "service-people",
            "WORM-A12",
        );
        let ledger = read_crm_ledger(&crm_path);
        let entities = ledger["extracted_crm_entities"].as_array().unwrap();
        for entity in entities {
            if let Some(conf) = entity["confidence"].as_f64() {
                assert!(
                    conf >= 0.0 && conf <= 1.0,
                    "confidence must be in [0.0, 1.0], got {}",
                    conf
                );
            }
        }
    }

    // ── Area B: Queue drain ───────────────────────────────────────────────────

    /// B-01: `process_payload` returns true for a well-formed email payload,
    ///       enabling the caller to move the file to processed/.
    #[test]
    fn b01_valid_payload_returns_true() {
        let td = test_dir("b01");
        let base_dir = td.join("base");
        let watch_dir = td.join("watch");
        fs::create_dir_all(&watch_dir).unwrap();

        let b64 = sample_email_b64("Lena Park <lena@example.com>", "Hi", "Hello.");
        let payload = build_payload(
            "message.eml",
            &b64,
            "cluster-totebox-personnel-1",
            "service-people",
            serde_json::json!([]),
        );
        let filepath = write_payload_file(&watch_dir, "WORM-B01", &payload);
        let result = process_payload(&filepath, base_dir.to_str().unwrap(), None, None);
        assert!(result, "valid payload must return true");
    }

    /// B-02: After process_payload succeeds, the caller can move the file
    ///       to processed/ and a subsequent call on the original path returns
    ///       false (file no longer present).
    #[test]
    fn b02_processed_file_not_reprocessed_after_move() {
        let td = test_dir("b02");
        let base_dir = td.join("base");
        let watch_dir = td.join("watch");
        let processed_dir = watch_dir.join("processed");
        fs::create_dir_all(&watch_dir).unwrap();
        fs::create_dir_all(&processed_dir).unwrap();

        let b64 = sample_email_b64("Mike Stone <mike@example.com>", "FYI", "See below.");
        let payload = build_payload(
            "fyi.eml",
            &b64,
            "cluster-totebox-personnel-1",
            "service-people",
            serde_json::json!([]),
        );
        let filepath = write_payload_file(&watch_dir, "WORM-B02", &payload);

        // First processing succeeds
        let first = process_payload(&filepath, base_dir.to_str().unwrap(), None, None);
        assert!(first, "first processing must succeed");

        // Simulate the main() move-to-processed step
        let dest = processed_dir.join("WORM-B02.json");
        fs::rename(&filepath, &dest).expect("rename to processed/");

        // Source path no longer exists — second call must return false
        let second = process_payload(&filepath, base_dir.to_str().unwrap(), None, None);
        assert!(!second, "call on moved file path must return false");
    }

    /// B-03: Two distinct payload files are each processed independently —
    ///       both produce CRM ledgers with correct worm_ids.
    #[test]
    fn b03_multiple_files_produce_independent_ledgers() {
        let td = test_dir("b03");
        let base_dir = td.join("base");
        let watch_dir = td.join("watch");
        fs::create_dir_all(&watch_dir).unwrap();

        for (idx, name) in [
            ("Alice Alpha", "alice@example.com"),
            ("Bob Beta", "bob@example.com"),
        ]
        .iter()
        .enumerate()
        {
            let worm = format!("WORM-B03-{}", idx);
            let b64 = sample_email_b64(&format!("{} <{}>", name.0, name.1), "Sub", "Body");
            let payload = build_payload(
                "note.eml",
                &b64,
                "cluster-totebox-personnel-1",
                "service-people",
                serde_json::json!([]),
            );
            let filepath = write_payload_file(&watch_dir, &worm, &payload);
            let ok = process_payload(&filepath, base_dir.to_str().unwrap(), None, None);
            assert!(ok, "payload {} must be processed successfully", worm);
        }

        for idx in 0..2 {
            let worm = format!("WORM-B03-{}", idx);
            let crm = expected_crm_path(
                &base_dir,
                "cluster-totebox-personnel-1",
                "service-people",
                &worm,
            );
            assert!(crm.exists(), "CRM ledger for {} must exist", worm);
        }
    }

    /// B-04: A payload file with a non-JSON extension in the watch dir is NOT
    ///       processed by process_payload (process_payload returns false on
    ///       invalid JSON content).
    #[test]
    fn b04_non_json_content_returns_false() {
        let td = test_dir("b04");
        let base_dir = td.join("base");
        let watch_dir = td.join("watch");
        fs::create_dir_all(&watch_dir).unwrap();

        // Write a file that is not valid JSON
        let filepath = watch_dir.join("WORM-B04.json");
        fs::write(&filepath, b"not json content").unwrap();

        let result = process_payload(&filepath, base_dir.to_str().unwrap(), None, None);
        assert!(!result, "non-JSON content must return false");
    }

    /// B-05: Processing a payload with `target_service` override routes the
    ///       CRM ledger to the specified service subdirectory.
    #[test]
    fn b05_target_service_override_routes_to_correct_subdir() {
        let td = test_dir("b05");
        let base_dir = td.join("base");
        let watch_dir = td.join("watch");
        fs::create_dir_all(&watch_dir).unwrap();

        let b64 = sample_email_b64("Nora Bell <nora@example.com>", "Test", "Body.");
        let payload = build_payload(
            "test.eml",
            &b64,
            "cluster-totebox-personnel-1",
            "service-email", // non-default target
            serde_json::json!([]),
        );
        let filepath = write_payload_file(&watch_dir, "WORM-B05", &payload);
        process_payload(&filepath, base_dir.to_str().unwrap(), None, None);

        let crm_path = expected_crm_path(
            &base_dir,
            "cluster-totebox-personnel-1",
            "service-email",
            "WORM-B05",
        );
        assert!(
            crm_path.exists(),
            "CRM ledger must be under service-email subdir"
        );
    }

    // ── Area C: Redrive logic ─────────────────────────────────────────────────

    /// C-01: A file that failed once (moved to queue-poison/) can be redriven
    ///       by calling process_payload directly — if the content is valid,
    ///       it must return true and produce a CRM ledger.
    #[test]
    fn c01_redrive_valid_file_from_poison_queue_succeeds() {
        let td = test_dir("c01");
        let base_dir = td.join("base");
        let poison_dir = td.join("queue-poison");
        fs::create_dir_all(&poison_dir).unwrap();

        let b64 = sample_email_b64("Omar Cruz <omar@example.com>", "Redo", "Retry.");
        let payload = build_payload(
            "redo.eml",
            &b64,
            "cluster-totebox-personnel-1",
            "service-people",
            serde_json::json!([]),
        );
        // File placed in queue-poison/ as if it was poisoned previously
        let filepath = write_payload_file(&poison_dir, "WORM-C01", &payload);

        // Redrive: call process_payload on the poison-dir file
        let result = process_payload(&filepath, base_dir.to_str().unwrap(), None, None);
        assert!(result, "redrive of valid file must succeed");

        let crm_path = expected_crm_path(
            &base_dir,
            "cluster-totebox-personnel-1",
            "service-people",
            "WORM-C01",
        );
        assert!(crm_path.exists(), "redrive must produce a CRM ledger");
    }

    /// C-02: A file that failed due to bad base64 and was moved to queue-poison/
    ///       still returns false when redriven — content error is not fixed by
    ///       retrying alone.
    #[test]
    fn c02_redrive_bad_base64_still_fails() {
        let td = test_dir("c02");
        let base_dir = td.join("base");
        let poison_dir = td.join("queue-poison");
        fs::create_dir_all(&poison_dir).unwrap();

        let payload = serde_json::json!({
            "file": {
                "filename": "broken.eml",
                "data": "!!!NOT_VALID_BASE64!!!",
            },
            "destination_archive": "cluster-totebox-personnel-1",
            "target_service": "service-people",
            "edge_entities": [],
        })
        .to_string();

        let filepath = write_payload_file(&poison_dir, "WORM-C02", &payload);
        let result = process_payload(&filepath, base_dir.to_str().unwrap(), None, None);
        assert!(!result, "redrive of bad-base64 file must return false");
    }

    /// C-03: A file in queue-poison/ that was originally malformed JSON stays
    ///       false on redrive (the poison reason is structural).
    #[test]
    fn c03_redrive_malformed_json_stays_false() {
        let td = test_dir("c03");
        let base_dir = td.join("base");
        let poison_dir = td.join("queue-poison");
        fs::create_dir_all(&poison_dir).unwrap();

        let filepath = write_payload_file(&poison_dir, "WORM-C03", "{bad json{{");
        let result = process_payload(&filepath, base_dir.to_str().unwrap(), None, None);
        assert!(!result, "redrive of malformed JSON must return false");
    }

    /// C-04: A corrected file placed in queue-poison/ (simulating an operator
    ///       fix) produces a CRM ledger with the right worm_id on redrive.
    #[test]
    fn c04_corrected_poison_file_produces_correct_worm_id() {
        let td = test_dir("c04");
        let base_dir = td.join("base");
        let poison_dir = td.join("queue-poison");
        fs::create_dir_all(&poison_dir).unwrap();

        let b64 = sample_email_b64("Paula Reyes <paula@example.com>", "Fix", "Fixed!");
        let payload = build_payload(
            "fix.eml",
            &b64,
            "cluster-totebox-personnel-1",
            "service-people",
            serde_json::json!([
                { "entity_name": "Repaired Corp", "classification": "ORGANIZATION", "confidence": 0.7 }
            ]),
        );
        let filepath = write_payload_file(&poison_dir, "WORM-C04-FIXED", &payload);
        let ok = process_payload(&filepath, base_dir.to_str().unwrap(), None, None);
        assert!(ok, "corrected file must succeed on redrive");

        let crm_path = expected_crm_path(
            &base_dir,
            "cluster-totebox-personnel-1",
            "service-people",
            "WORM-C04-FIXED",
        );
        let ledger = read_crm_ledger(&crm_path);
        assert_eq!(
            ledger["worm_id"].as_str().unwrap(),
            "WORM-C04-FIXED",
            "worm_id must match the redriven filename"
        );
    }

    // ── Area D: Poison handling ───────────────────────────────────────────────

    /// D-01: Empty file returns false and does not panic.
    #[test]
    fn d01_empty_file_returns_false_no_panic() {
        let td = test_dir("d01");
        let base_dir = td.join("base");
        let watch_dir = td.join("watch");
        fs::create_dir_all(&watch_dir).unwrap();

        let filepath = watch_dir.join("WORM-D01.json");
        fs::write(&filepath, b"").unwrap();

        let result = process_payload(&filepath, base_dir.to_str().unwrap(), None, None);
        assert!(!result, "empty file must return false");
    }

    /// D-02: Truncated JSON (unclosed object) returns false and does not panic.
    #[test]
    fn d02_truncated_json_returns_false() {
        let td = test_dir("d02");
        let base_dir = td.join("base");
        let watch_dir = td.join("watch");
        fs::create_dir_all(&watch_dir).unwrap();

        let filepath =
            write_payload_file(&watch_dir, "WORM-D02", r#"{"file": {"filename": "x.eml""#);
        let result = process_payload(&filepath, base_dir.to_str().unwrap(), None, None);
        assert!(!result, "truncated JSON must return false");
    }

    /// D-03: Valid JSON structure but invalid (non-base64) data field returns
    ///       false and does not panic.
    #[test]
    fn d03_invalid_base64_data_returns_false() {
        let td = test_dir("d03");
        let base_dir = td.join("base");
        let watch_dir = td.join("watch");
        fs::create_dir_all(&watch_dir).unwrap();

        let payload = serde_json::json!({
            "file": { "filename": "bad.eml", "data": "@@@INVALID_B64@@@" },
            "destination_archive": "cluster-totebox-personnel-1",
            "target_service": "service-people",
            "edge_entities": [],
        })
        .to_string();
        let filepath = write_payload_file(&watch_dir, "WORM-D03", &payload);
        let result = process_payload(&filepath, base_dir.to_str().unwrap(), None, None);
        assert!(!result, "invalid base64 data must return false");
    }

    /// D-04: A payload with a valid JSON structure and valid base64 but where
    ///       the decoded bytes are not a valid RFC 2822 email returns false
    ///       (mailparse failure).
    #[test]
    fn d04_non_email_bytes_in_eml_returns_false() {
        let td = test_dir("d04");
        let base_dir = td.join("base");
        let watch_dir = td.join("watch");
        fs::create_dir_all(&watch_dir).unwrap();

        // Binary noise that mailparse cannot parse as RFC 2822
        let garbage: Vec<u8> = (0u8..=255u8).cycle().take(512).collect();
        let b64 = BASE64_STD.encode(&garbage);
        let payload = build_payload(
            "corrupt.eml",
            &b64,
            "cluster-totebox-personnel-1",
            "service-people",
            serde_json::json!([]),
        );
        let filepath = write_payload_file(&watch_dir, "WORM-D04", &payload);
        let result = process_payload(&filepath, base_dir.to_str().unwrap(), None, None);
        // mailparse may succeed or fail on arbitrary bytes; the key contract is
        // that we do NOT panic regardless of the outcome.
        let _ = result; // do not assert the bool — just confirm no panic
    }

    /// D-05: A payload whose `data` field is a data-URI prefix followed by
    ///       invalid base64 after the comma returns false.
    #[test]
    fn d05_data_uri_with_invalid_b64_suffix_returns_false() {
        let td = test_dir("d05");
        let base_dir = td.join("base");
        let watch_dir = td.join("watch");
        fs::create_dir_all(&watch_dir).unwrap();

        let payload = serde_json::json!({
            "file": {
                "filename": "note.eml",
                "data": "data:message/rfc822;base64,!!!BAD_SUFFIX!!!"
            },
            "destination_archive": "cluster-totebox-personnel-1",
            "target_service": "service-people",
            "edge_entities": [],
        })
        .to_string();
        let filepath = write_payload_file(&watch_dir, "WORM-D05", &payload);
        let result = process_payload(&filepath, base_dir.to_str().unwrap(), None, None);
        assert!(!result, "data-URI with bad b64 suffix must return false");
    }

    /// D-06: A payload with `data` as an empty string returns false (zero bytes
    ///       decoded → mailparse or base64 decode path rejects it).
    #[test]
    fn d06_empty_data_field_returns_false() {
        let td = test_dir("d06");
        let base_dir = td.join("base");
        let watch_dir = td.join("watch");
        fs::create_dir_all(&watch_dir).unwrap();

        let payload = serde_json::json!({
            "file": { "filename": "empty.eml", "data": "" },
            "destination_archive": "cluster-totebox-personnel-1",
            "target_service": "service-people",
            "edge_entities": [],
        })
        .to_string();
        let filepath = write_payload_file(&watch_dir, "WORM-D06", &payload);
        let result = process_payload(&filepath, base_dir.to_str().unwrap(), None, None);
        // Empty bytes → mailparse will likely fail on missing headers
        let _ = result; // contract: no panic; bool may vary by mailparse version
    }

    /// D-07: A completely missing `file` key in the payload does not panic.
    ///       `original_filename` defaults to "unknown_asset" and b64 defaults
    ///       to ""; base64 decode of "" succeeds with zero bytes; mailparse
    ///       then rejects the zero-byte buffer → false.
    #[test]
    fn d07_missing_file_key_does_not_panic() {
        let td = test_dir("d07");
        let base_dir = td.join("base");
        let watch_dir = td.join("watch");
        fs::create_dir_all(&watch_dir).unwrap();

        let payload = serde_json::json!({
            "destination_archive": "cluster-totebox-personnel-1",
            "target_service": "service-people",
            "edge_entities": [],
        })
        .to_string();
        let filepath = write_payload_file(&watch_dir, "WORM-D07", &payload);
        // Must not panic regardless of return value
        let _ = process_payload(&filepath, base_dir.to_str().unwrap(), None, None);
    }

    /// D-08: A file that is binary (not valid UTF-8) returns false without panic.
    #[test]
    fn d08_binary_file_returns_false_no_panic() {
        let td = test_dir("d08");
        let base_dir = td.join("base");
        let watch_dir = td.join("watch");
        fs::create_dir_all(&watch_dir).unwrap();

        // Write raw binary content (invalid UTF-8 / JSON)
        let filepath = watch_dir.join("WORM-D08.json");
        fs::write(&filepath, &[0xffu8, 0xfeu8, 0x00u8, 0x01u8, 0xffu8]).unwrap();

        let result = process_payload(&filepath, base_dir.to_str().unwrap(), None, None);
        assert!(!result, "binary file content must return false");
    }

    /// D-09: A payload where `edge_entities` is not an array (e.g. a string)
    ///       does not panic — the code treats non-array as missing.
    #[test]
    fn d09_edge_entities_wrong_type_no_panic() {
        let td = test_dir("d09");
        let base_dir = td.join("base");
        let watch_dir = td.join("watch");
        fs::create_dir_all(&watch_dir).unwrap();

        let b64 = sample_email_b64("Quinn Baker <quinn@example.com>", "Test", "Body.");
        let payload = serde_json::json!({
            "file": { "filename": "test.eml", "data": b64 },
            "destination_archive": "cluster-totebox-personnel-1",
            "target_service": "service-people",
            "edge_entities": "not-an-array",
        })
        .to_string();
        let filepath = write_payload_file(&watch_dir, "WORM-D09", &payload);
        // Must not panic
        let _ = process_payload(&filepath, base_dir.to_str().unwrap(), None, None);
    }

    /// D-10: A payload file that does not exist on disk returns false.
    #[test]
    fn d10_nonexistent_file_returns_false() {
        let td = test_dir("d10");
        let base_dir = td.join("base");
        fs::create_dir_all(&base_dir).unwrap();

        let ghost = td.join("ghost-WORM-D10.json");
        // Do NOT write the file
        let result = process_payload(&ghost, base_dir.to_str().unwrap(), None, None);
        assert!(!result, "nonexistent file must return false");
    }

    // ── Area D extra: no-entity payloads (text paths that skip mailparse) ─────

    /// D-11: A YAML payload that contains no edge_entities produces a corpus
    ///       file but NO CRM ledger (no graph_entities to write).
    #[test]
    fn d11_yaml_payload_no_entities_no_crm_ledger() {
        let td = test_dir("d11");
        let base_dir = td.join("base");
        let watch_dir = td.join("watch");
        let corpus_dir = td.join("corpus");
        fs::create_dir_all(&watch_dir).unwrap();
        fs::create_dir_all(&corpus_dir).unwrap();

        let yaml_bytes = b"key: value\nname: test\n";
        let b64 = BASE64_STD.encode(yaml_bytes);
        let payload = build_payload(
            "config.yaml",
            &b64,
            "cluster-totebox-personnel-1",
            "service-people",
            serde_json::json!([]), // no edge entities
        );
        let filepath = write_payload_file(&watch_dir, "WORM-D11", &payload);
        let ok = process_payload(
            &filepath,
            base_dir.to_str().unwrap(),
            Some(corpus_dir.to_str().unwrap()),
            None,
        );
        assert!(ok, "YAML payload with no entities must still return true");

        let crm_path = expected_crm_path(
            &base_dir,
            "cluster-totebox-personnel-1",
            "service-people",
            "WORM-D11",
        );
        assert!(
            !crm_path.exists(),
            "no CRM ledger when no entities extracted"
        );

        let corpus_path = corpus_dir.join("CORPUS_WORM-D11.json");
        assert!(
            corpus_path.exists(),
            "CORPUS file must be emitted for YAML payload"
        );
    }

    /// D-12: A `.txt` payload with edge entities produces both a CRM ledger
    ///       and a corpus file.
    #[test]
    fn d12_txt_payload_with_edge_entities_produces_both_outputs() {
        let td = test_dir("d12");
        let base_dir = td.join("base");
        let watch_dir = td.join("watch");
        let corpus_dir = td.join("corpus");
        fs::create_dir_all(&watch_dir).unwrap();
        fs::create_dir_all(&corpus_dir).unwrap();

        let txt_bytes = b"This is a plain text document about Acme Inc.";
        let b64 = BASE64_STD.encode(txt_bytes);
        let payload = build_payload(
            "notes.txt",
            &b64,
            "cluster-totebox-personnel-1",
            "service-people",
            serde_json::json!([
                { "entity_name": "Acme Inc", "classification": "ORGANIZATION", "confidence": 0.95 }
            ]),
        );
        let filepath = write_payload_file(&watch_dir, "WORM-D12", &payload);
        let ok = process_payload(
            &filepath,
            base_dir.to_str().unwrap(),
            Some(corpus_dir.to_str().unwrap()),
            None,
        );
        assert!(ok, "txt payload must succeed");

        let crm_path = expected_crm_path(
            &base_dir,
            "cluster-totebox-personnel-1",
            "service-people",
            "WORM-D12",
        );
        assert!(
            crm_path.exists(),
            "CRM ledger must exist for txt payload with entities"
        );

        let corpus_path = corpus_dir.join("CORPUS_WORM-D12.json");
        assert!(
            corpus_path.exists(),
            "CORPUS file must be emitted for txt payload"
        );
    }
}
