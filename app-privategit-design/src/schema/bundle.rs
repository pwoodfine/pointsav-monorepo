use crate::render::render_markdown;
use std::collections::HashMap;

struct BundleMember {
    filename: String,
    role: String,
}

fn role_from_ext(filename: &str) -> &'static str {
    if filename.ends_with(".json") {
        "TOKEN"
    } else if filename.ends_with(".css") {
        "STYLESHEET"
    } else if filename.ends_with(".html") {
        "TEMPLATE"
    } else if filename.ends_with(".md") {
        "DOC"
    } else {
        "ASSET"
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn render(frontmatter: Option<&HashMap<String, String>>, body: &str) -> String {
    let empty = HashMap::new();
    let fm = frontmatter.unwrap_or(&empty);

    let id = fm.get("id").map(|s| s.as_str()).unwrap_or("—");
    let name = fm
        .get("name")
        .or_else(|| fm.get("title"))
        .map(|s| s.as_str())
        .unwrap_or("Unnamed Bundle");
    let version = fm.get("version").map(|s| s.as_str()).unwrap_or("—");
    let namespace = fm.get("namespace").map(|s| s.as_str()).unwrap_or("—");

    // Parse comma-separated members list from frontmatter
    let members: Vec<BundleMember> = fm
        .get("members")
        .map(|s| {
            s.split(',')
                .map(|f| f.trim())
                .filter(|f| !f.is_empty())
                .map(|filename| BundleMember {
                    role: role_from_ext(filename).to_string(),
                    filename: filename.to_string(),
                })
                .collect()
        })
        .unwrap_or_default();

    let mut out = String::new();
    out.push_str("<div class=\"schema-badge schema-badge--bundle\">DESIGN-BUNDLE</div>\n");

    // Identity header
    out.push_str(&format!(
        "<div class=\"bundle-header\">\
         <h2 class=\"bundle-name\">{}</h2>\
         <span class=\"bundle-version-badge\">v{}</span>\
         </div>\n",
        html_escape(name),
        html_escape(version)
    ));

    // Metadata panel
    out.push_str("<dl class=\"bundle-meta schema-meta\">\n");
    out.push_str(&format!(
        "<dt>id</dt><dd><code>{}</code></dd>\n",
        html_escape(id)
    ));
    out.push_str(&format!(
        "<dt>namespace</dt><dd><code>{}</code></dd>\n",
        html_escape(namespace)
    ));
    out.push_str(&format!(
        "<dt>members</dt><dd>{}</dd>\n",
        if members.is_empty() {
            "—".to_string()
        } else {
            members.len().to_string()
        }
    ));
    out.push_str("</dl>\n");

    // Member list with role chips
    if !members.is_empty() {
        out.push_str("<div class=\"bundle-members\">\n");
        out.push_str("<h3>Bundle Members</h3>\n");
        out.push_str("<ul class=\"bundle-member-list\">\n");
        for m in &members {
            out.push_str(&format!(
                "<li><span class=\"role-chip role-chip--{}\">{}</span> \
                 <code class=\"bundle-filename\">{}</code></li>\n",
                html_escape(&m.role.to_lowercase()),
                html_escape(&m.role),
                html_escape(&m.filename)
            ));
        }
        out.push_str("</ul>\n");
        out.push_str("</div>\n");
    }

    // Description / body
    if !body.trim().is_empty() {
        out.push_str("<div class=\"bundle-body\">\n");
        out.push_str(&render_markdown(body));
        out.push_str("</div>\n");
    }

    out
}
