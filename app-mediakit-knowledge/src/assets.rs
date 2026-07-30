//! Embedded static assets — CSS, JS, fonts.
//!
//! In release builds, assets are baked into the binary. In debug
//! builds, rust-embed reads from disk for live edit. Single-binary
//! constraint preserved.

use rust_embed::Embed;

#[derive(Embed)]
#[folder = "static/"]
pub struct StaticAsset;
