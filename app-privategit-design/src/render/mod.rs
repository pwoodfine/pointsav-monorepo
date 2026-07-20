// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

use crate::i18n::{ChromeStrings, Lang, PageLang};
use minijinja::{context, Environment};
use pulldown_cmark::{html, Options, Parser};

pub fn render_markdown(md: &str) -> String {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    let parser = Parser::new_ext(md, opts);
    let mut out = String::new();
    html::push_html(&mut out, parser);
    out
}

/// The sidebar only exists on component-detail pages now (nav/sidebar structural
/// rebuild — the mockup has no sidebar at all on any other page type; see BRIEF Phase
/// 25), scoped to just the current component's own real category (generic UI / GIS /
/// Knowledge Platform / Org Charts), never the full mixed section tree this function
/// used to render on every route. Returns an empty string for every non-component
/// route (and `shell.html`'s `{% if nav_html %}` treats empty as "no sidebar at all"),
/// and for a components route whose slug isn't found in any group (shouldn't happen for
/// a real slug, but fails safe rather than panicking).
pub fn render_nav(
    env: &Environment<'static>,
    component_groups: &[(String, Vec<String>)],
    active_section: &str,
    active_slug: &str,
) -> String {
    if active_section != "components" {
        return String::new();
    }
    let Some((label, slugs)) = crate::vault::component_group_for_slug(component_groups, active_slug) else {
        return String::new();
    };
    let (heading, see_also) = crate::vault::sidebar_heading_for_group_label(label);
    env.get_template("nav.html")
        .expect("nav.html missing")
        .render(context! {
            heading => heading,
            see_also => see_also,
            slugs => slugs,
            active_slug => active_slug,
        })
        .expect("render nav.html failed")
}

pub fn render_tab_bar(
    env: &Environment<'static>,
    section: &str,
    slug: &str,
    tabs: &[String],
    active_tab: &str,
) -> String {
    env.get_template("tab_bar.html")
        .expect("tab_bar.html missing")
        .render(context! {
            section => section,
            slug => slug,
            tabs => tabs,
            active_tab => active_tab,
        })
        .expect("render tab_bar.html failed")
}

/// Generic, site-level description used as the floor for og:description/meta
/// description on pages without a more specific one. Grounded, not invented: no
/// per-page description field exists anywhere in the real vault content (checked
/// directly — the only frontmatter-shaped match across the whole vault was a
/// body-text false positive, not a real field), so callers pass an explicit,
/// per-route description rather than relying on template-side fallback logic.
pub const SITE_DESCRIPTION: &str = "PointSav design system — tokens, components, and research documentation for the PointSav/Woodfine product family, covering visual language and UI primitives.";

#[allow(clippy::too_many_arguments)]
pub fn shell(
    env: &Environment<'static>,
    vault: &std::path::Path,
    component_groups: &[(String, Vec<String>)],
    site_origin: &str,
    title: &str,
    description: &str,
    path: &str,
    page_lang: &PageLang,
    nav_html: &str,
    tab_bar: &str,
    page_title: &str,
    content: &str,
) -> String {
    let chrome = ChromeStrings::for_lang(page_lang.lang);
    // Real, live-computed counts for the footer identity plate — never hardcoded (this
    // initiative's own locked rule: every number on the site must be real, not
    // illustrative). Token count sums every tier's flattened entries the gallery itself
    // renders; component count sums every category group's slugs.
    let total_tokens: usize = crate::tokens_gallery::load_and_flatten(vault)
        .iter()
        .map(|tier| tier.groups.iter().map(|g| g.entries.len()).sum::<usize>())
        .sum();
    let total_components: usize = component_groups.iter().map(|(_, slugs)| slugs.len()).sum();
    let lang_code = page_lang.lang.code();
    // Toggle points at THIS page's counterpart in the other language; empty when no
    // counterpart exists yet, so shell() never links to a page that isn't real.
    let toggle_href = match page_lang.lang {
        Lang::Es => page_lang.alt_en_path.clone(),
        Lang::En => page_lang.alt_es_path.clone(),
    };
    let footer_html = env
        .get_template("footer.html")
        .expect("footer.html missing")
        .render(context! {
            version => env!("CARGO_PKG_VERSION"),
            site_label => chrome.footer_site_label,
            canonical_title => chrome.footer_canonical_title,
            overview => chrome.footer_overview,
            components => chrome.footer_components,
            tokens => chrome.footer_tokens,
            guidelines => chrome.footer_guidelines,
            elements => chrome.nav_elements,
            paper => chrome.footer_paper,
            writing => chrome.footer_writing,
            bundles => chrome.footer_bundles,
            adoption => chrome.footer_adoption,
            get_started => chrome.footer_get_started,
            knowledge_platform => chrome.nav_knowledge_platform,
            org_charts => chrome.nav_org_charts,
            releases => chrome.nav_releases,
            machine_surface_title => chrome.footer_machine_surface_title,
            family_prefix => chrome.footer_family_prefix,
            copyright => chrome.footer_copyright,
            live => chrome.footer_live,
            powered_by => chrome.footer_powered_by,
            total_tokens => total_tokens,
            total_components => total_components,
            identity_tagline => chrome.footer_identity_tagline,
            identity_standards => chrome.footer_identity_standards,
            identity_license => chrome.footer_identity_license,
            identity_source_label => chrome.footer_identity_source_label,
            network_title => chrome.footer_network_title,
            network_pointsav => chrome.footer_network_pointsav,
            network_documentation => chrome.footer_network_documentation,
            network_software => chrome.footer_network_software,
            network_woodfine => chrome.footer_network_woodfine,
            locations => chrome.footer_locations,
            trademark => chrome.footer_trademark,
            disclosure_notice => chrome.footer_disclosure_notice,
            disclosure_summary => chrome.disclosure_summary,
            disclosure_label => chrome.disclosure_label,
            disclosure_body => chrome.disclosure_body,
        })
        .expect("render footer.html failed");
    // Built server-side with real JSON string escaping and inserted via `| safe` in the
    // template — letting minijinja's HTML auto-escape touch this would corrupt the `url`
    // field (it HTML-entity-escapes `/` to `&#x2f;`, which is harmless inside an HTML
    // attribute but is literal, un-decoded garbage inside a <script> block, breaking the
    // URL for any JSON-LD consumer). Confirmed by direct curl against a local test
    // instance before landing this fix.
    let json_ld = serde_json::json!({
        "@context": "https://schema.org",
        "@type": "SoftwareApplication",
        "name": "PointSav Design System",
        "applicationCategory": "DesignSystem",
        "url": format!("{site_origin}{path}"),
        "provider": {"@type": "Organization", "@id": "https://pointsav.com/#organization"},
    })
    .to_string();
    env.get_template("shell.html")
        .expect("shell.html missing")
        .render(context! {
            title => title,
            description => description,
            path => path,
            site_origin => site_origin,
            json_ld => json_ld,
            lang => lang_code,
            alt_en_path => page_lang.alt_en_path,
            alt_es_path => page_lang.alt_es_path,
            toggle_href => toggle_href,
            toggle_label => chrome.lang_switch_label,
            skip_to_content => chrome.skip_to_content,
            search_placeholder => chrome.search_placeholder,
            search_aria_label => chrome.search_aria_label,
            theme_toggle_aria_label => chrome.theme_toggle_aria_label,
            nav_toggle_aria_label => chrome.nav_toggle_aria_label,
            nav_tokens => chrome.nav_tokens,
            nav_components => chrome.nav_components,
            nav_guidelines => chrome.nav_guidelines,
            nav_accessibility => chrome.nav_accessibility,
            nav_elements => chrome.nav_elements,
            nav_writing => chrome.nav_writing,
            nav_paper => chrome.nav_paper,
            nav_agents => chrome.nav_agents,
            nav_adoption => chrome.nav_adoption,
            nav_product_lines => chrome.nav_product_lines,
            nav_knowledge_platform => chrome.nav_knowledge_platform,
            nav_org_charts => chrome.nav_org_charts,
            nav_more => chrome.nav_more,
            nav_releases => chrome.nav_releases,
            nav_html => nav_html,
            tab_bar => tab_bar,
            page_title => page_title,
            content => content,
            footer_html => footer_html,
        })
        .expect("render shell.html failed")
}
