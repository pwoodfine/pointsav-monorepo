//! Checkout invoice page — `GET /checkout/:product_id`.
//!
//! Phase 2 (paid-flow UI rework): replaces the old flow where "Pay with Polygon
//! USDC" linked straight to a bare `GET /v1/wallet/address` JSON response. This
//! renders a real page: the product, its price, the wallet address to send USDC
//! to, and a single form where the customer pastes their transaction hash once
//! sent — submitting to `/order` (which redirects to the canonical
//! `/order/:tx_hash?product=<id>` status page). No QR code (deferred — a visual
//! nice-to-have, not core to closing the raw-JSON-hop problem this phase fixes).
//! No account creation, no extra fields — kept as simple as the operator asked.

use crate::Installer;
use maud::{html, Markup, PreEscaped};

pub fn checkout_markup(installer: &Installer, wallet_address: &str) -> Markup {
    let dollars = installer.price_usdc as f64 / 1_000_000.0;
    html! {
        (checkout_style())
        div."sw-co-wrap" {
            h1."sw-co-title" { "Checkout" }
            article."sw-co-card" {
                span."sw-co-id" { (installer.id) }
                h2."sw-co-name" { (installer.name) }
                p."sw-co-tier" { (installer.license_tier.label()) }
                div."sw-co-amount" {
                    span."sw-co-amount__val" { "$" (format!("{dollars:.2}")) }
                    span."sw-co-amount__unit" { " USDC \u{2014} Polygon PoS" }
                }
                div."sw-co-wallet" {
                    span."sw-co-wallet__label" { "Send to" }
                    code."sw-co-wallet__addr" { (wallet_address) }
                }
                p."sw-co-hint" {
                    "Send exactly this amount in native USDC on Polygon. Once your transaction \
                     is confirmed on-chain, paste its transaction hash below — this becomes \
                     your permanent, portable proof of ownership."
                }
                form."sw-co-form" method="get" action="/order" {
                    input type="hidden" name="product" value=(installer.id);
                    label."sw-co-form__label" for="tx_hash" { "Transaction hash" }
                    input."sw-co-form__input" type="text" id="tx_hash" name="tx_hash"
                        placeholder="0x..." required autocomplete="off";
                    button."sw-co-form__submit" type="submit" { "Check payment status" }
                }
            }
        }
    }
}

fn checkout_style() -> Markup {
    let css = r#".sw-co-wrap{max-width:640px;margin:0 auto;padding:40px 24px 64px;box-sizing:border-box;}
.sw-co-title{margin:0 0 24px;font-family:Georgia,"Times New Roman",serif;font-size:26px;color:#111827;}
.sw-co-card{border:1px solid #e4e7ec;border-radius:10px;padding:24px;background:#fff;box-shadow:0 1px 2px rgba(16,24,40,.04);}
.sw-co-id{font-family:ui-monospace,SFMono-Regular,Menlo,monospace;font-size:11px;color:#667085;}
.sw-co-name{font-family:Georgia,"Times New Roman",serif;font-size:21px;margin:4px 0 4px;color:#111827;}
.sw-co-tier{font-size:12px;font-weight:600;color:#164679;margin:0 0 16px;}
.sw-co-amount{margin:0 0 20px;}
.sw-co-amount__val{font-size:32px;font-weight:700;font-family:Georgia,"Times New Roman",serif;color:#111827;}
.sw-co-amount__unit{font-size:14px;color:#667085;}
.sw-co-wallet{margin:0 0 16px;padding:14px;border:1px dashed #d0d5dd;border-radius:8px;background:#fcfcfd;}
.sw-co-wallet__label{display:block;font-size:11px;letter-spacing:.08em;text-transform:uppercase;color:#667085;margin:0 0 6px;}
.sw-co-wallet__addr{font-family:ui-monospace,SFMono-Regular,Menlo,monospace;font-size:13px;color:#164679;word-break:break-all;}
.sw-co-hint{font-size:13px;line-height:1.55;color:#475467;margin:0 0 24px;}
.sw-co-form{display:flex;flex-direction:column;gap:8px;}
.sw-co-form__label{font-size:12px;font-weight:600;color:#344054;}
.sw-co-form__input{padding:10px 12px;border:1px solid #d0d5dd;border-radius:8px;font-family:ui-monospace,SFMono-Regular,Menlo,monospace;font-size:13px;}
.sw-co-form__submit{margin-top:8px;padding:12px 16px;border:0;border-radius:8px;background:#164679;color:#fff;font-size:14px;font-weight:600;cursor:pointer;}
.sw-co-form__submit:hover{background:#0e3055;}"#;
    html! { style { (PreEscaped(css)) } }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LicenseTier;

    fn fixture() -> Installer {
        Installer {
            id: "os-console".into(),
            name: "PointSav Console OS".into(),
            description: "Operator Terminal Surface.".into(),
            edition: "2026.05.144".into(),
            platform: "macOS \u{b7} Win \u{b7} Linux".into(),
            size_mb: 412,
            path: "os-console/2026.05.144".into(),
            license_tier: LicenseTier::Commercial,
            price_usdc: 1_000_000,
            fsl_conversion_date: None,
            guide_url: None,
        }
    }

    #[test]
    fn checkout_page_renders_product_price_and_wallet() {
        let html = checkout_markup(&fixture(), "0xTESTWALLET").into_string();
        assert!(html.contains("os-console"));
        assert!(html.contains("PointSav Console OS"));
        assert!(html.contains("PointSav Commercial"));
        assert!(html.contains("$1.00"));
        assert!(html.contains("0xTESTWALLET"));
        assert!(html.contains("action=\"/order\""));
        assert!(html.contains(r#"name="product" value="os-console""#));
        assert!(html.contains(r#"name="tx_hash""#));
    }
}
