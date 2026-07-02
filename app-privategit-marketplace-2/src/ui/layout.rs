//! Sovereign Editorial chrome: dark navy masthead (wordmark left / search centre /
//! utility right) and near-black institutional footer with the verbatim WCP Inc.
//! trademark line.
//!
//! The chrome is emitted as maud `Markup` and spliced into the storefront's
//! prerendered static HTML by [`wrap_static_html`] — the P1 static-file-reading
//! logic in `main.rs` is preserved; this only swaps the page's own light chrome
//! for the Sovereign masthead + footer and injects the scoped chrome stylesheet.
//!
//! Structure follows the app-mediakit-knowledge-2 `src/ui/layout.rs` pattern:
//! free `-> Markup` functions driven by the [`SoftwareSurface`] enum.

use maud::{html, Markup, PreEscaped, DOCTYPE};

use super::surface::SoftwareSurface;
use super::tokens;

// ── Inline SVG glyphs (currentColor → inherit the container's ink token) ────────

const GLYPH_SVG: &str = r##"<svg viewBox="0 0 24 24" width="26" height="26" fill="currentColor" aria-hidden="true" focusable="false"><path d="M6 2h7.5L19 7.5V22H6a1 1 0 0 1-1-1V3a1 1 0 0 1 1-1zm7 1.6V8h4.4L13 3.6zM8 12h8v1.5H8V12zm0 3.5h8V17H8v-1.5z"/></svg>"##;

const SEARCH_ICON: &str = r##"<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" focusable="false"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>"##;

const BURGER_ICON: &str = r##"<svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true" focusable="false"><line x1="3" y1="6" x2="21" y2="6"/><line x1="3" y1="12" x2="21" y2="12"/><line x1="3" y1="18" x2="21" y2="18"/></svg>"##;

// ── Scoped chrome stylesheet ────────────────────────────────────────────────────
//
// All selectors are `sw-` prefixed so they never collide with the storefront's
// own inline styles. Token custom properties are emitted from `tokens.rs` below
// so the single source of truth is the Rust consts. The mobile drawer is a
// pure-CSS off-canvas panel (checkbox toggle) — no JavaScript asset in this phase.

fn chrome_style() -> Markup {
    let css = format!(
        r#":root{{
--sw-topnav-bg:{topnav};
--sw-accent:{accent};
--sw-footer-bg:{footer_bg};
--sw-footer-fg:{footer_fg};
--sw-footer-divider:{footer_div};
--sw-wordmark:{wordmark};
}}
.sw-nav-toggle{{position:absolute;width:1px;height:1px;opacity:0;pointer-events:none;}}
.sw-masthead{{background:var(--sw-topnav-bg);color:var(--sw-wordmark);width:100%;}}
.sw-masthead__inner{{display:flex;align-items:center;gap:24px;height:64px;max-width:1280px;margin:0 auto;padding:0 24px;box-sizing:border-box;}}
.sw-brand{{display:flex;align-items:center;gap:10px;color:var(--sw-wordmark);text-decoration:none;flex:0 0 auto;}}
.sw-brand__glyph{{display:inline-flex;color:var(--sw-accent);}}
.sw-brand__lockup{{display:flex;flex-direction:column;line-height:1.05;}}
.sw-brand__name{{font-family:"Playfair Display",Georgia,serif;font-weight:700;font-size:18px;letter-spacing:.01em;}}
.sw-brand__desc{{font-size:10px;letter-spacing:.18em;text-transform:uppercase;color:var(--sw-accent);}}
.sw-search{{flex:1 1 auto;display:flex;justify-content:center;}}
.sw-search__form{{display:flex;width:100%;max-width:420px;background:rgba(255,255,255,.10);border:1px solid rgba(255,255,255,.18);border-radius:6px;overflow:hidden;}}
.sw-search__input{{flex:1;background:transparent;border:0;color:#fff;padding:8px 12px;font-size:13px;outline:none;}}
.sw-search__input::placeholder{{color:rgba(255,255,255,.6);}}
.sw-search__btn{{background:transparent;border:0;color:rgba(255,255,255,.85);padding:0 12px;cursor:pointer;display:inline-flex;align-items:center;}}
.sw-utility{{display:flex;align-items:center;gap:18px;flex:0 0 auto;}}
.sw-utility__link{{color:rgba(255,255,255,.9);text-decoration:none;font-size:12px;font-weight:600;letter-spacing:.06em;text-transform:uppercase;}}
.sw-utility__link:hover{{color:var(--sw-accent);}}
.sw-lang{{display:flex;gap:4px;font-size:12px;font-weight:600;color:rgba(255,255,255,.55);}}
.sw-lang__active{{color:var(--sw-wordmark);}}
.sw-burger{{display:none;flex:0 0 auto;cursor:pointer;color:#fff;padding:6px;border-radius:4px;align-items:center;}}
.sw-subnav{{background:color-mix(in oklab,var(--sw-topnav-bg) 86%,#000);width:100%;}}
.sw-subnav__inner{{display:flex;gap:28px;height:48px;align-items:center;max-width:1280px;margin:0 auto;padding:0 24px;box-sizing:border-box;overflow-x:auto;}}
.sw-subnav__link{{color:rgba(255,255,255,.82);text-decoration:none;font-size:13px;font-weight:600;letter-spacing:.04em;white-space:nowrap;}}
.sw-subnav__link:hover{{color:var(--sw-accent);}}
.sw-drawer{{position:fixed;top:0;left:0;bottom:0;width:82%;max-width:320px;background:var(--sw-topnav-bg);color:#fff;transform:translateX(-100%);transition:transform .25s ease;z-index:60;padding:20px;box-sizing:border-box;overflow-y:auto;}}
.sw-drawer__close{{display:block;text-align:right;color:var(--sw-accent);cursor:pointer;font-size:24px;line-height:1;margin-bottom:12px;}}
.sw-drawer__link{{display:block;padding:12px 4px;color:#fff;text-decoration:none;font-size:15px;border-bottom:1px solid rgba(255,255,255,.12);}}
.sw-scrim{{position:fixed;inset:0;background:rgba(0,0,0,.5);opacity:0;visibility:hidden;transition:opacity .25s ease;z-index:55;}}
.sw-nav-toggle:checked ~ .sw-drawer{{transform:translateX(0);}}
.sw-nav-toggle:checked ~ .sw-scrim{{opacity:1;visibility:visible;}}
.sw-footer{{background:var(--sw-footer-bg);color:var(--sw-footer-fg);width:100%;}}
.sw-footer__inner{{max-width:1280px;margin:0 auto;padding:48px 24px 28px;box-sizing:border-box;}}
.sw-footer__top{{display:grid;grid-template-columns:1.4fr 1fr 1fr;gap:32px;}}
.sw-footer__brand-name{{color:#fff;font-family:"Playfair Display",Georgia,serif;font-weight:700;font-size:18px;}}
.sw-footer__tagline{{margin-top:8px;font-size:13px;max-width:30ch;}}
.sw-footer__col h2{{color:#fff;font-size:12px;letter-spacing:.12em;text-transform:uppercase;margin:0 0 12px;}}
.sw-footer__col ul{{list-style:none;margin:0;padding:0;}}
.sw-footer__col li{{margin-bottom:8px;}}
.sw-footer__col a{{color:var(--sw-footer-fg);text-decoration:none;font-size:13px;}}
.sw-footer__col a:hover{{color:var(--sw-accent);}}
.sw-footer__cities{{margin-top:36px;padding-top:20px;border-top:1px solid var(--sw-footer-divider);font-size:12px;letter-spacing:.08em;text-transform:uppercase;color:#fff;}}
.sw-footer__meta{{margin-top:10px;font-size:12px;}}
.sw-footer__meta a{{color:var(--sw-footer-fg);text-decoration:none;}}
.sw-footer__meta a:hover{{color:var(--sw-accent);}}
.sw-footer__legal{{margin-top:20px;padding-top:18px;border-top:1px solid var(--sw-footer-divider);font-size:11.5px;line-height:1.6;}}
.sw-footer__copyright{{color:#fff;margin:0 0 8px;}}
.sw-footer__trademark{{margin:0;color:var(--sw-footer-fg);max-width:80ch;}}
@media (max-width:768px){{
.sw-search,.sw-utility,.sw-subnav{{display:none;}}
.sw-burger{{display:inline-flex;}}
.sw-masthead__inner{{gap:12px;}}
.sw-footer__top{{grid-template-columns:1fr;gap:24px;}}
}}"#,
        topnav = tokens::TOPNAV_BG,
        accent = tokens::ACCENT,
        footer_bg = tokens::FOOTER_BG,
        footer_fg = tokens::FOOTER_FG,
        footer_div = tokens::FOOTER_DIVIDER,
        wordmark = tokens::WORDMARK,
    );
    html! { style { (PreEscaped(css)) } }
}

// ── Masthead + off-canvas drawer ────────────────────────────────────────────────

/// The dark navy masthead: wordmark (left) · product search (centre) · utility
/// controls (right) · 48px sub-nav. Also emits the pure-CSS off-canvas drawer and
/// its scrim — all rendered as siblings so the `#sw-nav-toggle` checkbox drives
/// them via the general-sibling selector once spliced under `<body>`.
pub fn masthead(surface: SoftwareSurface) -> Markup {
    html! {
        input."sw-nav-toggle" #"sw-nav-toggle" type="checkbox" aria-hidden="true";
        header."sw-masthead" role="banner" {
            div."sw-masthead__inner" {
                a."sw-brand" href="/" aria-label=(surface.home_label()) {
                    span."sw-brand__glyph" { (PreEscaped(GLYPH_SVG)) }
                    span."sw-brand__lockup" {
                        span."sw-brand__name" { "PointSav" }
                        span."sw-brand__desc" { "Software" }
                    }
                }
                div."sw-search" {
                    form."sw-search__form" role="search" action="/software" method="get" {
                        label."sw-search__input" style="display:none" for="sw-q" { "Search products" }
                        input."sw-search__input" #"sw-q" type="search" name="q"
                            placeholder="Search products\u{2026}" autocomplete="off"
                            aria-label="Search products";
                        button."sw-search__btn" type="submit" aria-label="Search" {
                            (PreEscaped(SEARCH_ICON))
                        }
                    }
                }
                div."sw-utility" {
                    @if surface.show_account_nav() {
                        a."sw-utility__link" href="/licensing" { "Account" }
                    }
                    span."sw-lang" aria-label="Language" {
                        span."sw-lang__active" { "EN" }
                        span aria-hidden="true" { " | " }
                        span { "ES" }
                    }
                }
                label."sw-burger" for="sw-nav-toggle" aria-label="Open menu" {
                    (PreEscaped(BURGER_ICON))
                }
            }
        }
        nav."sw-subnav" aria-label="Primary" {
            div."sw-subnav__inner" {
                @for l in surface.nav_links() {
                    a."sw-subnav__link" href=(l.href) { (l.label) }
                }
            }
        }
        // Off-canvas drawer + scrim — opened by the #sw-nav-toggle checkbox (no JS).
        aside."sw-drawer" aria-label="Menu" {
            label."sw-drawer__close" for="sw-nav-toggle" aria-label="Close menu" { "\u{00d7}" }
            @for l in surface.nav_links() {
                a."sw-drawer__link" href=(l.href) { (l.label) }
            }
            @if surface.show_account_nav() {
                a."sw-drawer__link" href="/licensing" { "Account" }
            }
        }
        label."sw-scrim" for="sw-nav-toggle" aria-hidden="true" {}
    }
}

// ── Footer ──────────────────────────────────────────────────────────────────────

/// Near-black institutional footer: link columns, cities line, meta row, and the
/// legal block (copyright + verbatim WCP Inc. trademark line).
///
/// Note: the brand lockup reads "PointSav Software" WITHOUT a ™ — that exact string
/// is not one of the enumerated marks in TRADEMARK.md v1.1, so asserting a mark on
/// it would be inaccurate. The enumerated marks carry ™ in the trademark line only.
pub fn footer(surface: SoftwareSurface) -> Markup {
    html! {
        footer."sw-footer" role="contentinfo" {
            div."sw-footer__inner" {
                div."sw-footer__top" {
                    div."sw-footer__brand" {
                        div."sw-footer__brand-name" { "PointSav Software" }
                        p."sw-footer__tagline" {
                            "Sovereign binary distribution and licensing for the PointSav platform."
                        }
                    }
                    div."sw-footer__col" {
                        h2 { "Catalog" }
                        ul {
                            li { a href="/software" { "Products" } }
                            li { a href="/software#downloads" { "Downloads" } }
                            li { a href="/licensing" { "Pricing" } }
                            li { a href="/licensing" { "Support" } }
                        }
                    }
                    div."sw-footer__col" {
                        h2 { "Legal & Policy" }
                        ul {
                            li { a href="/page/privacy" { "Privacy" } }
                            li { a href="/page/disclaimer" { "Disclaimer" } }
                            li { a href="/page/accessibility" { "Accessibility" } }
                            li { a href="/page/contact" { "Contact us" } }
                        }
                    }
                }
                div."sw-footer__cities" {
                    @for (i, c) in surface.cities().iter().enumerate() {
                        @if i > 0 { span aria-hidden="true" { " | " } }
                        span { (c) }
                    }
                }
                p."sw-footer__meta" {
                    a href="/page/contact" { "Contact us" }
                    span aria-hidden="true" { " \u{00b7} " }
                    a href="/page/disclaimer" { "Disclaimer" }
                    span aria-hidden="true" { " \u{00b7} " }
                    a href="/page/privacy" { "Privacy" }
                }
                div."sw-footer__legal" {
                    p."sw-footer__copyright" {
                        "\u{00a9} 2026 " (surface.copyright_holder()) " All rights reserved."
                    }
                    p."sw-footer__trademark" { (surface.trademark_line()) }
                }
            }
        }
    }
}

// ── Full-document render: dynamic body content + Sovereign chrome ───────────────

/// Render a complete HTML document with the Sovereign Editorial chrome around
/// caller-supplied `content` [`Markup`].
///
/// This is the dynamic counterpart to [`wrap_static_html`]: instead of splicing chrome
/// into a prerendered static file, it builds the whole page — the same masthead, scoped
/// chrome stylesheet, and near-black footer — around content generated at request time
/// (e.g. the dynamic product catalog in `ui::catalog`). The `/software` route uses this
/// so its cards can never drift from `products.yaml`. `/licensing` keeps using
/// [`wrap_static_html`] because it is a static legal document, not catalog data.
pub fn render_page(surface: SoftwareSurface, title: &str, content: Markup) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (title) }
                (chrome_style())
            }
            body {
                (masthead(surface))
                main { (content) }
                (footer(surface))
            }
        }
    }
}

// ── Splice: wrap prerendered static HTML with the Sovereign chrome ──────────────

/// Wrap a storefront static page with the Sovereign Editorial chrome.
///
/// Preserves the P1 static-file read (the caller still reads the file from disk);
/// this only rewrites the served bytes: it strips the page's own light `topnav`
/// header and thin footer, injects the scoped chrome stylesheet before `</head>`,
/// mounts the navy masthead immediately after `<body …>`, and the near-black
/// footer immediately before `</body>`.
///
/// Defensive: if the essential anchors are missing the page is served unchanged.
pub fn wrap_static_html(raw: &str, surface: SoftwareSurface) -> String {
    if !raw.contains("</head>") || !raw.contains("</body>") {
        tracing::warn!("static page missing </head> or </body>; serving without Sovereign chrome");
        return raw.to_string();
    }

    // 1. Strip the page's own light chrome (single header/footer per file).
    let mut out = remove_between(raw, "<header class=\"topnav\">", "</header>");
    out = remove_between(&out, "<footer>", "</footer>");

    // 2. Inject the scoped chrome stylesheet before </head>.
    let style = chrome_style().into_string();
    out = out.replacen("</head>", &format!("{style}\n</head>"), 1);

    // 3. Mount the masthead (+ drawer) right after the <body …> open tag.
    out = insert_after_body_open(&out, &masthead(surface).into_string());

    // 4. Mount the footer right before </body>.
    out = out.replacen(
        "</body>",
        &format!("{}\n</body>", footer(surface).into_string()),
        1,
    );

    out
}

/// Remove the first `open …close` span (inclusive). Returns the input unchanged if
/// either delimiter is absent.
fn remove_between(s: &str, open: &str, close: &str) -> String {
    if let Some(i) = s.find(open) {
        if let Some(rel) = s[i..].find(close) {
            let j = i + rel + close.len();
            let mut out = String::with_capacity(s.len() - (j - i));
            out.push_str(&s[..i]);
            out.push_str(&s[j..]);
            return out;
        }
    }
    s.to_string()
}

/// Insert `insert` immediately after the `<body …>` open tag. Falls back to
/// prepending if no `<body>` tag is found.
fn insert_after_body_open(s: &str, insert: &str) -> String {
    if let Some(i) = s.find("<body") {
        if let Some(gt_rel) = s[i..].find('>') {
            let pos = i + gt_rel + 1;
            let mut out = String::with_capacity(s.len() + insert.len());
            out.push_str(&s[..pos]);
            out.push('\n');
            out.push_str(insert);
            out.push_str(&s[pos..]);
            return out;
        }
    }
    format!("{insert}{s}")
}
