//! The typed Section component vocabulary — the contract an AI author writes
//! against, and the only set of building blocks a page may be composed from.
//!
//! A page manifest names a section `type` and binds data to that type's
//! fields. Validation is structural: a manifest either deserializes into one
//! of these variants (all required fields present, correct types) or it is
//! rejected. There is no path to arbitrary HTML or CSS — that is what keeps
//! AI-authored pages on-brand and mobile-correct (each component owns its CSS
//! in `static/sections.css`).
//!
//! P2 catalogue: `hero`, `prose`, `cta`, `card-grid`, `feature`, `media`.
//! The set embeds the hyperscaler/institutional mobile patterns (hero imagery
//! of real assets, equal-weight cards stacking single-column on mobile,
//! full-width CTAs, image+text feature rows). Further types are additive.

use maud::{html, Markup, PreEscaped};
use serde::{Deserialize, Serialize};

use crate::render::markdown_to_fragment;

/// A reusable call-to-action button (used inside a hero, feature, or a
/// standalone CTA band).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CtaButton {
    /// Button text.
    pub label: String,
    /// Destination URL or path.
    pub href: String,
}

impl CtaButton {
    pub(crate) fn render(&self) -> Markup {
        html! { a class="cta-button" href=(self.href) { (self.label) } }
    }
}

/// A responsive image. `src` is a path/URL; `alt` is mandatory (accessibility).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Image {
    pub src: String,
    pub alt: String,
    #[serde(default)]
    pub caption: Option<String>,
}

impl Image {
    fn render(&self, class: &str) -> Markup {
        html! {
            figure class=(class) {
                img src=(self.src) alt=(self.alt) loading="lazy";
                @if let Some(cap) = &self.caption { figcaption { (cap) } }
            }
        }
    }
}

/// One cell in a card grid — the atomic equal-weight unit (mobile research §2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Card {
    pub title: String,
    #[serde(default)]
    pub body: Option<String>,
    /// Optional short icon/emoji rendered in a chip.
    #[serde(default)]
    pub icon: Option<String>,
    /// When set, the whole card becomes a link.
    #[serde(default)]
    pub href: Option<String>,
}

impl Card {
    fn render(&self) -> Markup {
        let inner = html! {
            @if let Some(icon) = &self.icon { span class="card-icon" { (icon) } }
            h3 class="card-title" { (self.title) }
            @if let Some(body) = &self.body { p class="card-body" { (body) } }
        };
        html! {
            @if let Some(href) = &self.href {
                a class="card card-link" href=(href) { (inner) }
            } @else {
                div class="card" { (inner) }
            }
        }
    }
}

/// A typed page section. The `type` discriminator selects the variant; the
/// remaining fields are that variant's data.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Section {
    /// Statement headline + one support line + optional CTA + optional hero
    /// image (mobile research §3 + §4: statement headline, real-asset imagery).
    Hero(HeroSection),
    /// Free Markdown prose (CommonMark + GFM).
    Prose(ProseSection),
    /// A standalone call-to-action band.
    Cta(CtaSection),
    /// Equal-weight cards; single-column on mobile, multi-column on desktop.
    CardGrid(CardGridSection),
    /// An image + text row (alternating media side on desktop; stacked mobile).
    Feature(FeatureSection),
    /// A standalone full-width image with optional caption.
    Media(MediaSection),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeroSection {
    pub headline: String,
    #[serde(default)]
    pub subhead: Option<String>,
    #[serde(default)]
    pub cta: Option<CtaButton>,
    #[serde(default)]
    pub image: Option<Image>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProseSection {
    /// Section body in Markdown.
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CtaSection {
    pub heading: String,
    pub cta: CtaButton,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardGridSection {
    #[serde(default)]
    pub heading: Option<String>,
    /// Desktop column count (1–4); mobile is always single-column. Default 3.
    #[serde(default)]
    pub columns: Option<u8>,
    pub cards: Vec<Card>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureSection {
    pub heading: String,
    /// Body in Markdown.
    pub body: String,
    #[serde(default)]
    pub image: Option<Image>,
    #[serde(default)]
    pub cta: Option<CtaButton>,
    /// `"right"` puts the media on the right at desktop width (default left).
    #[serde(default)]
    pub media_side: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaSection {
    pub image: Image,
}

impl Section {
    /// Human-readable kind label (used in diffs, MCP listings, logs).
    pub fn kind(&self) -> &'static str {
        match self {
            Section::Hero(_) => "hero",
            Section::Prose(_) => "prose",
            Section::Cta(_) => "cta",
            Section::CardGrid(_) => "card-grid",
            Section::Feature(_) => "feature",
            Section::Media(_) => "media",
        }
    }

    /// Render this section to an HTML fragment. No CSS is emitted inline —
    /// styling comes entirely from `static/sections.css`.
    pub fn render(&self) -> Markup {
        match self {
            Section::Hero(h) => html! {
                section class="section section-hero" {
                    @if let Some(img) = &h.image { (img.render("hero-media")) }
                    h1 class="hero-headline" { (h.headline) }
                    @if let Some(sub) = &h.subhead {
                        p class="hero-subhead" { (sub) }
                    }
                    @if let Some(cta) = &h.cta { (cta.render()) }
                }
            },
            Section::Prose(p) => html! {
                section class="section section-prose" {
                    div class="prose-body" { (PreEscaped(markdown_to_fragment(&p.body))) }
                }
            },
            Section::Cta(c) => html! {
                section class="section section-cta" {
                    h2 class="cta-heading" { (c.heading) }
                    (c.cta.render())
                }
            },
            Section::CardGrid(g) => {
                let cols = g.columns.unwrap_or(3).clamp(1, 4);
                html! {
                    section class="section section-card-grid" {
                        @if let Some(heading) = &g.heading {
                            h2 class="cards-heading" { (heading) }
                        }
                        div class="card-grid" style=(format!("--cols:{cols}")) {
                            @for card in &g.cards { (card.render()) }
                        }
                    }
                }
            }
            Section::Feature(f) => {
                let side = if f.media_side.as_deref() == Some("right") {
                    "section-feature feature-media-right"
                } else {
                    "section-feature"
                };
                html! {
                    section class=(format!("section {side}")) {
                        @if let Some(img) = &f.image { (img.render("feature-media")) }
                        div class="feature-text" {
                            h2 class="feature-heading" { (f.heading) }
                            div class="feature-body" { (PreEscaped(markdown_to_fragment(&f.body))) }
                            @if let Some(cta) = &f.cta { (cta.render()) }
                        }
                    }
                }
            }
            Section::Media(m) => html! {
                section class="section section-media" { (m.image.render("media-figure")) }
            },
        }
    }
}

/// A machine-readable description of the section vocabulary, returned by the
/// MCP `list_section_types` tool so an AI author discovers what it may emit.
pub fn section_catalog() -> serde_json::Value {
    serde_json::json!({
        "hero": {
            "required": ["headline"],
            "optional": ["subhead", "cta{label,href}", "image{src,alt,caption?}"],
            "note": "Statement headline + one support line + optional CTA + optional real-asset hero image."
        },
        "prose": {
            "required": ["body"],
            "note": "Markdown body (CommonMark + GFM)."
        },
        "cta": {
            "required": ["heading", "cta{label,href}"],
            "note": "Standalone call-to-action band."
        },
        "card-grid": {
            "required": ["cards[]{title}"],
            "optional": ["heading", "columns(1-4,default 3)", "cards[].body", "cards[].icon", "cards[].href"],
            "note": "Equal-weight cards; single-column on mobile, multi-column on desktop."
        },
        "feature": {
            "required": ["heading", "body"],
            "optional": ["image{src,alt,caption?}", "cta{label,href}", "media_side(left|right)"],
            "note": "Image + Markdown text row; alternating media side on desktop, stacked on mobile."
        },
        "media": {
            "required": ["image{src,alt}"],
            "optional": ["image.caption"],
            "note": "Standalone full-width image with optional caption."
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hero_renders_headline_cta_and_image() {
        let s = Section::Hero(HeroSection {
            headline: "Sovereign sites, simply".into(),
            subhead: Some("One support line.".into()),
            cta: Some(CtaButton {
                label: "Enquire".into(),
                href: "/contact".into(),
            }),
            image: Some(Image {
                src: "/media/aerial.webp".into(),
                alt: "Aerial of the site".into(),
                caption: None,
            }),
        });
        let html = s.render().into_string();
        assert!(html.contains("hero-headline"));
        assert!(html.contains("hero-media"));
        assert!(html.contains("cta-button"));
        assert_eq!(s.kind(), "hero");
    }

    #[test]
    fn prose_renders_markdown() {
        let s = Section::Prose(ProseSection {
            body: "## Heading\n\nA paragraph with **bold**.".into(),
        });
        let html = s.render().into_string();
        assert!(html.contains("<h2>"));
        assert!(html.contains("<strong>bold</strong>"));
    }

    #[test]
    fn card_grid_renders_cards_and_column_var() {
        let s = Section::CardGrid(CardGridSection {
            heading: Some("What we do".into()),
            columns: Some(2),
            cards: vec![
                Card {
                    title: "Develop".into(),
                    body: Some("Ground-up.".into()),
                    icon: Some("▣".into()),
                    href: None,
                },
                Card {
                    title: "Operate".into(),
                    body: None,
                    icon: None,
                    href: Some("/page/operate".into()),
                },
            ],
        });
        let html = s.render().into_string();
        assert!(html.contains("card-grid"));
        assert!(html.contains("--cols:2"));
        assert!(html.contains("card-link")); // the card with href
        assert_eq!(s.kind(), "card-grid");
    }

    #[test]
    fn feature_renders_media_side_and_body() {
        let s = Section::Feature(FeatureSection {
            heading: "A feature".into(),
            body: "Some **markdown**.".into(),
            image: Some(Image {
                src: "/media/x.webp".into(),
                alt: "x".into(),
                caption: None,
            }),
            cta: None,
            media_side: Some("right".into()),
        });
        let html = s.render().into_string();
        assert!(html.contains("feature-media-right"));
        assert!(html.contains("<strong>markdown</strong>"));
        assert_eq!(s.kind(), "feature");
    }
}
