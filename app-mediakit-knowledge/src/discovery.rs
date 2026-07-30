// SPDX-License-Identifier: FSL-1.1-ALv2
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

//! Discovery surfaces built from the content index: `robots.txt`, `sitemap.xml`,
//! Atom + JSON feeds (recently edited), and `llms.txt` (a machine-readable index
//! for AI agents). All absolute URLs use the site's `canonical_url`.

use crate::content::DocRef;

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// `YYYY-MM-DD` → RFC 3339 (`…T00:00:00Z`); a fixed epoch when absent/malformed.
fn rfc3339(date_iso: &Option<String>) -> String {
    match date_iso {
        Some(d) if d.len() >= 10 => format!("{}T00:00:00Z", &d[..10]),
        _ => "1970-01-01T00:00:00Z".to_string(),
    }
}

pub fn robots_txt(base_url: &str) -> String {
    format!("User-agent: *\nAllow: /\n\nSitemap: {base_url}/sitemap.xml\n")
}

/// Every article (+ the home page), with `lastmod` where known.
pub fn sitemap_xml(base_url: &str, docs: &[&DocRef]) -> String {
    let mut s = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n",
    );
    s.push_str(&format!(
        "  <url><loc>{}/</loc></url>\n",
        xml_escape(base_url)
    ));
    for d in docs {
        let loc = xml_escape(&format!("{base_url}/wiki/{}", d.slug));
        match &d.last_edited {
            Some(lm) => s.push_str(&format!(
                "  <url><loc>{loc}</loc><lastmod>{}</lastmod></url>\n",
                xml_escape(lm)
            )),
            None => s.push_str(&format!("  <url><loc>{loc}</loc></url>\n")),
        }
    }
    s.push_str("</urlset>\n");
    s
}

/// Atom 1.0 feed of the most-recently-edited articles (caller pre-sorts/truncates).
pub fn atom_feed(base_url: &str, title: &str, docs: &[&DocRef]) -> String {
    let updated = docs
        .first()
        .map(|d| rfc3339(&d.last_edited))
        .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string());
    let mut s = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<feed xmlns=\"http://www.w3.org/2005/Atom\">\n",
    );
    s.push_str(&format!("  <title>{}</title>\n", xml_escape(title)));
    s.push_str(&format!("  <link href=\"{}/\"/>\n", xml_escape(base_url)));
    s.push_str(&format!(
        "  <link rel=\"self\" href=\"{}/feed.atom\"/>\n",
        xml_escape(base_url)
    ));
    s.push_str(&format!("  <id>{}/</id>\n", xml_escape(base_url)));
    s.push_str(&format!("  <updated>{updated}</updated>\n"));
    for d in docs {
        let url = format!("{base_url}/wiki/{}", d.slug);
        s.push_str("  <entry>\n");
        s.push_str(&format!("    <title>{}</title>\n", xml_escape(&d.title)));
        s.push_str(&format!("    <link href=\"{}\"/>\n", xml_escape(&url)));
        s.push_str(&format!("    <id>{}</id>\n", xml_escape(&url)));
        s.push_str(&format!(
            "    <updated>{}</updated>\n",
            rfc3339(&d.last_edited)
        ));
        if let Some(sd) = &d.short_description {
            s.push_str(&format!("    <summary>{}</summary>\n", xml_escape(sd)));
        }
        s.push_str("  </entry>\n");
    }
    s.push_str("</feed>\n");
    s
}

/// `llms.txt` — a Markdown index for AI agents (proposed llmstxt.org convention).
pub fn llms_txt(base_url: &str, title: &str, docs: &[&DocRef]) -> String {
    let mut s = format!(
        "# {title}\n\n> Machine-readable index of {title}. Content is licensed CC BY 4.0.\n\n## Articles\n\n"
    );
    for d in docs {
        let url = format!("{base_url}/wiki/{}", d.slug);
        match &d.short_description {
            Some(desc) if !desc.is_empty() => {
                s.push_str(&format!("- [{}]({}): {}\n", d.title, url, desc))
            }
            _ => s.push_str(&format!("- [{}]({})\n", d.title, url)),
        }
    }
    s
}
