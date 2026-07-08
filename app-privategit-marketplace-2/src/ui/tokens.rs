//! Brand tokens for the software.pointsav.com chrome.
//!
//! **Corrected 2026-07-07** (operator-flagged visual audit — see
//! `BRIEF-binary-library-repositioning.md`'s redesign section). The prior version of
//! this file inherited two values from `BRIEF-sovereign-editorial-software.md`
//! (2026-06-24) that never got cross-checked against what the rest of the
//! PointSav/Woodfine family actually ships, and both turned out wrong:
//!
//!   - **Footer:** was `oklch(16% 0.06 250)` (near-black), cited as "REUSE" from
//!     `pointsav-design-system/tokens/editorial-surface/editorial-surface.dtcg.json`.
//!     That token file's own `$description` claims it's "live on
//!     documentation.pointsav.com" — verified false: `documentation.pointsav.com`'s
//!     actual served `tokens.css` has `--k-surface-sunken: #f8f9fa` (light grey) as
//!     its real footer background, and `home.pointsav.com`'s tokens.css explicitly
//!     documents dropping a near-black footer "in favor of full family consistency
//!     with the wiki" (2026-07-01 operator call). The design-system DTCG file is
//!     stale, registered before that correction and never updated to match.
//!   - **Accent:** was `#C7A961` gold, honestly flagged even at the time as "GAP —
//!     no `brand-accent.software` entry exists." Verified: gold appears nowhere in
//!     `home.pointsav.com` or `documentation.pointsav.com`'s real CSS. The family's
//!     one accent color is navy/blue (`#164679` on the marketing site, `#1a4480` on
//!     the wiki) — used for links and interactive elements on light surfaces, never
//!     a second decorative color.
//!
//! All values below are read directly from the live, served CSS of
//! `home.pointsav.com` (`app-mediakit-marketing-2/static/tokens.css`) and
//! `documentation.pointsav.com` (`app-mediakit-knowledge-2/static/tokens.css`),
//! not re-derived or approximated. `--sw-*` chrome consts here should be treated
//! as this crate's own copy of those verified values, not a distinct design
//! direction — if either upstream site's tokens change, re-verify here.
//!
//! These consts are the single source of truth; `layout::chrome_style` emits them
//! as CSS custom properties so the stylesheet and any Rust-side markup agree.

/// Masthead navy — verified against `home.pointsav.com`'s `--m-navy-700`
/// (`--m-surface-chrome`). Unchanged from the prior tokens; this one was correct.
pub const TOPNAV_BG: &str = "#164679";

/// Text/wordmark on the navy masthead — verified against
/// `home.pointsav.com`'s `--m-ink-on-chrome` (`--m-white`).
pub const ON_CHROME: &str = "#ffffff";

/// Muted secondary text on the navy masthead (e.g. the "Software" subtitle under
/// the wordmark) — verified against `home.pointsav.com`'s `--m-ink-on-chrome-muted`
/// (`--m-slate-300`), the PointSav hero-slate keep-list color.
pub const ON_CHROME_MUTED: &str = "#b4c5d5";

/// The family's one accent color — navy, for links/interactive elements on light
/// surfaces. Verified against `home.pointsav.com`'s `--m-link` (`--m-navy-700`,
/// same value as the masthead) and `documentation.pointsav.com`'s `--k-accent`
/// (`#1a4480`, same navy family). Replaces the unverified gold placeholder.
pub const ACCENT: &str = "#164679";

/// Hover/active state for the navy accent on light surfaces — verified against
/// `home.pointsav.com`'s `--m-link-hover` (`--m-navy-600`).
pub const ACCENT_HOVER: &str = "#1d5795";

/// Footer background — verified against `documentation.pointsav.com`'s
/// `--k-surface-sunken` and `home.pointsav.com`'s `--m-surface-footer`, both
/// `#f8f9fa`. Replaces the stale near-black value.
pub const FOOTER_BG: &str = "#f8f9fa";

/// Primary ink on the light footer — verified against `home.pointsav.com`'s
/// `--m-ink-on-footer` (`--m-ink-550`), matching the wiki's own footer ink.
pub const FOOTER_FG: &str = "#54595d";

/// Muted/secondary ink on the light footer — verified against
/// `home.pointsav.com`'s `--m-ink-on-footer-muted` (`--m-ink-525`).
pub const FOOTER_FG_MUTED: &str = "#646a70";

/// Footer divider/border — verified against `documentation.pointsav.com`'s
/// `--k-border` and `home.pointsav.com`'s `--m-grey-350`, both `#c8ccd1`. Replaces
/// the stale dark-on-dark divider that assumed a near-black footer.
pub const FOOTER_DIVIDER: &str = "#c8ccd1";

/// Primary body ink (headings, primary text on light surfaces) — verified against
/// `home.pointsav.com`'s `--m-ink-900` / `documentation.pointsav.com`'s body ink.
pub const INK: &str = "#111827";

/// `primitive.white` — the masthead wordmark glyph renders with `currentColor`.
pub const WORDMARK: &str = "#ffffff";
