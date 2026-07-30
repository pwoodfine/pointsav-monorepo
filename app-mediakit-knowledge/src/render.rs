//! Markdown rendering with frontmatter parsing.
//!
//! The frontmatter schema is documented in ARCHITECTURE.md §6. Phase
//! 1 reads only the fields needed for rendering chrome (title); the
//! rest are captured as a flat `extra` map for later phases (linter,
//! disclosure-mode validation, citation-graph).
//!
//! Phase 1.1 additions (additive — no removals):
//! - `hatnote`: optional italic note rendered above the article body
//! - `translations`: optional map of language code → slug for language switcher
//! - `categories`: optional list of category labels for footer rendering
//!
//! Iteration-2 additions (additive — no removals):
//! - `short_description`: one-sentence article summary; rendered as italic
//!   subtitle below the H1 (Wikipedia Vector 2022 article-subtitle pattern)

use comrak::{
    format_html,
    nodes::{NodeHtmlBlock, NodeValue},
    parse_document, Arena, Options,
};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeMap;

/// Accept a frontmatter field that may be either a bare YAML string or a YAML
/// sequence of strings. Content authored with `audience: customer-woodfine`
/// (scalar) and `audience: [customer-woodfine, operator]` (sequence) are both
/// valid. Without this, serde_yaml fails the entire Frontmatter parse on scalar
/// input, silently defaulting all fields including `title` → raw-slug display.
fn deser_string_or_vec<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<String>, D::Error> {
    use serde::de::{SeqAccess, Visitor};
    use std::fmt;
    struct StrOrVec;
    impl<'de> Visitor<'de> for StrOrVec {
        type Value = Vec<String>;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("a string or list of strings")
        }
        fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Vec<String>, E> {
            Ok(vec![v.to_string()])
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Vec<String>, A::Error> {
            let mut out = Vec::new();
            while let Some(s) = seq.next_element::<String>()? {
                out.push(s);
            }
            Ok(out)
        }
    }
    d.deserialize_any(StrOrVec)
}

/// Translation entry: language code (e.g. "es") → slug of sibling page.
pub type TranslationMap = BTreeMap<String, String>;

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Frontmatter {
    #[serde(default)]
    pub title: Option<String>,

    #[serde(default)]
    pub slug: Option<String>,

    #[serde(default)]
    pub document_version: Option<String>,

    #[serde(default)]
    pub forward_looking: bool,

    #[serde(default)]
    pub disclosure_class: Option<String>,

    /// Italic note rendered at the top of the article body (above the infobox
    /// in source order, per Wikipedia hatnote convention). Phase 1.1 chrome.
    /// When `hatnote_type` is absent, this field is rendered as freehand text.
    /// When `hatnote_type` is present, this field supplies the target slug for
    /// `"main"` and `"see-also"` types; unused for `"disambig"` and `"note"`.
    #[serde(default)]
    pub hatnote: Option<String>,

    /// Sprint L: typed hatnote vocabulary. Closed set:
    ///   `"main"`     → "Main article: [link]" (uses `hatnote:` as the target slug)
    ///   `"see-also"` → "See also: [link]"     (uses `hatnote:` as the target slug)
    ///   `"disambig"` → "This page is a disambiguation page." (no companion field)
    ///   `"note"`     → renders `hatnote:` as freehand text (same as absent type)
    /// When absent, falls back to freehand `hatnote:` rendering (backward compat).
    #[serde(default)]
    pub hatnote_type: Option<String>,

    /// Language code → slug map; drives the language-switcher button next to
    /// the title. Phase 1.1 chrome. Example: `{ es: "topic-hello.es" }`.
    #[serde(default)]
    pub translations: Option<TranslationMap>,

    /// Category labels for the end-of-article footer block. Phase 1.1 chrome.
    #[serde(default)]
    pub categories: Option<Vec<String>>,

    /// Home-page bucketing category per
    /// `content-wiki-documentation/.claude/rules/content-contract.md` §4.
    /// One of the 9 ratified categories (architecture, services, systems,
    /// applications, governance, infrastructure, company, reference, help)
    /// per naming-convention.md §10 Q5-A. The value `root` is reserved for
    /// `index.md` itself and is suppressed from category-panel bucketing.
    #[serde(default)]
    pub category: Option<String>,

    /// Date of the last meaningful edit in `YYYY-MM-DD` format.
    /// Drives the recent-additions feed on the home page. When absent,
    /// the engine falls back to git-commit-date via a shell-out to
    /// `git log -1 --format=%cI -- <path>`, then to filesystem mtime.
    #[serde(default)]
    pub last_edited: Option<String>,

    /// One-sentence article summary. Rendered as `<p class="topic-short-description"><em>…</em></p>`
    /// immediately below the article H1, matching Wikipedia Vector 2022's italic subtitle
    /// pattern. Also used in the featured-article panel on the home page.
    /// Omitted gracefully when absent.
    #[serde(default)]
    pub short_description: Option<String>,

    /// Article quality grade. Closed enum: `complete | core | stub`.
    /// Rendered as a badge adjacent to the article title in wiki_chrome().
    #[serde(default)]
    pub quality: Option<String>,

    /// Article lifecycle status. Closed enum: `stable | pre-build | draft | stub`.
    /// When `stub`, a hatnote notice is injected below the FLI banner.
    #[serde(default)]
    pub status: Option<String>,

    /// Redirect target slug. When set, `wiki_page()` issues a 301 to `/wiki/<target>`
    /// before any rendering occurs. Allows content authors to define redirects with
    /// a single frontmatter field: `redirect_to: "canonical-slug"`.
    #[serde(default)]
    pub redirect_to: Option<String>,

    /// Marks this page as a disambiguation page. When true, a hatnote notice is
    /// rendered above the article body.
    #[serde(default)]
    pub disambig: Option<bool>,

    /// Sprint E1: list of citation IDs declared in this article. Each ID is
    /// resolved against `citations.yaml`; the aggregate verification status drives
    /// the Citation Authority Ribbon colour (green / amber / red).
    #[serde(default)]
    pub cites: Option<Vec<String>>,

    /// Sprint E2: research-trail metadata block. Fields:
    ///   query, sources, date, confidence, notes
    /// Rendered as a collapsible `<details>` block at the end of the article body.
    #[serde(default)]
    pub research_trail: Option<BTreeMap<String, serde_yaml::Value>>,

    /// Phase 7F: article layout variant. When `"journal"`, marginal sidenotes
    /// are injected at ≥1280px for footnotes. Other values are ignored (default
    /// prose layout applies).
    #[serde(default)]
    pub layout: Option<String>,

    /// Phase 7G: opt-out flag for auto-numbered sections on corporate instance.
    /// When `false`, CSS counters are suppressed even when data-instance="woodfine-corporate".
    #[serde(default = "default_true")]
    pub auto_number: bool,

    /// Leapfrog 2030 Phase 5 — Kirby blueprint content type.
    /// Drives template branching and structured-field rendering.
    /// Values: article (default) | guide | topic | research | category
    #[serde(default)]
    pub content_type: Option<String>,

    /// Frontmatter-driven infobox. Rendered as a float-right summary table
    /// at the start of the article body, before the prose div. Alternative
    /// to the code-fence infobox block (which remains supported).
    #[serde(default)]
    pub infobox: Option<Infobox>,

    /// Target audience chips shown below the article H1 and status badge.
    /// E.g. ["operator", "developer", "public"]. Also accepts a bare string
    /// (`audience: operator`) for backward compat with older content.
    #[serde(default, deserialize_with = "deser_string_or_vec")]
    pub audience: Vec<String>,

    /// Alternative slugs that 301-redirect to this article's canonical slug.
    /// Allows renaming articles without breaking existing links.
    /// Also accepts a bare string for single-alias content.
    #[serde(default, deserialize_with = "deser_string_or_vec")]
    pub aliases: Vec<String>,

    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_yaml::Value>,
}

/// A single row in a frontmatter-driven infobox table.
#[derive(Debug, Default, Deserialize, Serialize, Clone)]
pub struct InfoboxRow {
    pub label: String,
    pub value: String,
}

/// Frontmatter-driven infobox: title, optional image, and data rows.
#[derive(Debug, Default, Deserialize, Serialize, Clone)]
pub struct Infobox {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub rows: Vec<InfoboxRow>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug)]
pub struct ParsedPage {
    pub frontmatter: Frontmatter,
    pub body_md: String,
}

/// Split a Markdown file into frontmatter + body.
///
/// Frontmatter is delimited by lines containing only `---`. A file
/// without frontmatter is treated as body-only with a default
/// frontmatter struct.
pub fn parse_page(text: &str) -> Result<ParsedPage, serde_yaml::Error> {
    if let Some(rest) = text.strip_prefix("---\n") {
        if let Some(end_idx) = rest.find("\n---\n") {
            let yaml = &rest[..end_idx];
            let body = &rest[end_idx + "\n---\n".len()..];
            let fm: Frontmatter = match serde_yaml::from_str(yaml) {
                Ok(fm) => fm,
                Err(e) => {
                    tracing::warn!("frontmatter YAML parse error in article: {e}");
                    Frontmatter::default()
                }
            };
            return Ok(ParsedPage {
                frontmatter: fm,
                body_md: body.to_string(),
            });
        }
    }
    Ok(ParsedPage {
        frontmatter: Frontmatter::default(),
        body_md: text.to_string(),
    })
}

/// Render Markdown body to HTML with wikilinks + GFM extensions enabled.
///
/// Phase 1.1: after the comrak pass, `inject_edit_pencils` walks the output
/// and inserts a right-floated `[edit]` anchor after every h2–h6 opening tag.
/// The anchors use `href="#"` placeholders; Phase 2 wires them to the edit
/// surface.
///
/// Callers that need to extract headings for TOC generation should call
/// `render_html_raw` first (for heading extraction), then `inject_edit_pencils`
/// for the final body HTML — or use the convenience wrapper pair
/// `render_html_with_toc`. The edit-pencil pass happens after heading
/// extraction so that TOC text is clean (no "[edit]" fragments).
pub fn render_html(
    body_md: &str,
    content_dir: &std::path::Path,
    extra_roots: &[&std::path::Path],
) -> String {
    let raw = render_html_raw(body_md, content_dir, extra_roots);
    inject_edit_pencils(&raw)
}

/// Like `render_html` but returns the raw comrak output without edit-pencil
/// injection. Use this as the input to `extract_headings` for TOC generation.
///
/// Sprint B/AC: uses comrak's AST API so that fenced code blocks with info strings
/// "infobox", "navbox", and "main" can be walked and replaced with structured HTML before
/// final rendering.
pub fn render_html_raw(
    body_md: &str,
    content_dir: &std::path::Path,
    extra_roots: &[&std::path::Path],
) -> String {
    let mut options = Options::default();
    options.extension.wikilinks_title_after_pipe = true;
    options.extension.table = true;
    options.extension.strikethrough = true;
    options.extension.tasklist = true;
    options.extension.footnotes = true;
    options.extension.description_lists = true;
    options.extension.autolink = true;
    options.extension.header_id_prefix = Some("h-".to_string());
    // Enable raw HTML so our programmatically-injected HtmlBlock nodes (infobox,
    // navbox, main) are not suppressed by the renderer.  All injected HTML goes
    // through escape_html(), so there is no XSS risk from our own code.  Raw HTML
    // authored directly in markdown is a separate concern addressed by Phase 5 auth.
    options.render.r#unsafe = true;

    let arena = Arena::new();
    let root = parse_document(&arena, body_md, &options);

    // B2/B3/AC: Walk AST and replace infobox/navbox/main fenced blocks with HTML.
    for node in root.descendants() {
        let new_val = {
            let data = node.data.borrow();
            if let NodeValue::CodeBlock(ref cb) = data.value {
                if cb.info == "infobox" {
                    render_infobox(&cb.literal).map(|html| {
                        NodeValue::HtmlBlock(NodeHtmlBlock {
                            block_type: 6,
                            literal: html,
                        })
                    })
                } else if cb.info == "navbox" {
                    render_navbox(&cb.literal).map(|html| {
                        NodeValue::HtmlBlock(NodeHtmlBlock {
                            block_type: 6,
                            literal: html,
                        })
                    })
                } else if cb.info == "main" {
                    render_main(&cb.literal).map(|html| {
                        NodeValue::HtmlBlock(NodeHtmlBlock {
                            block_type: 6,
                            literal: html,
                        })
                    })
                } else {
                    None
                }
            } else {
                None
            }
        };
        if let Some(v) = new_val {
            node.data.borrow_mut().value = v;
        }
    }

    let mut raw = String::new();
    format_html(root, &options, &mut raw).expect("comrak format_html");
    inject_wiki_prefixes(&raw, content_dir, extra_roots)
}

/// B2/AC: Render an infobox YAML body as a Wikipedia-style float-right summary table.
///
/// Special keys (not rendered as data rows):
///   `title`         → `<caption>` element at top of table
///   `image`         → full-width image row; value is the `src` URL
///   `image_caption` → caption text below the image (only if `image` is also present)
///
/// All other keys render as `<th>label</th><td>value</td>` rows.
/// Returns None if the YAML fails to parse — the code block is left unchanged.
fn render_infobox(yaml: &str) -> Option<String> {
    let map: serde_yaml::Mapping = serde_yaml::from_str(yaml).ok()?;

    let get_str = |key: &str| -> Option<String> {
        map.get(serde_yaml::Value::String(key.to_string()))
            .and_then(|v| {
                if let serde_yaml::Value::String(s) = v {
                    Some(s.clone())
                } else {
                    None
                }
            })
    };
    let title = get_str("title");
    let image = get_str("image");
    let image_caption = get_str("image_caption");

    let mut html = String::from("<table class=\"infobox\">\n");
    if let Some(ref t) = title {
        html.push_str(&format!(
            "<caption class=\"infobox-title\">{}</caption>\n",
            escape_html(t)
        ));
    }
    html.push_str("<tbody>\n");
    if let Some(ref img) = image {
        let alt = image_caption.as_deref().or(title.as_deref()).unwrap_or("");
        html.push_str("<tr><td colspan=\"2\" class=\"infobox-image\">");
        html.push_str(&format!(
            "<img src=\"{}\" alt=\"{}\">",
            escape_html(img),
            escape_html(alt)
        ));
        if let Some(ref cap) = image_caption {
            html.push_str(&format!(
                "<div class=\"infobox-caption\">{}</div>",
                escape_html(cap)
            ));
        }
        html.push_str("</td></tr>\n");
    }

    const RESERVED: &[&str] = &["title", "image", "image_caption"];
    for (k, v) in &map {
        let key = yaml_val_to_string(k);
        if RESERVED.contains(&key.as_str()) {
            continue;
        }
        let val = yaml_val_to_string(v);
        html.push_str(&format!(
            "<tr><th>{}</th><td>{}</td></tr>\n",
            escape_html(&key),
            escape_html(&val)
        ));
    }
    html.push_str("</tbody>\n</table>\n");
    Some(html)
}

/// B3: Render a navbox YAML body as a collapsible horizontal navigation table.
///
/// Expected YAML structure:
/// ```yaml
/// title: "Navigation title"
/// groups:
///   - label: "Group label"
///     links:
///       - text: "Link text"
///         slug: "article-slug"
/// ```
/// Returns None if the YAML fails to parse.
fn render_navbox(yaml: &str) -> Option<String> {
    let val: serde_yaml::Value = serde_yaml::from_str(yaml).ok()?;
    let title = val
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("Navigation");
    let mut html = format!(
        "<div class=\"navbox\">\n<div class=\"navbox-title\">{}</div>\n<div class=\"navbox-content\">\n",
        escape_html(title)
    );
    if let Some(groups) = val.get("groups").and_then(|g| g.as_sequence()) {
        for group in groups {
            let label = group.get("label").and_then(|l| l.as_str()).unwrap_or("");
            html.push_str(&format!("<div class=\"navbox-group\">\n<span class=\"navbox-group-label\">{}</span>\n<ul class=\"navbox-list\">\n", escape_html(label)));
            if let Some(links) = group.get("links").and_then(|l| l.as_sequence()) {
                for link in links {
                    let text = link.get("text").and_then(|t| t.as_str()).unwrap_or("");
                    let slug = link.get("slug").and_then(|s| s.as_str()).unwrap_or("");
                    html.push_str(&format!(
                        "<li><a href=\"/wiki/{}\">{}</a></li>\n",
                        escape_html(slug),
                        escape_html(text)
                    ));
                }
            }
            html.push_str("</ul>\n</div>\n");
        }
    }
    html.push_str("</div>\n</div>\n");
    Some(html)
}

/// AC: Render a `main` fenced block as a Wikipedia-style "Main article:" hatnote.
///
/// Block body formats:
///   `slug`              — display text derived from the last slug segment (hyphens → spaces, title-cased)
///   `slug|Display Text` — explicit display text after the pipe
///
/// Renders with `class="wiki-hatnote"` so it shares the existing hatnote styling.
fn render_main(body: &str) -> Option<String> {
    let body = body.trim();
    if body.is_empty() {
        return None;
    }
    let (slug, display) = if let Some(pipe) = body.find('|') {
        (body[..pipe].trim(), body[pipe + 1..].trim().to_string())
    } else {
        let last = body.rsplit('/').next().unwrap_or(body);
        let display = last
            .split('-')
            .map(|w| {
                let mut chars = w.chars();
                match chars.next() {
                    None => String::new(),
                    Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
        (body, display)
    };
    Some(format!(
        "<div class=\"wiki-hatnote\">Main article: <a href=\"/wiki/{}\">{}</a></div>\n",
        escape_html(slug),
        escape_html(&display)
    ))
}

fn yaml_val_to_string(v: &serde_yaml::Value) -> String {
    match v {
        serde_yaml::Value::String(s) => s.clone(),
        serde_yaml::Value::Number(n) => n.to_string(),
        serde_yaml::Value::Bool(b) => b.to_string(),
        _ => String::new(),
    }
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Walk rendered HTML and route any `href="slug" data-wikilink="true"` (emitted by
/// comrak) to `/wiki/<slug>` when the target exists. Existence is checked across
/// `content_dir` AND every `extra_roots` entry (the federated guide dirs), so a
/// TOPIC↔GUIDE wikilink resolves regardless of which mount holds the target.
///
/// L18 (zero dead links): an unresolved wikilink is NOT emitted as a (dead) anchor —
/// its link text is unwrapped and rendered as plain text. The red-link class is gone.
fn inject_wiki_prefixes(
    html: &str,
    content_dir: &std::path::Path,
    extra_roots: &[&std::path::Path],
) -> String {
    const MARKER: &str = " data-wikilink=\"true\">";
    let mut out = String::with_capacity(html.len() + 128);
    let mut rest = html;

    while let Some(pos) = rest.find(MARKER) {
        let before_marker = &rest[..pos];
        let after_marker = &rest[pos + MARKER.len()..];

        if let Some(href_pos) = before_marker.rfind("href=\"") {
            let raw_slug = before_marker[href_pos + 6..].trim_end_matches('"');

            if raw_slug.starts_with("/category/") {
                // Category links pass through with their original href intact.
                out.push_str(before_marker);
                out.push_str(MARKER);
                rest = after_marker;
                continue;
            }

            let base = raw_slug.strip_prefix("/wiki/").unwrap_or(raw_slug);
            let decoded = base.replace("%20", " ");
            let norm_slug = decoded.trim().to_lowercase().replace(' ', "-");

            if page_exists(&norm_slug, content_dir, extra_roots) {
                // Resolved → emit the routed anchor with class="wikilink".
                // Find the <a tag opening to inject the class attribute.
                let a_open = before_marker[..href_pos].rfind("<a").unwrap_or(href_pos);
                out.push_str(&before_marker[..a_open]);
                out.push_str("<a class=\"wikilink\" href=\"/wiki/");
                out.push_str(&norm_slug);
                out.push_str("\" data-wikilink=\"true\">");
                rest = after_marker;
            } else {
                // Unresolved wikilink: gate active — emit display text only, no anchor.
                // cargo xtask check-content blocks any promote that has dead links,
                // so this branch only fires in dev/test. No wrapper element (L18).
                let a_open = before_marker[..href_pos].rfind("<a").unwrap_or(href_pos);
                out.push_str(&before_marker[..a_open]);
                if let Some(close_pos) = after_marker.find("</a>") {
                    out.push_str(&after_marker[..close_pos]);
                    rest = &after_marker[close_pos + 4..];
                } else {
                    rest = after_marker;
                }
            }
        } else {
            // Malformed marker — copy through verbatim.
            out.push_str(before_marker);
            out.push_str(MARKER);
            rest = after_marker;
        }
    }
    out.push_str(rest);
    out
}

/// True if `<norm_slug>.md` exists at the flat level OR one level of category
/// subdirectory under `content_dir` or any `extra_roots` entry.
pub(crate) fn page_exists(
    norm_slug: &str,
    content_dir: &std::path::Path,
    extra_roots: &[&std::path::Path],
) -> bool {
    for root in std::iter::once(content_dir).chain(extra_roots.iter().copied()) {
        if root.join(format!("{}.md", norm_slug)).exists() {
            return true;
        }
        if !norm_slug.contains('/') {
            let found = std::fs::read_dir(root)
                .into_iter()
                .flatten()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
                .any(|dir| dir.path().join(format!("{}.md", norm_slug)).exists());
            if found {
                return true;
            }
        }
    }
    false
}

/// Phase 7D — append a freshness dot inside comrak footnote `<sup>` markers.
/// Comrak emits inline refs as:
///   `<sup class="footnote-ref"><a href="#fn-N" id="fnref-N">N</a></sup>`
/// This pass inserts `<span class="freshness-dot" data-status="unknown"></span>`
/// before `</sup>` so JS can render hover cards and CSS can colour the dot.
/// The class `footnote-ref` is left unchanged; the dot is purely additive.
pub fn inject_citation_markers(html: &str) -> String {
    const MARKER: &str = r#"<sup class="footnote-ref">"#;
    const CLOSE: &str = "</sup>";
    const DOT: &str = r#"<span class="freshness-dot" data-status="unknown"></span>"#;

    let mut out = String::with_capacity(html.len() + 128);
    let mut rest = html;

    while let Some(pos) = rest.find(MARKER) {
        out.push_str(&rest[..pos]);
        let after_marker = &rest[pos + MARKER.len()..];
        out.push_str(MARKER);

        if let Some(close_rel) = after_marker.find(CLOSE) {
            out.push_str(&after_marker[..close_rel]);
            out.push_str(DOT);
            out.push_str(CLOSE);
            rest = &after_marker[close_rel + CLOSE.len()..];
        } else {
            out.push_str(after_marker);
            break;
        }
    }
    out.push_str(rest);
    out
}

/// Phase 7F — transform comrak footnotes into Tufte-style marginal sidenotes.
///
/// Only active when `is_journal` is true (frontmatter `layout: journal`). When
/// false, returns the HTML unchanged. The transform:
/// 1. Parses all `<li id="fn-N">` definitions from the `<section class="footnotes">` block.
/// 2. Replaces each `<sup class="footnote-ref">` inline marker with a
///    `<span class="sidenote-anchor">` containing a label+checkbox toggle and the
///    sidenote text inline.
/// 3. Removes the `<section class="footnotes">` block (now redundant).
///
/// CSS drives the layout: ≥1280px → absolute-positioned margin notes; <1280px →
/// checkbox-toggle expander.
pub fn inject_sidenotes(html: &str, is_journal: bool) -> String {
    if !is_journal {
        return html.to_string();
    }

    // Step 1: extract footnote text from <section class="footnotes" ...>
    let mut defs: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    const FN_SECTION: &str = r#"<section class="footnotes""#;
    const LI_PREFIX: &str = r#"<li id="fn-"#;

    if let Some(sec_start) = html.find(FN_SECTION) {
        let mut rest = &html[sec_start..];
        while let Some(li_pos) = rest.find(LI_PREFIX) {
            let after_fn = &rest[li_pos + LI_PREFIX.len()..];
            if let Some(quote) = after_fn.find('"') {
                let n = after_fn[..quote].to_string();
                let li_rest = &after_fn[quote + 1..]; // starts at '>' of <li ...>
                if let Some(li_end) = li_rest.find("</li>") {
                    let li_content = &li_rest[..li_end];
                    defs.insert(n, sidenote_extract_text(li_content));
                    rest = &li_rest[li_end + 5..];
                } else {
                    break;
                }
            } else {
                break;
            }
        }
    }

    if defs.is_empty() {
        return html.to_string();
    }

    // Step 2: replace inline <sup class="footnote-ref"> with sidenote-anchor
    let mut out = String::with_capacity(html.len() + 256);
    let mut rest = html;
    const SUP_MARKER: &str = r#"<sup class="footnote-ref">"#;
    const SUP_CLOSE: &str = "</sup>";

    while let Some(sup_pos) = rest.find(SUP_MARKER) {
        out.push_str(&rest[..sup_pos]);
        let after_sup = &rest[sup_pos + SUP_MARKER.len()..];
        if let Some(close_rel) = after_sup.find(SUP_CLOSE) {
            let sup_inner = &after_sup[..close_rel];
            if let Some(n) = sidenote_extract_n(sup_inner) {
                if let Some(text) = defs.get(&n) {
                    out.push_str(&format!(
                        concat!(
                            r#"<span class="sidenote-anchor" id="sn-{n}">"#,
                            r#"<label class="sn-toggle" for="sn-toggle-{n}">{n}</label>"#,
                            r#"<input type="checkbox" class="sn-toggle-input" id="sn-toggle-{n}">"#,
                            r#"<span class="sidenote" id="sn-note-{n}">{text}</span>"#,
                            r#"</span>"#,
                        ),
                        n = n,
                        text = text,
                    ));
                    rest = &after_sup[close_rel + SUP_CLOSE.len()..];
                    continue;
                }
            }
            // Fallback: emit original sup unchanged
            out.push_str(SUP_MARKER);
            out.push_str(sup_inner);
            out.push_str(SUP_CLOSE);
            rest = &after_sup[close_rel + SUP_CLOSE.len()..];
        } else {
            out.push_str(SUP_MARKER);
            out.push_str(after_sup);
            break;
        }
    }
    out.push_str(rest);

    // Step 3: remove the now-redundant <section class="footnotes"> block
    sidenote_remove_section(&out)
}

/// Extract N from `href="#fn-N"` inside a footnote-ref sup inner.
fn sidenote_extract_n(sup_inner: &str) -> Option<String> {
    const HREF: &str = "href=\"#fn-";
    let pos = sup_inner.find(HREF)?;
    let after = &sup_inner[pos + HREF.len()..];
    let end = after.find('"')?;
    Some(after[..end].to_string())
}

/// Extract plain text from a `<li>` inner slice: `>...<p>text backref</p>...`.
/// Strips the enclosing `<p>`, the `↩` backref link, and trailing whitespace.
fn sidenote_extract_text(li_content: &str) -> String {
    // li_content: ">\n<p>text. <a ...>↩</a></p>\n"
    let mut s = li_content;
    // Skip leading '>'
    if let Some(pos) = s.find('>') {
        s = s[pos + 1..].trim_start();
    }
    // Strip outer <p>…</p>
    if s.starts_with("<p>") {
        s = &s[3..];
    }
    let s = if let Some(pos) = s.rfind("</p>") {
        &s[..pos]
    } else {
        s
    };
    // Remove backref anchor <a … class="footnote-backref" …>↩</a>
    sidenote_remove_backref(s).trim().to_string()
}

/// Remove the `<a class="footnote-backref" ...>↩</a>` link from a text fragment.
fn sidenote_remove_backref(text: &str) -> String {
    // Identify the backref anchor by its href="#fnref-" or class="footnote-backref"
    let marker = if text.contains(r#"class="footnote-backref""#) {
        r#"class="footnote-backref""#
    } else if text.contains("href=\"#fnref-") {
        "href=\"#fnref-"
    } else {
        return text.to_string();
    };
    if let Some(attr_pos) = text.find(marker) {
        let before = &text[..attr_pos];
        if let Some(a_start) = before.rfind("<a ") {
            if let Some(close_rel) = text[a_start..].find("</a>") {
                let full_end = a_start + close_rel + 4;
                return format!("{}{}", &text[..a_start], &text[full_end..]);
            }
        }
    }
    text.to_string()
}

/// Remove the `<section class="footnotes">…</section>` block from rendered HTML.
fn sidenote_remove_section(html: &str) -> String {
    const OPEN: &str = r#"<section class="footnotes""#;
    const CLOSE: &str = "</section>";
    if let Some(start) = html.find(OPEN) {
        if let Some(rel_end) = html[start..].find(CLOSE) {
            let full_end = start + rel_end + CLOSE.len();
            return format!("{}{}", &html[..start], &html[full_end..]);
        }
    }
    html.to_string()
}

/// Walk rendered HTML and insert a right-floated `[edit]` span after every
/// h2–h6 opening tag (h1 is the page title — it gets its own tab chrome).
///
/// This is a straightforward string-level pass; a proper HTML parser is
/// overkill for a constrained tag set and would add a build dependency.
/// The transform is additive and idempotent when the edit-pencil class is
/// already present.
pub fn inject_edit_pencils(html: &str) -> String {
    const PENCIL: &str =
        r##"<span class="edit-pencil"><a href="#" title="Edit this section">[edit]</a></span>"##;

    let mut out = String::with_capacity(html.len() + 64);
    let mut rest = html;

    while !rest.is_empty() {
        // Look for any h2–h6 opening tag (comrak emits lowercase tags).
        let tag_start = rest
            .find("<h2")
            .into_iter()
            .chain(rest.find("<h3"))
            .chain(rest.find("<h4"))
            .chain(rest.find("<h5"))
            .chain(rest.find("<h6"))
            .min();

        match tag_start {
            None => {
                out.push_str(rest);
                break;
            }
            Some(pos) => {
                // Find the end of this opening tag so we can append the pencil
                // immediately inside the heading element (before its text).
                if let Some(close) = rest[pos..].find('>') {
                    let tag_end = pos + close + 1; // index after '>'
                    out.push_str(&rest[..tag_end]);
                    out.push_str(PENCIL);
                    rest = &rest[tag_end..];
                } else {
                    // Malformed — emit as-is and stop.
                    out.push_str(rest);
                    break;
                }
            }
        }
    }

    out
}

/// Extract a flat list of `(id, text, level)` heading triples from rendered
/// HTML for TOC generation.  Only h2–h6 are included (h1 is the page title).
///
/// comrak with `header_ids = Some(...)` emits an inner anchor inside the
/// heading element rather than putting the id on the heading tag itself,
/// e.g. `<h2><a id="h-alpha" ...></a>Alpha</h2>`. So this scan extracts the
/// id from anywhere inside the heading element, not just the opening tag.
/// Text is the heading content with nested tags stripped so the TOC shows
/// plain text only.
pub fn extract_headings(html: &str) -> Vec<(String, String, u8)> {
    let mut headings = Vec::new();
    let mut rest = html;

    loop {
        // Find the nearest h2–h6 opening tag.
        let candidates: Vec<_> = [
            (rest.find("<h2"), 2u8),
            (rest.find("<h3"), 3),
            (rest.find("<h4"), 4),
            (rest.find("<h5"), 5),
            (rest.find("<h6"), 6),
        ]
        .into_iter()
        .filter_map(|(pos, lvl)| pos.map(|p| (p, lvl)))
        .collect();

        let Some((pos, level)) = candidates.into_iter().min_by_key(|(p, _)| *p) else {
            break;
        };

        // Find the matching closing tag.
        let closing_tag = format!("</h{level}>");
        let Some(close_rel) = rest[pos..].find(&closing_tag) else {
            break;
        };
        let close_abs = pos + close_rel;
        let element_html = &rest[pos..close_abs];

        // Extract id from anywhere within the heading element (comrak puts it
        // on the inner <a> when header_ids is configured). Leading space
        // avoids false matches against attribute names ending in -id.
        let id = if let Some(id_start) = element_html.find(r#" id=""#) {
            let after = &element_html[id_start + 5..];
            if let Some(id_end) = after.find('"') {
                after[..id_end].to_string()
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        // Extract text by stripping inner tags. Content starts after the
        // first '>' (end of the heading's opening tag).
        let text = if let Some(content_start_rel) = element_html.find('>') {
            let content = &element_html[content_start_rel + 1..];
            content
                .split('<')
                .enumerate()
                .map(|(i, part)| {
                    if i == 0 {
                        part.to_string()
                    } else if let Some(gt) = part.find('>') {
                        part[gt + 1..].to_string()
                    } else {
                        String::new()
                    }
                })
                .collect::<String>()
                .trim()
                .to_string()
        } else {
            String::new()
        };

        if !id.is_empty() && !text.is_empty() {
            headings.push((id, text, level));
        }

        rest = &rest[close_abs + closing_tag.len()..];
    }

    headings
}

/// Sprint K — resolve `[citation-id]` inline tokens in rendered HTML.
///
/// After comrak emits HTML, this pass scans text nodes (content outside `<…>` tags)
/// for patterns matching `[<id>]` where `<id>` exists in the citation registry.
/// Matching tokens are replaced with:
///   `<a class="cite-ref" href="/api/citations#<id>" title="<entry.title>"><sup>[<id>]</sup></a>`
/// Unknown `[foo]` patterns that are not in the registry are left verbatim — Markdown
/// link text fragments like `[text](url)` will have already been expanded by comrak and
/// won't appear in the HTML; only unresolved bracket tokens survive to this pass.
///
/// When `registry` is `None` (unit tests or when the YAML is absent), returns `html` unchanged.
pub fn inject_citation_refs(
    html: &str,
    registry: Option<&crate::citations::CitationRegistry>,
) -> String {
    let registry = match registry {
        Some(r) if !r.entries.is_empty() => r,
        _ => return html.to_string(),
    };

    // Regex: `[<id>]` where id matches the citation ID alphabet (lowercase, digits, hyphens).
    // We only want to match these when they appear outside HTML tags.
    use regex::Regex;
    use std::sync::OnceLock;
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE
        .get_or_init(|| Regex::new(r"\[([a-z][a-z0-9-]*(?:-\d+)?)\]").expect("citation ref regex"));

    let mut out = String::with_capacity(html.len());
    let mut rest = html;

    while !rest.is_empty() {
        // Skip over HTML tags verbatim so we don't replace tokens inside attributes.
        if rest.starts_with('<') {
            if let Some(close) = rest.find('>') {
                out.push_str(&rest[..close + 1]);
                rest = &rest[close + 1..];
                continue;
            }
        }
        // Find the next `<` — everything before it is text content.
        let text_end = rest.find('<').unwrap_or(rest.len());
        let text = &rest[..text_end];
        rest = &rest[text_end..];

        // Scan text for citation tokens.
        let mut last = 0;
        for cap in re.captures_iter(text) {
            let m = cap.get(0).unwrap();
            let id = cap.get(1).unwrap().as_str();
            if let Some(entry) = registry.get(id) {
                out.push_str(&text[last..m.start()]);
                let title = entry.title.replace('"', "&quot;");
                out.push_str(&format!(
                    r#"<a class="cite-ref" href="/api/citations#{id}" title="{title}"><sup>[{id}]</sup></a>"#
                ));
                last = m.end();
            }
        }
        out.push_str(&text[last..]);
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_page_with_frontmatter() {
        let text = "---\ntitle: Hello\n---\n# body\n";
        let parsed = parse_page(text).unwrap();
        assert_eq!(parsed.frontmatter.title.as_deref(), Some("Hello"));
        assert_eq!(parsed.body_md, "# body\n");
    }

    #[test]
    fn parses_page_without_frontmatter() {
        let text = "# body only\n";
        let parsed = parse_page(text).unwrap();
        assert!(parsed.frontmatter.title.is_none());
        assert_eq!(parsed.body_md, "# body only\n");
    }

    #[test]
    fn renders_wikilinks() {
        let dir = std::env::temp_dir().join(format!("wikilink-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("other-page.md"), "# Other Page\n").unwrap();

        // Existing target → routed anchor with class="wikilink".
        let html = render_html("see [[Other Page]] for context", &dir, &[]);
        assert!(
            html.contains("Other Page"),
            "wikilink text should be in output: {html}"
        );
        assert!(
            html.contains("href=\"/wiki/other-page\""),
            "existing wikilink should produce a routed anchor: {html}"
        );
        assert!(
            html.contains("class=\"wikilink\""),
            "existing wikilink anchor must carry class=wikilink: {html}"
        );

        // Missing target → display text only; no anchor, no wrapper (L18 gate active).
        let html2 = render_html("see [[No Such Page]] here", &dir, &[]);
        assert!(
            html2.contains("No Such Page"),
            "missing wikilink text should be retained: {html2}"
        );
        assert!(
            !html2.contains("wikilink-unresolved"),
            "gate-active: unresolved wikilink must not emit span wrapper: {html2}"
        );
        assert!(
            !html2.contains("wikilink-missing"),
            "red-link class must be absent (L18 gate active): {html2}"
        );
        assert!(
            !html2.contains("href=\"/wiki/no-such-page\""),
            "unresolved wikilink must not produce a dead anchor: {html2}"
        );

        // Target reachable only via an extra (guide) root → resolves (TOPIC↔GUIDE).
        let guide = std::env::temp_dir().join(format!("wikilink-guide-{}", std::process::id()));
        std::fs::create_dir_all(&guide).unwrap();
        std::fs::write(guide.join("setup-guide.md"), "# Setup\n").unwrap();
        let html3 = render_html("see [[Setup Guide]]", &dir, &[guide.as_path()]);
        assert!(
            html3.contains("href=\"/wiki/setup-guide\""),
            "wikilink to a guide-root target should resolve: {html3}"
        );

        std::fs::remove_dir_all(&dir).ok();
        std::fs::remove_dir_all(&guide).ok();
    }

    #[test]
    fn renders_gfm_table() {
        let md = "| a | b |\n|---|---|\n| 1 | 2 |\n";
        let html = render_html(md, std::path::Path::new("."), &[]);
        assert!(html.contains("<table>"), "GFM table should render: {html}");
    }

    // Phase 1.1 tests — additive; all existing tests remain unchanged.

    /// Edit pencils appear on h2+ but not on h1.
    #[test]
    fn edit_pencils_injected_on_h2_not_h1() {
        let md = "# Title\n\n## Section\n\ntext\n";
        let html = render_html(md, std::path::Path::new("."), &[]);
        // The h1 should not carry an edit pencil.
        let h1_pos = html.find("<h1").unwrap();
        let h1_end = html[h1_pos..].find("</h1>").unwrap() + h1_pos;
        assert!(
            !html[h1_pos..h1_end].contains("edit-pencil"),
            "h1 should not have an edit pencil: {html}"
        );
        // The h2 should carry an edit pencil.
        assert!(
            html.contains("edit-pencil"),
            "h2 should have an edit pencil: {html}"
        );
    }

    /// Headings are extracted correctly from comrak output.
    #[test]
    fn extracts_headings_from_html() {
        let md = "## Alpha\n\ntext\n\n### Beta\n\nmore\n";
        let raw = render_html_raw(md, std::path::Path::new("."), &[]);
        let headings = extract_headings(&raw);
        assert_eq!(
            headings.len(),
            2,
            "should extract 2 headings: {:?}",
            headings
        );
        assert_eq!(headings[0].1, "Alpha");
        assert_eq!(headings[0].2, 2);
        assert_eq!(headings[1].1, "Beta");
        assert_eq!(headings[1].2, 3);
    }

    /// TOC text is clean — no "[edit]" fragments from pencil injection.
    #[test]
    fn toc_text_has_no_edit_fragments() {
        let md = "## A Section\n\ntext\n";
        let raw = render_html_raw(md, std::path::Path::new("."), &[]);
        let headings = extract_headings(&raw);
        assert_eq!(headings.len(), 1);
        assert!(
            !headings[0].1.contains("[edit]"),
            "TOC text must not contain [edit]: {:?}",
            headings
        );
    }

    /// Hatnote and categories fields deserialise from frontmatter.
    #[test]
    fn parses_phase11_frontmatter_fields() {
        let text = "---\ntitle: Test\nhatnote: \"See elsewhere.\"\ncategories:\n  - Foo\n  - Bar\ntranslations:\n  es: test.es\n---\nbody\n";
        let parsed = parse_page(text).unwrap();
        assert_eq!(
            parsed.frontmatter.hatnote.as_deref(),
            Some("See elsewhere.")
        );
        let cats = parsed.frontmatter.categories.unwrap();
        assert_eq!(cats, vec!["Foo", "Bar"]);
        let trans = parsed.frontmatter.translations.unwrap();
        assert_eq!(trans.get("es").map(|s| s.as_str()), Some("test.es"));
    }

    /// `short_description` field deserialises from frontmatter.
    #[test]
    fn parses_short_description() {
        let text = "---\ntitle: Substrate\nshort_description: \"The five structural properties that define the platform.\"\n---\nbody\n";
        let parsed = parse_page(text).unwrap();
        assert_eq!(
            parsed.frontmatter.short_description.as_deref(),
            Some("The five structural properties that define the platform.")
        );
    }

    /// Engine Verification Gate — `claim-authoring-convention.md` §3.
    /// Claim markers authored as HTML comments must pass through comrak
    /// unchanged and inert: the markers survive verbatim in the output,
    /// the claim prose renders normally, and surrounding content is
    /// unaffected. This proves the convention's graceful-degradation
    /// guarantee against the engine's actual comrak option set.
    #[test]
    fn claim_markers_pass_through_inert() {
        // Block claim — markers on their own lines.
        let block = "Before the claim.\n\n\
            <!--claim id=derived-state cites=[] confidence=structural-->\n\
            The search index is derived state.\n\
            <!--/claim-->\n\n\
            After the claim.\n";
        let html = render_html(block, std::path::Path::new("."), &[]);
        assert!(
            html.contains("<!--claim id=derived-state cites=[] confidence=structural-->"),
            "opening marker must survive verbatim: {html}"
        );
        assert!(
            html.contains("<!--/claim-->"),
            "closing marker must survive verbatim: {html}"
        );
        assert!(
            html.contains("The search index is derived state."),
            "claim prose must render: {html}"
        );
        assert!(
            html.contains("Before the claim.") && html.contains("After the claim."),
            "surrounding prose must be unaffected: {html}"
        );

        // Inline claim — markers mid-paragraph.
        let inline =
            "An auditor <!--claim id=audit confidence=established cites=[rfc-9162]-->can verify \
             integrity<!--/claim--> independently.";
        let html2 = render_html(inline, std::path::Path::new("."), &[]);
        assert!(
            html2.contains("<!--claim id=audit confidence=established cites=[rfc-9162]-->")
                && html2.contains("<!--/claim-->"),
            "inline markers must survive verbatim: {html2}"
        );
        assert!(
            html2.contains("can verify integrity"),
            "inline claim prose must render: {html2}"
        );
    }
}
