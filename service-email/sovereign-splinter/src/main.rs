use mailparse::*;
use std::env;
use std::fs;
use std::path::Path;
use uuid::Uuid;
use serde_json::json;
use chrono::Utc;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 { std::process::exit(1); }

    let file_path = &args[1];
    let totebox_root = &args[2];
    let filename = Path::new(file_path).file_name().unwrap_or_default().to_string_lossy().to_string();

    let raw_email = fs::read(file_path).expect("[ERROR] Failed to read immutable .eml file.");
    let parsed_mail = parse_mail(&raw_email).expect("[ERROR] Failed to parse MIME boundaries.");

    let route_slm = format!("{}/service-slm/transient-queues", totebox_root);
    let route_people = format!("{}/service-people/discovery-queue", totebox_root);
    let route_assets = format!("{}/assets/inert-media", totebox_root);

    fs::create_dir_all(&route_slm).unwrap();
    fs::create_dir_all(&route_people).unwrap();
    fs::create_dir_all(&route_assets).unwrap();

    let transaction_id = Uuid::new_v4().to_string()[..8].to_string().to_uppercase();

    let mut sender = String::new();
    let mut subject = String::new();
    let mut date_received = Utc::now().to_rfc3339();
    
    for header in parsed_mail.headers.iter() {
        let key = header.get_key().to_lowercase();
        // THE FIX: Aggressively strip carriage returns, newlines, and escape characters
        if key == "from" { sender = header.get_value().replace('\r', "").replace('\n', "").replace('\\', "").trim().to_string(); }
        if key == "subject" { subject = header.get_value().replace('\r', "").replace('\n', "").trim().to_string(); }
        if key == "date" { date_received = header.get_value().replace('\r', "").replace('\n', "").trim().to_string(); }
    }

    let identity_payload = json!({
        "transaction_id": transaction_id,
        "source_artifact": filename,
        "timestamp_extracted": Utc::now().to_rfc3339(),
        "timestamp_received": date_received,
        "spatial_vectors": {
            "sender": sender,
            "subject": subject
        },
        "state": "AWAITING_SPATIAL_RESOLUTION"
    });

    let identity_path = format!("{}/TX-{}_identity.json", route_people, transaction_id);
    fs::write(&identity_path, serde_json::to_string_pretty(&identity_payload).unwrap()).expect("Failed to route Identity Ledger");

    traverse_and_shatter(&parsed_mail, &transaction_id, &filename, &route_slm, &route_assets);
}

fn traverse_and_shatter(part: &ParsedMail, tx_id: &str, filename: &str, route_slm: &str, route_assets: &str) {
    let ctype = part.ctype.mimetype.clone();
    let disposition = part.get_content_disposition().disposition;
    
    if disposition != DispositionType::Attachment && (ctype == "text/plain" || ctype == "text/html") {
        let raw_body = part.get_body().unwrap_or_default();
        if !raw_body.trim().is_empty() {
            let clean_text = if ctype == "text/html" {
                html2text::from_read(raw_body.as_bytes(), 120)
            } else { raw_body };

            let truncated_body: String = clean_text.chars().take(2000).collect();

            let skeleton_payload = format!(
                "---\n\
                asset_id: \"TX-{}\"\n\
                source: \"{}\"\n\
                system_directive: \"Synthesize taxonomy.\"\n\
                status: \"PENDING_COGNITIVE_STATE\"\n\
                ---\n\n\
                {}", tx_id, filename, truncated_body
            );

            let out_path = format!("{}/TX-{}_skeleton.txt", route_slm, tx_id);
            fs::write(&out_path, skeleton_payload).expect("Failed to route to SLM Staging");
        }
    }

    if disposition == DispositionType::Attachment {
        let att_filename = part.get_content_disposition().params.get("filename").cloned().unwrap_or_else(|| format!("unnamed_{}.dat", tx_id));
        let binary_payload = part.get_body_raw().unwrap_or_default();
        let out_path = format!("{}/TX-{}_{}", route_assets, tx_id, att_filename);
        fs::write(&out_path, binary_payload).expect("Failed to write to Asset vault");
    }

    for subpart in &part.subparts {
        traverse_and_shatter(subpart, tx_id, filename, route_slm, route_assets);
    }
}
