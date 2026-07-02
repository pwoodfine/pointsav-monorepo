//! Sovereign Editorial brand tokens for the software.pointsav.com chrome.
//!
//! Provenance per the 2026-07-01 token-reconciliation research (read-only phase).
//! The values are split into two classes:
//!
//!   REUSE — registered design-system tokens, adopted verbatim for cross-site
//!           consistency:
//!             footer.bg / footer.fg / footer.divider
//!               → pointsav-design-system tokens/editorial-surface/editorial-surface.dtcg.json
//!             white
//!               → pointsav-design-system dtcg-vault/tokens/primitive.json
//!
//!   GAP  — genuinely unregistered values; hex fallbacks used here pending
//!          DESIGN-TOKEN-CHANGE drafts staged to .agent/drafts-outbound/:
//!             masthead navy  (reconciliation item e-2; live but unregistered in the vault)
//!             gold accent    (reconciliation item e-1; no brand-accent.software entry exists)
//!
//! These consts are the single source of truth; `layout::chrome_style` emits them
//! as CSS custom properties so the stylesheet and any Rust-side markup agree.

/// GAP — masthead / top-nav navy. Live in wiki CSS but unregistered in the token
/// vault (reconciliation item e-2). ≈ oklch(39.1% 0.100 253).
pub const TOPNAV_BG: &str = "#164679";

/// GAP — gold accent. No `brand-accent.software` entry exists (reconciliation
/// item e-1). ≈ oklch(74.6% 0.098 87); passes contrast on both navy and near-black.
pub const ACCENT: &str = "#C7A961";

/// REUSE — registered `footer.bg` (editorial-surface.dtcg.json). Adopted over the
/// BRIEF's near-neutral #0e1117 for cross-site consistency, per the reconciliation.
pub const FOOTER_BG: &str = "oklch(16% 0.06 250)";

/// REUSE — registered `footer.fg`.
pub const FOOTER_FG: &str = "oklch(70% 0.03 255)";

/// REUSE — registered `footer.divider`.
pub const FOOTER_DIVIDER: &str = "oklch(25% 0.04 255)";

/// REUSE — `primitive.white`. In practice an ink token on the masthead container
/// (the wordmark glyph renders with `currentColor`).
pub const WORDMARK: &str = "#ffffff";
