// SPDX-License-Identifier: FSL-1.1-ALv2
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

//! Compile-time embedded static assets (CSS, JS, fonts, SVG).
//!
//! In release builds the `static/` tree is baked into the binary, so the
//! engine has no runtime filesystem dependency for its assets. In debug
//! builds rust-embed reads from disk, allowing live CSS/JS edits.

use rust_embed::Embed;

#[derive(Embed)]
#[folder = "static/"]
pub struct StaticAssets;
