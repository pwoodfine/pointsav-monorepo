use crate::render::render_markdown;
use std::collections::HashMap;

struct MarketingSection {
    kind: String,
    content: String,
}

/// Parse `:::kind` … `:::` fenced blocks from the body.
fn parse_sections(body: &str) -> Vec<MarketingSection> {
    let mut sections = Vec::new();
    let mut current_kind: Option<String> = None;
    let mut buf = String::new();

    for line in body.lines() {
        if let Some(stripped) = line.strip_prefix(":::") {
            let fence = stripped.trim();
            if fence.is_empty() {
                // closing fence
                if let Some(kind) = current_kind.take() {
                    sections.push(MarketingSection {
                        kind,
                        content: buf.trim().to_string(),
                    });
                    buf.clear();
                }
            } else {
                // opening fence — nested blocks not supported; close any open block first
                if let Some(kind) = current_kind.take() {
                    sections.push(MarketingSection {
                        kind,
                        content: buf.trim().to_string(),
                    });
                    buf.clear();
                }
                current_kind = Some(fence.to_string());
            }
        } else if current_kind.is_some() {
            buf.push_str(line);
            buf.push('\n');
        }
    }
    // flush unclosed block
    if let Some(kind) = current_kind {
        sections.push(MarketingSection {
            kind,
            content: buf.trim().to_string(),
        });
    }
    sections
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn render(frontmatter: Option<&HashMap<String, String>>, body: &str) -> String {
    let sections = parse_sections(body);
    let theme = frontmatter
        .and_then(|fm| fm.get("theme"))
        .map(|s| s.as_str())
        .unwrap_or("light");
    let brand = frontmatter
        .and_then(|fm| fm.get("brand"))
        .map(|s| s.as_str())
        .unwrap_or("pointsav");

    let mut out = String::new();
    out.push_str(
        "<div class=\"schema-badge schema-badge--marketing\">DESIGN-MARKETING</div>\n",
    );
    out.push_str(&format!(
        "<div class=\"marketing-page\" data-theme=\"{}\" data-brand=\"{}\">\n",
        html_escape(theme),
        html_escape(brand)
    ));

    if sections.is_empty() {
        out.push_str(&render_markdown(body));
    } else {
        for section in &sections {
            let kind = html_escape(&section.kind);
            out.push_str(&format!("<div class=\"section section--{}\">\n", kind));
            match section.kind.as_str() {
                "hero" | "cta" => {
                    out.push_str(&render_markdown(&section.content));
                }
                "feature-grid" => {
                    out.push_str("<div class=\"feature-grid\">\n");
                    out.push_str(&render_markdown(&section.content));
                    out.push_str("</div>\n");
                }
                "pricing" => {
                    out.push_str("<div class=\"pricing-table\">\n");
                    out.push_str(&render_markdown(&section.content));
                    out.push_str("</div>\n");
                }
                "logo-wall" => {
                    out.push_str("<div class=\"logo-wall\">\n");
                    out.push_str(&render_markdown(&section.content));
                    out.push_str("</div>\n");
                }
                _ => {
                    out.push_str(&render_markdown(&section.content));
                }
            }
            out.push_str("</div>\n");
        }
    }

    out.push_str("</div>\n");
    out
}
