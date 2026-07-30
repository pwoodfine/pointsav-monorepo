use std::env;
use std::fs;
use reqwest::Client;
use serde_json::{json, Value};
use serde::{Serialize, Deserialize};

const SLM_SERVER_URL: &str = "http://127.0.0.1:8080/v1/chat/completions";

/// Unified contract for knowledge-graph items produced by cognitive-forge.
/// Written as JSON to service-content/knowledge-graph/{filename}.json for
/// downstream consumption by content-compiler per ARCHITECTURE.md §Ring 3a.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct KnowledgeItem {
    /// Source transaction ID from transient-queues
    pub source_id: String,
    /// Extracted objective facts as markdown bullet points
    pub extracted_content: String,
    /// LLM model used for extraction (e.g., "olmo-3-7b-instruct")
    pub model: String,
    /// Extraction timestamp (ISO 8601)
    pub extracted_at: String,
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("[ERROR] Usage: router <TOTEBOX_ROOT>");
        std::process::exit(1);
    }
    let totebox_root = &args[1];
    let queue_dir = format!("{}/service-slm/transient-queues", totebox_root);
    let output_dir = format!("{}/service-content/knowledge-graph", totebox_root);

    let client = Client::new();

    if let Ok(entries) = fs::read_dir(&queue_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().unwrap_or_default() == "txt" {
                if let Ok(content) = fs::read_to_string(&path) {
                    println!("[FORGE] Processing: {:?}", path.file_name().unwrap());
                    
                    let payload = json!({
                        "messages": [
                            {"role": "system", "content": "Extract the core objective facts from this text. Output clean markdown bullet points."},
                            {"role": "user", "content": content}
                        ],
                        "temperature": 0.2
                    });

                    if let Ok(res) = client.post(SLM_SERVER_URL).json(&payload).send().await {
                        if let Ok(json_res) = res.json::<Value>().await {
                            if let Some(extracted) = json_res["choices"][0]["message"]["content"].as_str() {
                                // Build KnowledgeItem with extracted content
                                let item = KnowledgeItem {
                                    source_id: path.file_stem().unwrap().to_string_lossy().to_string(),
                                    extracted_content: extracted.to_string(),
                                    model: "olmo-3-7b-instruct".to_string(),
                                    extracted_at: chrono::Utc::now().to_rfc3339(),
                                };

                                // Write as JSON to knowledge-graph
                                let out_path = format!("{}/{}.json", output_dir, path.file_stem().unwrap().to_string_lossy());
                                if let Ok(json_str) = serde_json::to_string_pretty(&item) {
                                    fs::write(&out_path, json_str).unwrap();
                                    fs::remove_file(&path).unwrap(); // Destroy raw asset after extraction
                                    println!("  -> [SUCCESS] Knowledge item written to {}", out_path);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
