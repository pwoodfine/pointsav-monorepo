//! The continuous chrome that wraps every page: `<head>`, sitenotice, sticky
//! white header, off-canvas mobile nav, and the institutional footer.
//!
//! `page()` is the single public entry — it composes the whole document as one
//! `html!{}` tree so maud balances every tag. Structure follows Wikipedia
//! Vector 2022; the visual system (white header, brand-as-accent) lives in
//! `static/{tokens,app}.css`. Class names match the `k-*` manifest.

use maud::{html, Markup, PreEscaped, DOCTYPE};

use super::tenant::Tenant;

/// `<head>` contents (not the `<head>` element itself — `page()` supplies that).
pub fn doc_head(title: &str, tenant: Tenant) -> Markup {
    html! {
        meta charset="utf-8";
        meta name="viewport" content="width=device-width, initial-scale=1";
        meta name="color-scheme" content="light dark";
        meta name="theme-color" content=(tenant.accent());
        title { (title) " — " (tenant.home_label()) }
        link rel="stylesheet" href="/static/fonts.css";
        link rel="stylesheet" href="/static/tokens.css";
        link rel="stylesheet" href="/static/app.css";
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

/// Sitenotice strip above the header: entity · seat · [spacer] · sibling · badge.
pub fn sitenotice(tenant: Tenant) -> Markup {
    html! {
        div."k-sitenotice" {
            div."k-sitenotice__inner" {
                a."k-sitenotice__entity" href=(tenant.home_url()) { (tenant.entity_name()) }
                span."k-sitenotice__sep" aria-hidden="true" { "·" }
                span."k-sitenotice__seat" { (tenant.seat()) }
                span."k-sitenotice__spacer" {}
                @if let Some(sib) = tenant.sibling_wiki() {
                    a."k-sitenotice__sibling" href=(sib.url) { (sib.label) }
                    span."k-sitenotice__sep" aria-hidden="true" { "·" }
                }
                span."k-sitenotice__badge" { "Current of record" }
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
                        span."k-logo__text" { (tenant.home_label()) }
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
                @if let Some(sib) = tenant.sibling_wiki() {
                    section."k-nav-section" {
                        h2."k-nav-section__title" { "Related registries" }
                        ul."k-nav-list" { li { a."k-nav-link" href=(sib.url) { (sib.label) } } }
                    }
                }
            }
        }
    }
}

/// Institutional footer: 3-column grid + legal strip.
pub fn footer(tenant: Tenant) -> Markup {
    html! {
        footer."k-footer" role="contentinfo" {
            div."k-footer__inner" {
                div."k-footer__grid" {
                    div."k-footer__col" {
                        h2."k-footer__col-title" { "Navigate" }
                        ul."k-footer__list" {
                            li { a."k-footer__link" href="/" { "Home" } }
                            li { a."k-footer__link" href="/special/all-pages" { "Index of record" } }
                            li { a."k-footer__link" href="/special/recent-changes" { "Recent changes" } }
                            li { a."k-footer__link" href="/random" { "Random entry" } }
                        }
                    }
                    div."k-footer__col" {
                        h2."k-footer__col-title" { "Resources" }
                        ul."k-footer__list" {
                            li { a."k-footer__link" href="/special/categories" { "Categories" } }
                            li { a."k-footer__link" href="/feed.atom" { "Atom feed" } }
                            li { a."k-footer__link" href="/sitemap.xml" { "Sitemap" } }
                            @if let Some(sib) = tenant.sibling_wiki() {
                                li { a."k-footer__link" href=(sib.url) { (sib.label) } }
                            }
                        }
                    }
                    div."k-footer__col" {
                        h2."k-footer__col-title" { "About" }
                        ul."k-footer__list" {
                            li { a."k-footer__link" href="/page/about" { "About this registry" } }
                            li { a."k-footer__link" href="/page/privacy" { "Privacy" } }
                            li { a."k-footer__link" href="/page/terms" { "Terms of use" } }
                            li { a."k-footer__link" href="/page/contact" { "Contact" } }
                        }
                    }
                }
                div."k-footer__legal" {
                    span."k-footer__legal-item" {
                        span."k-footer__brand" { (tenant.entity_name()) }
                    }
                    span."k-footer__legal-item" { "Registered seat: " (tenant.seat()) }
                    span."k-footer__legal-item" { (tenant.trademark_line()) }
                    span."k-footer__legal-item" {
                        "\u{00a9} " (tenant.copyright_holder()) " All rights reserved."
                    }
                }
            }
        }
    }
}

/// The full document as one balanced tree.
pub fn page(tenant: Tenant, lang: &str, head: Markup, body: Markup) -> Markup {
    html! {
        (DOCTYPE)
        html lang=(lang) data-instance=(tenant.instance_str()) {
            head { (head) }
            body {
                a."k-skip-link" href="#k-main" { "Skip to content" }
                (mobile_nav(tenant))
                div."k-page" {
                    (sitenotice(tenant))
                    (header(tenant, lang))
                    main."k-page__body" #"k-main" tabindex="-1" { (body) }
                    (footer(tenant))
                }
                script src="/static/app.js" defer {}
            }
        }
    }
}
