//! The shared chrome: the persistent header/footer/page frame every
//! `app-mediakit-*` instance renders inside, plus the page render entry point.
//!
//! Ported to maud from `templates/_shell-header.html`, `_shell-footer.html`,
//! and `shell.css`. Tenant-parameterized through [`Brand`] so one binary can
//! serve multiple instances (Woodfine, PointSav) with the same chrome shape
//! and different marks/links — the seamless cross-instance header/footer the
//! family requires.

use maud::{html, Markup, PreEscaped, DOCTYPE};

use crate::page::Page;
use crate::section::CtaButton;
use crate::tokens::{SECTIONS_CSS, SHELL_CSS, WOODFINE_WORDMARK_SVG};

/// A navigation link in the header.
#[derive(Debug, Clone)]
pub struct NavLink {
    pub label: String,
    pub href: String,
    /// Renders the `↗` opens-in-new-tab glyph and `target="_blank"`.
    pub external: bool,
}

impl NavLink {
    pub fn internal(label: &str, href: &str) -> Self {
        Self {
            label: label.into(),
            href: href.into(),
            external: false,
        }
    }
    pub fn external(label: &str, href: &str) -> Self {
        Self {
            label: label.into(),
            href: href.into(),
            external: true,
        }
    }
}

/// Per-tenant chrome configuration. The chrome *shape* is identical across
/// tenants; only the mark, links, and labels differ.
#[derive(Debug, Clone)]
pub struct Brand {
    /// Tenant module id (e.g. "woodfine", "pointsav").
    pub module_id: String,
    /// Display name (browser tab, `<title>` suffix).
    pub site_title: String,
    /// Inline SVG wordmark.
    pub wordmark_svg: String,
    /// Accessible label for the wordmark link.
    pub wordmark_label: String,
    /// Left-hand utility nav (Disclaimer, Contact, …).
    pub left_nav: Vec<NavLink>,
    /// Right-hand property nav (Corporate, Projects, Newsroom, …).
    pub right_nav: Vec<NavLink>,
    /// Footer cities line (segments joined by a separator).
    pub cities: Vec<String>,
    /// Footer nav links.
    pub footer_nav: Vec<NavLink>,
    /// Copyright line.
    pub copyright: String,
    /// Persistent enquire / click-to-call CTA shown in the header on every
    /// page (mobile research: persistent enquire/click-to-call in header).
    pub header_cta: Option<CtaButton>,
    // --- SEO ---
    /// Canonical base URL for this tenant (no trailing slash).
    pub canonical_base: &'static str,
    /// Open Graph `og:site_name`.
    pub og_site_name: &'static str,
    /// schema.org `@type` for the root LD+JSON block.
    pub ld_json_type: &'static str,
    /// Site-level description used in LD+JSON.
    pub ld_json_description: &'static str,
    /// Google Search Console verification token (set from env at startup).
    pub google_verify: Option<String>,
    /// Trademark disclaimer line rendered at the bottom of the page footer.
    /// Source: factory-release-engineering/tokens/legal-tokens-woodfine.yaml § website.footer_trademark_en
    pub trademark: Option<String>,
}

impl Brand {
    /// The Woodfine marketing tenant (home.woodfinegroup.com).
    pub fn woodfine() -> Self {
        Self {
            module_id: "woodfine".into(),
            site_title: "Woodfine Capital Projects".into(),
            wordmark_svg: WOODFINE_WORDMARK_SVG.to_string(),
            wordmark_label: "Woodfine Capital Projects".into(),
            left_nav: vec![
                NavLink::internal("Disclaimer", "/page/disclaimer"),
                NavLink::internal("Contact us", "/page/contact"),
            ],
            right_nav: vec![
                NavLink::external("Corporate", "https://corporate.woodfinegroup.com/"),
                NavLink::external("Projects", "https://projects.woodfinegroup.com/"),
                NavLink::external("Newsroom", "https://newsroom.woodfinegroup.com/"),
            ],
            cities: vec!["Vancouver".into(), "New York".into()],
            footer_nav: vec![
                NavLink::internal("Contact us", "/page/contact"),
                NavLink::internal("Disclaimer", "/page/disclaimer"),
            ],
            copyright: "© 2026 Woodfine Capital Projects Inc. All rights reserved.".into(),
            header_cta: Some(CtaButton {
                label: "Enquire".into(),
                href: "/page/contact".into(),
            }),
            canonical_base: "https://home.woodfinegroup.com",
            og_site_name: "Woodfine Capital Projects",
            ld_json_type: "Organization",
            ld_json_description: "A real property developer with 40 years\u{2019} experience in the procurement, development, and management of real property.",
            google_verify: None,
            trademark: Some(
                "Woodfine Capital Projects\u{2122}, Woodfine Management Corp\u{2122}, \
                 PointSav Digital Systems\u{2122}, Totebox Orchestration\u{2122}, \
                 Totebox Archive\u{2122}, and Capability Geometry\u{2122} are trademarks \
                 of Woodfine Capital Projects Inc., used in Canada, the United States, \
                 Latin America, and Europe. All other trademarks are the property of \
                 their respective owners."
                    .into(),
            ),
        }
    }

    /// The PointSav marketing tenant (home.pointsav.com). Uses a text wordmark
    /// until the PointSav mark asset is ratified.
    pub fn pointsav() -> Self {
        Self {
            module_id: "pointsav".into(),
            site_title: "PointSav Digital Systems".into(),
            wordmark_svg:
                "<span class=\"logo-svg\" style=\"font-family:var(--display);font-size:28px;\
                 font-weight:600;letter-spacing:0.08em;color:var(--ink)\">PointSav</span>"
                    .to_string(),
            wordmark_label: "PointSav Digital Systems".into(),
            left_nav: vec![NavLink::internal("Disclaimer", "/page/disclaimer")],
            right_nav: vec![
                NavLink::external("Monorepo", "https://software.pointsav.com/"),
                NavLink::external("Design System", "https://design.pointsav.com/"),
            ],
            cities: vec!["Vancouver".into(), "New York".into()],
            footer_nav: vec![NavLink::internal("Disclaimer", "/page/disclaimer")],
            copyright: "© 2026 PointSav Digital Systems. All rights reserved.".into(),
            header_cta: None,
            canonical_base: "https://home.pointsav.com",
            og_site_name: "PointSav Digital Systems",
            ld_json_type: "SoftwareApplication",
            ld_json_description: "A fully transferable data management platform for the procurement, development, and management of real properties.",
            google_verify: None,
            trademark: Some(
                "Woodfine Capital Projects\u{2122}, Woodfine Management Corp\u{2122}, \
                 PointSav Digital Systems\u{2122}, Totebox Orchestration\u{2122}, \
                 Totebox Archive\u{2122}, and Capability Geometry\u{2122} are trademarks \
                 of Woodfine Capital Projects Inc., used in Canada, the United States, \
                 Latin America, and Europe. All other trademarks are the property of \
                 their respective owners."
                    .into(),
            ),
        }
    }

    /// Resolve a tenant by module id, defaulting to Woodfine.
    pub fn by_module_id(id: &str) -> Self {
        match id {
            "pointsav" => Self::pointsav(),
            _ => Self::woodfine(),
        }
    }
}

fn render_nav(links: &[NavLink], class: &str) -> Markup {
    html! {
        nav class=(class) {
            @for link in links {
                @if link.external {
                    a class="external" href=(link.href) target="_blank" rel="noopener"
                        aria-label=(format!("{} (opens in new tab)", link.label)) { (link.label) }
                } @else {
                    a href=(link.href) { (link.label) }
                }
            }
        }
    }
}

fn header(brand: &Brand) -> Markup {
    html! {
        header class="topnav" {
            (render_nav(&brand.left_nav, "left"))
            a class="wordmark" href="/" aria-label=(brand.wordmark_label) {
                (PreEscaped(&brand.wordmark_svg))
            }
            div class="right-cluster" {
                (render_nav(&brand.right_nav, "right"))
                @if let Some(cta) = &brand.header_cta {
                    a class="header-cta" href=(cta.href) { (cta.label) }
                }
            }
        }
    }
}

fn footer(brand: &Brand) -> Markup {
    html! {
        footer class="footer" {
            div class="cities" {
                @for (i, city) in brand.cities.iter().enumerate() {
                    @if i > 0 { span class="sep" { "|" } }
                    (city)
                }
            }
            (render_nav(&brand.footer_nav, "footnav"))
        }
        div class="copyright" { (brand.copyright) }
        @if let Some(tm) = &brand.trademark {
            div class="trademark" { (tm) }
        }
    }
}

/// Render a complete HTML document: chrome + the page's ordered sections.
///
/// `tokens_css` is the active DTCG token bundle (see [`crate::tokens`]); it is
/// injected first so the canonical design-system bundle can override the
/// built-in fallback. `path` is the request path used to build canonical URLs
/// and Open Graph tags. No client-side bundler/template DOM-swap is used — the
/// document is fully server-rendered (this is the clean-sheet replacement for
/// the legacy 1.2 MB single-file monolith).
pub fn render_page(brand: &Brand, page: &Page, tokens_css: &str, path: &str) -> String {
    let canonical_url = format!("{}{}", brand.canonical_base, path);
    let page_title = format!("{} \u{2014} {}", page.title, brand.site_title);
    let ld_json = format!(
        r#"{{"@context":"https://schema.org","@type":"{}","name":"{}","url":"{}","description":"{}"}}"#,
        brand.ld_json_type, brand.og_site_name, brand.canonical_base, brand.ld_json_description,
    );
    let markup = html! {
        (DOCTYPE)
        html lang=(page.lang) {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (page_title) }
                @if let Some(desc) = &page.description {
                    meta name="description" content=(desc);
                }
                link rel="canonical" href=(canonical_url);
                meta name="robots" content="index, follow";
                meta property="og:type" content="website";
                meta property="og:site_name" content=(brand.og_site_name);
                meta property="og:title" content=(page_title);
                @if let Some(desc) = &page.description {
                    meta property="og:description" content=(desc);
                }
                meta property="og:url" content=(canonical_url);
                meta name="twitter:card" content="summary";
                meta name="twitter:title" content=(page_title);
                @if let Some(desc) = &page.description {
                    meta name="twitter:description" content=(desc);
                }
                script type="application/ld+json" { (PreEscaped(&ld_json)) }
                @if let Some(token) = &brand.google_verify {
                    meta name="google-site-verification" content=(token);
                }
                style { (PreEscaped(tokens_css)) }
                style { (PreEscaped(SHELL_CSS)) }
                style { (PreEscaped(SECTIONS_CSS)) }
            }
            body {
                div class="page" {
                    (header(brand))
                    main class="landing-main" {
                        @for section in &page.sections {
                            (section.render())
                        }
                    }
                    (footer(brand))
                }
            }
        }
    };
    markup.into_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::DEFAULT_TOKENS_CSS;

    #[test]
    fn renders_full_document_without_bundler_template() {
        let page =
            Page::from_yaml("title: Home\nsections:\n  - type: hero\n    headline: Hi\n").unwrap();
        let html = render_page(&Brand::woodfine(), &page, DEFAULT_TOKENS_CSS, "/");
        assert!(html.starts_with("<!DOCTYPE html>"));
        assert!(html.contains("topnav"));
        assert!(html.contains("hero-headline"));
        assert!(html.contains("Woodfine Capital Projects"));
        assert!(html.contains("Capability Geometry"));
        // The legacy fragile pattern must be structurally absent.
        assert!(!html.contains("__bundler/template"));
        assert!(html.contains("width=device-width"));
        // SEO tags must be present.
        assert!(html.contains(r#"rel="canonical""#));
        assert!(html.contains(r#"property="og:title""#));
        assert!(html.contains("application/ld+json"));
    }

    #[test]
    fn pointsav_brand_resolves() {
        let b = Brand::by_module_id("pointsav");
        assert_eq!(b.module_id, "pointsav");
    }
}
