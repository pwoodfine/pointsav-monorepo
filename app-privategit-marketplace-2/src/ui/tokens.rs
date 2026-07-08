//! Brand tokens for the software.pointsav.com chrome.
//!
//! **Corrected again 2026-07-07** (second pass, same day — operator caught a real
//! mistake in the first correction). `home.pointsav.com`'s `tokens.css` is
//! **two-tenant**: a `:root` block of base values, then a `[data-brand="pointsav"]`
//! block that *overrides* the navy scale for anything rendered as PointSav. The
//! `:root` base navy (`#164679`) is explicitly commented in that file as
//! **"not Woodfine's navy"** being overridden — i.e. `#164679` **is Woodfine's
//! color**, the fallback used when no `data-brand` override applies. The real,
//! distinct **PointSav** brand color only exists inside the `[data-brand="pointsav"]`
//! block: `--m-navy-700: #234ed8` ("canonical PointSav brand color", per that
//! file's own inline comment). `home.pointsav.com`'s actual `<html>` tag carries
//! `data-brand="pointsav"` — confirmed directly from its served HTML — so `#234ed8`,
//! not `#164679`, is what that site actually renders. The first correction pass
//! today read only the un-overridden `:root` block and missed the tenant override
//! entirely, so it accidentally re-introduced Woodfine's color instead of fixing to
//! PointSav's. software.pointsav.com is unambiguously a PointSav property — it must
//! use the PointSav override values, not the Woodfine-default base values.
//!
//! The footer/typography findings from the first pass still stand (verified
//! separately, unaffected by the brand-override discovery):
//!   - Footer near-black → light grey `#f8f9fa`, matching both real family sites.
//!   - Gold accent → removed; the family's accent is this navy/blue scale, not gold.
//!
//! Full PointSav override scale, verified verbatim from `home.pointsav.com`'s
//! `[data-brand="pointsav"]` block:
//! `navy-900: #0c2785` · `navy-800: #173ab1` · `navy-700: #234ed8` (primary accent)
//! · `navy-600: #3f6cf2` (link hover) · `navy-100: #eef3ff` (tint).
//!
//! All values below are read directly from the live, served CSS of
//! `home.pointsav.com` (`app-mediakit-marketing-2/static/tokens.css`) and
//! `documentation.pointsav.com` (`app-mediakit-knowledge-2/static/tokens.css`),
//! not re-derived or approximated. `--sw-*` chrome consts here should be treated
//! as this crate's own copy of those verified values, not a distinct design
//! direction — if either upstream site's tokens change, re-verify here. **Always
//! check for a `[data-brand="pointsav"]` (or equivalent tenant) override block
//! before trusting a `:root` base value from a shared multi-tenant token file.**
//!
//! These consts are the single source of truth; `layout::chrome_style` emits them
//! as CSS custom properties so the stylesheet and any Rust-side markup agree.

/// Masthead — PointSav's actual brand blue (`[data-brand="pointsav"]`'s
/// `--m-navy-700`/`--m-surface-chrome`), NOT the `:root` base value (`#164679`,
/// which is Woodfine's color — see module doc for the full mistake/correction).
pub const TOPNAV_BG: &str = "#234ed8";

/// Text/wordmark on the masthead — verified against
/// `home.pointsav.com`'s `--m-ink-on-chrome` (`--m-white`); not brand-overridden.
pub const ON_CHROME: &str = "#ffffff";

/// Muted secondary text on the masthead (e.g. the "Software" subtitle under
/// the wordmark) — verified against `home.pointsav.com`'s `--m-ink-on-chrome-muted`
/// (`--m-slate-300`); not brand-overridden, same value for every tenant.
pub const ON_CHROME_MUTED: &str = "#b4c5d5";

/// The one accent color — PointSav's brand blue, for links/interactive elements on
/// light surfaces. Verified against `home.pointsav.com`'s `--m-link`
/// (`[data-brand="pointsav"]`'s `--m-navy-700`, same value as the masthead).
pub const ACCENT: &str = "#234ed8";

/// Hover/active/large-fill state for the accent — verified against
/// `home.pointsav.com`'s `--m-cta-bg-hover`/`--m-footer-link-hover`
/// (`[data-brand="pointsav"]`'s `--m-navy-800`).
pub const ACCENT_HOVER: &str = "#173ab1";

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
