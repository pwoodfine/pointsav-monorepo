// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

//! Contact page for software.pointsav.com — a self-contained page, not a link
//! out to the wiki's or marketing sites' own pages (same operator instruction
//! as `disclaimer.rs`).
//!
//! Closes the highest-priority finding from the original Sovereign Editorial
//! audit (`/page/contact` returning HTTP 0 — a dead customer-facing link,
//! flagged twice: once in the original 2026-06-24 audit and once in an
//! operator dogfood-test escalation on 2026-06-30).
//!
//! `open.source@pointsav.com` is the only real, corpus-documented contact
//! channel found anywhere in this workspace (`~/Foundry/CLAUDE.md` §1, the
//! System Administrator contact). No ticketing system, phone number, or
//! physical mailing address is documented — none is invented here.

use maud::{html, Markup};

use super::surface::SoftwareSurface;

/// The full self-contained contact page (`GET /page/contact`).
pub fn contact_markup() -> Markup {
    html! {
        div."sw-legal" {
            h1 { "Contact us" }
            p."sw-legal__lede" {
                strong { "PointSav Digital Systems" } " — a trade name of Woodfine Capital "
                "Projects Inc."
            }

            h2 { "1. Support channel" }
            p {
                "The fastest way to reach us is " a href="mailto:open.source@pointsav.com" { "open.source@pointsav.com" } "."
            }

            h2 { "2. What to contact us about" }
            p {
                "License and order issues (reference your transaction hash), binary or "
                "download problems, accessibility reports, and security disclosures."
            }

            h2 { "3. Response expectations" }
            p {
                "We do not publish a guaranteed response-time commitment for this "
                "channel."
            }

            h2 { "4. Offices" }
            p { "Vancouver \u{00b7} New York \u{00b7} Berlin." }

            hr;

            p."sw-legal__copyright" { "\u{00a9} 2026 Woodfine Capital Projects Inc. All rights reserved." }
            p."sw-legal__trademark" { (SoftwareSurface::Marketplace.trademark_line()) }
        }
    }
}
