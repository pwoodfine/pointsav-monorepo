//! `app-mediakit-shell` — the shared chrome chassis for the `os-mediakit`
//! application family.
//!
//! Mirrors the role `app-console-keys` plays for the console family: it is
//! the crate that owns the shared chrome and the component vocabulary, while
//! the OS binary (`os-mediakit`, future) launches the app instances and each
//! app crate (`app-mediakit-marketing`, and — planned — `-knowledge` /
//! `-distributions`) depends on this one.
//!
//! Three responsibilities:
//!
//! 1. **Chrome** — the persistent header/footer/page frame ([`shell`]),
//!    identical across every instance. The AI never edits it.
//! 2. **Component vocabulary** — the typed [`Section`](section::Section) set
//!    an AI author assembles a page from ([`section`], [`page`]). The schema
//!    is the contract: a manifest either deserializes into these types or it
//!    is rejected.
//! 3. **Tokens** — DTCG design-token loading ([`tokens`]); components
//!    reference only token custom properties, never hard-coded values, so a
//!    composition cannot produce off-brand or broken-responsive output.
//!
//! The render entry point is [`shell::render_page`].

pub mod page;
pub mod render;
pub mod section;
pub mod shell;
pub mod tokens;

pub use page::Page;
pub use section::Section;
pub use shell::{render_page, Brand};

/// Library version, surfaced by consumers (e.g. the MCP `serverInfo` block).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
