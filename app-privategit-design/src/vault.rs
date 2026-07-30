use std::{collections::HashMap, fs, path::Path};

pub const SECTIONS: &[&str] = &["elements"];

pub fn discover_nav(vault: &Path) -> HashMap<String, Vec<String>> {
    let mut nav = HashMap::new();
    for section in SECTIONS {
        let dir = vault.join(section);
        if let Ok(entries) = fs::read_dir(&dir) {
            let mut slugs: Vec<String> = entries
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
                .filter_map(|e| e.file_name().into_string().ok())
                .collect();
            slugs.sort();
            if !slugs.is_empty() {
                nav.insert(section.to_string(), slugs);
            }
        }
    }
    nav
}

pub fn discover_tabs(vault: &Path, section: &str, slug: &str) -> Vec<String> {
    let dir = vault.join(section).join(slug);
    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut tabs: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            if name.ends_with(".md") && !name.ends_with(".es.md") {
                Some(name[..name.len() - 3].to_string())
            } else {
                None
            }
        })
        .collect();
    tabs.sort();
    if let Some(pos) = tabs.iter().position(|t| t == "overview") {
        tabs.remove(pos);
        tabs.insert(0, "overview".to_string());
    }
    tabs
}

pub fn to_title(s: &str) -> String {
    s.split('-')
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Parse YAML-style frontmatter delimited by `---\n`.
/// Returns (fields, body) — fields is empty if no valid frontmatter found.
pub fn parse_frontmatter(content: &str) -> (HashMap<String, String>, String) {
    if !content.starts_with("---\n") {
        return (HashMap::new(), content.to_string());
    }
    let rest = &content[4..];
    let end = match rest.find("\n---") {
        Some(pos) => pos,
        None => return (HashMap::new(), content.to_string()),
    };
    let fm_text = &rest[..end];
    // consume the closing `---` line and optional newline
    let after_close = end + 4; // "\n---".len()
    let body = rest
        .get(after_close..)
        .unwrap_or("")
        .trim_start_matches('\n');

    let mut fields = HashMap::new();
    for line in fm_text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(colon_pos) = line.find(": ") {
            let key = line[..colon_pos].trim().to_string();
            let mut val = line[colon_pos + 2..].trim().to_string();
            if val.starts_with('"') && val.ends_with('"') && val.len() >= 2 {
                val = val[1..val.len() - 1].to_string();
            }
            fields.insert(key, val);
        } else if let Some(colon_pos) = line.find(':') {
            let key = line[..colon_pos].trim().to_string();
            if !key.is_empty() && !key.starts_with('-') {
                fields.insert(key, String::new());
            }
        }
    }
    (fields, body.to_string())
}
