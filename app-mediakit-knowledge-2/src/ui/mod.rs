//! UI layer — the visible website: per-tenant identity and the chrome shell.
//! The visual system lives in `static/{tokens,app}.css`; this module emits the
//! matching `k-*` HTML structure (Wikipedia Vector 2022 pattern).

pub mod layout;
pub mod tenant;

pub use layout::{article, doc_head, footer, header, mobile_nav, page, utility_bar};
pub use tenant::{SiblingWiki, Tenant};
