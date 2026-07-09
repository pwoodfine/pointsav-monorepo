// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

//! Privacy page for software.pointsav.com — a self-contained page, not a link
//! out to the wiki's or marketing sites' own privacy pages (same operator
//! instruction as `disclaimer.rs`: this site has its own content, no
//! cross-site links, because it does its own thing — software binary
//! distribution and a USDC-payment license marketplace).
//!
//! Content is deliberately narrow: only what this crate actually does
//! (payment-verification data, on-chain USDC records, license issuance) is
//! described. No retention policy or physical mailing address is invented —
//! neither is documented anywhere in the corpus as of 2026-07-03.

use maud::{html, Markup};

use super::surface::SoftwareSurface;

/// The full self-contained privacy page (`GET /page/privacy`).
pub fn privacy_markup() -> Markup {
    html! {
        div."sw-legal" {
            h1 { "Privacy" }
            p."sw-legal__lede" {
                strong { "PointSav Digital Systems" } " — a trade name of Woodfine Capital "
                "Projects Inc."
            }
            p {
                "This site distributes software binaries and issues product licenses. "
                "This page describes what data it collects and how it is used."
            }

            h2 { "1. What this site collects" }
            p {
                "This site collects payment-verification data only: the transaction hash "
                "and wallet address you supply when purchasing a license, and the receipt "
                "and claim records generated from a confirmed payment. This site does not "
                "use tracking cookies for analytics or advertising. It does use a session "
                "token for signed-in account/license-status functionality where "
                "applicable — this is a functional exception, not a tracking mechanism."
            }

            h2 { "2. On-chain data" }
            p {
                "Payments are made in USDC on the Polygon network. Once submitted, a "
                "transaction and its associated wallet address are recorded permanently "
                "and publicly on the Polygon blockchain. This is inherent to how "
                "on-chain payment works and cannot be retracted or deleted by PointSav "
                "Digital Systems."
            }

            h2 { "3. How data is used" }
            p {
                "Payment-verification data is used to issue and validate product "
                "licenses, to answer order-status lookups, and to maintain the "
                "transaction-log bookkeeping this business is required to keep. It is "
                "not sold or shared with third parties for marketing purposes."
            }

            h2 { "4. Data retention and access" }
            p {
                "Receipt and claim records are stored on PointSav Digital Systems' own "
                "infrastructure. No specific retention period is documented for this "
                "site as of this writing; this is a genuine open item, not an oversight "
                "concealed here. If you have a question about a specific record, contact "
                "us using the details below."
            }

            h2 { "5. Contact" }
            p {
                "Questions about this privacy page can be sent to " a href="mailto:open.source@pointsav.com" { "open.source@pointsav.com" } "."
            }

            hr;

            p."sw-legal__copyright" { "\u{00a9} 2026 Woodfine Capital Projects Inc. All rights reserved." }
            p."sw-legal__trademark" { (SoftwareSurface::Marketplace.trademark_line()) }
        }
    }
}
