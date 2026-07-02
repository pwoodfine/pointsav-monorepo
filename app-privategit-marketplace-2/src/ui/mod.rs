//! Sovereign Editorial chrome for the marketplace storefront.
//!
//! `layout::wrap_static_html` wraps the P1 prerendered static pages (`software.html`,
//! `licensing.html`) with the dark navy masthead and near-black institutional
//! footer without changing the static-file-reading logic in `main.rs`.
//!
//! Design-system provenance for every token is recorded in `tokens.rs`; the
//! per-surface identity (nav links, verbatim trademark line) lives in `surface.rs`.

pub mod layout;
pub mod surface;
pub mod tokens;

pub use layout::wrap_static_html;
pub use surface::SoftwareSurface;
