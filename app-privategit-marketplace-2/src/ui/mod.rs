//! Sovereign Editorial chrome for the marketplace storefront.
//!
//! `layout::wrap_static_html` wraps the P1 prerendered static pages (`software.html`,
//! `licensing.html`) with the dark navy masthead and near-black institutional
//! footer without changing the static-file-reading logic in `main.rs`.
//!
//! Design-system provenance for every token is recorded in `tokens.rs`; the
//! per-surface identity (nav links, verbatim trademark line) lives in `surface.rs`.

pub mod catalog;
pub mod checkout;
pub mod disclaimer;
pub mod layout;
pub mod order;
pub mod surface;
pub mod tokens;

pub use catalog::catalog_markup;
pub use checkout::checkout_markup;
pub use disclaimer::{disclaimer_markup, disclosure_body};
pub use layout::{render_page, wrap_static_html};
pub use order::{order_confirmed_markup, order_not_found_markup, order_pending_markup};
pub use surface::SoftwareSurface;
