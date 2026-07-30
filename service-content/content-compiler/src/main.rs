use std::env;
use std::fs;
use chrono::Utc;

fn main() {
    println!("========================================================");
    println!(" 🗄️ CONTENT COMPILER: ONTOLOGICAL CLASSIFICATION");
    println!("========================================================");

    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("[ERROR] Usage: content-compiler <TOTEBOX_ROOT>");
        std::process::exit(1);
    }

    let totebox_root = &args[1];
    let graph_dir = format!("{}/service-content/knowledge-graph", totebox_root);
    let ledger_dir = format!("{}/service-content/verified-ledger", totebox_root);
    let ontology_dir = format!("{}/service-content/ontology/domains", totebox_root);

    let _ = fs::create_dir_all(&ledger_dir);

    // 1. Load the Ontological Glossaries into memory
    let mut domain_map: Vec<(String, String)> = Vec::new();
    
    if let Ok(entries) = fs::read_dir(&ontology_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("csv") {
                let domain_name = path.file_stem().unwrap().to_str().unwrap().replace("domain_", "").to_uppercase();
                if let Ok(content) = fs::read_to_string(&path) {
                    for line in content.lines().skip(1) { // Skip header
                        let parts: Vec<&str> = line.split(',').collect();
                        if !parts.is_empty() {
                            let term = parts[0].to_lowercase().trim().to_string();
                            if !term.is_empty() {
                                domain_map.push((term, domain_name.clone()));
                            }
                        }
                    }
                }
            }
        }
    }

    // 2. Process the Extracted JSON
    let paths = match fs::read_dir(&graph_dir) {
        Ok(p) => p,
        Err(_) => {
            println!("  -> [SYSTEM] The knowledge graph directory is mathematically empty.");
            return;
        }
    };
    
    let mut files_processed = 0;

    for path in paths {
        let file_path = path.unwrap().path();
        if file_path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }

        let filename = file_path.file_name().unwrap().to_str().unwrap();
        println!("  -> Compiling {}...", filename);

        let content = fs::read_to_string(&file_path).unwrap_or_default();
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
            let entity_name = json["primaryEntityName"].as_str().unwrap_or("UNKNOWN_ENTITY");
            let entity_type = json["entityType"].as_str().unwrap_or("UNKNOWN_TYPE");
            let summary = json["summary"].as_str().unwrap_or("No summary provided.");
            
            let summary_lower = summary.to_lowercase();
            let mut assigned_domain = "UNMAPPED_DOMAIN".to_string();

            // 3. Zero-Inference Ontological Cross-Reference
            for (term, domain) in &domain_map {
                if summary_lower.contains(term) {
                    assigned_domain = domain.clone();
                    println!("     [ONTOLOGY LOCK] Term '{}' mapped to Domain: {}", term, assigned_domain);
                    break;
                }
            }

            // 4. Institutional Brutalism Markdown Forge
            let now = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
            let safe_filename = entity_name.replace(" ", "_").replace("/", "-").to_uppercase();
            
            let markdown = format!(
"---
entity_id: {}
type: {}
domain: {}
status: VERIFIED
timestamp: {}
---

# {}

## I. EXECUTIVE SUMMARY
{}

---
*© {} Woodfine Management Corp. Verified by PointSav Sovereign Engine.*
", 
                safe_filename, entity_type.to_uppercase(), assigned_domain, now, 
                entity_name.to_uppercase(), 
                summary,
                Utc::now().format("%Y")
            );

            let out_path = format!("{}/{}.md", ledger_dir, safe_filename);
            let _ = fs::write(&out_path, markdown);
            
            println!("     [SUCCESS] Markdown Ledger Forged: {}.md", safe_filename);
            
            // Delete the raw JSON to maintain hygiene
            let _ = fs::remove_file(&file_path);
            files_processed += 1;
        }
    }
    
    if files_processed == 0 {
        println!("  -> [SYSTEM] The knowledge graph is mathematically empty.");
    }
    
    println!("--------------------------------------------------------");
    println!("[SUCCESS] Ontological Compilation complete.");
}
