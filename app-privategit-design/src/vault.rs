// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

use std::{collections::HashMap, fs, path::Path, path::PathBuf};

/// On-disk layout for a vault section. `Nested` is `section/slug/tab.md` (elements,
/// components, guidelines — components in particular have real multiple tabs per slug:
/// usage/style/code/accessibility). `Flat` is `section/slug.md` directly — used by
/// research/developing/designing/about, which predate this app's routing and were
/// invisible to nav/search/edit until this distinction was added (2026-07-03, Phase 2).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    Nested,
    Flat,
}

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

/// Look up which `discover_component_groups` label a given slug belongs to — used by
/// `component_meta` to show a small origin badge on the component's own page (Phase 3),
/// not just the sidebar grouping. Returns `None` for the generic substrate group (empty
/// label — no badge needed, same as the sidebar renders it unlabeled) or if not found.
pub fn component_origin_label<'a>(
    component_groups: &'a [(String, Vec<String>)],
    slug: &str,
) -> Option<&'a str> {
    component_groups
        .iter()
        .find(|(label, slugs)| !label.is_empty() && slugs.iter().any(|s| s == slug))
        .map(|(label, _)| label.as_str())
}

/// Human-readable "last modified" freshness for a vault file. Uses filesystem mtime, not
/// git history — the deployed vault is a gitignored copy synced from the canonical
/// `pointsav-design-system` repo, so git log against the running app's own vault
/// directory would show nothing meaningful. mtime is the honest signal actually available
/// to the running binary.
pub fn file_freshness_label(path: &Path) -> Option<String> {
    let modified = fs::metadata(path).ok()?.modified().ok()?;
    let elapsed = modified.elapsed().ok()?;
    let days = elapsed.as_secs() / 86400;
    Some(match days {
        0 => "Updated today".to_string(),
        1 => "Updated yesterday".to_string(),
        2..=13 => format!("Updated {days} days ago"),
        14..=59 => format!("Updated {} weeks ago", days / 7),
        _ => format!("Updated {} months ago", days / 30),
    })
}

/// (section, default/landing tab, on-disk layout) — components land on `usage`, not
/// `overview`, since components have no overview.md (usage/style/code/accessibility.md
/// instead). `research`/`developing`/`designing`/`about` are `Flat`: their default tab
/// name doubles as the one implicit tab every slug in that section resolves to.
pub const SECTIONS: &[(&str, &str, Layout)] = &[
    ("elements", "overview", Layout::Nested),
    ("components", "usage", Layout::Nested),
    ("guidelines", "overview", Layout::Nested),
    ("developing", "overview", Layout::Flat),
    ("designing", "overview", Layout::Flat),
    ("about", "overview", Layout::Flat),
    ("research", "overview", Layout::Flat),
];

pub fn default_tab(section: &str) -> &'static str {
    SECTIONS
        .iter()
        .find(|(s, _, _)| *s == section)
        .map(|(_, t, _)| *t)
        .unwrap_or("overview")
}

pub fn is_known_section(section: &str) -> bool {
    SECTIONS.iter().any(|(s, _, _)| *s == section)
}

pub fn is_flat_section(section: &str) -> bool {
    SECTIONS
        .iter()
        .any(|(s, _, l)| *s == section && *l == Layout::Flat)
}

/// The one place that knows flat sections ignore the tab segment on disk — every
/// content-path lookup (browsing, editing, WYSIWYG save) should go through this rather
/// than building the path inline, so the flat/nested distinction stays in one function.
pub fn content_path(vault: &Path, section: &str, slug: &str, tab: &str) -> PathBuf {
    if is_flat_section(section) {
        vault.join(section).join(format!("{slug}.md"))
    } else {
        vault.join(section).join(slug).join(format!("{tab}.md"))
    }
}

pub fn discover_nav(vault: &Path) -> HashMap<String, Vec<String>> {
    let mut nav = HashMap::new();
    for (section, _, layout) in SECTIONS {
        let dir = vault.join(section);
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        let mut slugs: Vec<String> = entries
            .filter_map(|e| e.ok())
            .filter_map(|e| {
                let ft = e.file_type().ok()?;
                let name = e.file_name().into_string().ok()?;
                match layout {
                    Layout::Nested => ft.is_dir().then_some(name),
                    Layout::Flat => {
                        (ft.is_file() && name.ends_with(".md") && !name.ends_with(".es.md"))
                            .then(|| name[..name.len() - 3].to_string())
                    }
                }
            })
            .collect();
        slugs.sort();
        if !slugs.is_empty() {
            nav.insert(section.to_string(), slugs);
        }
    }
    nav
}

pub fn discover_tabs(vault: &Path, section: &str, slug: &str) -> Vec<String> {
    if is_flat_section(section) {
        let file = vault.join(section).join(format!("{slug}.md"));
        return if file.is_file() {
            vec![default_tab(section).to_string()]
        } else {
            Vec::new()
        };
    }

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

/// Slugs that are acronyms/initialisms — rendered fully uppercase rather than
/// title-cased. Found via FABLE visual-QA pass (Phase 5, 2026-07-04): sidebar/card labels
/// like "Wiki Toc Sidebar" read as auto-title-cased file names, not curated nav, next to
/// real prose labels. (bim/guid/3d/rs1/ifc removed 2026-07-04 — those were BIM-specific
/// slugs, and BIM content no longer lives in this substrate.)
const ACRONYM_WORDS: &[&str] = &["gis", "toc"];

pub fn to_title(s: &str) -> String {
    s.split('-')
        .map(|w| {
            if ACRONYM_WORDS.contains(&w.to_lowercase().as_str()) {
                return w.to_uppercase();
            }
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
