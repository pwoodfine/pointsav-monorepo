use std::env;
use std::fs;
use regex::Regex;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("[ERROR] Usage: spatial-crm <TOTEBOX_ROOT>");
        std::process::exit(1);
    }
    
    let totebox_root = &args[1];
    let spatial_queue = format!("{}/service-people/spatial-queue", totebox_root);
    let slm_queue = format!("{}/service-slm/transient-queues", totebox_root);
    
    fs::create_dir_all(&spatial_queue).unwrap();
    fs::create_dir_all(&slm_queue).unwrap();

    let email_re = Regex::new(r"(?i)\b[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}\b").unwrap();
    let loc_re = Regex::new(r"(?i)\b(Vancouver|New York|Berlin|Madrid|Mexico City|London|Toronto|Seattle|San Francisco|Chicago|Dallas|Houston|Miami|Paris|Rome|Tokyo|Sydney|Jalisco|Virginia|Amsterdam)\b").unwrap();

    if let Ok(entries) = fs::read_dir(&spatial_queue) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().unwrap_or_default() == "txt" {
                let content = fs::read_to_string(&path).unwrap_or_default();
                let filename = path.file_name().unwrap().to_string_lossy();
                
                println!("[SPATIAL-CRM] Distilling payload: {}", filename);

                // 1. Deterministic Extraction
                let mut emails: Vec<&str> = email_re.find_iter(&content).map(|m| m.as_str()).collect();
                let mut locations: Vec<&str> = loc_re.find_iter(&content).map(|m| m.as_str()).collect();
                
                emails.sort(); emails.dedup();
                locations.sort(); locations.dedup();

                // 2. Forge the Reduced "Skeleton" for the SLM
                // We pass only the first 600 characters of the raw text to protect the 1GB RAM context limit
                let truncated_content: String = content.chars().take(600).collect();
                
                let skeleton = format!(
                    "---\n\
                    system_directive: \"Synthesize Taxonomy (Archetypes, Domains, Themes) from this Skeleton.\"\n\
                    extracted_identities: {:?}\n\
                    extracted_locations: {:?}\n\
                    ---\n\n\
                    [RAW CONTEXT SUMMARY]\n\
                    {}...",
                    emails, locations, truncated_content
                );

                // 3. Transmit Skeleton to the SLM Transient Queue
                let out_path = format!("{}/{}", slm_queue, filename);
                fs::write(&out_path, skeleton).expect("Failed to write Skeleton.");
                
                // 4. Destroy the heavy raw text file
                fs::remove_file(&path).unwrap();
                
                println!("  -> [SUCCESS] Extracted {} locations. Skeleton routed to Nano-SLM.", locations.len());
            }
        }
    }
}
