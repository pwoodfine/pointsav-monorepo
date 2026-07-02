//! Chrome shell: masthead, hero band, footer, mobile drawer. Tenant-dispatched
//! through [`Tenant`] so one binary serves two brands with the same chrome
//! shape and different marks/links/legal text — the Sovereign Editorial
//! direction's locked architecture, reimplemented fresh (not ported) here.
//!
//! Per DESIGN-SYSTEM.md: no masthead search bar (no search corpus to justify
//! one); the mobile drawer mirrors the wiki's proven pre-rendered slide-in
//! pattern; the footer is three-tier (nav columns → on-page jurisdiction
//! disclosure slots → badge/trademark/copyright base row).

use maud::{html, Markup, PreEscaped, DOCTYPE};

use crate::content::{Page, Section};

#[derive(Debug, Clone)]
pub struct NavLink {
    pub label: &'static str,
    pub href: &'static str,
    pub external: bool,
}

impl NavLink {
    pub const fn internal(label: &'static str, href: &'static str) -> Self {
        Self {
            label,
            href,
            external: false,
        }
    }
    pub const fn external(label: &'static str, href: &'static str) -> Self {
        Self {
            label,
            href,
            external: true,
        }
    }
}

/// A single on-page jurisdiction disclosure slot (footer tier 2). Per SEC
/// Marketing Rule "clear and prominent" guidance: on-page, not just linked.
#[derive(Debug, Clone)]
pub struct DisclosureSlot {
    pub label: &'static str,
    pub body: &'static str,
}

/// Per-tenant chrome configuration. Chrome *shape* is identical across
/// tenants (see [`page_shell`]); only marks, links, and legal text differ.
#[derive(Debug, Clone)]
pub struct Tenant {
    pub module_id: &'static str,
    pub site_title: &'static str,
    pub wordmark_label: &'static str,
    pub nav_links: Vec<NavLink>,
    pub footer_nav: Vec<NavLink>,
    pub cities: Vec<&'static str>,
    /// Always "Woodfine Capital Projects Inc." per TRADEMARK.md v1.1 —
    /// never the tenant's own operating entity, on either brand's site.
    pub copyright_holder: &'static str,
    /// Legal trademark string — the one field that genuinely differs in
    /// content (not just styling) between the two brands.
    pub trademark_line: &'static str,
    pub disclosure_slots: Vec<DisclosureSlot>,

    // --- SEO (P4) ---
    /// Canonical base URL for this tenant (no trailing slash).
    pub canonical_base: &'static str,
    /// Open Graph `og:site_name`.
    pub og_site_name: &'static str,
    /// schema.org `@type` for the root LD+JSON block.
    pub ld_json_type: &'static str,
    /// Site-level description used in LD+JSON when a page has none.
    pub ld_json_description: &'static str,
}

impl Tenant {
    pub fn woodfine() -> Self {
        Self {
            module_id: "woodfine",
            site_title: "Woodfine Capital Projects",
            wordmark_label: "Woodfine Capital Projects",
            nav_links: vec![
                NavLink::internal("Contact us", "/page/contact"),
                NavLink::internal("Disclaimer", "/page/disclaimer"),
                NavLink::external("Corporate", "https://corporate.woodfinegroup.com/"),
                NavLink::external("Projects", "https://projects.woodfinegroup.com/"),
            ],
            footer_nav: vec![
                NavLink::internal("Contact us", "/page/contact"),
                NavLink::internal("Disclaimer", "/page/disclaimer"),
                NavLink::internal("Privacy", "/page/privacy"),
            ],
            cities: vec!["Vancouver", "New York"],
            copyright_holder: "Woodfine Capital Projects Inc.",
            trademark_line: "Woodfine Capital Projects\u{2122}, Woodfine Management Corp\u{2122}, \
                PointSav Digital Systems\u{2122}, Totebox Orchestration\u{2122}, and Totebox \
                Archive\u{2122} are trademarks of Woodfine Capital Projects Inc., used in Canada, \
                the United States, Latin America, and Europe. All other trademarks are the \
                property of their respective owners.",
            disclosure_slots: vec![DisclosureSlot {
                label: "Securities disclosure",
                body: "Securities of Woodfine-sponsored direct-hold solutions are offered only to \
                    qualified investors pursuant to applicable prospectus exemptions under National \
                    Instrument 45-106. This page does not constitute an offer to sell or a \
                    solicitation of an offer to buy any security.",
            }],
            canonical_base: "https://home.woodfinegroup.com",
            og_site_name: "Woodfine Capital Projects",
            ld_json_type: "Organization",
            ld_json_description: "A real property developer with more than 35 years\u{2019} \
                experience in the procurement, development, and management of real property.",
        }
    }

    pub fn pointsav() -> Self {
        Self {
            module_id: "pointsav",
            site_title: "PointSav Digital Systems",
            wordmark_label: "PointSav Digital Systems",
            nav_links: vec![
                NavLink::internal("Disclaimer", "/page/disclaimer"),
                NavLink::external("Software", "https://software.pointsav.com/"),
                NavLink::external("Design System", "https://design.pointsav.com/"),
                NavLink::external("Documentation", "https://documentation.pointsav.com/"),
            ],
            footer_nav: vec![
                NavLink::internal("Disclaimer", "/page/disclaimer"),
                NavLink::internal("Privacy", "/page/privacy"),
            ],
            cities: vec!["Vancouver", "New York"],
            copyright_holder: "Woodfine Capital Projects Inc.",
            trademark_line: "PointSav Digital Systems\u{2122}, Totebox Orchestration\u{2122}, and \
                Totebox Archive\u{2122} are trademarks of Woodfine Capital Projects Inc., used in \
                Canada, the United States, Latin America, and Europe. All other trademarks are the \
                property of their respective owners.",
            disclosure_slots: vec![DisclosureSlot {
                label: "Product disclosure",
                body: "Product descriptions on this page describe intended capabilities. Actual \
                    feature availability may vary by release and partner agreement.",
            }],
            canonical_base: "https://home.pointsav.com",
            og_site_name: "PointSav Digital Systems",
            ld_json_type: "SoftwareApplication",
            ld_json_description: "A fully transferable data management platform for the \
                procurement, development, and management of real properties.",
        }
    }

    pub fn by_module_id(id: &str) -> Self {
        match id {
            "pointsav" => Self::pointsav(),
            _ => Self::woodfine(),
        }
    }
}

fn render_nav(links: &[NavLink], class: &str, aria_label: &str) -> Markup {
    html! {
        nav class=(class) aria-label=(aria_label) {
            @for link in links {
                @if link.external {
                    a href=(link.href) target="_blank" rel="noopener"
                        aria-label={ (link.label) " (opens in new tab)" } { (link.label) }
                } @else {
                    a href=(link.href) { (link.label) }
                }
            }
        }
    }
}

fn masthead(tenant: &Tenant) -> Markup {
    html! {
        header.m-masthead {
            a.m-masthead__wordmark href="/" aria-label=(tenant.wordmark_label) {
                (tenant.site_title)
            }
            (render_nav(&tenant.nav_links, "m-masthead__nav", "Primary"))
            button.m-masthead__burger
                type="button"
                aria-label="Open menu"
                aria-expanded="false"
                aria-controls="m-drawer"
                data-m-drawer-toggle {
                span.m-masthead__burger-bar {}
                span.m-masthead__burger-bar {}
                span.m-masthead__burger-bar {}
            }
        }
    }
}

fn drawer(tenant: &Tenant) -> Markup {
    html! {
        div.m-drawer-scrim data-m-drawer-scrim {}
        div #m-drawer .m-drawer role="dialog" aria-modal="true" aria-label="Site navigation" hidden {
            div.m-drawer__header {
                span { (tenant.site_title) }
                button.m-drawer__close type="button" aria-label="Close menu" data-m-drawer-toggle {
                    "\u{00d7}"
                }
            }
            (render_nav(&tenant.nav_links, "m-drawer__nav", "Mobile"))
        }
    }
}

fn footer(tenant: &Tenant) -> Markup {
    html! {
        footer.m-footer {
            div.m-footer__columns {
                div.m-footer__col {
                    p.m-footer__col-title { "Site" }
                    (render_nav(&tenant.footer_nav, "m-footer__nav", "Footer"))
                }
            }
            @if !tenant.disclosure_slots.is_empty() {
                div.m-footer__disclosure {
                    @for slot in &tenant.disclosure_slots {
                        div.m-footer__slot {
                            p.m-footer__slot-label { (slot.label) }
                            p.m-footer__slot-body { (slot.body) }
                        }
                    }
                }
            }
            div.m-footer__base {
                div.m-footer__meta {
                    div.m-footer__cities {
                        @for (i, city) in tenant.cities.iter().enumerate() {
                            @if i > 0 { span aria-hidden="true" { " | " } }
                            span { (city) }
                        }
                    }
                    p.m-footer__copyright {
                        "\u{00a9} 2026 " (tenant.copyright_holder) " All rights reserved."
                    }
                    p.m-footer__trademark { (tenant.trademark_line) }
                }
                div.m-footer__badges {
                    a.m-badge href="/page/about" {
                        span.m-badge__glyph aria-hidden="true" {
                            svg viewBox="0 0 24 24" width="15" height="15" {
                                path fill="currentColor"
                                    d="M3 5.5A1.5 1.5 0 0 1 4.5 4h15A1.5 1.5 0 0 1 21 5.5v13A1.5 1.5 0 0 1 19.5 20h-15A1.5 1.5 0 0 1 3 18.5v-13zM6 8v8l3.2-2.4L6 8zm7 6.5h5V13h-5v1.5zm0-3h5V10h-5v1.5z" {}
                            }
                        }
                        span.m-badge__text {
                            span.m-badge__label { "Powered by" }
                            span.m-badge__name { "MediaKit" }
                        }
                    }
                }
            }
        }
    }
}

fn hero(section_headline: &str, section_subhead: Option<&str>) -> Markup {
    html! {
        section.m-hero {
            div.m-hero__inner {
                h1.m-hero__headline { (section_headline) }
                @if let Some(sub) = section_subhead {
                    p.m-hero__subhead { (sub) }
                }
            }
        }
    }
}

fn card_grid(columns: u8, cards: &[crate::content::Card]) -> Markup {
    html! {
        section.m-card-grid style={ "--m-grid-cols: " (columns) } {
            @for card in cards {
                div.m-card {
                    h2.m-card__title {
                        @if let Some(href) = &card.href {
                            a href=(href) { (card.title.clone()) }
                        } @else {
                            (card.title.clone())
                        }
                    }
                    @if let Some(body) = &card.body {
                        p.m-card__body { (body) }
                    }
                }
            }
        }
    }
}

fn prose(body: &str) -> Markup {
    let rendered = crate::content::render_markdown(body);
    html! {
        section.m-prose {
            (PreEscaped(rendered))
        }
    }
}

fn render_section(section: &Section, seen_h1: &mut bool) -> Markup {
    match section {
        Section::Hero { headline, subhead } => {
            let markup = if *seen_h1 {
                // WCAG: exactly one <h1> per page. A second hero section
                // (none currently exist, but content is data — guard anyway)
                // demotes to h2 rather than emitting a duplicate <h1>.
                html! {
                    section.m-hero {
                        div.m-hero__inner {
                            h2.m-hero__headline { (headline) }
                            @if let Some(sub) = subhead {
                                p.m-hero__subhead { (sub) }
                            }
                        }
                    }
                }
            } else {
                *seen_h1 = true;
                hero(headline, subhead.as_deref())
            };
            markup
        }
        Section::CardGrid { columns, cards } => card_grid(*columns, cards),
        Section::Prose { body } => prose(body),
    }
}

/// Render a complete HTML document: skip-link + masthead + sections + footer
/// + drawer. No client-side bundler/template DOM-swap — fully server-rendered.
///
/// `en_path`/`es_path` are the two language variants of the current page
/// (e.g. `/page/contact` / `/es/page/contact`) — used for the canonical link
/// and `hreflang` alternates. `google_verify` comes from
/// `SERVICE_MARKETING_GOOGLE_VERIFY` at startup, per-instance.
pub fn page_shell(
    tenant: &Tenant,
    page: &Page,
    module_id: &str,
    en_path: &str,
    es_path: &str,
    google_verify: Option<&str>,
) -> Markup {
    let page_title = format!("{} \u{2014} {}", page.title, tenant.site_title);
    let self_path = if page.lang == "es" { es_path } else { en_path };
    let canonical_url = format!("{}{}", tenant.canonical_base, self_path);
    let en_url = format!("{}{}", tenant.canonical_base, en_path);
    let es_url = format!("{}{}", tenant.canonical_base, es_path);
    let ld_description = if page.description.is_empty() {
        tenant.ld_json_description
    } else {
        page.description.as_str()
    };
    let ld_json = format!(
        r#"{{"@context":"https://schema.org","@type":"{}","name":"{}","url":"{}","description":"{}"}}"#,
        tenant.ld_json_type, tenant.og_site_name, tenant.canonical_base, ld_description,
    );
    let mut seen_h1 = false;
    html! {
        (DOCTYPE)
        html lang=(page.lang) data-brand=(module_id) {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (page_title) }
                meta name="description" content=(page.description);
                link rel="canonical" href=(canonical_url);
                link rel="alternate" hreflang="en" href=(en_url);
                link rel="alternate" hreflang="es" href=(es_url);
                link rel="alternate" hreflang="x-default" href=(en_url);
                meta name="robots" content="index, follow";
                meta property="og:type" content="website";
                meta property="og:site_name" content=(tenant.og_site_name);
                meta property="og:title" content=(page_title);
                meta property="og:description" content=(page.description);
                meta property="og:url" content=(canonical_url);
                meta name="twitter:card" content="summary";
                meta name="twitter:title" content=(page_title);
                meta name="twitter:description" content=(page.description);
                script type="application/ld+json" { (PreEscaped(&ld_json)) }
                @if let Some(token) = google_verify {
                    meta name="google-site-verification" content=(token);
                }
                link rel="stylesheet" href="/static/tokens.css";
                link rel="stylesheet" href="/static/fonts.css";
                link rel="stylesheet" href="/static/app.css";
            }
            body {
                a.m-skiplink href="#m-main" { "Skip to content" }
                (masthead(tenant))
                main #m-main {
                    @for section in &page.sections {
                        (render_section(section, &mut seen_h1))
                    }
                }
                (footer(tenant))
                (drawer(tenant))
                script src="/static/app.js" {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::load_page;

    #[test]
    fn woodfine_and_pointsav_carry_distinct_trademark_lines() {
        let w = Tenant::woodfine();
        let p = Tenant::pointsav();
        assert_ne!(w.trademark_line, p.trademark_line);
        assert!(w.trademark_line.contains("Woodfine Management Corp"));
        assert!(!p.trademark_line.contains("Woodfine Management Corp"));
    }

    #[test]
    fn both_tenants_share_the_same_copyright_holder() {
        // Per TRADEMARK.md v1.1: the copyright holder is always Woodfine
        // Capital Projects Inc., even on the PointSav-branded site.
        let w = Tenant::woodfine();
        let p = Tenant::pointsav();
        assert_eq!(w.copyright_holder, "Woodfine Capital Projects Inc.");
        assert_eq!(p.copyright_holder, "Woodfine Capital Projects Inc.");
    }

    #[test]
    fn renders_exactly_one_h1_even_with_multiple_hero_sections() {
        let dir = tempfile::tempdir().unwrap();
        let page_dir = dir.path().join("home");
        std::fs::create_dir_all(&page_dir).unwrap();
        std::fs::write(
            page_dir.join("page.yaml"),
            r#"
title: Home
slug: home
description: Test.
sections:
  - type: hero
    headline: First
  - type: hero
    headline: Second
"#,
        )
        .unwrap();
        let page = load_page(dir.path(), "home", None).unwrap();
        let html = page_shell(&Tenant::woodfine(), &page, "woodfine", "/", "/es", None).into_string();
        assert_eq!(html.matches("<h1").count(), 1);
        assert!(html.contains("<h2"));
    }

    #[test]
    fn page_shell_has_no_bundler_dom_swap_pattern() {
        let dir = tempfile::tempdir().unwrap();
        let page_dir = dir.path().join("home");
        std::fs::create_dir_all(&page_dir).unwrap();
        std::fs::write(
            page_dir.join("page.yaml"),
            "title: Home\nslug: home\ndescription: Test.\nsections:\n  - type: hero\n    headline: Hi\n",
        )
        .unwrap();
        let page = load_page(dir.path(), "home", None).unwrap();
        let html = page_shell(&Tenant::woodfine(), &page, "woodfine", "/", "/es", None).into_string();
        assert!(!html.contains("__bundler"));
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains(r#"lang="en""#));
        assert!(html.contains(r#"data-brand="woodfine""#));
    }

    #[test]
    fn masthead_has_no_search_bar() {
        // Per DESIGN-SYSTEM.md: marketing has no search corpus, so unlike
        // the wiki masthead there is deliberately no search input here.
        let html = masthead(&Tenant::woodfine()).into_string();
        assert!(!html.contains(r#"type="search""#));
        assert!(!html.contains("role=\"search\""));
    }

    #[test]
    fn footer_badge_links_to_about() {
        let html = footer(&Tenant::woodfine()).into_string();
        assert!(html.contains("Powered by"));
        assert!(html.contains("MediaKit"));
        assert!(html.contains(r#"href="/page/about""#));
    }

    #[test]
    fn nav_landmarks_have_distinct_aria_labels() {
        // axe-core landmark-unique: every <nav> needs a distinct accessible
        // name when more than one is present on a page.
        let masthead_nav = render_nav(&[], "m-masthead__nav", "Primary").into_string();
        let footer_nav = render_nav(&[], "m-footer__nav", "Footer").into_string();
        let drawer_nav = render_nav(&[], "m-drawer__nav", "Mobile").into_string();
        assert!(masthead_nav.contains(r#"aria-label="Primary""#));
        assert!(footer_nav.contains(r#"aria-label="Footer""#));
        assert!(drawer_nav.contains(r#"aria-label="Mobile""#));
    }

    #[test]
    fn drawer_root_is_not_a_nav_element() {
        // axe-core aria-allowed-role: role="dialog" is not permitted on a
        // <nav> (a navigation landmark can't also be a dialog widget).
        let html = drawer(&Tenant::woodfine()).into_string();
        assert!(html.contains(r#"role="dialog""#));
        assert!(!html.contains(r#"<nav id="m-drawer""#));
        assert!(!html.contains(r#"<nav #m-drawer"#));
    }

    #[test]
    fn card_titles_are_h2_not_h3() {
        // axe-core heading-order: card-grid follows the hero's h1 directly
        // with no intermediate h2, so card titles must be h2, not h3.
        let cards = vec![crate::content::Card {
            title: "Example".to_string(),
            body: None,
            href: None,
        }];
        let html = card_grid(4, &cards).into_string();
        assert!(html.contains("<h2"));
        assert!(!html.contains("<h3"));
    }
}
