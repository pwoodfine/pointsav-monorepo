//! DTCG design-token loading.
//!
//! Components reference only token custom properties (`var(--…)`), never
//! hard-coded values. The built-in [`DEFAULT_TOKENS_CSS`] is the fallback;
//! in production the canonical bundle emitted by Style Dictionary from
//! `pointsav-design-system` is loaded from disk and overrides it without any
//! HTML or component change.

use std::path::Path;

/// Built-in fallback token bundle (the Woodfine light-mode shell tokens).
pub const DEFAULT_TOKENS_CSS: &str = include_str!("../static/tokens.css");

/// The shared chrome stylesheet (header/footer/page frame).
pub const SHELL_CSS: &str = include_str!("../static/shell.css");

/// The section-component stylesheet (the only place section CSS lives).
pub const SECTIONS_CSS: &str = include_str!("../static/sections.css");

/// The canonical Woodfine institutional wordmark (inline SVG).
pub const WOODFINE_WORDMARK_SVG: &str = include_str!("../static/woodfine-wordmark.svg");

/// Load the active token bundle. When `external` points at a readable DTCG
/// `tokens.css` (e.g. the Style-Dictionary output synced from the design
/// system), its contents are returned; otherwise the built-in fallback.
pub fn load_tokens(external: Option<&Path>) -> String {
    if let Some(path) = external {
        if let Ok(css) = std::fs::read_to_string(path) {
            return css;
        }
    }
    DEFAULT_TOKENS_CSS.to_string()
}
