//! Accessibility page for software.pointsav.com — a self-contained page, not
//! a link out to the wiki's or marketing sites' own pages (same operator
//! instruction as `disclaimer.rs`).
//!
//! Targets WCAG 2.1 AA (the standard already stated for a sibling PointSav
//! product in this corpus, kept consistent rather than invented fresh). Does
//! not claim a completed compliance certification — no formal audit re-score
//! is on record for this crate specifically, only the original 2026-06-24
//! audit finding (74 axe-violation nodes, full mobile layout breakdown) that
//! this crate's Sovereign Editorial rebuild addressed.

use maud::{html, Markup};

use super::surface::SoftwareSurface;

/// The full self-contained accessibility page (`GET /page/accessibility`).
pub fn accessibility_markup() -> Markup {
    html! {
        div."sw-legal" {
            h1 { "Accessibility" }
            p."sw-legal__lede" {
                strong { "PointSav Digital Systems" } " — a trade name of Woodfine Capital "
                "Projects Inc."
            }

            h2 { "1. Standard targeted" }
            p {
                "This site targets WCAG 2.1 Level AA, the same standard used across the "
                "PointSav platform's other properties."
            }

            h2 { "2. What has been addressed" }
            p {
                "A 2026-06-24 audit of this site identified a complete mobile layout "
                "breakdown and a high density of automated accessibility-check findings. "
                "As part of the 2026-07 storefront rebuild, contrast tokens, responsive "
                "layout, and semantic landmarks were addressed across the site's chrome "
                "and content pages. This is a factual description of work completed, not "
                "a claim of full WCAG 2.1 AA conformance — no formal re-audit score has "
                "been recorded for this rebuild."
            }

            h2 { "3. Known gaps and how to report one" }
            p {
                "If you encounter a barrier using this site, we want to know about it. "
                "Reports are reviewed and addressed on an ongoing basis; we do not claim "
                "the site is free of accessibility issues."
            }

            h2 { "4. Contact for accessibility issues" }
            p {
                "Report accessibility issues to " a href="mailto:open.source@pointsav.com" { "open.source@pointsav.com" } "."
            }

            hr;

            p."sw-legal__copyright" { "\u{00a9} 2026 Woodfine Capital Projects Inc. All rights reserved." }
            p."sw-legal__trademark" { (SoftwareSurface::Marketplace.trademark_line()) }
        }
    }
}
