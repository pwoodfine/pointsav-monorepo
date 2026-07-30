//! Article chrome — tabs, TOC, hatnote, infobox, status badge, related articles.
//!
//! Phase 3: full implementation of article page shell.
//! - Wikipedia Vector 2022 tab model (Article / Talk / Edit / History)
//! - Scroll-spy right-rail TOC (L26 borrow from Stripe/Vercel docs)
//! - Hatnote block (italic, indented; from frontmatter `hatnote:`)
//! - Article status badge (`status:` → Seedling/In Development/Active/Evergreen)
//! - Category chip (clickable, links to category landing page)
//! - `relates_to` rendered as "See Also" sidebar card
//! - Backlinks portlet ("Referenced by N articles" from redb graph stub)
//! - Next/previous within category links (ordered by `position:`)

use crate::chrome::{t, Locale};
use crate::mounts::Mount;
use crate::render::{extract_headings, inject_edit_pencils, render_html_raw, Frontmatter};
use maud::{html, Markup, PreEscaped};

/// Render output bundle produced by the render pipeline.
pub struct RenderOutput {
    /// Rendered HTML body (Markdown → HTML; wikilinks resolved).
    pub html: String,
    /// Extracted table-of-contents entries for the right-rail TOC.
    pub toc: Vec<TocEntry>,
    /// Slugs referenced by `[[wikilinks]]` in the source — used for dead-link detection.
    pub wikilinks: Vec<String>,
}

/// A single table-of-contents entry.
pub struct TocEntry {
    /// Heading level (1–6).
    pub level: u8,
    /// Heading display text (plain text, no HTML).
    pub text: String,
    /// CSS `id` attribute value for in-page anchor linking.
    pub id: String,
}

/// Map article status string to display badge text.
pub fn status_badge_text(status: &str, locale: Locale) -> &'static str {
    match status {
        "stub" | "pre-build" => t(locale, "status_stub"),
        "draft" => t(locale, "status_pre_build"),
        "active" => t(locale, "status_active"),
        "complete" | "evergreen" => t(locale, "status_complete"),
        _ => t(locale, "status_active"),
    }
}

/// Map article status to CSS class suffix (public for home.rs use).
pub fn status_css_class_pub(status: &str) -> &'static str {
    status_css_class(status)
}

/// Map article status to CSS class suffix.
fn status_css_class(status: &str) -> &'static str {
    match status {
        "stub" | "pre-build" => "stub",
        "draft" => "draft",
        "active" => "active",
        "complete" | "evergreen" => "complete",
        _ => "active",
    }
}

/// Render a single table-of-contents entry (recursive for nesting).
fn render_toc_entry(entry: &TocEntry) -> Markup {
    html! {
        li class="toc-entry" data-level=(entry.level) {
            a href=(format!("#{}", entry.id)) class="toc-entry__link" {
                (entry.text)
            }
        }
    }
}

/// Render the right-rail Table of Contents sidebar.
pub fn toc_sidebar(toc: &[TocEntry], locale: Locale) -> Markup {
    if toc.is_empty() {
        return html! {};
    }

    let toc_heading = t(locale, "toc_heading");

    html! {
        aside class="toc" aria-label=(toc_heading) {
            div class="toc__header" {
                span class="toc__title" { (toc_heading) }
                div class="toc__controls" {
                    button id="toc-pin-btn" class="toc__pin-btn" aria-pressed="false" title="Pin table of contents" {
                        "[pin]"
                    }
                    button id="toc-toggle" class="toc__toggle-btn" aria-expanded="true" {
                        "[hide]"
                    }
                }
            }
            ol id="toc-list" class="toc__list" {
                @for entry in toc {
                    (render_toc_entry(entry))
                }
            }
        }
    }
}

/// Render the Wikipedia-style article tabs (Article / Talk / Edit / History).
pub fn article_tabs(slug: &str, locale: Locale) -> Markup {
    let article_label = t(locale, "article");
    let talk_label = t(locale, "talk");
    let edit_label = t(locale, "edit");
    let history_label = t(locale, "history");

    html! {
        nav class="article-tabs" aria-label="Article actions" {
            div class="article-tabs__left" {
                a href=(format!("/wiki/{slug}")) class="article-tab article-tab--active article-tab--article" {
                    (article_label)
                }
                a href=(format!("/wiki/{slug}/talk")) class="article-tab article-tab--talk" {
                    (talk_label)
                }
            }
            div class="article-tabs__right" {
                // Git-only workflow: the Edit tab links to the raw Markdown
                // source (`/git/{slug}`); contributions are made via git, not an
                // in-browser editor.
                a href=(format!("/git/{slug}")) class="article-tab article-tab--edit" {
                    (edit_label)
                }
                a href=(format!("/history/{slug}")) class="article-tab article-tab--history" {
                    (history_label)
                }
            }
        }
    }
}

/// Render the breadcrumb navigation.
pub fn breadcrumb(category: Option<&str>, title: &str, locale: Locale) -> Markup {
    let home_label = t(locale, "home");

    html! {
        nav class="crumb" aria-label="Breadcrumb" {
            a href="/" { (home_label) }
            @if let Some(cat) = category {
                span class="crumb__sep" { " › " }
                a href=(format!("/category/{}", cat.to_lowercase().replace(' ', "-"))) {
                    (cat)
                }
            }
            span class="crumb__sep" { " › " }
            span class="crumb__current" { (title) }
        }
    }
}

/// Render the article page body — tabs, header, hatnote, body, see-also, backlinks, nav.
///
/// Read-only chrome: there is no in-browser editor. Contributions flow through
/// git, and the Edit tab links to the raw Markdown source (`/git/{slug}`).
#[allow(clippy::too_many_arguments)] // chrome renderer: each arg is a distinct content slot
pub fn article_page(
    slug: &str,
    title: &str,
    status: &str,
    category: Option<&str>,
    hatnote: Option<&str>,
    html_body: &str,
    toc: &[TocEntry],
    relates_to: &[String],
    backlink_count: usize,
    prev_article: Option<(&str, &str)>, // (slug, title)
    next_article: Option<(&str, &str)>,
    locale: Locale,
) -> Markup {
    let see_also_label = t(locale, "see_also");
    let backlinks_label = t(locale, "backlinks");
    let status_text = status_badge_text(status, locale);
    let status_css = status_css_class(status);

    html! {
        div class="article-page" {
            // Wikipedia-style tabs
            (article_tabs(slug, locale))

            // Breadcrumb
            (breadcrumb(category, title, locale))

            // Article layout: body + TOC rail
            div class="article-wrap" {
                // Main article column
                article class="article-body" id="mw-content-text" {
                    // Article title
                    h1 class="article__title" { (title) }

                    // Article meta row: status badge + category chip
                    div class="article__meta-row" {
                        span class=(format!("status-badge status-badge--{}", status_css)) {
                            (status_text)
                        }
                        @if let Some(cat) = category {
                            a href=(format!("/category/{}", cat.to_lowercase().replace(' ', "-")))
                              class="category-chip" {
                                (cat)
                            }
                        }
                    }

                    // Hatnote (italic notice block from frontmatter)
                    @if let Some(note) = hatnote {
                        div class="hatnote" role="note" {
                            em { (note) }
                        }
                    }

                    // Article body (Markdown → HTML, pre-rendered)
                    div class="prose" {
                        (PreEscaped(html_body))
                    }

                    // See Also block (from frontmatter relates_to)
                    @if !relates_to.is_empty() {
                        section class="see-also" aria-label=(see_also_label) {
                            h2 class="see-also__heading" { (see_also_label) }
                            ul class="see-also__list" {
                                @for related_slug in relates_to {
                                    li {
                                        a href=(format!("/wiki/{}", related_slug)) {
                                            (related_slug.replace('-', " "))
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Backlinks portlet ("Referenced by N articles")
                    @if backlink_count > 0 {
                        aside class="backlinks-portlet" {
                            span class="backlinks-portlet__label" { (backlinks_label) }
                            span class="backlinks-portlet__count" { (backlink_count) " articles" }
                        }
                    }

                    // Next/Previous within category
                    @if prev_article.is_some() || next_article.is_some() {
                        nav class="article-nav" aria-label="Article navigation" {
                            @if let Some((prev_slug, prev_title)) = prev_article {
                                a href=(format!("/wiki/{prev_slug}")) class="article-nav__prev" {
                                    "← " (prev_title)
                                }
                            }
                            @if let Some((next_slug, next_title)) = next_article {
                                a href=(format!("/wiki/{next_slug}")) class="article-nav__next" {
                                    (next_title) " →"
                                }
                            }
                        }
                    }
                }

                // Right-rail TOC
                (toc_sidebar(toc, locale))
            }
        }
    }
}

/// Render a Markdown article body to HTML, extracting TOC entries and tracking
/// wikilink slugs referenced by the source.
pub fn render_page(content: &str, meta: &Frontmatter, mounts: &[Mount]) -> RenderOutput {
    let extra_roots: Vec<&std::path::Path> = mounts.iter().map(|m| m.path.as_path()).collect();
    let (primary, extra) = match extra_roots.as_slice() {
        [] => (std::path::Path::new("."), &[][..]),
        [first, rest @ ..] => (*first, rest),
    };

    let raw_html = render_html_raw(content, primary, extra);
    let headings = extract_headings(&raw_html);
    let wikilinks = collect_wikilink_slugs(&raw_html);

    let toc: Vec<TocEntry> = headings
        .into_iter()
        .map(|(id, text, level)| TocEntry { level, text, id })
        .collect();

    let html = inject_edit_pencils(&raw_html);

    let _ = meta;

    RenderOutput {
        html,
        toc,
        wikilinks,
    }
}

/// Extract the set of unique wikilink target slugs from rendered HTML.
fn collect_wikilink_slugs(html: &str) -> Vec<String> {
    const MARKER: &str = " data-wikilink=\"true\">";
    const HREF_PREFIX: &str = "/wiki/";
    let mut slugs = Vec::new();
    let mut rest = html;
    while let Some(pos) = rest.find(MARKER) {
        let before = &rest[..pos];
        if let Some(href_pos) = before.rfind("href=\"") {
            let after_href = &before[href_pos + 6..];
            if let Some(quote_end) = after_href.find('"') {
                let href_val = &after_href[..quote_end];
                let slug = href_val
                    .strip_prefix(HREF_PREFIX)
                    .unwrap_or(href_val)
                    .to_string();
                if !slug.is_empty() && !slugs.contains(&slug) {
                    slugs.push(slug);
                }
            }
        }
        rest = &rest[pos + MARKER.len()..];
    }
    slugs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_badge_text_maps_correctly() {
        assert_eq!(status_badge_text("stub", Locale::En), "Seedling");
        assert_eq!(status_badge_text("pre-build", Locale::En), "Seedling");
        assert_eq!(status_badge_text("draft", Locale::En), "In Development");
        assert_eq!(status_badge_text("active", Locale::En), "Active");
        assert_eq!(status_badge_text("complete", Locale::En), "Evergreen");
        assert_eq!(status_badge_text("evergreen", Locale::En), "Evergreen");
    }

    #[test]
    fn status_badge_text_es() {
        assert_eq!(status_badge_text("stub", Locale::Es), "Semilla");
        assert_eq!(status_badge_text("complete", Locale::Es), "Permanente");
    }
}
