// SPDX-License-Identifier: FSL-1.1-ALv2
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

//! The continuous chrome that wraps every page: `<head>`, sitenotice, sticky
//! white header, off-canvas mobile nav, and the institutional footer.
//!
//! `page()` is the single public entry — it composes the whole document as one
//! `html!{}` tree so maud balances every tag. Structure follows Wikipedia
//! Vector 2022; the visual system (white header, brand-as-accent) lives in
//! `static/{tokens,app}.css`. Class names match the `k-*` manifest.

use maud::{html, Markup, PreEscaped, DOCTYPE};

use super::tenant::Tenant;
use crate::content::render::Heading;
use crate::history::{FileDiff, Revision};

/// "article" / "articles" for a count.
fn count_word(n: usize) -> &'static str {
    if n == 1 {
        "article"
    } else {
        "articles"
    }
}

/// `<head>` contents (not the `<head>` element itself — `page()` supplies that).
/// `description` may be empty (e.g. listing pages).
pub fn doc_head(title: &str, description: &str, tenant: Tenant) -> Markup {
    // Don't double-brand when the page title already is the site name (home).
    let full_title = if title == tenant.home_label() {
        title.to_string()
    } else {
        format!("{title} — {}", tenant.home_label())
    };
    html! {
        meta charset="utf-8";
        meta name="viewport" content="width=device-width, initial-scale=1";
        meta name="color-scheme" content="light dark";
        meta name="theme-color" content=(tenant.accent());
        title { (full_title) }
        @if !description.is_empty() {
            meta name="description" content=(description);
        }
        meta property="og:type" content="website";
        meta property="og:site_name" content=(tenant.home_label());
        meta property="og:title" content=(full_title);
        @if !description.is_empty() {
            meta property="og:description" content=(description);
        }
        // schema.org structured data — identifies the site + its publisher to
        // search engines and AI agents (SYS-ADR-07-safe: static, no user data).
        script type="application/ld+json" {
            (PreEscaped(format!(
                r#"{{"@context":"https://schema.org","@type":"WebSite","name":"{}","url":"{}","publisher":{{"@type":"Organization","name":"{}"}}}}"#,
                tenant.home_label(),
                tenant.home_url().trim_end_matches('/'),
                tenant.issuer()
            )))
        }
        link rel="icon" type="image/svg+xml" href="/static/favicon.svg";
        link rel="stylesheet" href="/static/fonts.css";
        link rel="stylesheet" href="/static/tokens.css";
        link rel="stylesheet" href="/static/app.css";
        link rel="stylesheet" href="/static/content.css";
        link rel="stylesheet" href="/static/syntax.css";
        // Pre-paint theme guard — sets data-theme before first paint (no flash).
        // Key 'k-theme' is shared with app.js.
        script {
            (PreEscaped(r#"(function(){try{var t=localStorage.getItem('k-theme');if(t!=='light'&&t!=='dark'){t=matchMedia('(prefers-color-scheme: dark)').matches?'dark':'light';}document.documentElement.setAttribute('data-theme',t);}catch(e){}})();"#))
        }
    }
}

/// A search block. Header and drawer copies use different input ids.
fn search_block(input_id: &str, query: &str) -> Markup {
    html! {
        div."k-search" {
            form."k-search__form" role="search" action="/search" method="get" {
                svg."k-search__icon" viewBox="0 0 20 20" aria-hidden="true" focusable="false" {
                    path d="M12.9 14.32a8 8 0 1 1 1.41-1.41l5.35 5.33-1.42 1.42-5.33-5.34zM8 14A6 6 0 1 0 8 2a6 6 0 0 0 0 12z" {}
                }
                label."k-visually-hidden" for=(input_id) { "Search this registry" }
                input."k-search__input" id=(input_id) type="search" name="q" value=(query)
                    placeholder="Search" autocomplete="off" spellcheck="false";
                button."k-search__button" type="submit" { "Search" }
            }
        }
    }
}

/// Logo mark — a document-of-record glyph (folded-corner page) in currentColor,
/// which inherits `--k-accent` from `.k-logo`.
fn logo_mark() -> Markup {
    html! {
        svg."k-logo__mark" viewBox="0 0 24 24" width="22" height="22"
            aria-hidden="true" focusable="false" {
            path fill="currentColor"
                d="M6 2h7.5L19 7.5V22H6a1 1 0 0 1-1-1V3a1 1 0 0 1 1-1zm7 1.6V8h4.4L13 3.6zM8 12h8v1.5H8V12zm0 3.5h8V17H8v-1.5z" {}
        }
    }
}

/// Top utility strip — cross-property links, mirroring the marketing site's
/// right-hand nav (Home · Monorepo · Design System · GitHub, per tenant).
/// External links open in a new tab.
pub fn utility_bar(tenant: Tenant) -> Markup {
    html! {
        div."k-utility" {
            div."k-utility__inner" {
                // Left: the maintaining entity → its corporate home.
                a."k-utility__home" href=(tenant.marketing_home()) {
                    (tenant.entity_name())
                }
                // Right: the property links (GitHub · Software · Design System).
                nav."k-utility__nav" aria-label="Network" {
                    @for (label, url) in tenant.cross_property_links() {
                        a."k-utility__link" href=(url) target="_blank" rel="noopener" {
                            (label)
                        }
                    }
                }
            }
        }
    }
}

/// Sticky white header: logo · search · controls.
pub fn header(tenant: Tenant, _lang: &str, query: &str) -> Markup {
    html! {
        header."k-header" role="banner" {
            div."k-header__inner" {
                div."k-header__start" {
                    a."k-logo" href="/" aria-label=(tenant.home_label()) {
                        (logo_mark())
                        span."k-logo__lockup" {
                            span."k-logo__brand" { (tenant.brand_word()) }
                            span."k-logo__descriptor" { (tenant.descriptor()) }
                        }
                    }
                }
                div."k-header__center" { (search_block("k-search-input", query)) }
                div."k-header__end" {
                    nav."k-controls" aria-label="Site controls" {
                        // Language toggle hidden until the /es routes ship (no dead link).
                        button."k-control k-control--theme" type="button"
                               aria-pressed="false" aria-label="Switch theme" {
                            svg."k-control__icon k-icon-moon" viewBox="0 0 20 20" aria-hidden="true" focusable="false" {
                                path d="M17 12.3A7 7 0 0 1 7.7 3 7 7 0 1 0 17 12.3z" {}
                            }
                            svg."k-control__icon k-icon-sun" viewBox="0 0 20 20" aria-hidden="true" focusable="false" {
                                path d="M10 3a1 1 0 0 1 1 1v1a1 1 0 1 1-2 0V4a1 1 0 0 1 1-1zm0 10a3 3 0 1 1 0-6 3 3 0 0 1 0 6zm0 2a1 1 0 0 1 1 1v1a1 1 0 1 1-2 0v-1a1 1 0 0 1 1-1zm7-5a1 1 0 0 1-1 1h-1a1 1 0 1 1 0-2h1a1 1 0 0 1 1 1zM5 10a1 1 0 0 1-1 1H3a1 1 0 1 1 0-2h1a1 1 0 0 1 1 1zm10.07-5.07a1 1 0 0 1 0 1.41l-.7.71a1 1 0 1 1-1.42-1.42l.71-.7a1 1 0 0 1 1.41 0zM6.05 13.95a1 1 0 0 1 0 1.41l-.71.71A1 1 0 0 1 3.93 14.66l.7-.71a1 1 0 0 1 1.42 0zm9.02.71a1 1 0 0 1-1.42 1.42l-.7-.71a1 1 0 0 1 1.41-1.41l.71.7zM6.05 6.05a1 1 0 0 1-1.42 0l-.7-.71A1 1 0 0 1 5.34 3.93l.71.7a1 1 0 0 1 0 1.42z" {}
                            }
                        }
                        button."k-control k-control--menu" type="button"
                               aria-controls="k-nav-drawer" aria-expanded="false"
                               aria-label="Open menu" {
                            svg."k-control__icon" viewBox="0 0 20 20" aria-hidden="true" focusable="false" {
                                path d="M3 5h14v2H3V5zm0 4h14v2H3V9zm0 4h14v2H3v-2z" {}
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Off-canvas mobile nav drawer + overlay (ships hidden; app.js manages state).
pub fn mobile_nav(tenant: Tenant, query: &str) -> Markup {
    html! {
        div."k-overlay" #"k-overlay" hidden {}
        div."k-nav-drawer" #"k-nav-drawer" role="dialog" aria-modal="true"
            aria-label=(format!("{} menu", tenant.home_label())) aria-hidden="true" hidden {
            div."k-nav-drawer__header" {
                span."k-nav-drawer__title" { "Menu" }
                button."k-nav-drawer__close" type="button" aria-label="Close menu" {
                    svg."k-control__icon" viewBox="0 0 20 20" aria-hidden="true" focusable="false" {
                        path d="M5 5l10 10M15 5L5 15" stroke="currentColor" stroke-width="2" fill="none" {}
                    }
                }
            }
            div."k-nav-drawer__body" {
                div."k-nav-drawer__search" { (search_block("k-search-input-mobile", query)) }
                section."k-nav-section" {
                    h2."k-nav-section__title" { "Navigate" }
                    ul."k-nav-list" {
                        li { a."k-nav-link" href="/" { "Home" } }
                        li { a."k-nav-link" href="/special/all-pages" { "Index of record" } }
                        li { a."k-nav-link" href="/special/recent-changes" { "Recent changes" } }
                    }
                }
                section."k-nav-section" {
                    h2."k-nav-section__title" { "Resources" }
                    ul."k-nav-list" {
                        li { a."k-nav-link" href="/feed.atom" { "Atom feed" } }
                    }
                }
                section."k-nav-section" {
                    h2."k-nav-section__title" { "PointSav network" }
                    ul."k-nav-list" {
                        li {
                            a."k-nav-link" href=(tenant.marketing_home()) target="_blank" rel="noopener" {
                                (tenant.entity_name())
                            }
                        }
                        @for (label, url) in tenant.cross_property_links() {
                            li { a."k-nav-link" href=(url) target="_blank" rel="noopener" { (label) } }
                        }
                    }
                }
            }
        }
    }
}

/// Footer — mirrors the marketing footer (cities line) with plain-language
/// link columns. Disclaimer and Contact live here only.
pub fn footer(tenant: Tenant) -> Markup {
    html! {
        footer."k-footer" role="contentinfo" {
            div."k-footer__inner" {
                div."k-footer__grid" {
                    div."k-footer__col" {
                        h2."k-footer__col-title" { "Browse" }
                        ul."k-footer__list" {
                            li { a."k-footer__link" href="/" { "Home" } }
                            li { a."k-footer__link" href="/special/all-pages" { "All articles" } }
                            li { a."k-footer__link" href="/special/recent-changes" { "Recent changes" } }
                        }
                    }
                    div."k-footer__col" {
                        h2."k-footer__col-title" { "This site" }
                        ul."k-footer__list" {
                            li { a."k-footer__link" href="/wiki/about" { "About" } }
                            li { a."k-footer__link" href="/wiki/disclaimers" { "Disclaimer" } }
                            li { a."k-footer__link" href="/wiki/contact" { "Contact us" } }
                            li { a."k-footer__link" href="/wiki/privacy" { "Privacy" } }
                        }
                    }
                    div."k-footer__col" {
                        h2."k-footer__col-title" { "Network" }
                        ul."k-footer__list" {
                            li { a."k-footer__link" href=(tenant.marketing_home()) target="_blank" rel="noopener" { (tenant.entity_name()) } }
                            @for (label, url) in tenant.cross_property_links() {
                                li { a."k-footer__link" href=(url) target="_blank" rel="noopener" { (label) } }
                            }
                            // Cross-company link last — related but separate org.
                            @let (other_label, other_url) = tenant.other_org();
                            li { a."k-footer__link" href=(other_url) target="_blank" rel="noopener" { (other_label) } }
                        }
                    }
                }
                // Base row — copyright + cities on the left, badges on the right.
                div."k-footer__base" {
                    div."k-footer__meta" {
                        div."k-footer__cities" {
                            @for (i, city) in tenant.cities().iter().enumerate() {
                                @if i > 0 { span."k-footer__cities-sep" aria-hidden="true" { "|" } }
                                span { (city) }
                            }
                        }
                        p."k-footer__copyright" {
                            "\u{00a9} 2026 " (tenant.copyright_holder())
                        }
                    }
                    div."k-footer__badges" {
                        // Powered by MediaKit (the engine).
                        a."k-badge" href="/wiki/about" {
                            span."k-badge__glyph" aria-hidden="true" {
                                svg viewBox="0 0 24 24" width="15" height="15" {
                                    path fill="currentColor" d="M3 5.5A1.5 1.5 0 0 1 4.5 4h15A1.5 1.5 0 0 1 21 5.5v13A1.5 1.5 0 0 1 19.5 20h-15A1.5 1.5 0 0 1 3 18.5v-13zM6 8v8l3.2-2.4L6 8zm7 6.5h5V13h-5v1.5zm0-3h5V10h-5v1.5z" {}
                                }
                            }
                            span."k-badge__text" {
                                span."k-badge__lead" { "Powered by" }
                                span."k-badge__name" { "MediaKit" }
                            }
                        }
                        // Content licence — per tenant (CC BY for the open docs
                        // library; CC BY-ND for the verbatim disclosure records).
                        a."k-badge k-badge--license" href=(tenant.license_url())
                          target="_blank" rel="noopener license"
                          aria-label={ "Content licensed " (tenant.license_name()) } {
                            span."k-badge__cc" aria-hidden="true" {
                                img."k-cc-icon" src="/static/cc.svg" alt="" width="20" height="20";
                                img."k-cc-icon" src="/static/cc-by.svg" alt="" width="20" height="20";
                                @if tenant.license_nd() {
                                    img."k-cc-icon" src="/static/cc-nd.svg" alt="" width="20" height="20";
                                }
                            }
                            span."k-badge__text" {
                                span."k-badge__lead" { "Licensed" }
                                span."k-badge__name" { (tenant.license_name()) }
                            }
                        }
                    }
                }
                // Persistent one-line disclaimer (always visible; the band expands it).
                p."k-footer__disclaimer" { (tenant.disclaimer_line()) }
                // Trademark notice — verbatim from canonical TRADEMARK.md. The marks
                // are reserved independently of the CC BY 4.0 content licence, so no
                // blanket "all rights reserved" (content is openly licensed).
                p."k-footer__trademark" {
                    "Woodfine Capital Projects\u{2122}, MCorp\u{2122}, PointSav Digital Systems\u{2122}, Totebox Orchestration\u{2122}, Totebox Archive\u{2122}, and Capability Geometry\u{2122} are trademarks of Woodfine Capital Projects Inc., used in Canada, the United States, Latin America, and Europe. All other trademarks are the property of their respective owners."
                }
            }
        }
    }
}

/// Format an ISO `YYYY-MM-DD` as "25 May 2026"; pass anything else through.
fn format_date(iso: &str) -> String {
    const MONTHS: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    let p: Vec<&str> = iso.trim().split('-').collect();
    if p.len() == 3 {
        if let (Ok(y), Ok(m), Ok(d)) = (
            p[0].parse::<i32>(),
            p[1].parse::<usize>(),
            p[2].parse::<u32>(),
        ) {
            if (1..=12).contains(&m) {
                return format!("{d} {} {y}", MONTHS[m - 1]);
            }
        }
    }
    iso.to_string()
}

/// The article action-tab bar (Wikipedia Vector 2022 pattern). `active` is
/// "article" or "history"; the current one is a non-link span, the other links.
/// (No "Notes" placeholder — a reviewer-annotation channel ships as a real tab or
/// not at all; no dead controls in front of auditors.)
fn tab_bar(slug: &str, active: &str) -> Markup {
    html! {
        nav."k-tabs" aria-label="Views" {
            @if active == "article" {
                span."k-tab k-tab--active" aria-current="page" { "Article" }
            } @else {
                a."k-tab" href={ "/wiki/" (slug) } { "Article" }
            }
            @if active == "history" {
                span."k-tab k-tab--active" aria-current="page" { "History" }
            } @else {
                a."k-tab" href={ "/history/" (slug) } { "History" }
            }
        }
    }
}

/// Wrap a rendered article body in the reading shell: action tabs (+ "Last
/// updated"), ruled title, prose column. `body_html` is trusted, pre-rendered.
/// `sha` is the short commit hash the render is drawn from (provenance line);
/// `asof` is set only for the point-in-time view (a historical revision) and
/// carries that revision's date, which switches the meta label + shows a banner.
pub fn article(
    title: &str,
    slug: &str,
    updated: Option<&str>,
    sha: Option<&str>,
    asof: Option<&str>,
    alt_lang: Option<(&str, &str)>,
    body_html: &str,
) -> Markup {
    html! {
        article."k-article" {
            div."k-article-nav" {
                (tab_bar(slug, "article"))
                @if updated.is_some() || asof.is_some() || sha.is_some() {
                    p."k-article__meta" {
                        @if let Some(d) = asof {
                            "Revision as of "
                            time."k-article__date" datetime=(d) { (format_date(d)) }
                        } @else if let Some(d) = updated.filter(|s| !s.trim().is_empty()) {
                            "Last updated "
                            time."k-article__date" datetime=(d) { (format_date(d)) }
                        }
                        @if let Some(s) = sha {
                            " \u{00b7} " code."k-article__sha" { (s) }
                        }
                        @if let Some((url, label)) = alt_lang {
                            " \u{00b7} " a."k-article__lang" href=(url) { (label) }
                        }
                    }
                }
            }
            @if let Some(d) = asof {
                div."k-asof" role="note" {
                    strong { "Historical revision" }
                    " — this record as it stood on " (format_date(d)) ", not the current version. "
                    a."k-asof__link" href={ "/wiki/" (slug) } { "View the current record \u{2192}" }
                }
            }
            h1."k-article__title" { (title) }
            div."k-prose" { (PreEscaped(body_html)) }
        }
    }
}

/// Article revision history — the git log of the article's file (the History tab).
pub fn history_page(title: &str, slug: &str, issuer: &str, revs: &[Revision]) -> Markup {
    html! {
        article."k-article" {
            div."k-article-nav" { (tab_bar(slug, "history")) }
            h1."k-article__title" { (title) }
            div."k-home__stat" { strong { (revs.len()) } " " (count_word_rev(revs.len())) }
            p."k-history__note" {
                "Maintained by " (issuer) ". Each revision is content-addressed by its commit hash."
            }
            @if revs.is_empty() {
                p."k-searchpage__hint" {
                    "No revision history found for this article — it may not yet be committed to the content repository."
                }
            } @else {
                ul."k-history" {
                    @for r in revs {
                        li."k-history__item" {
                            time."k-history__date" datetime=(r.date_iso) { (format_date(&r.date_iso)) }
                            a."k-history__msg" href={ "/history/" (slug) "?rev=" (r.sha) } { (r.message) }
                            span."k-history__meta" {
                                code."k-history__sha" { (r.short_sha) }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// "revision" / "revisions" for a count.
fn count_word_rev(n: usize) -> &'static str {
    if n == 1 {
        "revision"
    } else {
        "revisions"
    }
}

fn diff_line_class(origin: char) -> &'static str {
    match origin {
        '+' => "k-diff__line k-diff__line--add",
        '-' => "k-diff__line k-diff__line--del",
        'H' => "k-diff__line k-diff__line--hunk",
        _ => "k-diff__line",
    }
}

/// A single revision's diff for one article (reached from the History tab).
pub fn diff_page(title: &str, slug: &str, issuer: &str, diff: &FileDiff) -> Markup {
    html! {
        article."k-article" {
            div."k-article-nav" { (tab_bar(slug, "history")) }
            h1."k-article__title" { (title) }
            div."k-diff__head" {
                a."k-diff__back" href={ "/history/" (slug) } { "\u{2190} All revisions" }
                p."k-diff__meta" {
                    code."k-history__sha" { (diff.short_sha) }
                    " \u{00b7} " (issuer)
                    " \u{00b7} " time datetime=(diff.date_iso) { (format_date(&diff.date_iso)) }
                }
                p."k-diff__msg" { (diff.message) }
                p."k-diff__asof" {
                    a href={ "/wiki/" (slug) "?rev=" (diff.short_sha) } {
                        "View the full record as of this revision \u{2192}"
                    }
                }
            }
            @if diff.lines.is_empty() {
                p."k-searchpage__hint" { "No textual changes to this file in this revision." }
            } @else {
                pre."k-diff" {
                    @for l in &diff.lines {
                        span class=(diff_line_class(l.origin)) { (l.content) "\n" }
                    }
                }
            }
        }
    }
}

/// Home page — the front page (Main Page): title, the index lede, an article
/// count, and a "Browse by area" grid of category cards. `cats` is
/// `(slug, label, count)` in display order.
pub fn home_page(
    tenant: Tenant,
    lede_html: &str,
    total: usize,
    cats: &[(String, String, usize)],
    guides: &[(String, String, String)],
) -> Markup {
    let guides_shown = guides.len().min(8);
    html! {
        div."k-home" {
            h1."k-article__title" { (tenant.home_label()) }
            @if !lede_html.is_empty() {
                div."k-prose k-home__lede" { (PreEscaped(lede_html)) }
            }
            div."k-home__stat" {
                strong { (total) } " " (count_word(total)) " in the registry"
            }
            section."k-home__browse" aria-label="Browse by area" {
                h2."k-home__browse-title" { "Browse by area" }
                div."k-home__grid" {
                    @for (slug, label, count) in cats {
                        a."k-cat-card" href={ "/category/" (slug) } {
                            span."k-cat-card__name" { (label) }
                            span."k-cat-card__count" { (count) " " (count_word(*count)) }
                        }
                    }
                }
            }

            // How-to guides — operational runbooks, distinct from the reference topics.
            @if !guides.is_empty() {
                section."k-home__guides" aria-label="How-to guides" {
                    div."k-home__guides-head" {
                        h2."k-home__browse-title" { "How-to guides" }
                        a."k-home__browse-all" href="/category/how-to" {
                            "All " (guides.len()) " guides \u{2192}"
                        }
                    }
                    p."k-home__guides-lede" {
                        "Step-by-step operational runbooks — how to install, configure, and run the platform."
                    }
                    ul."k-guide-list" {
                        @for (slug, title, desc) in guides.iter().take(guides_shown) {
                            li."k-guide-card" {
                                a."k-guide-card__title" href={ "/wiki/" (slug) } {
                                    span."k-guide-list__icon" aria-hidden="true" {
                                        svg viewBox="0 0 16 16" width="14" height="14" {
                                            path fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" d="M6 3.5 10.5 8 6 12.5" {}
                                        }
                                    }
                                    (title)
                                }
                                @if !desc.is_empty() {
                                    p."k-guide-card__desc" { (desc) }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Category listing — a category's articles as a scannable index.
/// `docs` is `(slug, title, description)` sorted.
pub fn category_index(label: &str, docs: &[(String, String, String)]) -> Markup {
    html! {
        div."k-catpage" {
            div."k-catpage__eyebrow" { "Category" }
            h1."k-article__title" { (label) }
            div."k-home__stat" { strong { (docs.len()) } " " (count_word(docs.len())) }
            ul."k-cat-list" {
                @for (slug, title, desc) in docs {
                    li."k-cat-entry" {
                        a."k-cat-entry__title" href={ "/wiki/" (slug) } { (title) }
                        @if !desc.is_empty() {
                            p."k-cat-entry__desc" { (desc) }
                        }
                    }
                }
            }
        }
    }
}

/// A generic index page — "Index of record" (A–Z all articles) and "Recent
/// changes". `items` is `(slug, title, meta)`; `meta` is a description or a date.
pub fn special_list(heading: &str, eyebrow: &str, items: &[(String, String, String)]) -> Markup {
    html! {
        div."k-catpage" {
            div."k-catpage__eyebrow" { (eyebrow) }
            h1."k-article__title" { (heading) }
            div."k-home__stat" { strong { (items.len()) } " " (count_word(items.len())) }
            ul."k-cat-list" {
                @for (slug, title, meta) in items {
                    li."k-cat-entry" {
                        a."k-cat-entry__title" href={ "/wiki/" (slug) } { (title) }
                        @if !meta.is_empty() { p."k-cat-entry__desc" { (meta) } }
                    }
                }
            }
        }
    }
}

/// A minimal chrome-wrapped message page (used for 404 — never a bare error).
pub fn simple_message(heading: &str, text: &str) -> Markup {
    html! {
        div."k-catpage" {
            h1."k-article__title" { (heading) }
            p."k-searchpage__hint" { (text) }
            p { a href="/" { "\u{2190} Back to the main page" } }
        }
    }
}

/// Search results page — a query box plus result cards (same card style as the
/// category listing). `results` is `(slug, title, description)`, ranked.
pub fn search_results(query: &str, results: &[(String, String, String)]) -> Markup {
    let q = query.trim();
    html! {
        div."k-catpage" {
            div."k-catpage__eyebrow" { "Search" }
            h1."k-article__title" { "Search" }
            // The header search bar carries the query — no second on-page box.
            @if q.is_empty() {
                p."k-searchpage__hint" { "Use the search bar above to search article titles and text." }
            } @else {
                div."k-home__stat" {
                    strong { (results.len()) } " " (count_word(results.len()))
                    " for \u{201c}" (q) "\u{201d}"
                }
                @if results.is_empty() {
                    p."k-searchpage__hint" { "No articles matched. Try different or fewer terms." }
                } @else {
                    ul."k-cat-list" {
                        @for (slug, title, desc) in results {
                            li."k-cat-entry" {
                                a."k-cat-entry__title" href={ "/wiki/" (slug) } { (title) }
                                @if !desc.is_empty() {
                                    p."k-cat-entry__desc" { (desc) }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The left navigation column (Wikipedia Vector 2022 pattern): Main page,
/// Browse-by-area, Guides. Sticky on desktop; hidden below the tablet breakpoint
/// where the off-canvas drawer covers navigation. `cats` is `(slug, label)`.
fn sidebar(tenant: Tenant, cats: &[(String, String)], toc: &[Heading]) -> Markup {
    html! {
        aside."k-sidebar" aria-label="Site navigation" {
            nav."k-sidenav" {
                a."k-sidenav__home" href="/" { "Main page" }
                @if !cats.is_empty() {
                    div."k-sidenav__group" {
                        h2."k-sidenav__heading" { "Browse by area" }
                        ul."k-sidenav__list" {
                            @for (slug, label) in cats {
                                li { a."k-sidenav__link" href={ "/category/" (slug) } { (label) } }
                            }
                        }
                    }
                }
                @if tenant.serves_guides() {
                    div."k-sidenav__group" {
                        h2."k-sidenav__heading" { "Guides" }
                        ul."k-sidenav__list" {
                            li { a."k-sidenav__link" href="/category/how-to" { "How-to guides" } }
                        }
                    }
                }
                // Article table of contents (present only on pages with headings).
                @if !toc.is_empty() {
                    nav."k-sidenav__group k-toc" aria-label="Contents" {
                        h2."k-sidenav__heading" { "Contents" }
                        ul."k-toc__list" {
                            @for h in toc {
                                li."k-toc__item"."k-toc__item--sub"[h.level == 3] {
                                    a."k-toc__link" href={ "#" (h.id) } { (h.text) }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The "Important Information" band above the footer (native `<details>`, no JS).
/// Content is the counsel-owned `important-information.md` when present, else a
/// safe tenant default; forced open in print so the record copy carries it.
fn compliance_band(tenant: Tenant, important_info: Option<&str>) -> Markup {
    html! {
        section."k-compliance" aria-label="Important information" {
            details."k-compliance__details" {
                summary."k-compliance__summary" { "Important Information" }
                div."k-compliance__body k-prose" {
                    @if let Some(html) = important_info {
                        (PreEscaped(html))
                    } @else {
                        p {
                            "This site presents records maintained by " (tenant.issuer())
                            ". The information is provided for general information only and does not "
                            "constitute an offer to sell, a solicitation of an offer to buy, or "
                            "investment, legal, tax, or accounting advice. Statements regarding "
                            "planned, intended, or targeted future activities are forward-looking and "
                            "subject to change without notice; they are not undertaken to be updated "
                            "except as required by law."
                        }
                    }
                    p."k-compliance__more" {
                        a href="/wiki/disclaimers" { "Read the full disclaimer \u{2192}" }
                    }
                }
            }
        }
    }
}

/// The full document as one balanced tree. `cats` drives the sidebar nav;
/// `disclaimer` is the Important Information band content (None → tenant default).
#[allow(clippy::too_many_arguments)]
pub fn page(
    tenant: Tenant,
    lang: &str,
    head: Markup,
    body: Markup,
    cats: &[(String, String)],
    toc: &[Heading],
    query: &str,
    disclaimer: Option<&str>,
) -> Markup {
    html! {
        (DOCTYPE)
        html lang=(lang) data-instance=(tenant.instance_str()) {
            head { (head) }
            body {
                a."k-skip-link" href="#k-main" { "Skip to content" }
                (mobile_nav(tenant, query))
                div."k-page" {
                    (utility_bar(tenant))
                    (header(tenant, lang, query))
                    div."k-shell" {
                        (sidebar(tenant, cats, toc))
                        main."k-page__body" #"k-main" tabindex="-1" { (body) }
                    }
                    (compliance_band(tenant, disclaimer))
                    (footer(tenant))
                }
                script src="/static/app.js" defer {}
            }
        }
    }
}
