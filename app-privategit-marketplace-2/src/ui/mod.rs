// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

//! Sovereign Editorial chrome for the marketplace storefront.
//!
//! `layout::wrap_static_html` wraps the P1 prerendered static pages (`software.html`,
//! `licensing.html`) with the dark navy masthead and near-black institutional
//! footer without changing the static-file-reading logic in `main.rs`.
//!
//! Design-system provenance for every token is recorded in `tokens.rs`; the
//! per-surface identity (nav links, verbatim trademark line) lives in `surface.rs`.

pub mod accessibility;
pub mod catalog;
pub mod checkout;
pub mod contact;
pub mod disclaimer;
pub mod layout;
pub mod order;
pub mod pricing;
pub mod privacy;
pub mod product_detail;
pub mod surface;
pub mod tokens;

pub use accessibility::accessibility_markup;
pub use catalog::catalog_markup;
pub use checkout::checkout_markup;
pub use contact::contact_markup;
pub use disclaimer::{disclaimer_markup, disclosure_body};
pub use layout::{render_page, wrap_static_html};
pub use order::{order_confirmed_markup, order_not_found_markup, order_pending_markup};
pub use pricing::pricing_markup;
pub use privacy::privacy_markup;
pub use product_detail::product_detail_markup;
pub use surface::SoftwareSurface;
