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
fn search_block(input_id: &str) -> Markup {
    html! {
        div."k-search" {
            form."k-search__form" role="search" action="/search" method="get" {
                svg."k-search__icon" viewBox="0 0 20 20" aria-hidden="true" focusable="false" {
                    path d="M12.9 14.32a8 8 0 1 1 1.41-1.41l5.35 5.33-1.42 1.42-5.33-5.34zM8 14A6 6 0 1 0 8 2a6 6 0 0 0 0 12z" {}
                }
                label."k-visually-hidden" for=(input_id) { "Search this registry" }
                input."k-search__input" id=(input_id) type="search" name="q"
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
pub fn header(tenant: Tenant, lang: &str) -> Markup {
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
                div."k-header__center" { (search_block("k-search-input")) }
                div."k-header__end" {
                    nav."k-controls" aria-label="Site controls" {
                        a."k-control k-control--lang" href="/es/" aria-label="Change language" {
                            svg."k-control__icon" viewBox="0 0 20 20" aria-hidden="true" focusable="false" {
                                path d="M10 2a8 8 0 1 0 0 16 8 8 0 0 0 0-16zm5.3 5h-2.24a12.3 12.3 0 0 0-1.1-3.02A6.02 6.02 0 0 1 15.3 7zM10 3.8c.63.9 1.13 1.97 1.46 3.2H8.54C8.87 5.77 9.37 4.7 10 3.8zM3.8 12A6.1 6.1 0 0 1 3.6 10c0-.7.08-1.37.2-2h2.5a13.7 13.7 0 0 0 0 4H3.8zm.9 2h2.24c.28 1.12.66 2.14 1.1 3.02A6.02 6.02 0 0 1 4.7 14zm2.24-8H4.7a6.02 6.02 0 0 1 3.34-2.98A12.3 12.3 0 0 0 6.94 6zM10 16.2c-.63-.9-1.13-1.97-1.46-3.2h2.92C11.13 14.23 10.63 15.3 10 16.2zm1.79-4.2H8.2a12 12 0 0 1 0-4h3.6a12 12 0 0 1 0 4zm.17 5.02c.44-.88.82-1.9 1.1-3.02h2.24a6.02 6.02 0 0 1-3.34 2.98zM13.9 12a13.7 13.7 0 0 0 0-4h2.5c.12.63.2 1.3.2 2 0 .69-.08 1.36-.2 2h-2.5z" {}
                            }
                            span."k-control__label" { (lang.to_uppercase()) }
                        }
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
pub fn mobile_nav(tenant: Tenant) -> Markup {
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
                div."k-nav-drawer__search" { (search_block("k-search-input-mobile")) }
                section."k-nav-section" {
                    h2."k-nav-section__title" { "Navigate" }
                    ul."k-nav-list" {
                        li { a."k-nav-link" href="/" { "Home" } }
                        li { a."k-nav-link" href="/special/all-pages" { "Index of record" } }
                        li { a."k-nav-link" href="/special/recent-changes" { "Recent changes" } }
                        li { a."k-nav-link" href="/random" { "Random entry" } }
                    }
                }
                section."k-nav-section" {
                    h2."k-nav-section__title" { "Resources" }
                    ul."k-nav-list" {
                        li { a."k-nav-link" href="/special/categories" { "Categories" } }
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
                            li { a."k-footer__link" href="/special/categories" { "Categories" } }
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
                        // CC BY 4.0 — the content licence (Wikipedia-style badge).
                        a."k-badge k-badge--license" href="https://creativecommons.org/licenses/by/4.0/"
                          target="_blank" rel="noopener license" aria-label="Content licensed CC BY 4.0" {
                            span."k-badge__roundels" aria-hidden="true" {
                                svg viewBox="0 0 48 24" width="42" height="21" {
                                    circle cx="12" cy="12" r="10.4" fill="none" stroke="currentColor" stroke-width="1.6" {}
                                    text x="12" y="16" text-anchor="middle" font-size="11" font-weight="700"
                                         font-family="var(--k-font-sans)" fill="currentColor" { "cc" }
                                    circle cx="36" cy="12" r="10.4" fill="none" stroke="currentColor" stroke-width="1.6" {}
                                    circle cx="36" cy="8.4" r="2.1" fill="currentColor" {}
                                    path fill="currentColor" d="M32.4 17.6c0-2.2 1.6-3.7 3.6-3.7s3.6 1.5 3.6 3.7h-1.8v-.2c0-1-.8-1.8-1.8-1.8s-1.8.8-1.8 1.8v.2h-1.8z" {}
                                }
                            }
                            span."k-badge__text" {
                                span."k-badge__lead" { "Licensed" }
                                span."k-badge__name" { "CC BY 4.0" }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Format an ISO `YYYY-MM-DD` as "25 May 2026"; pass anything else through.
fn format_date(iso: &str) -> String {
    const MONTHS: [&str; 12] = [
        "January", "February", "March", "April", "May", "June", "July", "August", "September",
        "October", "November", "December",
    ];
    let p: Vec<&str> = iso.trim().split('-').collect();
    if p.len() == 3 {
        if let (Ok(y), Ok(m), Ok(d)) =
            (p[0].parse::<i32>(), p[1].parse::<usize>(), p[2].parse::<u32>())
        {
            if (1..=12).contains(&m) {
                return format!("{d} {} {y}", MONTHS[m - 1]);
            }
        }
    }
    iso.to_string()
}

/// The article action-tab bar (Wikipedia Vector 2022 pattern), sitting above the
/// `<h1>`. "Article" is the live view; Notes/History are declared but not yet
/// wired (they arrive with P5/P4) so they render as disabled tabs, not dead
/// links. The "Last updated" line rides on the right of the bar.
fn article_tabs(updated: Option<&str>) -> Markup {
    html! {
        div."k-article-nav" {
            nav."k-tabs" aria-label="Views" {
                span."k-tab k-tab--active" aria-current="page" { "Article" }
                span."k-tab k-tab--soon" aria-disabled="true" title="Coming soon" { "Notes" }
                span."k-tab k-tab--soon" aria-disabled="true" title="Coming soon" { "History" }
            }
            @if let Some(d) = updated.filter(|s| !s.trim().is_empty()) {
                p."k-article__meta" {
                    "Last updated "
                    time."k-article__date" datetime=(d) { (format_date(d)) }
                }
            }
        }
    }
}

/// Wrap a rendered article body in the reading shell: action tabs, ruled title,
/// prose column. `body_html` is trusted, pre-rendered HTML from the pipeline.
pub fn article(title: &str, updated: Option<&str>, body_html: &str) -> Markup {
    html! {
        article."k-article" {
            (article_tabs(updated))
            h1."k-article__title" { (title) }
            div."k-prose" { (PreEscaped(body_html)) }
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

/// Search results page — a query box plus result cards (same card style as the
/// category listing). `results` is `(slug, title, description)`, ranked.
pub fn search_results(query: &str, results: &[(String, String, String)]) -> Markup {
    let q = query.trim();
    html! {
        div."k-catpage" {
            div."k-catpage__eyebrow" { "Search" }
            h1."k-article__title" { "Search" }
            form."k-searchpage__form" role="search" action="/search" method="get" {
                input."k-search__input" type="search" name="q" value=(query)
                    placeholder="Search this registry" autocomplete="off" spellcheck="false";
                button."k-searchpage__submit" type="submit" { "Search" }
            }
            @if q.is_empty() {
                p."k-searchpage__hint" { "Enter a term to search article titles and text." }
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
    let _ = tenant;
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
                div."k-sidenav__group" {
                    h2."k-sidenav__heading" { "Guides" }
                    ul."k-sidenav__list" {
                        li { a."k-sidenav__link" href="/category/how-to" { "How-to guides" } }
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

/// The full document as one balanced tree. `cats` drives the sidebar nav.
pub fn page(
    tenant: Tenant,
    lang: &str,
    head: Markup,
    body: Markup,
    cats: &[(String, String)],
    toc: &[Heading],
) -> Markup {
    html! {
        (DOCTYPE)
        html lang=(lang) data-instance=(tenant.instance_str()) {
            head { (head) }
            body {
                a."k-skip-link" href="#k-main" { "Skip to content" }
                (mobile_nav(tenant))
                div."k-page" {
                    (utility_bar(tenant))
                    (header(tenant, lang))
                    div."k-shell" {
                        (sidebar(tenant, cats, toc))
                        main."k-page__body" #"k-main" tabindex="-1" { (body) }
                    }
                    (footer(tenant))
                }
                script src="/static/app.js" defer {}
            }
        }
    }
}
