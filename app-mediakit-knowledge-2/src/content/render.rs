//! Markdown rendering.
//!
//! Bodies are CommonMark (via comrak) with one platform extension: `[[slug]]`
//! and `[[slug|label]]` wikilinks resolve to internal `/wiki/{slug}` anchors.
//! Section headings (h2/h3) are extracted for the table of contents.

use comrak::{markdown_to_html, Options};

/// A rendered document body plus its heading outline.
#[derive(Debug, Clone)]
pub struct Rendered {
    pub html: String,
    pub headings: Vec<Heading>,
}

/// One section heading for the table of contents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heading {
    pub level: u8,
    pub id: String,
    pub text: String,
}

/// Render a Markdown body to HTML, resolving wikilinks and collecting headings.
#[allow(deprecated)] // extension.header_ids is deprecated but still the id source
pub fn render(body_md: &str) -> Rendered {
    let with_links = resolve_wikilinks(body_md);

    let mut opts = Options::default();
    opts.extension.strikethrough = true;
    opts.extension.table = true;
    opts.extension.autolink = true;
    opts.extension.tasklist = true;
    opts.extension.footnotes = true;
    opts.extension.header_ids = Some(String::new());
    opts.render.r#unsafe = true; // content is trusted (Git-authored, reviewed)

    let html = markdown_to_html(&with_links, &opts);
    let headings = extract_headings(&with_links);
    Rendered { html, headings }
}

/// Replace `[[slug]]` / `[[slug|label]]` with Markdown links to `/wiki/slug`.
/// A leading `#` in the target (e.g. `[[#section]]`) is treated as a same-page
/// anchor. Escaped `\[[` is left untouched.
fn resolve_wikilinks(md: &str) -> String {
    let bytes = md.as_bytes();
    let mut out = String::with_capacity(md.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'[' && i + 1 < bytes.len() && bytes[i + 1] == b'[' {
            // Guard against escaped `\[[`.
            let escaped = i > 0 && bytes[i - 1] == b'\\';
            if !escaped {
                if let Some(close) = md[i + 2..].find("]]") {
                    let inner = &md[i + 2..i + 2 + close];
                    let (target, label) = match inner.split_once('|') {
                        Some((t, l)) => (t.trim(), l.trim()),
                        None => (inner.trim(), inner.trim()),
                    };
                    let href = if let Some(anchor) = target.strip_prefix('#') {
                        format!("#{}", slugify(anchor))
                    } else {
                        format!("/wiki/{}", target)
                    };
                    out.push('[');
                    out.push_str(label);
                    out.push_str("](");
                    out.push_str(&href);
                    out.push(')');
                    i = i + 2 + close + 2;
                    continue;
                }
            }
        }
        // Default: copy this byte through, respecting UTF-8 boundaries.
        let ch_len = utf8_len(bytes[i]);
        out.push_str(&md[i..i + ch_len]);
        i += ch_len;
    }
    out
}

fn utf8_len(b: u8) -> usize {
    match b {
        0x00..=0x7f => 1,
        0xc0..=0xdf => 2,
        0xe0..=0xef => 3,
        _ => 4,
    }
}

/// Extract ATX headings of level 2 and 3 from Markdown source, computing the
/// same id comrak would (via `header_ids`).
fn extract_headings(md: &str) -> Vec<Heading> {
    let mut headings = Vec::new();
    let mut in_fence = false;
    for line in md.lines() {
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        let level = t.bytes().take_while(|&b| b == b'#').count();
        if (2..=3).contains(&level) && t.as_bytes().get(level) == Some(&b' ') {
            let text = t[level..].trim().to_string();
            if text.is_empty() {
                continue;
            }
            headings.push(Heading {
                level: level as u8,
                id: slugify(&text),
                text,
            });
        }
    }
    headings
}

/// Lowercase ASCII slug: alphanumerics kept, runs of other chars → single `-`.
pub fn slugify(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_dash = false;
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_basic_markdown() {
        let r = render("# Title\n\nSome **bold** text.\n");
        assert!(r.html.contains("<strong>bold</strong>"));
    }

    #[test]
    fn resolves_wikilinks_with_and_without_label() {
        let r = render("See [[zero-container-inference]] and [[yoyo-compute|GPU compute]].\n");
        assert!(r.html.contains(r#"href="/wiki/zero-container-inference""#));
        assert!(r.html.contains(r#"href="/wiki/yoyo-compute""#));
        assert!(r.html.contains(">GPU compute</a>"));
    }

    #[test]
    fn extracts_h2_h3_headings_only() {
        let r = render("# H1\n\n## Why no containers\n\n### Detail\n\n#### Too deep\n");
        let ids: Vec<_> = r.headings.iter().map(|h| h.id.as_str()).collect();
        assert_eq!(ids, vec!["why-no-containers", "detail"]);
        assert_eq!(r.headings[0].level, 2);
        assert_eq!(r.headings[1].level, 3);
    }

    #[test]
    fn headings_inside_code_fence_are_ignored() {
        let r = render("## Real\n\n```\n## Not a heading\n```\n");
        assert_eq!(r.headings.len(), 1);
        assert_eq!(r.headings[0].id, "real");
    }

    #[test]
    fn anchor_wikilink_stays_same_page() {
        let r = render("Jump to [[#Cold start|cold start]].\n");
        assert!(r.html.contains(r##"href="#cold-start""##));
    }
}
