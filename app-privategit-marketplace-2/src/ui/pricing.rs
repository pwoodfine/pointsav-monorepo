//! Pricing page — `GET /pricing`.
//!
//! Phase 4: catalog-driven (like `software_page`), not a static file — the
//! product counts per tier are always live, so this page cannot drift from
//! `products.yaml` the way the old `static/licensing.html` did (see that file's
//! Phase 4 rewrite for the fictional content it used to carry). Leads with the
//! buy-to-own framing established in the BRIEF's Positioning Pivot; carries the
//! BC tax line and AGPL §13 source link required by `BRIEF-software-distribution-
//! substrate.md` and previously missing anywhere in this crate.

use crate::{Catalog, LicenseTier};
use maud::{html, Markup, PreEscaped};

fn tier_card(tier: LicenseTier, catalog: &Catalog) -> Markup {
    let count = catalog
        .installers
        .iter()
        .filter(|i| i.license_tier == tier)
        .count();
    let dollars = tier.canonical_price_usdc() as f64 / 1_000_000.0;
    let desc = match tier {
        LicenseTier::Commercial => {
            "Apache-2.0-equivalent rights on the compiled binary. No copyleft \
             obligations. Fork, redistribute, or compete freely."
        }
        LicenseTier::Fsl => {
            "Source-readable. A two-year non-compete restriction, then automatic \
             conversion to Apache 2.0 for that version."
        }
    };
    html! {
        article."sw-pr-tier" {
            h2."sw-pr-tier__name" { (tier.label()) }
            p."sw-pr-tier__amt" { "$" (format!("{dollars:.2}")) " USDC" }
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
                (tier_card(LicenseTier::Commercial, catalog))
                (tier_card(LicenseTier::Fsl, catalog))
            }
            p."sw-pr-beta-note" {
                "Every product is currently free during BETA — the prices above are what \
                 apply once BETA lifts for a given product, not a charge in effect today. \
                 See each product's own listing on the "
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
.sw-pr-tier__amt{font-size:28px;font-weight:700;font-family:Georgia,"Times New Roman",serif;color:#111827;margin:0 0 4px;}
.sw-pr-tier__count{font-size:12px;color:#164679;font-weight:600;margin:0 0 12px;}
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
                    id: "os-console".into(),
                    name: "Console".into(),
                    description: "d".into(),
                    edition: "1.0".into(),
                    platform: "linux".into(),
                    size_mb: 1,
                    path: "os-console/1.0".into(),
                    license_tier: LicenseTier::Commercial,
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
                    license_tier: LicenseTier::Commercial,
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
            ],
        }
    }

    #[test]
    fn pricing_page_shows_both_tiers_with_live_counts() {
        let html = pricing_markup(&fixture()).into_string();
        assert!(html.contains("PointSav Commercial"));
        assert!(html.contains("$1.00 USDC"));
        assert!(html.contains("2 products"));
        assert!(html.contains("FSL"));
        assert!(html.contains("$19.00 USDC"));
        assert!(html.contains("1 product<"));
    }

    #[test]
    fn pricing_page_carries_beta_tax_and_agpl_notes() {
        let html = pricing_markup(&fixture()).into_string();
        assert!(html.contains("currently free during BETA"));
        assert!(html.contains("No tax collected"));
        assert!(html.contains("github.com/pointsav/pointsav-monorepo"));
    }
}
