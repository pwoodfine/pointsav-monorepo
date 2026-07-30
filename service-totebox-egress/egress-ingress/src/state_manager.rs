use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;

const CHECKPOINT_FILE: &str = "../data-ledgers/checkpoint.json";

pub fn save_checkpoint(next_link: &str) -> std::io::Result<()> {
    let mut file = File::create(CHECKPOINT_FILE)?;
    let payload = format!(r#"{{"nextLink": "{}"}}"#, next_link);
    file.write_all(payload.as_bytes())?;
    file.sync_all()?;
    Ok(())
}

pub fn load_checkpoint() -> Option<String> {
    if Path::new(CHECKPOINT_FILE).exists() {
        if let Ok(mut file) = File::open(CHECKPOINT_FILE) {
            let mut contents = String::new();
            if file.read_to_string(&mut contents).is_ok() {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&contents) {
                    if let Some(link) = json["nextLink"].as_str() {
                        return Some(link.to_string());
                    }
                }
            }
        }
    }
    None
}
