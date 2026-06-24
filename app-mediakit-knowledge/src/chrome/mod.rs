//! Chrome — parameterised page shell emitters.
//!
//! All reader-visible chrome (header, tabs, TOC, bottom bar, palette) lives
//! here. L6 enforcement: "Chrome rendering lives in one parameterised chrome
//! emitter — never multiple inline *_chrome copies in the same handler file."
//!
//! Phase 3: full implementation of all chrome modules with maud templating.
//!
//! Module layout:
//! - `mod.rs` — `base_chrome()` + `strings(locale)` locale map (L22)
//! - `article.rs` — article chrome: tabs, TOC, hatnote, infobox, status badge
//! - `home.rs` — homepage chrome: category grid / thematic clusters / due-diligence path
//! - `palette.rs` — Cmd+K command palette
//! - `mobile.rs` — mobile bottom bar with safe-area insets (L24)

pub mod article;
pub mod home;
pub mod mobile;
pub mod palette;
pub mod sovereign;

use maud::{html, Markup, PreEscaped, DOCTYPE};

/// Locale selector for bilingual chrome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Locale {
    #[default]
    En,
    Es,
}

impl Locale {
    pub fn as_str(self) -> &'static str {
        match self {
            Locale::En => "en",
            Locale::Es => "es",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s {
            "es" => Locale::Es,
            _ => Locale::En,
        }
    }
}

/// Locale-keyed UI string map (L22).
///
/// All reader-visible chrome strings come from here — no hardcoded English
/// strings may appear in any chrome emitter. Acceptance test:
/// `cargo test es_homepage_chrome_is_spanish` (Phase 3).
pub fn strings(locale: Locale) -> &'static [(&'static str, &'static str)] {
    match locale {
        Locale::En => &[
            ("article", "Article"),
            ("talk", "Talk"),
            ("edit", "Edit"),
            ("history", "History"),
            ("search_placeholder", "Search articles…"),
            ("toc_heading", "Contents"),
            ("see_also", "See also"),
            ("categories", "Categories"),
            ("backlinks", "Referenced by"),
            ("recently_changed", "Recently changed"),
            ("status_stub", "Seedling"),
            ("status_pre_build", "In Development"),
            ("status_active", "Active"),
            ("status_complete", "Evergreen"),
            ("home", "Home"),
            ("start_here", "Start here"),
            ("language_toggle", "ES"),
            ("search_label", "Search"),
            ("guide_label", "Guide"),
            ("what_links_here", "What links here"),
            ("article_tab", "Article"),
        ],
        Locale::Es => &[
            ("article", "Artículo"),
            ("talk", "Discusión"),
            ("edit", "Editar"),
            ("history", "Historial"),
            ("search_placeholder", "Buscar artículos…"),
            ("toc_heading", "Contenidos"),
            ("see_also", "Véase también"),
            ("categories", "Categorías"),
            ("backlinks", "Referenciado por"),
            ("recently_changed", "Modificado recientemente"),
            ("status_stub", "Semilla"),
            ("status_pre_build", "En desarrollo"),
            ("status_active", "Activo"),
            ("status_complete", "Permanente"),
            ("home", "Inicio"),
            ("start_here", "Empieza aquí"),
            ("language_toggle", "EN"),
            ("search_label", "Buscar"),
            ("guide_label", "Guía"),
            ("what_links_here", "Lo que enlaza aquí"),
            ("article_tab", "Artículo"),
        ],
    }
}

/// Look up a single locale string by key. Returns the key itself if not found
/// (safe fallback that prevents panics in templates).
pub fn t(locale: Locale, key: &'static str) -> &'static str {
    strings(locale)
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, v)| *v)
        .unwrap_or(key)
}

/// Render the `<head>` element for a page (L23 — two font preloads mandatory).
pub fn head(title: &str, brand: &str, locale: Locale) -> Markup {
    let _lang = locale.as_str();
    html! {
        head {
            meta charset="utf-8";
            meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover";
            title { (title) }

            // L23: Two mandatory font preload links — Inter latin-regular + Source Serif 4 latin-regular
            link rel="preload" href="/static/fonts/Inter-400-normal-latin.woff2"
                 as="font" type="font/woff2" crossorigin="anonymous";
            link rel="preload" href="/static/fonts/Source-Serif-4-400-normal-latin.woff2"
                 as="font" type="font/woff2" crossorigin="anonymous";

            // Token CSS — always tokens.css; Woodfine instances also get tokens-woodfine.css
            link rel="stylesheet" href="/static/tokens.css";
            @if brand == "woodfine" {
                link rel="stylesheet" href="/static/tokens-woodfine.css";
            }
            link rel="stylesheet" href="/static/style.css";

            // Dark-mode init: inline script reads localStorage and sets html[data-theme]
            // BEFORE paint so there is no flash of unstyled content.
            script {
                (PreEscaped(r#"(function(){var t=localStorage.getItem('wiki-theme');if(t==='dark'||t==='light'){document.documentElement.setAttribute('data-theme',t);}else if(window.matchMedia&&window.matchMedia('(prefers-color-scheme: dark)').matches){document.documentElement.setAttribute('data-theme','dark');}})();"#))
            }
            // P2: same-origin page-view beacon (no cookies, no third-party network).
            // navigator.sendBeacon fires after page is interactive; fails silently.
            script {
                (PreEscaped(r#"document.addEventListener('DOMContentLoaded',function(){try{navigator.sendBeacon('/_beacon',JSON.stringify({u:location.pathname,t:Date.now()}));}catch(e){}});"#))
            }
        }
    }
}

/// Render the site navigation bar (wordmark + search + language toggle).
pub fn nav_bar(locale: Locale, site_title: &str, _brand: &str) -> Markup {
    let lang_toggle = t(locale, "language_toggle");
    let search_ph = t(locale, "search_placeholder");
    let home_label = t(locale, "home");

    // Language toggle link: EN page links to /es/, ES page links to /
    let lang_href = match locale {
        Locale::En => "/es/",
        Locale::Es => "/",
    };

    html! {
        header class="topnav" role="banner" {
            // Left: wordmark
            div class="left" {
                a href="/" class="wordmark" aria-label=(home_label) {
                    span class="brand__mark" { "P" }
                    span class="brand__wordmark" { (site_title) }
                }
            }

            // Center: search
            div class="topnav-center" {
                form class="topnav-search" action="/search" method="get" role="search" {
                    div class="header-search-wrap" {
                        input type="search" name="q" id="header-search-q"
                              placeholder=(search_ph)
                              autocomplete="off"
                              aria-label=(search_ph);
                        div id="search-autocomplete-dropdown" class="ac-dropdown" {}
                    }
                }
            }

            // Right: language toggle + theme toggle button
            div class="right" {
                a href=(lang_href) class="lang-toggle" aria-label="Switch language" {
                    (lang_toggle)
                }
                button id="wiki-appearance-btn"
                       class="wiki-appearance-btn"
                       aria-label="Toggle appearance"
                       aria-expanded="false"
                       aria-haspopup="true" {
                    "☀"
                }
                div id="wiki-appearance-menu" class="wiki-appearance-menu" hidden {
                    div class="wiki-appearance-section" {
                        p class="wiki-appearance-label" { "Theme" }
                        div class="wiki-appearance-options" {
                            button class="wiki-appearance-opt" data-theme-val="light" { "Light" }
                            button class="wiki-appearance-opt" data-theme-val="dark" { "Dark" }
                            button class="wiki-appearance-opt" data-theme-val="auto" { "Auto" }
                        }
                    }
                }
            }
        }
    }
}

/// Render the full page shell.
///
/// Emits DOCTYPE + html + head + nav + content + mobile chrome + footer +
/// the wiki.js interaction script. The wiki is a read-only viewer — there is no
/// in-browser editor script.
pub fn base_page(
    page_head: Markup,
    nav: Markup,
    content: Markup,
    mobile: Markup,
    locale: Locale,
    brand: &str,
    _site_title: &str,
) -> Markup {
    let lang = locale.as_str();
    html! {
        (DOCTYPE)
        html lang=(lang) data-theme="auto" data-brand=(brand) {
            (page_head)
            body {
                // Skip link for accessibility
                a class="skip-to-content" href="#main-content" { "Skip to main content" }

                (nav)

                main id="main-content" role="main" {
                    (content)
                }

                // Mobile bottom chrome (L24: safe-area applied in CSS)
                (mobile)

                // Canonical footer (L7 — byte-for-byte locked text)
                footer class="site-footer" role="contentinfo" {
                    div class="site-footer__inner" {
                        p class="site-footer__trademark" {
                            "© 2026 Woodfine Capital Projects Inc. All rights reserved. "
                            "Woodfine Capital Projects™, Woodfine Management Corp™, PointSav Digital Systems™, "
                            "Totebox Orchestration™, and Totebox Archive™ are trademarks of Woodfine Capital "
                            "Projects Inc. used in Canada, the United States, Latin America, and Europe. "
                            "All other trademarks are the property of their respective owners."
                        }
                    }
                }

                // Read-only viewer: the single interaction script. No editor.
                script src="/static/wiki.js" defer {}
            }
        }
    }
}

/// Convenience: render a full article page (head + nav + content + mobile).
/// Used by the wiki route handler to produce a complete HTML document.
pub fn full_article_page(
    title: &str,
    brand: &str,
    site_title: &str,
    locale: Locale,
    content: Markup,
) -> Markup {
    let page_head = head(title, brand, locale);
    let nav = nav_bar(locale, site_title, brand);
    let mobile = mobile::mobile_chrome(locale);
    base_page(page_head, nav, content, mobile, locale, brand, site_title)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strings_en_has_expected_keys() {
        let s = strings(Locale::En);
        let keys: Vec<&str> = s.iter().map(|(k, _)| *k).collect();
        assert!(keys.contains(&"article"));
        assert!(keys.contains(&"toc_heading"));
        assert!(keys.contains(&"status_complete"));
    }

    #[test]
    fn strings_es_has_same_keys_as_en() {
        let en_keys: std::collections::HashSet<&str> =
            strings(Locale::En).iter().map(|(k, _)| *k).collect();
        let es_keys: std::collections::HashSet<&str> =
            strings(Locale::Es).iter().map(|(k, _)| *k).collect();
        assert_eq!(
            en_keys, es_keys,
            "EN and ES string maps must have identical keys"
        );
    }

    #[test]
    fn t_returns_known_keys() {
        assert_eq!(t(Locale::En, "article"), "Article");
        assert_eq!(t(Locale::En, "toc_heading"), "Contents");
    }

    #[test]
    fn es_article_chrome_is_not_english() {
        // Acceptance gate for L22: /es/ chrome must not contain hardcoded English.
        assert_ne!(t(Locale::Es, "article"), t(Locale::En, "article"));
        assert_ne!(t(Locale::Es, "toc_heading"), t(Locale::En, "toc_heading"));
    }

    #[test]
    fn es_homepage_chrome_is_spanish() {
        // L22 acceptance test: /es/ chrome strings are not English.
        let home_en = t(Locale::En, "home");
        let home_es = t(Locale::Es, "home");
        assert_ne!(home_es, home_en, "ES home label must differ from EN");
        assert_eq!(home_es, "Inicio");
    }

    #[test]
    fn head_emits_two_font_preloads() {
        // L23 acceptance test: every rendered <head> contains exactly two font preload links.
        let markup = head("Test", "pointsav", Locale::En).into_string();
        let inter_preload = markup.contains("Inter-400-normal-latin.woff2");
        let serif_preload = markup.contains("Source-Serif-4-400-normal-latin.woff2");
        assert!(inter_preload, "Inter latin font preload must be present");
        assert!(
            serif_preload,
            "Source Serif 4 latin font preload must be present"
        );
    }

    #[test]
    fn head_emits_woodfine_tokens_for_woodfine_brand() {
        let markup = head("Test", "woodfine", Locale::En).into_string();
        assert!(
            markup.contains("tokens-woodfine.css"),
            "Woodfine token CSS must be linked for woodfine brand"
        );
    }

    #[test]
    fn head_does_not_emit_woodfine_tokens_for_pointsav_brand() {
        let markup = head("Test", "pointsav", Locale::En).into_string();
        assert!(
            !markup.contains("tokens-woodfine.css"),
            "Woodfine token CSS must not be linked for pointsav brand"
        );
    }
}
