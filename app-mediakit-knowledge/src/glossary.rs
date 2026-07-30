use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Clone, Default)]
pub struct Glossary {
    pub map: HashMap<String, String>,
    re_terms: Option<regex::Regex>,
}

pub fn load_glossary(content_dir: &Path) -> Glossary {
    let mut map = HashMap::new();

    // Discover and load any glossary-*.csv files in the content directory
    if let Ok(entries) = std::fs::read_dir(content_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("csv") {
                if let Some(file_name) = path.file_name().and_then(|s| s.to_str()) {
                    if file_name.starts_with("glossary-") {
                        load_glossary_file(&path, &mut map);
                    }
                }
            }
        }
    }

    let re_terms = if map.is_empty() {
        None
    } else {
        let mut terms: Vec<&String> = map.keys().collect();
        terms.sort_by_key(|t| std::cmp::Reverse(t.len()));
        let escaped_terms: Vec<String> = terms.iter().map(|t| regex::escape(t)).collect();
        let pattern = format!(r"(?i)\b({})\b", escaped_terms.join("|"));
        regex::Regex::new(&pattern).ok()
    };

    Glossary { map, re_terms }
}

fn load_glossary_file(path: &Path, map: &mut HashMap<String, String>) {
    if let Ok(mut rdr) = csv::Reader::from_path(path) {
        for record in rdr.records().flatten() {
            if let (Some(en), Some(es), Some(defn)) = (record.get(0), record.get(1), record.get(2))
            {
                let defn = defn.trim();
                if !defn.is_empty() {
                    let en_term = en.trim();
                    if !en_term.is_empty() {
                        map.insert(en_term.to_lowercase(), defn.to_string());
                    }
                    let es_term = es.trim();
                    if !es_term.is_empty() {
                        map.insert(es_term.to_lowercase(), defn.to_string());
                    }
                }
            }
        }
    }
}

pub fn inject_glossary_tooltips(html: &str, glossary: &Glossary) -> String {
    if glossary.map.is_empty() {
        return html.to_string();
    }

    // First, handle explicit {{gli|Term}} syntax with placeholders
    let mut placeholders = Vec::new();
    let re_explicit = regex::Regex::new(r"\{\{gli\|([^}]+)\}\}").unwrap();
    let result = re_explicit
        .replace_all(html, |caps: &regex::Captures| {
            let term = &caps[1];
            let defn = glossary
                .map
                .get(&term.to_lowercase())
                .map(String::as_str)
                .unwrap_or("Definition not found");
            let replacement = format!(
                "<span class=\"wiki-glossary-term\" title=\"{}\" aria-label=\"{}\">{}</span>",
                html_escape(defn),
                html_escape(defn),
                term
            );
            let id = placeholders.len();
            placeholders.push(replacement);
            format!("GLOSSARY_PLACEHOLDER_{}_GLOSSARY", id)
        })
        .to_string();

    // Now auto-link
    let mut result = if let Some(ref re_terms) = glossary.re_terms {
        let mut out = String::with_capacity(result.len() * 2);
        let mut inside_a = false;
        let mut last_idx = 0;

        let re_tags = regex::Regex::new(r"(<[^>]+>)").unwrap();
        for mat in re_tags.find_iter(&result) {
            let text_segment = &result[last_idx..mat.start()];
            if !inside_a && !text_segment.is_empty() {
                let replaced = re_terms.replace_all(text_segment, |caps: &regex::Captures| {
                    let term = &caps[1];
                    let defn = glossary.map.get(&term.to_lowercase()).map(String::as_str).unwrap_or("Definition not found");
                    format!("<span class=\"wiki-glossary-term\" title=\"{}\" aria-label=\"{}\">{}</span>", html_escape(defn), html_escape(defn), term)
                });
                out.push_str(&replaced);
            } else {
                out.push_str(text_segment);
            }

            let tag = mat.as_str();
            if tag.starts_with("<a ") || tag == "<a>" {
                inside_a = true;
            } else if tag == "</a>" {
                inside_a = false;
            }
            out.push_str(tag);
            last_idx = mat.end();
        }

        let remaining = &result[last_idx..];
        if !inside_a && !remaining.is_empty() {
            let replaced = re_terms.replace_all(remaining, |caps: &regex::Captures| {
                let term = &caps[1];
                let defn = glossary
                    .map
                    .get(&term.to_lowercase())
                    .map(String::as_str)
                    .unwrap_or("Definition not found");
                format!(
                    "<span class=\"wiki-glossary-term\" title=\"{}\" aria-label=\"{}\">{}</span>",
                    html_escape(defn),
                    html_escape(defn),
                    term
                )
            });
            out.push_str(&replaced);
        } else {
            out.push_str(remaining);
        }
        out
    } else {
        result
    };

    // Restore placeholders
    for (id, replacement) in placeholders.iter().enumerate() {
        let placeholder = format!("GLOSSARY_PLACEHOLDER_{}_GLOSSARY", id);
        result = result.replace(&placeholder, replacement);
    }

    result
}

fn html_escape(text: &str) -> String {
    text.replace("&", "&amp;")
        .replace("<", "&lt;")
        .replace(">", "&gt;")
        .replace("\"", "&quot;")
        .replace("'", "&#x27;")
}

// ─── Phase 2 — YAML-based glossary ──────────────────────────────────────────

/// A single term in a YAML-format glossary file.
///
/// YAML structure:
/// ```yaml
/// - term: "Wikilink"
///   definition: "A cross-reference between articles using [[double-bracket]] syntax."
///   slug: "wikilink-syntax"   # optional — links to /wiki/<slug>
/// ```
#[derive(Debug, Clone, Deserialize)]
pub struct GlossaryTerm {
    /// Display term (matched case-insensitively in `inject_links`).
    pub term: String,
    /// One-sentence definition rendered as the link `title` attribute.
    pub definition: String,
    /// Optional article slug for a "click to read more" link. When set,
    /// the auto-link href is `/wiki/<slug>` rather than just a tooltip.
    #[serde(default)]
    pub slug: Option<String>,
}

/// A YAML-loaded glossary keyed by lowercased term for case-insensitive lookup.
///
/// Constructed from a `glossary.yaml` file alongside content mounts. Terms are
/// injected into rendered HTML via `inject_links`.
#[derive(Debug, Clone, Default)]
pub struct GlossaryYaml {
    /// All terms, keyed by their lowercased form.
    pub terms: HashMap<String, GlossaryTerm>,
}

impl GlossaryYaml {
    /// Load a YAML glossary from `path`.
    ///
    /// The file must be a YAML sequence of `GlossaryTerm` objects.  Returns an
    /// empty `GlossaryYaml` when the file does not exist (so callers can handle
    /// the missing-file case without special-casing).
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(GlossaryYaml::default());
        }
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("cannot read glossary: {}", path.display()))?;
        let raw: Vec<GlossaryTerm> = serde_yaml::from_str(&text)
            .with_context(|| format!("cannot parse glossary: {}", path.display()))?;
        let terms = raw
            .into_iter()
            .map(|t| (t.term.to_lowercase(), t))
            .collect();
        Ok(GlossaryYaml { terms })
    }

    /// Look up a term by its display form (case-insensitive).
    pub fn get(&self, term: &str) -> Option<&GlossaryTerm> {
        self.terms.get(&term.to_lowercase())
    }

    /// Post-process rendered HTML: wrap the first occurrence of each glossary
    /// term in an `<a class="glossary-link">` element.
    ///
    /// Rules:
    /// - Only the *first* occurrence of each term per document is linked.
    /// - Matching is case-insensitive (the original case is preserved in the output).
    /// - Terms inside `<a>`, `<code>`, and heading tags are skipped.
    /// - When `slug` is set, the `href` is `/wiki/<slug>`; otherwise the element
    ///   is a `<span>` with a `title` tooltip only.
    pub fn inject_links(&self, html: &str) -> String {
        if self.terms.is_empty() {
            return html.to_string();
        }

        // Build a regex that matches any glossary term (longest-first to prefer
        // longer matches when terms share a prefix).
        let mut sorted: Vec<&str> = self.terms.values().map(|t| t.term.as_str()).collect();
        sorted.sort_by_key(|t| std::cmp::Reverse(t.len()));
        let escaped: Vec<String> = sorted.iter().map(|t| regex::escape(t)).collect();
        let pattern = format!(r"(?i)\b({})\b", escaped.join("|"));
        let re = match regex::Regex::new(&pattern) {
            Ok(r) => r,
            Err(_) => return html.to_string(),
        };

        // One-pass scanner: skip content inside <a>, <code>, and heading tags.
        let re_tags = match regex::Regex::new(r"(<[^>]+>)") {
            Ok(r) => r,
            Err(_) => return html.to_string(),
        };

        let mut out = String::with_capacity(html.len() + 512);
        let mut skip_depth = 0usize; // >0 means we are inside a skip context
        let mut linked: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut last = 0;

        for mat in re_tags.find_iter(html) {
            // Process the text segment before this tag.
            let text_seg = &html[last..mat.start()];
            if skip_depth == 0 && !text_seg.is_empty() {
                let replaced = re.replace_all(text_seg, |caps: &regex::Captures| {
                    let matched = &caps[1];
                    let key = matched.to_lowercase();
                    if linked.contains(&key) {
                        return matched.to_string();
                    }
                    if let Some(term) = self.terms.get(&key) {
                        linked.insert(key);
                        let defn = html_escape(&term.definition);
                        if let Some(ref slug) = term.slug {
                            format!(
                                r#"<a class="glossary-link" href="/wiki/{slug}" title="{defn}">{matched}</a>"#,
                                slug = html_escape(slug),
                                defn = defn,
                                matched = matched,
                            )
                        } else {
                            format!(
                                r#"<span class="glossary-link" title="{defn}">{matched}</span>"#,
                                defn = defn,
                                matched = matched,
                            )
                        }
                    } else {
                        matched.to_string()
                    }
                });
                out.push_str(&replaced);
            } else {
                out.push_str(text_seg);
            }

            // Adjust skip depth based on the tag.
            let tag = mat.as_str();
            let tag_lower = tag.to_lowercase();
            let is_open_a = tag_lower.starts_with("<a ");
            let is_close_a = tag_lower == "</a>";
            let is_open_code = tag_lower.starts_with("<code");
            let is_close_code = tag_lower == "</code>";
            let is_open_heading = tag_lower.starts_with("<h1")
                || tag_lower.starts_with("<h2")
                || tag_lower.starts_with("<h3");
            let is_close_heading =
                tag_lower == "</h1>" || tag_lower == "</h2>" || tag_lower == "</h3>";

            if is_open_a || is_open_code || is_open_heading {
                skip_depth += 1;
            } else if (is_close_a || is_close_code || is_close_heading) && skip_depth > 0 {
                skip_depth -= 1;
            }

            out.push_str(tag);
            last = mat.end();
        }

        // Trailing text after the last tag.
        let tail = &html[last..];
        if skip_depth == 0 && !tail.is_empty() {
            let replaced = re.replace_all(tail, |caps: &regex::Captures| {
                let matched = &caps[1];
                let key = matched.to_lowercase();
                if linked.contains(&key) {
                    return matched.to_string();
                }
                if let Some(term) = self.terms.get(&key) {
                    linked.insert(key.clone());
                    let defn = html_escape(&term.definition);
                    if let Some(ref slug) = term.slug {
                        format!(
                            r#"<a class="glossary-link" href="/wiki/{slug}" title="{defn}">{matched}</a>"#,
                            slug = html_escape(slug),
                            defn = defn,
                            matched = matched,
                        )
                    } else {
                        format!(
                            r#"<span class="glossary-link" title="{defn}">{matched}</span>"#,
                            defn = defn,
                            matched = matched,
                        )
                    }
                } else {
                    matched.to_string()
                }
            });
            out.push_str(&replaced);
        } else {
            out.push_str(tail);
        }

        out
    }
}
