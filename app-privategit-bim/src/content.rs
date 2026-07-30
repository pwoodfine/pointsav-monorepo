// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

use serde_json::Value;
use std::{collections::HashMap, fs, path::Path};

/// Parse `---`-delimited flat `key: value` frontmatter, ported from
/// app-privategit-design/src/vault.rs::parse_frontmatter.
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

pub fn render_markdown(md: &str) -> String {
    use pulldown_cmark::{html, Options, Parser};
    let opts = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH;
    let parser = Parser::new_ext(md, opts);
    let mut out = String::new();
    html::push_html(&mut out, parser);
    out
}

/// The four content sections a category belongs to, grouped by DTCG entity
/// shape rather than a flat list: Taxonomy (IFC classification/scalar
/// tokens), Objects (instantiable/placeable BIM Object families), Compositions
/// (rules that assemble Objects into Tiles/Floor Plates), Context (site/
/// environmental overlay data). See NEXT.md 2026-07-03 IA reorganization.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Section {
    Taxonomy,
    Objects,
    Compositions,
    Context,
}

impl Section {
    pub fn label(&self) -> &'static str {
        match self {
            Section::Taxonomy => "Taxonomy",
            Section::Objects => "Objects",
            Section::Compositions => "Compositions",
            Section::Context => "Context",
        }
    }

    pub fn all() -> [Section; 4] {
        [
            Section::Taxonomy,
            Section::Objects,
            Section::Compositions,
            Section::Context,
        ]
    }

    fn from_frontmatter(s: &str) -> Option<Section> {
        match s.trim().to_lowercase().as_str() {
            "taxonomy" => Some(Section::Taxonomy),
            "objects" => Some(Section::Objects),
            "compositions" => Some(Section::Compositions),
            "context" => Some(Section::Context),
            _ => None,
        }
    }

    /// Static fallback for token files whose sidecar has no `section:` key
    /// yet (covers the 4 files added 2026-07-03, before this field existed).
    fn fallback_for_slug(slug: &str) -> Section {
        match slug {
            "spatial"
            | "elements"
            | "systems"
            | "materials"
            | "assemblies"
            | "performance"
            | "identity-codes"
            | "relationships"
            | "professional-office-subtypes"
            | "building-width-calculator" => Section::Taxonomy,
            "key-plans" | "amenity-key-plan" | "retail-select" | "tech-industrial" | "interior"
            | "furniture" => Section::Objects,
            "tile-system"
            | "floor-plate-standards"
            | "floor-plate-assembly-rules"
            | "building-grid"
            | "tenant-mix" => Section::Compositions,
            "climate-zones" | "landscape-parking" | "water-management" => Section::Context,
            _ => Section::Taxonomy,
        }
    }
}

#[derive(Clone)]
pub struct CategoryMeta {
    pub slug: String,
    pub display_name: String,
    pub ifc_anchor: String,
    pub uniclass: String,
    pub ifc_hierarchy: String,
    pub elements: String,
    pub card_desc: String,
    pub property_sets: Vec<(String, String, String)>,
    pub intro_html: String,
    pub section: Section,
}

fn parse_property_sets(raw: &str) -> Vec<(String, String, String)> {
    raw.split(';')
        .map(|entry| entry.trim())
        .filter(|entry| !entry.is_empty())
        .filter_map(|entry| {
            let parts: Vec<&str> = entry.split(',').map(|p| p.trim()).collect();
            match parts.as_slice() {
                [pset, prop, ty] => Some((pset.to_string(), prop.to_string(), ty.to_string())),
                _ => {
                    eprintln!("warn: malformed property_sets entry: {entry:?}");
                    None
                }
            }
        })
        .collect()
}

fn field(fields: &HashMap<String, String>, key: &str) -> String {
    fields.get(key).cloned().unwrap_or_default()
}

/// Render a token-file stem ("floor-plate-assembly-rules") as a fallback
/// display name ("Floor Plate Assembly Rules") for categories with no
/// site-content sidecar yet.
fn title_case_slug(slug: &str) -> String {
    slug.split('-')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Load BIM category metadata. `tokens/bim/*.dtcg.json` (already loaded by
/// the caller into `tokens`) is the source of truth for which categories
/// exist; `site-content/categories/*.md` is optional per-slug enrichment
/// (display_name/card_desc/ifc_anchor/etc), matched by filename stem after
/// stripping a leading "NN-" ordering prefix if present. A token file with
/// no matching sidecar still gets a full CategoryMeta — using its own
/// `$description` as fallback card/intro text and a title-cased slug as
/// display name — rather than being silently omitted from nav, cards, and
/// search the way it previously was.
pub fn load_categories(
    tokens: &HashMap<String, Value>,
    site_content_dir: &Path,
) -> Vec<CategoryMeta> {
    let dir = site_content_dir.join("categories");
    let mut sidecars: HashMap<String, (u32, HashMap<String, String>, String)> = HashMap::new();
    match fs::read_dir(&dir) {
        Ok(rd) => {
            for path in rd
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("md"))
            {
                let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                    continue;
                };
                let (order, slug) = match stem.split_once('-') {
                    Some((prefix, rest)) if prefix.chars().all(|c| c.is_ascii_digit()) => {
                        (prefix.parse().unwrap_or(u32::MAX), rest.to_string())
                    }
                    _ => (u32::MAX, stem.to_string()),
                };
                let Ok(raw) = fs::read_to_string(&path) else {
                    continue;
                };
                let (fields, body) = parse_frontmatter(&raw);
                sidecars.insert(slug, (order, fields, body));
            }
        }
        Err(e) => eprintln!("warn: site-content categories dir not found ({dir:?}): {e}"),
    }

    let mut slugs: Vec<&String> = tokens.keys().collect();
    slugs.sort();

    let mut ordered: Vec<(u32, CategoryMeta)> = slugs
        .into_iter()
        .map(|slug| {
            let fallback_desc = tokens
                .get(slug)
                .and_then(|v| v.get("$description"))
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            match sidecars.get(slug) {
                Some((order, fields, body)) => {
                    let mut card_desc = field(fields, "card_desc");
                    if card_desc.is_empty() {
                        card_desc = fallback_desc.clone();
                    }
                    let intro_source = if body.trim().is_empty() {
                        fallback_desc.as_str()
                    } else {
                        body.trim()
                    };
                    let section = Section::from_frontmatter(&field(fields, "section"))
                        .unwrap_or_else(|| Section::fallback_for_slug(slug));
                    (
                        *order,
                        CategoryMeta {
                            slug: slug.clone(),
                            display_name: field(fields, "display_name"),
                            ifc_anchor: field(fields, "ifc_anchor"),
                            uniclass: field(fields, "uniclass"),
                            ifc_hierarchy: field(fields, "ifc_hierarchy"),
                            elements: field(fields, "elements"),
                            card_desc,
                            property_sets: parse_property_sets(&field(fields, "property_sets")),
                            intro_html: render_markdown(intro_source),
                            section,
                        },
                    )
                }
                None => {
                    eprintln!(
                        "warn: no site-content/categories sidecar for token file '{slug}' — using fallback display metadata"
                    );
                    (
                        u32::MAX,
                        CategoryMeta {
                            slug: slug.clone(),
                            display_name: title_case_slug(slug),
                            ifc_anchor: String::new(),
                            uniclass: String::new(),
                            ifc_hierarchy: String::new(),
                            elements: String::new(),
                            card_desc: fallback_desc.clone(),
                            property_sets: Vec::new(),
                            intro_html: render_markdown(&fallback_desc),
                            section: Section::fallback_for_slug(slug),
                        },
                    )
                }
            }
        })
        .collect();

    ordered.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.slug.cmp(&b.1.slug)));
    ordered.into_iter().map(|(_, meta)| meta).collect()
}

pub struct PageSection {
    pub heading: String,
    pub body_html: String,
}

pub struct PageContent {
    pub sections: Vec<PageSection>,
}

/// Load a `site-content/pages/<name>.md` file as one rendered HTML blob —
/// no `## ` section splitting, unlike `load_page`. Used for short,
/// single-paragraph counsel-owned content (the Important Information band)
/// where the caller supplies its own safe default if the file is absent,
/// so `None` is a normal case, not an error to eprintln about.
pub fn load_simple_page(site_content_dir: &Path, name: &str) -> Option<String> {
    let path = site_content_dir.join("pages").join(format!("{name}.md"));
    let raw = fs::read_to_string(&path).ok()?;
    let (_fields, body) = parse_frontmatter(&raw);
    Some(render_markdown(body.trim()))
}

/// Load a `site-content/pages/<name>.md` file: a body split on `## `
/// headings into (heading, rendered-html) sections. Any frontmatter
/// scalars are parsed and discarded — the last reader of per-field
/// frontmatter (the pre-2026-07-03 hero eyebrow/statline/lead) was removed
/// when the homepage became the Envelope-as-Navigation diagram; this file's
/// own frontmatter fields are unused now, not an error.
pub fn load_page(site_content_dir: &Path, name: &str) -> Option<PageContent> {
    let path = site_content_dir.join("pages").join(format!("{name}.md"));
    let raw = fs::read_to_string(&path)
        .map_err(|e| eprintln!("warn: failed to read page {path:?}: {e}"))
        .ok()?;
    let (_fields, body) = parse_frontmatter(&raw);

    let mut sections = Vec::new();
    let mut current_heading: Option<String> = None;
    let mut current_body = String::new();
    for line in body.lines() {
        if let Some(heading) = line.strip_prefix("## ") {
            if let Some(h) = current_heading.take() {
                sections.push(PageSection {
                    heading: h,
                    body_html: render_markdown(current_body.trim()),
                });
                current_body.clear();
            }
            current_heading = Some(heading.trim().to_string());
        } else {
            current_body.push_str(line);
            current_body.push('\n');
        }
    }
    if let Some(h) = current_heading {
        sections.push(PageSection {
            heading: h,
            body_html: render_markdown(current_body.trim()),
        });
    }

    Some(PageContent { sections })
}
