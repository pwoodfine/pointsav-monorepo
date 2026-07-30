//! Mobile chrome — bottom navigation bar with safe-area insets.
//!
//! Phase 3: full implementation of mobile bottom bar.
//!
//! L24 enforcement: every fixed/sticky bottom chrome element uses
//! `calc(N + env(safe-area-inset-bottom))` — bare `padding-bottom: 56px`
//! on bottom chrome is a lint error. The CSS for this bar is in style.css §4.
//!
//! The bottom bar contains:
//! - Home icon → `/`
//! - Search icon → opens Cmd+K palette
//! - Category → `/category/`
//! - Edit → auth-gated edit route
//!
//! `viewport-fit=cover` is required in the viewport meta tag (L17).

use crate::chrome::{t, Locale};
use maud::{html, Markup};

/// Render the mobile bottom navigation bar.
///
/// L24 enforcement: padding-bottom uses `calc()` with `env(safe-area-inset-bottom)`.
/// Height: 56px tap target (44px icon + 12px padding) per Apple HIG.
pub fn mobile_chrome(locale: Locale) -> Markup {
    let home_label = t(locale, "home");
    let search_label = t(locale, "search_label");
    let categories_label = t(locale, "categories");

    html! {
        nav class="mobile-bottom-bar" aria-label="Mobile navigation" {
            a href="/" class="mobile-bottom-bar__item" aria-label=(home_label) {
                span class="mobile-bottom-bar__icon" aria-hidden="true" { "⌂" }
                span class="mobile-bottom-bar__label" { (home_label) }
            }
            button class="mobile-bottom-bar__item"
                   id="mobile-search-btn"
                   aria-label=(search_label)
                   aria-controls="cmd-palette"
                   type="button" {
                span class="mobile-bottom-bar__icon" aria-hidden="true" { "⌕" }
                span class="mobile-bottom-bar__label" { (search_label) }
            }
            a href="/category/" class="mobile-bottom-bar__item" aria-label=(categories_label) {
                span class="mobile-bottom-bar__icon" aria-hidden="true" { "☰" }
                span class="mobile-bottom-bar__label" { (categories_label) }
            }
        }
    }
}
