//! Markdown rendering for `prose` sections.
//!
//! Prose is the one place a manifest carries free text rather than typed
//! fields. It is CommonMark + GFM, rendered with comrak — the same parser the
//! knowledge wiki uses, so prose behaves identically across the family.

use comrak::{markdown_to_html, Options};

/// Render a Markdown string to an HTML fragment (CommonMark + GFM).
///
/// The output is a trusted fragment (authored content that passed the
/// human approval gate), embedded inside the `prose-body` container by the
/// section renderer.
pub fn markdown_to_fragment(md: &str) -> String {
    let mut opts = Options::default();
    opts.extension.table = true;
    opts.extension.strikethrough = true;
    opts.extension.autolink = true;
    markdown_to_html(md, &opts)
}
