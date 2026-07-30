use crate::render::render_markdown;
use std::collections::HashMap;

pub fn render(frontmatter: Option<&HashMap<String, String>>, body: &str) -> String {
    let mut out = String::new();
    out.push_str("<div class=\"schema-badge schema-badge--token\">DESIGN-TOKEN</div>\n");
    if let Some(fm) = frontmatter {
        let meta_fields = ["name", "status", "namespace", "brief-id"];
        let has_meta = meta_fields.iter().any(|f| fm.contains_key(*f));
        if has_meta {
            out.push_str("<dl class=\"schema-meta\">\n");
            for field in &meta_fields {
                if let Some(val) = fm.get(*field) {
                    out.push_str(&format!(
                        "<dt>{}</dt><dd>{}</dd>\n",
                        field,
                        html_escape(val)
                    ));
                }
            }
            out.push_str("</dl>\n");
        }
    }
    out.push_str(&render_markdown(body));
    out
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
