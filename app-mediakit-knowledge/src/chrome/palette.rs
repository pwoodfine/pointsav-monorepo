//! Command palette chrome — Cmd+K search-and-navigate dialog.
//!
//! Phase 3: full implementation using `<dialog>` element.
//! The palette is always in the DOM (hidden when dialog closed).
//! JavaScript in wiki.js handles open/close and fetches from /api/search.
//! Borrowed from Tailwind CSS docs pattern (§13 borrow list, P0).

use crate::chrome::{t, Locale};
use maud::{html, Markup};

/// Render the Cmd+K command palette `<dialog>` element.
///
/// The dialog is injected into every page by `base_chrome()` and activated
/// by the keyboard shortcut handler in `wiki.js`. It is hidden by default
/// (`<dialog>` closed state). JS handles open/close; server provides markup.
pub fn cmd_palette(locale: Locale) -> Markup {
    let search_ph = t(locale, "search_placeholder");

    html! {
        dialog id="cmd-palette"
               class="cmd-palette"
               aria-label="Command palette"
               aria-modal="true" {
            div class="cmd-palette__panel" {
                div class="cmd-palette__header" {
                    input type="search"
                          class="cmd-palette__input"
                          id="cmd-palette-input"
                          placeholder=(search_ph)
                          autocomplete="off"
                          spellcheck="false"
                          aria-label=(search_ph)
                          autofocus;
                    button class="cmd-palette__close"
                           id="cmd-palette-close"
                           aria-label="Close search" {
                        "×"
                    }
                }
                div class="cmd-palette__results"
                    id="cmd-palette-results"
                    role="listbox"
                    aria-label="Search results" {
                    // Results populated by wiki.js fetch to /api/search
                }
            }
        }
    }
}
