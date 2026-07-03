// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

use std::{collections::HashMap, fs, path::Path};

/// Group `components/` slugs by their recipe.json `category` field, so the sidebar can
/// separate the generic design-system substrate from components contributed by other
/// product clusters (GIS, the wiki engine) rather than one flat alphabetical list that
/// mixes `Button`/`Badge` with `Map Side Drawer`/`Wiki Toc Sidebar`. `category` is a
/// real, already-authored field in every component's `recipe.json` — not inferred from
/// naming. Returns ordered groups: the generic substrate first (empty label, rendered
/// exactly as it always has been), then non-generic categories in a fixed order.
pub fn discover_component_groups(vault: &Path, slugs: &[String]) -> Vec<(String, Vec<String>)> {
    let mut by_category: HashMap<String, Vec<String>> = HashMap::new();
    for slug in slugs {
        let recipe_path = vault.join("components").join(slug).join("recipe.json");
        let category = fs::read_to_string(&recipe_path)
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .and_then(|v| v.get("category").and_then(|c| c.as_str()).map(String::from))
            .unwrap_or_else(|| "components".to_string());
        by_category.entry(category).or_default().push(slug.clone());
    }
    for group in by_category.values_mut() {
        group.sort();
    }

    fn label(category: &str) -> String {
        match category {
            "components" => String::new(),
            "map" => "Also used on gis.woodfinegroup.com".to_string(),
            "wiki" => "Also used by the wiki engine".to_string(),
            other => format!("Also used by {}", to_title(other)),
        }
    }

    let mut ordered = Vec::new();
    if let Some(generic) = by_category.remove("components") {
        ordered.push((label("components"), generic));
    }
    let mut rest: Vec<(String, Vec<String>)> = by_category.into_iter().collect();
    rest.sort_by(|a, b| a.0.cmp(&b.0));
    for (category, group_slugs) in rest {
        ordered.push((label(&category), group_slugs));
    }
    ordered
}

/// (section, default/landing tab) — components land on `usage`, not `overview`,
/// since components have no overview.md (usage/style/code/accessibility.md instead).
pub const SECTIONS: &[(&str, &str)] = &[
    ("elements", "overview"),
    ("components", "usage"),
    ("guidelines", "overview"),
    ("developing", "overview"),
    ("designing", "overview"),
    ("about", "overview"),
    ("research", "overview"),
];

pub fn default_tab(section: &str) -> &'static str {
    SECTIONS
        .iter()
        .find(|(s, _)| *s == section)
        .map(|(_, t)| *t)
        .unwrap_or("overview")
}

pub fn is_known_section(section: &str) -> bool {
    SECTIONS.iter().any(|(s, _)| *s == section)
}

pub fn discover_nav(vault: &Path) -> HashMap<String, Vec<String>> {
    let mut nav = HashMap::new();
    for (section, _) in SECTIONS {
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
    let landing = default_tab(section);
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
    if let Some(pos) = tabs.iter().position(|t| t == landing) {
        tabs.remove(pos);
        tabs.insert(0, landing.to_string());
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
