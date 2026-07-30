// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

//! Pricing page — `GET /pricing`.
//!
//! Phase 4: catalog-driven (like `software_page`), not a static file — the
//! product counts per tier are always live, so this page cannot drift from
//! `products.yaml` the way the old `static/licensing.html` did (see that file's
//! Phase 4 rewrite for the fictional content it used to carry). Leads with the
//! buy-to-own framing established in the BRIEF's Positioning Pivot; carries the
//! BC tax line and AGPL §13 source link required by `BRIEF-software-distribution-
//! substrate.md` and previously missing anywhere in this crate.
//!
//! **Rebuilt 2026-07-07** to match `BRIEF-software-licensing-structure.md`'s
//! four-tier model (`Proprietary`/`Fsl`/`Agpl`/`Apache`, see `LicenseTier`'s doc
//! comment in `main.rs`) and, separately, to stop displaying a per-tier dollar
//! figure. The prior version showed "$1.00 USDC" / "$19.00 USDC" as each tier's
//! "canonical future price" — that BRIEF's §4 explicitly states **no non-zero
//! future price has been ratified for any tier**: "Actual, non-zero future
//! pricing for any product — this BRIEF only ratifies $0-for-now; real pricing
//! is a separate, later decision per product." Those two numbers were this
//! crate's own earlier invention, not a ratified fact, and showing them as tier
//! pricing on a real storefront page was a real accuracy problem independent of
//! the naming mismatch — removed rather than relabeled.

use crate::{Catalog, LicenseTier};
use maud::{html, Markup, PreEscaped};

fn tier_card(tier: LicenseTier, catalog: &Catalog) -> Markup {
    let count = catalog
        .installers
        .iter()
        .filter(|i| i.license_tier == tier)
        .count();
    let desc = match tier {
        LicenseTier::Proprietary => {
            "No source grant. The company's stated commercial moat — solo use is \
             free during BETA; aggregation or resale requires a separate \
             commercial agreement, permanently."
        }
        LicenseTier::Fsl => {
            "Source-readable now. A two-year non-compete restriction, then \
             automatic conversion to Apache-2.0 for that release."
        }
        LicenseTier::Agpl => {
            "AGPL-3.0-or-later. Source is freely available under copyleft terms; \
             a purchase buys the official signed binary and commercial \
             redistribution rights, not access to the code itself."
        }
        LicenseTier::Apache => {
            "A genuine, unconditional Apache-2.0 grant. No purchase, no license \
             key, no BETA gate to lift — free to use, fork, or redistribute."
        }
    };
    html! {
        article."sw-pr-tier" {
            h2."sw-pr-tier__name" { (tier.label()) }
            p."sw-pr-tier__count" { (count) " product" (if count == 1 { "" } else { "s" }) }
            p."sw-pr-tier__desc" { (desc) }
        }
    }
}

pub fn pricing_markup(catalog: &Catalog) -> Markup {
    html! {
        (pricing_style())
        div."sw-pr-wrap" {
            h1."sw-pr-title" { "Pricing" }
            p."sw-pr-lede" {
                "Buy it once. Run it anywhere. Own it forever. No subscription, no cloud \
                 dependency, no kill switch."
            }
            div."sw-pr-trust" {
                span { "Air-gap capable" }
                span aria-hidden="true" { "\u{b7}" }
                span { "No telemetry" }
                span aria-hidden="true" { "\u{b7}" }
                span { "Runs offline" }
                span aria-hidden="true" { "\u{b7}" }
                span { "Your keys, your license" }
            }
            div."sw-pr-tiers" {
                (tier_card(LicenseTier::Proprietary, catalog))
                (tier_card(LicenseTier::Fsl, catalog))
                (tier_card(LicenseTier::Agpl, catalog))
                (tier_card(LicenseTier::Apache, catalog))
            }
            p."sw-pr-beta-note" {
                "Every product is currently free during BETA, regardless of tier — future \
                 pricing has not been set for any product yet. See each product's own \
                 listing on the "
                a href="/software" { "Products" }
                " page for its current, active price."
            }
            p."sw-pr-tax" {
                "No tax collected — PointSav Digital Systems operates below the GST \
                 small-supplier threshold."
            }
            p."sw-pr-agpl" {
                "Source: "
                a href="https://github.com/pointsav/pointsav-monorepo" {
                    "github.com/pointsav/pointsav-monorepo"
                }
            }
        }
    }
}

fn pricing_style() -> Markup {
    let css = r#".sw-pr-wrap{max-width:840px;margin:0 auto;padding:40px 24px 64px;box-sizing:border-box;}
.sw-pr-title{margin:0 0 8px;font-family:Georgia,"Times New Roman",serif;font-size:30px;color:#111827;}
.sw-pr-lede{margin:0 0 16px;font-size:16px;line-height:1.5;color:#344054;max-width:56ch;}
.sw-pr-trust{margin:0 0 32px;font-size:12px;letter-spacing:.04em;color:#667085;display:flex;gap:10px;flex-wrap:wrap;}
.sw-pr-tiers{display:grid;grid-template-columns:repeat(auto-fit,minmax(280px,1fr));gap:20px;margin:0 0 32px;}
.sw-pr-tier{border:1px solid #e4e7ec;border-radius:10px;padding:24px;background:#fff;box-shadow:0 1px 2px rgba(16,24,40,.04);}
.sw-pr-tier__name{font-family:Georgia,"Times New Roman",serif;font-size:20px;margin:0 0 8px;color:#111827;}
.sw-pr-tier__count{font-size:12px;color:#234ed8;font-weight:600;margin:0 0 12px;}
.sw-pr-tier__desc{font-size:13.5px;line-height:1.55;color:#475467;margin:0;}
.sw-pr-beta-note{font-size:13px;line-height:1.55;color:#475467;margin:0 0 16px;padding:14px;border:1px dashed #d0d5dd;border-radius:8px;background:#fcfcfd;}
.sw-pr-tax{font-size:12.5px;color:#667085;margin:0 0 8px;}
.sw-pr-agpl{font-size:12.5px;color:#667085;margin:0;}"#;
    html! { style { (PreEscaped(css)) } }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Installer;

    fn fixture() -> Catalog {
        Catalog {
            installers: vec![
                Installer {
                    id: "os-orchestration".into(),
                    name: "Orchestration OS".into(),
                    description: "d".into(),
                    edition: "1.0".into(),
                    platform: "linux".into(),
                    size_mb: 1,
                    path: "os-orchestration/1.0".into(),
                    license_tier: LicenseTier::Proprietary,
                    price_usdc: 0,
                    fsl_conversion_date: None,
                    guide_url: None,
                },
                Installer {
                    id: "os-console".into(),
                    name: "Console".into(),
                    description: "d".into(),
                    edition: "1.0".into(),
                    platform: "linux".into(),
                    size_mb: 1,
                    path: "os-console/1.0".into(),
                    license_tier: LicenseTier::Agpl,
                    price_usdc: 0,
                    fsl_conversion_date: None,
                    guide_url: None,
                },
                Installer {
                    id: "os-privategit".into(),
                    name: "PrivateGit".into(),
                    description: "d".into(),
                    edition: "1.0".into(),
                    platform: "linux".into(),
                    size_mb: 1,
                    path: "os-privategit/1.0".into(),
                    license_tier: LicenseTier::Fsl,
                    price_usdc: 0,
                    fsl_conversion_date: None,
                    guide_url: None,
                },
                Installer {
                    id: "os-mediakit".into(),
                    name: "MediaKit".into(),
                    description: "d".into(),
                    edition: "1.0".into(),
                    platform: "linux".into(),
                    size_mb: 1,
                    path: "os-mediakit/1.0".into(),
                    license_tier: LicenseTier::Fsl,
                    price_usdc: 0,
                    fsl_conversion_date: None,
                    guide_url: None,
                },
                Installer {
                    id: "tool-wallet".into(),
                    name: "PointSav Wallet".into(),
                    description: "d".into(),
                    edition: "1.0".into(),
                    platform: "linux".into(),
                    size_mb: 1,
                    path: "tool-wallet/1.0".into(),
                    license_tier: LicenseTier::Apache,
                    price_usdc: 0,
                    fsl_conversion_date: None,
                    guide_url: None,
                },
            ],
        }
    }

    #[test]
    fn pricing_page_shows_all_four_tiers_with_live_counts() {
        let html = pricing_markup(&fixture()).into_string();
        assert!(html.contains("Proprietary"));
        assert!(html.contains("1 product<"));
        assert!(html.contains("FSL-1.1-ALv2"));
        assert!(html.contains("2 products"));
        assert!(html.contains("AGPL-3.0-or-later"));
        assert!(html.contains("Apache-2.0 (Open Source)"));
        assert!(
            !html.contains("sw-pr-tier__amt"),
            "must not display a fabricated per-tier dollar figure — \
             BRIEF-software-licensing-structure.md §4 ratifies no future price"
        );
    }

    #[test]
    fn pricing_page_carries_beta_tax_and_agpl_notes() {
        let html = pricing_markup(&fixture()).into_string();
        assert!(html.contains("currently free during BETA"));
        assert!(html.contains("No tax collected"));
        assert!(html.contains("github.com/pointsav/pointsav-monorepo"));
    }
}
