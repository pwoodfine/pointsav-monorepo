// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

use serde_json::Value;
use std::{collections::HashMap, fs, path::Path};

pub fn load_tokens(
    design_system_dir: &Path,
) -> Result<HashMap<String, Value>, Box<dyn std::error::Error>> {
    let bim_dir = design_system_dir.join("tokens").join("bim");
    let mut map = HashMap::new();
    if !bim_dir.exists() {
        eprintln!("warn: BIM token dir not found: {}", bim_dir.display());
        return Ok(map);
    }
    for entry in fs::read_dir(&bim_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let stem = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .trim_end_matches(".dtcg.json")
            .to_string();
        let raw = fs::read_to_string(&path)?;
        match serde_json::from_str::<Value>(&raw) {
            Ok(v) => {
                map.insert(stem, v);
            }
            Err(e) => eprintln!("warn: failed to parse {}: {e}", path.display()),
        }
    }
    Ok(map)
}
