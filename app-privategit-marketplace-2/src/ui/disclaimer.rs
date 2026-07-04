//! Disclaimer content for software.pointsav.com — a self-contained page, not
//! a link out to the wiki's or marketing sites' own disclaimer (operator
//! instruction, 2026-07-02: this site has its own content, no cross-site
//! links, because it does its own thing — software binary distribution and
//! a USDC-payment license marketplace, not an investment offering).
//!
//! Adapted from `factory-release-engineering/policies/DISCLAIMER.md` and the
//! `app-mediakit-marketing` tenant disclaimers, but rewritten for what this
//! site actually does: the LP-investment-offering sections (accredited
//! investor exemptions, Private Placement Memorandum, per-jurisdiction
//! exemption table) are dropped — no partnership units are sold here. Added:
//! a license-terms section and an on-chain USDC payment-risk section, which
//! is genuinely new content with no directly equivalent precedent elsewhere
//! in the corpus (flagged for counsel review before this is final, same as
//! the source DISCLAIMER.md's own closing note).

use maud::{html, Markup};

use super::surface::SoftwareSurface;

/// The condensed "Important information" disclosure-slot content, rendered
/// inside the footer's collapsed-by-default accordion
/// (`layout::footer`) — matches the `app-mediakit-marketing-2`
/// `DisclosureSlot` pattern: on-page and readable without JS, but collapsed
/// so it doesn't read as a second copy of the trademark paragraph directly
/// below it. Links to `disclaimer_markup` (`/page/disclaimer`) for the full
/// text.
pub fn disclosure_body() -> Markup {
    html! {
        p {
            "This site sells software licenses only — not an offer of securities or "
            "investment. See the full " a href="/page/disclaimer" { "Disclaimer" } "."
        }
        p {
            strong { "Payments are made in USDC on the Polygon network." }
            " Verify the address and network before sending — on-chain payments are "
            "irreversible and cannot be refunded if sent to the wrong address, wrong "
            "network, or wrong amount. License issuance depends on blockchain "
            "confirmation and may take a few minutes."
        }
    }
}

/// The full self-contained disclaimer page (`GET /page/disclaimer`).
pub fn disclaimer_markup() -> Markup {
    html! {
        div."sw-legal" {
            h1 { "Disclaimer" }
            p."sw-legal__lede" {
                strong { "PointSav Digital Systems" } " — a trade name of Woodfine Capital "
                "Projects Inc."
            }
            p {
                "This site distributes software binaries and issues product licenses. "
                "By using this site, you acknowledge the following."
            }

            h2 { "1. No warranty" }
            p {
                "Software made available on this site is provided \"as is,\" without "
                "warranty of any kind, express or implied, including but not limited to "
                "warranties of merchantability, fitness for a particular purpose, and "
                "non-infringement. PointSav Digital Systems does not guarantee that any "
                "binary distributed here is free of defects or uninterrupted in operation. "
                "Product descriptions, including BETA status, reflect intended "
                "capabilities and may change without notice."
            }

            h2 { "2. Not an offer of securities" }
            p {
                "Nothing on this site constitutes an offer to sell, or a solicitation of "
                "an offer to buy, any security, partnership interest, or investment "
                "product. This site sells software licenses only. Woodfine Capital "
                "Projects Inc.'s securities offerings are described separately at its own "
                "investor-facing properties and are not made through this site."
            }

            h2 { "3. License terms" }
            p {
                "Each product distributed here is governed by its own license terms, "
                "referenced from that product's catalog entry: open-source licenses for "
                "products distributed under one (Apache-2.0, FSL, or as otherwise "
                "stated), and the applicable commercial license terms for paid tiers. "
                "Purchasing a license here does not transfer ownership of, or any right "
                "in, Woodfine Capital Projects Inc. or its affiliates."
            }

            h2 { "4. Payment, on the Polygon network, in USDC" }
            p {
                "Licenses on this site are purchased by sending USDC on the Polygon "
                "network to the address shown at time of purchase. You are solely "
                "responsible for verifying that address, the network (Polygon, not "
                "Ethereum mainnet or any other chain), and the payment amount before "
                "sending funds. On-chain transactions are irreversible. PointSav Digital "
                "Systems cannot reverse, refund, or recover a payment sent to the wrong "
                "address, on the wrong network, or for the wrong amount. License "
                "issuance depends on confirmation of your transaction on the Polygon "
                "network, which is outside PointSav Digital Systems' control and may be "
                "delayed by network conditions."
            }

            h2 { "5. Restrictions on use of materials" }
            p {
                "Information on this site may be reproduced in hard copy for personal "
                "reference only, provided that all copyright and proprietary notices are "
                "retained. Other reproduction or distribution, in any form or by any "
                "means, without the express written permission of Woodfine Capital "
                "Projects Inc. is prohibited."
            }

            h2 { "6. Forward-looking information" }
            p {
                "This site may describe planned or intended product capabilities, "
                "including BETA features and pricing. Words such as \"plans,\" "
                "\"intends,\" \"expects,\" \"may,\" \"will,\" and \"targets\" are intended "
                "to identify forward-looking statements. Actual availability, "
                "functionality, and pricing may differ materially from what is "
                "described, and PointSav Digital Systems disclaims any obligation to "
                "update these statements except where required by applicable law."
            }

            h2 { "7. Jurisdictional restrictions" }
            p {
                "Access to and use of this site, including the ability to purchase a "
                "license or send cryptocurrency payment, may be restricted or unlawful "
                "in some jurisdictions. You are responsible for determining whether "
                "your use of this site complies with the laws of your jurisdiction."
            }

            hr;

            p."sw-legal__copyright" { "\u{00a9} 2026 Woodfine Capital Projects Inc. All rights reserved." }
            p."sw-legal__trademark" { (SoftwareSurface::Marketplace.trademark_line()) }
        }
    }
}
