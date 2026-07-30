//! JSON-LD baseline for TOPIC pages — schema.org markup for AEO eligibility.
//!
//! Per PHASE-2-PLAN.md §1 Step 1 + ARCHITECTURE.md §3 Phase 2:
//! - Every rendered TOPIC carries a `<script type="application/ld+json">` block
//!   in the page `<head>` from Phase 2 onward.
//! - Profile selection: TechArticle by default; DefinedTerm when frontmatter
//!   `disclosure_class: glossary` is set (extends ARCHITECTURE.md §6 enum).
//! - Cumulative — costs nothing in later phases, accumulates AEO-eligibility.
//!
//! The output is a complete `<script>` element including tags so callers can
//! emit it inside `<head>` via `maud::PreEscaped`.

use crate::render::Frontmatter;
use crate::walker::Frontmatter as ArticleMeta;
use serde_json::{json, Value};

/// Build a `<script type="application/ld+json">…</script>` block for one TOPIC.
///
/// The page chrome calls this from inside `<head>`. Profile selection:
/// - `disclosure_class: glossary` → schema.org `DefinedTerm`
/// - else → schema.org `TechArticle`
pub fn jsonld_for_topic(meta: &Frontmatter, slug: &str) -> String {
    let title = meta.title.as_deref().unwrap_or(slug);
    let schema_type = match meta.disclosure_class.as_deref() {
        Some("glossary") => "DefinedTerm",
        _ => "TechArticle",
    };

    let mut obj: Value = json!({
        "@context": "https://schema.org",
        "@type": schema_type,
        "name": title,
        "identifier": slug,
        "inLanguage": "en",
        "isPartOf": {
            "@type": "WebSite",
            "name": "PointSav Knowledge"
        }
    });

    // Optional fields — all soft-fail when absent.
    if let Some(serde_yaml::Value::String(date)) = meta.extra.get("last_revised") {
        obj["datePublished"] = json!(date);
    }
    if let Some(date) = &meta.last_edited {
        obj["dateModified"] = json!(date);
    }
    if let Some(desc) = &meta.short_description {
        obj["description"] = json!(desc);
    }
    if let Some(ver) = &meta.document_version {
        obj["version"] = json!(ver);
    }
    if let Some(cats) = &meta.categories {
        if !cats.is_empty() {
            obj["keywords"] = json!(cats);
        }
    }
    if let Some(cites) = &meta.cites {
        if !cites.is_empty() {
            obj["citation"] = json!(cites);
        }
    }

    if let Some(authors_val) = meta.extra.get("authors") {
        if let Ok(authors) = serde_yaml::from_value::<Vec<String>>(authors_val.clone()) {
            obj["author"] = Value::Array(
                authors
                    .into_iter()
                    .map(|a| json!({ "@type": "Person", "name": a }))
                    .collect(),
            );
        }
    }

    // Forward-looking-information disclosure flag flows to JSON-LD as an
    // additionalProperty so AEO crawlers + downstream consumers see the
    // FLI label on the structured-data side, not just in rendered chrome.
    if meta.forward_looking {
        obj["additionalProperty"] = json!([{
            "@type": "PropertyValue",
            "name": "forward_looking",
            "value": true
        }]);
    }

    format!(
        r#"<script type="application/ld+json">{}</script>"#,
        serde_json::to_string(&obj).unwrap_or_default()
    )
}

/// Build a `<script type="application/ld+json">…</script>` block for one article
/// using the walker-layer `ArticleMeta` (which carries `summary`, `category`, etc.)
/// and a fully-qualified base URL string.
///
/// Returns a schema.org `Article` object. This is the modular-pipeline counterpart
/// to `jsonld_for_topic()`, which takes the render-layer `Frontmatter`. Both produce
/// the same `<script>` wrapper and can be used interchangeably depending on which
/// metadata type is available at the call site.
///
/// # Parameters
/// - `meta` — walker `Frontmatter` from `walker::parse_frontmatter()`
/// - `base_url` — instance base URL, e.g. `"https://documentation.pointsav.com"`.
///   Must not have a trailing `/`.
pub fn jsonld_for_article(meta: &ArticleMeta, base_url: &str) -> String {
    let slug = meta.slug.as_deref().unwrap_or("");
    let title = meta.title.as_deref().unwrap_or(slug);

    let mut obj: Value = json!({
        "@context": "https://schema.org",
        "@type": "Article",
        "headline": title,
        "url": format!("{}/wiki/{}", base_url, slug),
        "inLanguage": "en"
    });

    if let Some(desc) = &meta.summary {
        obj["description"] = json!(desc);
    }
    if let Some(date) = &meta.last_edited {
        obj["dateModified"] = json!(date);
    }
    if let Some(cat) = &meta.category {
        obj["keywords"] = json!(cat);
    }
    if meta.forward_looking {
        obj["additionalProperty"] = json!([{
            "@type": "PropertyValue",
            "name": "forward_looking",
            "value": true
        }]);
    }

    format!(
        r#"<script type="application/ld+json">{}</script>"#,
        serde_json::to_string(&obj).unwrap_or_default()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fm() -> Frontmatter {
        Frontmatter::default()
    }

    #[test]
    fn emits_script_tag() {
        let html = jsonld_for_topic(&fm(), "topic-x");
        assert!(html.starts_with(r#"<script type="application/ld+json">"#));
        assert!(html.ends_with("</script>"));
    }

    #[test]
    fn defaults_to_techarticle() {
        let html = jsonld_for_topic(&fm(), "topic-x");
        assert!(html.contains(r#""@type":"TechArticle""#));
    }

    #[test]
    fn glossary_emits_definedterm() {
        let mut f = fm();
        f.disclosure_class = Some("glossary".to_string());
        let html = jsonld_for_topic(&f, "topic-x");
        assert!(html.contains(r#""@type":"DefinedTerm""#));
    }

    #[test]
    fn includes_title_when_present() {
        let mut f = fm();
        f.title = Some("Hello World".to_string());
        let html = jsonld_for_topic(&f, "topic-x");
        assert!(html.contains(r#""name":"Hello World""#));
    }

    #[test]
    fn falls_back_to_slug_when_title_missing() {
        let html = jsonld_for_topic(&fm(), "topic-x");
        assert!(html.contains(r#""name":"topic-x""#));
    }

    #[test]
    fn date_modified_from_last_edited() {
        let mut f = fm();
        f.last_edited = Some("2026-05-22".to_string());
        let html = jsonld_for_topic(&f, "topic-x");
        assert!(html.contains(r#""dateModified":"2026-05-22""#), "{html}");
    }

    #[test]
    fn description_from_short_description() {
        let mut f = fm();
        f.short_description = Some("A short desc.".to_string());
        let html = jsonld_for_topic(&f, "topic-x");
        assert!(html.contains(r#""description":"A short desc.""#), "{html}");
    }

    #[test]
    fn version_from_document_version() {
        let mut f = fm();
        f.document_version = Some("1.2.3".to_string());
        let html = jsonld_for_topic(&f, "topic-x");
        assert!(html.contains(r#""version":"1.2.3""#), "{html}");
    }

    #[test]
    fn keywords_from_categories() {
        let mut f = fm();
        f.categories = Some(vec!["architecture".to_string(), "systems".to_string()]);
        let html = jsonld_for_topic(&f, "topic-x");
        assert!(html.contains(r#""keywords""#), "{html}");
        assert!(html.contains("architecture"), "{html}");
    }

    #[test]
    fn citation_from_cites() {
        let mut f = fm();
        f.cites = Some(vec!["ni-51-102".to_string(), "rfc-9162".to_string()]);
        let html = jsonld_for_topic(&f, "topic-x");
        assert!(html.contains(r#""citation""#), "{html}");
        assert!(html.contains("ni-51-102"), "{html}");
    }

    #[test]
    fn fli_flag_emits_additional_property() {
        let mut f = fm();
        f.forward_looking = true;
        let html = jsonld_for_topic(&f, "topic-x");
        assert!(html.contains(r#""additionalProperty""#));
        assert!(html.contains(r#""forward_looking""#));
    }

    #[test]
    fn output_body_is_valid_json() {
        let html = jsonld_for_topic(&fm(), "topic-x");
        let prefix = r#"<script type="application/ld+json">"#;
        let suffix = "</script>";
        let body = &html[prefix.len()..html.len() - suffix.len()];
        let parsed: serde_json::Value = serde_json::from_str(body).unwrap();
        assert_eq!(parsed["@context"], "https://schema.org");
        assert_eq!(parsed["isPartOf"]["name"], "PointSav Knowledge");
    }
}
