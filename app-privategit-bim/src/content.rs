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

/// Load `site-content/categories/NN-<slug>.md` in filename order. The `NN-`
/// prefix is the nav-order source of truth; the slug (after the prefix,
/// before `.md`) must match the corresponding `tokens/bim/<slug>.dtcg.json`
/// stem.
pub fn load_categories(site_content_dir: &Path) -> Vec<CategoryMeta> {
    let dir = site_content_dir.join("categories");
    let mut paths: Vec<_> = match fs::read_dir(&dir) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("md"))
            .collect(),
        Err(e) => {
            eprintln!("warn: site-content categories dir not found ({dir:?}): {e}");
            return Vec::new();
        }
    };
    paths.sort();

    paths
        .into_iter()
        .filter_map(|path| {
            let stem = path.file_stem().and_then(|s| s.to_str())?.to_string();
            // Strip a leading "NN-" ordering prefix if present.
            let slug = match stem.split_once('-') {
                Some((prefix, rest)) if prefix.chars().all(|c| c.is_ascii_digit()) => {
                    rest.to_string()
                }
                _ => stem.clone(),
            };
            let raw = fs::read_to_string(&path).ok()?;
            let (fields, body) = parse_frontmatter(&raw);
            Some(CategoryMeta {
                slug,
                display_name: field(&fields, "display_name"),
                ifc_anchor: field(&fields, "ifc_anchor"),
                uniclass: field(&fields, "uniclass"),
                ifc_hierarchy: field(&fields, "ifc_hierarchy"),
                elements: field(&fields, "elements"),
                card_desc: field(&fields, "card_desc"),
                property_sets: parse_property_sets(&field(&fields, "property_sets")),
                intro_html: render_markdown(body.trim()),
            })
        })
        .collect()
}

pub struct PageSection {
    pub heading: String,
    pub body_html: String,
}

pub struct PageContent {
    pub fields: HashMap<String, String>,
    pub sections: Vec<PageSection>,
}

impl PageContent {
    pub fn field(&self, key: &str) -> String {
        self.fields.get(key).cloned().unwrap_or_default()
    }
}

/// Load a `site-content/pages/<name>.md` file: frontmatter scalars plus a
/// body split on `## ` headings into (heading, rendered-html) sections.
pub fn load_page(site_content_dir: &Path, name: &str) -> Option<PageContent> {
    let path = site_content_dir.join("pages").join(format!("{name}.md"));
    let raw = fs::read_to_string(&path)
        .map_err(|e| eprintln!("warn: failed to read page {path:?}: {e}"))
        .ok()?;
    let (fields, body) = parse_frontmatter(&raw);

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

    Some(PageContent { fields, sections })
}
