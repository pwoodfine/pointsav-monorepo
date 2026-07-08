//! Order status/entitlement page — `GET /order/:tx_hash`.
//!
//! Phase 2 (paid-flow UI rework): renders the three states the payment state
//! machine already returns (`resolve_license` in `main.rs`), replacing the old
//! flow where a customer had to manually poll a bare JSON endpoint and never saw
//! their license key on any page. This page is permanent and re-visitable at any
//! time from the same tx_hash — the durable ownership record.

use maud::{html, Markup, PreEscaped};

fn order_style() -> Markup {
    let css = r#".sw-or-wrap{max-width:640px;margin:0 auto;padding:40px 24px 64px;box-sizing:border-box;}
.sw-or-title{margin:0 0 24px;font-family:Georgia,"Times New Roman",serif;font-size:26px;color:#111827;}
.sw-or-card{border:1px solid #e4e7ec;border-radius:10px;padding:24px;background:#fff;box-shadow:0 1px 2px rgba(16,24,40,.04);}
.sw-or-status{display:inline-block;font-size:11px;font-weight:700;letter-spacing:.06em;text-transform:uppercase;padding:4px 10px;border-radius:999px;margin:0 0 16px;}
.sw-or-status--pending{background:#fef3c7;color:#92400e;}
.sw-or-status--confirmed{background:#ecfdf3;color:#067647;}
.sw-or-status--notfound{background:#fee4e2;color:#912018;}
.sw-or-receipt{margin:16px 0;padding:14px;border:1px dashed #d0d5dd;border-radius:8px;background:#fcfcfd;}
.sw-or-receipt__label{display:block;font-size:11px;letter-spacing:.08em;text-transform:uppercase;color:#667085;margin:0 0 6px;}
.sw-or-receipt__key{font-family:ui-monospace,SFMono-Regular,Menlo,monospace;font-size:15px;color:#164679;word-break:break-all;}
.sw-or-receipt__note{margin:10px 0 0;font-size:12px;color:#667085;line-height:1.5;}
.sw-or-download{display:inline-block;margin-top:8px;padding:12px 20px;border-radius:8px;background:#164679;color:#fff;font-size:14px;font-weight:600;text-decoration:none;}
.sw-or-download:hover{background:#0e3055;}
.sw-or-hint{font-size:13px;line-height:1.55;color:#475467;margin:16px 0 0;}
.sw-or-back{display:inline-block;margin-top:16px;font-size:13px;color:#164679;}"#;
    html! { style { (PreEscaped(css)) } }
}

pub fn order_pending_markup(tx_hash: &str, retry_after: u64) -> Markup {
    html! {
        (order_style())
        div."sw-or-wrap" {
            h1."sw-or-title" { "Order status" }
            article."sw-or-card" {
                span."sw-or-status sw-or-status--pending" { "Pending" }
                p { "Your transaction " code { (tx_hash) } " has not yet confirmed on Polygon." }
                p."sw-or-hint" {
                    "This page checks the same payment record every visit — reload in about "
                    (retry_after) " seconds, or come back later. This page stays valid forever \
                     once your payment confirms."
                }
            }
        }
    }
}

pub fn order_confirmed_markup(tx_hash: &str, license_key: &str, product_id: &str) -> Markup {
    let download_href = format!("/order/{tx_hash}/download?product={product_id}");
    html! {
        (order_style())
        div."sw-or-wrap" {
            h1."sw-or-title" { "Order status" }
            article."sw-or-card" {
                span."sw-or-status sw-or-status--confirmed" { "Confirmed" }
                p { "Payment confirmed for " strong { (product_id) } "." }
                div."sw-or-receipt" {
                    span."sw-or-receipt__label" { "Your receipt \u{2014} proof of ownership" }
                    span."sw-or-receipt__key" { (license_key) }
                    p."sw-or-receipt__note" {
                        "A permanent, portable record you hold \u{2014} verifiable, transferable, \
                         ours to honor but never to revoke. Keep it for your records; you do not \
                         need it to download."
                    }
                }
                a."sw-or-download" href=(download_href) { "Download" }
                p."sw-or-hint" {
                    "This page is permanent \u{2014} bookmark it. Revisiting later and clicking \
                     Download again always gets you a fresh, working download link."
                }
            }
        }
    }
}

pub fn order_not_found_markup(tx_hash: &str, product_id: Option<&str>) -> Markup {
    let back_href = product_id
        .map(|p| format!("/checkout/{p}"))
        .unwrap_or_else(|| "/software".to_string());
    html! {
        (order_style())
        div."sw-or-wrap" {
            h1."sw-or-title" { "Order status" }
            article."sw-or-card" {
                span."sw-or-status sw-or-status--notfound" { "Not found" }
                p { "We could not find a recognised USDC payment for " code { (tx_hash) } "." }
                p."sw-or-hint" {
                    "Double-check the transaction hash you pasted, or confirm the payment has \
                     been sent to the correct address."
                }
                a."sw-or-back" href=(back_href) { "\u{2190} Back to checkout" }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_state_shows_retry_hint() {
        let html = order_pending_markup("0xabc", 30).into_string();
        assert!(html.contains("Pending"));
        assert!(html.contains("0xabc"));
        assert!(html.contains("30"));
    }

    #[test]
    fn confirmed_state_shows_receipt_and_download_link() {
        let html =
            order_confirmed_markup("0xabc", "aaaa-bbbb-cccc-dddd", "os-console").into_string();
        assert!(html.contains("Confirmed"));
        assert!(html.contains("aaaa-bbbb-cccc-dddd"));
        assert!(html.contains("os-console"));
        assert!(html.contains("href=\"/order/0xabc/download?product=os-console\""));
        assert!(html.contains("proof of ownership"));
    }

    #[test]
    fn not_found_state_links_back_to_checkout_when_product_known() {
        let html = order_not_found_markup("0xabc", Some("os-console")).into_string();
        assert!(html.contains("Not found"));
        assert!(html.contains("href=\"/checkout/os-console\""));
    }

    #[test]
    fn not_found_state_falls_back_to_software_page_without_product() {
        let html = order_not_found_markup("0xabc", None).into_string();
        assert!(html.contains("href=\"/software\""));
    }
}
