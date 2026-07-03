// SPDX-License-Identifier: FSL-1.1-ALv2
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

//! Section-manifest content model. Carries forward the retired engine's
//! proven content contract (`<content_dir>/<slug>/page.yaml`,
//! `page.es.yaml` for Spanish) as a schema — reimplemented fresh here, not
//! reused. Section vocabulary observed across all six live pages: `hero`,
//! `card-grid`, `prose`. Prose bodies are Markdown, rendered via comrak.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::MarketingError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page {
    pub title: String,
    pub slug: String,
    pub description: String,
    #[serde(default = "default_lang")]
    pub lang: String,
    pub sections: Vec<Section>,
}

fn default_lang() -> String {
    "en".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Section {
    Hero {
        headline: String,
        #[serde(default)]
        subhead: Option<String>,
    },
    CardGrid {
        columns: u8,
        cards: Vec<Card>,
        /// Visual treatment hint. `None`/anything else renders the default
        /// informational card (top-rule, title, body). `"buttons"` renders a
        /// compact link-button row instead — for grids of pure navigation
        /// links (e.g. "Manifest" / "BIM Library" / "Location Intelligence")
        /// that have no body text and shouldn't look like content cards.
        #[serde(default)]
        style: Option<String>,
    },
    Prose {
        /// Markdown source. Rendered to HTML at render time via
        /// [`render_markdown`], not pre-rendered at load time — keeps the
        /// manifest the single source of truth and avoids caching staleness.
        body: String,
    },
    /// A row of graphic tiles (self-hosted SVG, path under `/static/`).
    /// Restores the "Development Classes" / capability icons that existed on
    /// the retired production sites' `.classes` band, ported here as a real
    /// content section rather than a per-tenant chrome fixture, since the
    /// set of icons is page content (currently only used on `home`).
    IconStrip {
        icons: Vec<IconTile>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IconTile {
    pub src: String,
    /// The label is rendered as vector text baked into the SVG itself (it's
    /// a graphic, not semantic HTML) — `alt` must repeat it verbatim so
    /// screen readers get the same information sighted users do.
    pub alt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Card {
    pub title: String,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub href: Option<String>,
}

/// Load a page manifest. `lang` selects the file: `None`/`"en"` loads
/// `page.yaml`; any other value loads `page.<lang>.yaml` (e.g. `page.es.yaml`).
pub fn load_page(content_dir: &Path, slug: &str, lang: Option<&str>) -> Result<Page, MarketingError> {
    let filename = match lang {
        None | Some("en") => "page.yaml".to_string(),
        Some(code) => format!("page.{code}.yaml"),
    };
    let path: PathBuf = content_dir.join(slug).join(&filename);
    let raw = std::fs::read_to_string(&path).map_err(|_| {
        MarketingError::PageNotFound(format!("{slug} ({filename})"))
    })?;
    serde_yaml::from_str(&raw).map_err(|source| MarketingError::Manifest {
        slug: slug.to_string(),
        source,
    })
}

/// Enumerate available page slugs — any immediate subdirectory of
/// `content_dir` containing a `page.yaml`.
pub fn list_slugs(content_dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(content_dir) else {
        return Vec::new();
    };
    let mut slugs: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().join("page.yaml").is_file())
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    slugs.sort();
    slugs
}

/// Render a Markdown prose body to HTML (GFM-flavored: tables, strikethrough,
/// autolinks).
pub fn render_markdown(body: &str) -> String {
    let mut options = comrak::Options::default();
    options.extension.table = true;
    options.extension.strikethrough = true;
    options.extension.autolink = true;
    comrak::markdown_to_html(body, &options)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_page(dir: &Path, slug: &str, filename: &str, yaml: &str) {
        let page_dir = dir.join(slug);
        std::fs::create_dir_all(&page_dir).unwrap();
        std::fs::write(page_dir.join(filename), yaml).unwrap();
    }

    #[test]
    fn loads_hero_and_prose_sections() {
        let dir = tempfile::tempdir().unwrap();
        write_page(
            dir.path(),
            "home",
            "page.yaml",
            r#"
title: Home
slug: home
description: A test page.
lang: en
sections:
  - type: hero
    headline: Hello
    subhead: World
  - type: prose
    body: |
      **bold** text
"#,
        );
        let page = load_page(dir.path(), "home", None).unwrap();
        assert_eq!(page.title, "Home");
        assert_eq!(page.sections.len(), 2);
        match &page.sections[0] {
            Section::Hero { headline, subhead } => {
                assert_eq!(headline, "Hello");
                assert_eq!(subhead.as_deref(), Some("World"));
            }
            other => panic!("expected Hero, got {other:?}"),
        }
    }

    #[test]
    fn loads_spanish_variant_by_suffix() {
        let dir = tempfile::tempdir().unwrap();
        write_page(
            dir.path(),
            "home",
            "page.es.yaml",
            r#"
title: Inicio
slug: home
description: Una página de prueba.
lang: es
sections:
  - type: hero
    headline: Hola
"#,
        );
        let page = load_page(dir.path(), "home", Some("es")).unwrap();
        assert_eq!(page.lang, "es");
        assert_eq!(page.title, "Inicio");
    }

    #[test]
    fn missing_page_is_not_found_error() {
        let dir = tempfile::tempdir().unwrap();
        let err = load_page(dir.path(), "nope", None).unwrap_err();
        assert!(matches!(err, MarketingError::PageNotFound(_)));
    }

    #[test]
    fn card_grid_parses_optional_fields() {
        let dir = tempfile::tempdir().unwrap();
        write_page(
            dir.path(),
            "home",
            "page.yaml",
            r#"
title: Home
slug: home
description: A test page.
sections:
  - type: card-grid
    columns: 3
    cards:
      - title: One
      - title: Two
        body: some body
        href: https://example.com
"#,
        );
        let page = load_page(dir.path(), "home", None).unwrap();
        match &page.sections[0] {
            Section::CardGrid { columns, cards, style } => {
                assert_eq!(*columns, 3);
                assert_eq!(cards.len(), 2);
                assert!(cards[0].body.is_none());
                assert_eq!(cards[1].href.as_deref(), Some("https://example.com"));
                assert!(style.is_none());
            }
            other => panic!("expected CardGrid, got {other:?}"),
        }
    }

    #[test]
    fn icon_strip_parses_icon_paths() {
        let dir = tempfile::tempdir().unwrap();
        write_page(
            dir.path(),
            "home",
            "page.yaml",
            r#"
title: Home
slug: home
description: A test page.
sections:
  - type: icon-strip
    icons:
      - src: /static/graphics/woodfine/class-1.svg
        alt: Professional Centres
      - src: /static/graphics/woodfine/class-2.svg
        alt: Tech Industrial
"#,
        );
        let page = load_page(dir.path(), "home", None).unwrap();
        match &page.sections[0] {
            Section::IconStrip { icons } => {
                assert_eq!(icons.len(), 2);
                assert_eq!(icons[0].src, "/static/graphics/woodfine/class-1.svg");
                assert_eq!(icons[0].alt, "Professional Centres");
            }
            other => panic!("expected IconStrip, got {other:?}"),
        }
    }

    #[test]
    fn card_grid_style_hint_parses() {
        let dir = tempfile::tempdir().unwrap();
        write_page(
            dir.path(),
            "home",
            "page.yaml",
            r#"
title: Home
slug: home
description: A test page.
sections:
  - type: card-grid
    columns: 3
    style: buttons
    cards:
      - title: Manifest
        href: https://example.com
"#,
        );
        let page = load_page(dir.path(), "home", None).unwrap();
        match &page.sections[0] {
            Section::CardGrid { style, .. } => assert_eq!(style.as_deref(), Some("buttons")),
            other => panic!("expected CardGrid, got {other:?}"),
        }
    }

    #[test]
    fn markdown_renders_gfm() {
        let html = render_markdown("**bold** and ~~strike~~ and https://example.com");
        assert!(html.contains("<strong>bold</strong>"));
        assert!(html.contains("<del>strike</del>"));
        assert!(html.contains("<a href=\"https://example.com\""));
    }

    #[test]
    fn list_slugs_finds_only_dirs_with_page_yaml() {
        let dir = tempfile::tempdir().unwrap();
        write_page(dir.path(), "home", "page.yaml", "title: Home\nslug: home\ndescription: d\nsections: []\n");
        write_page(dir.path(), "contact", "page.yaml", "title: Contact\nslug: contact\ndescription: d\nsections: []\n");
        std::fs::create_dir_all(dir.path().join("empty")).unwrap();
        let slugs = list_slugs(dir.path());
        assert_eq!(slugs, vec!["contact".to_string(), "home".to_string()]);
    }
}
