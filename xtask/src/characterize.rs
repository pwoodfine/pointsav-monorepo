//! `xtask characterize` — characterization-test harness for the
//! privategit NG rewrite (app-privategit-source-2 / app-privategit-marketplace-2).
//!
//! Three modes:
//!
//! ```text
//! cargo run -p xtask -- characterize snapshot --out <dir> --source-port N --marketplace-port N
//! cargo run -p xtask -- characterize diff <old-dir> <new-dir>
//! cargo run -p xtask -- characterize print-keys
//! ```
//!
//! `snapshot` replays a FIXED fixture set against running binaries and writes
//! one JSON snapshot file per fixture. Ports are REQUIRED (parity-phase safety
//! rule: the live services on 9201/9202 must never be touched — run the
//! binaries under test on scratch ports in the 19200–19299 range).
//!
//! `diff` compares two snapshot directories (convention: `<old-dir>` from the
//! OLD crates, `<new-dir>` from the `-2` rewrites) and classifies every
//! difference against the fixture table's expectation column:
//!   - `Expect::Same`        — any difference is UNEXPLAINED (exit 1)
//!   - `Expect::RootRedirect`— 303 (old, axum `Redirect::to`) vs 302 (new, P1
//!     contract) with an identical Location header; verified, then EXPECTED
//!   - `Expect::PricingFix`  — the Checkpoint 2 approved `/v1/license` pricing-
//!     unit fix; the exact old/new `product_id` pair is asserted and both
//!     sides' `license_key` derivations are recomputed; verified, then EXPECTED
//!   - `Expect::ChromeDiff`  — P2/P3 presentation rewrites; the DATA view is
//!     compared instead of raw bytes (see `Normalize`), and the new side is
//!     additionally required to be complete (all catalog entries rendered /
//!     all old long-form text preserved)
//!
//! `print-keys` emits the deterministic test Ed25519 verify key (hex) and the
//! revocation fingerprint of the `{TOKEN_REVOKED}` fixture token, so the
//! launcher script can write `VERIFY_KEY_PUB` / `REVOCATION_LIST_PATH` files
//! that both OLD and NEW `app-privategit-source*` binaries share. The keypair
//! is derived from a FIXED seed: snapshots taken in separate invocations use
//! byte-identical tokens, keeping the old/new runs comparable.
//!
//! Determinism: the volatile `date` and `server` response headers are always
//! excluded; JSON bodies are parsed and re-serialized (BTreeMap key order);
//! large/binary bodies are stored as sha256 + length. Per-fixture `Normalize`
//! modes additionally mask genuinely volatile JSON fields (`confirmed_at`,
//! `claimed_at`, claim `token`) and reduce chrome-wrapped HTML to a data view
//! (`content-length` is dropped whenever a body is normalized).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use ed25519_dalek::{Signer, SigningKey};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

/// Which service a fixture targets. Resolved to a port at run time.
#[derive(Clone, Copy, PartialEq)]
enum Service {
    Source,      // app-privategit-source[-2]
    Marketplace, // app-privategit-marketplace[-2]
}

#[derive(Clone, Copy)]
enum Method {
    Get,
    Post,
}

impl Method {
    fn as_str(self) -> &'static str {
        match self {
            Method::Get => "GET",
            Method::Post => "POST",
        }
    }
}

/// How the response body is reduced before snapshotting.
#[derive(Clone, Copy)]
enum Normalize {
    /// Verbatim (JSON canonicalized, large/binary bodies hashed).
    Raw,
    /// Parse JSON body and mask the listed top-level fields (volatile values).
    MaskJson(&'static [&'static str]),
    /// Chrome-wrapped catalog page: reduce the HTML to a presence map of the
    /// catalog entries served by `/v1/products` on the SAME port (ids, names,
    /// paid-tier price displays). Presentation bytes are deliberately ignored.
    CatalogData,
    /// Chrome-wrapped static document: reduce the HTML to the sorted set of
    /// text runs >= 80 chars (tag-stripped, whitespace-collapsed, excluding
    /// style/script and the header/footer chrome elements on both sides).
    /// Captures the document's substance, not its chrome.
    LongTextRuns,
}

impl Normalize {
    fn label(self) -> &'static str {
        match self {
            Normalize::Raw => "raw",
            Normalize::MaskJson(_) => "mask-json",
            Normalize::CatalogData => "catalog-data",
            Normalize::LongTextRuns => "long-text-runs",
        }
    }
}

/// What the old-vs-new diff is expected to show for a fixture.
#[derive(Clone, Copy)]
enum Expect {
    /// Byte parity (after normalization). Any diff is an unexplained gap.
    Same,
    /// `GET /` — old crate uses axum `Redirect::to` (303 See Other); the new
    /// crate deliberately returns 302 Found per the P1 contract (documented in
    /// `app-privategit-marketplace-2/src/main.rs::root`). Location must match.
    RootRedirect,
    /// The Checkpoint 2 approved `/v1/license` pricing-unit fix. The OLD crate
    /// re-multiplied catalog micro-USDC by 1e6 (never matching a real payment)
    /// and/or lost precision via the f64 `amount_usdc` round-trip; the NEW
    /// crate compares exact micro-units. Assert the precise product_id pair
    /// and recompute both sides' license_key derivations.
    PricingFix {
        old_product: &'static str,
        new_product: &'static str,
    },
    /// P2/P3 visual rewrite: presentation may differ; the data view is what is
    /// compared, and the NEW side must be complete.
    ChromeDiff(&'static str),
}

struct Fixture {
    name: &'static str,
    service: Service,
    method: Method,
    /// Request path; may contain `{TOKEN_*}` placeholders (see `test_tokens`).
    path: &'static str,
    /// Extra request headers; values may contain `{TOKEN_*}` placeholders.
    headers: &'static [(&'static str, &'static str)],
    /// Optional request body template (JSON; placeholders expanded).
    body: Option<&'static str>,
    normalize: Normalize,
    expect: Expect,
}

const NO_HDRS: &[(&str, &str)] = &[];
const JSON_HDRS: &[(&str, &str)] = &[("Content-Type", "application/json")];
const VOLATILE_LICENSE: &[&str] = &["confirmed_at"];
const VOLATILE_CLAIM: &[&str] = &["token", "claimed_at"];

/// The complete, fixed fixture set for full-route parity verification.
///
/// Source fixtures assume the shared scratch RELEASES_DIR layout built by the
/// launcher: `os-test-open` (requires_license: false; versions 1.2.0 and
/// 1.10.0 — also probes numeric version ordering) and `os-test-licensed`
/// (requires_license: true; version 0.3.1), each with `install.sh`, per-version
/// `MANIFEST.json`, an `x86_64-linux` artifact and a detached `.sig`.
///
/// Marketplace fixtures assume both binaries share the repo catalog
/// (`app-privategit-marketplace/catalog/products.yaml`), the same static dir,
/// the same wallet address, per-instance scratch receipts/claims dirs seeded
/// with an identical receipt for `0xseededreceipttx`, and the `tool-wallet`
/// test double (OLD: first on PATH; NEW: `TOOL_WALLET_BIN`) that answers
/// `check <tx>` by substring: confirmed1usd → $1.00 / 1_000_000 units,
/// confirmed19usd → $19.00, confirmed0usd → $0.00, confirmed201c → $2.01 /
/// 2_010_000 units, pending → unconfirmed, anything else → exit 1.
const FIXTURES: &[Fixture] = &[
    // ── app-privategit-source[-2] ── P1 routes + P5 admin/licensing surfaces ──
    Fixture {
        name: "src-healthz",
        service: Service::Source,
        method: Method::Get,
        path: "/healthz",
        headers: NO_HDRS,
        body: None,
        normalize: Normalize::Raw,
        expect: Expect::Same,
    },
    Fixture {
        name: "src-releases-index",
        service: Service::Source,
        method: Method::Get,
        path: "/releases/",
        headers: NO_HDRS,
        body: None,
        normalize: Normalize::Raw,
        expect: Expect::Same,
    },
    Fixture {
        name: "src-product-index",
        service: Service::Source,
        method: Method::Get,
        path: "/releases/os-test-open/",
        headers: NO_HDRS,
        body: None,
        normalize: Normalize::Raw,
        expect: Expect::Same,
    },
    Fixture {
        name: "src-product-missing",
        service: Service::Source,
        method: Method::Get,
        path: "/releases/nonexistent-product-xyz/",
        headers: NO_HDRS,
        body: None,
        normalize: Normalize::Raw,
        expect: Expect::Same,
    },
    Fixture {
        name: "src-manifest",
        service: Service::Source,
        method: Method::Get,
        path: "/releases/os-test-open/1.2.0/MANIFEST",
        headers: NO_HDRS,
        body: None,
        normalize: Normalize::Raw,
        expect: Expect::Same,
    },
    Fixture {
        name: "src-manifest-missing",
        service: Service::Source,
        method: Method::Get,
        path: "/releases/os-test-open/9.9.9/MANIFEST",
        headers: NO_HDRS,
        body: None,
        normalize: Normalize::Raw,
        expect: Expect::Same,
    },
    Fixture {
        name: "src-latest-redirect",
        service: Service::Source,
        method: Method::Get,
        path: "/releases/os-test-open/latest/x86_64-linux",
        headers: NO_HDRS,
        body: None,
        normalize: Normalize::Raw,
        expect: Expect::Same,
    },
    Fixture {
        name: "src-latest-redirect-token",
        service: Service::Source,
        method: Method::Get,
        path: "/releases/os-test-licensed/latest/x86_64-linux?token={TOKEN_VALID}",
        headers: NO_HDRS,
        body: None,
        normalize: Normalize::Raw,
        expect: Expect::Same,
    },
    Fixture {
        name: "src-latest-missing-platform",
        service: Service::Source,
        method: Method::Get,
        path: "/releases/os-test-open/latest/armv7-none",
        headers: NO_HDRS,
        body: None,
        normalize: Normalize::Raw,
        expect: Expect::Same,
    },
    Fixture {
        name: "src-binary-open",
        service: Service::Source,
        method: Method::Get,
        path: "/releases/os-test-open/1.2.0/x86_64-linux",
        headers: NO_HDRS,
        body: None,
        normalize: Normalize::Raw,
        expect: Expect::Same,
    },
    Fixture {
        name: "src-binary-sig",
        service: Service::Source,
        method: Method::Get,
        path: "/releases/os-test-licensed/0.3.1/x86_64-linux.sig",
        headers: NO_HDRS,
        body: None,
        normalize: Normalize::Raw,
        expect: Expect::Same,
    },
    Fixture {
        name: "src-binary-licensed-noauth",
        service: Service::Source,
        method: Method::Get,
        path: "/releases/os-test-licensed/0.3.1/x86_64-linux",
        headers: NO_HDRS,
        body: None,
        normalize: Normalize::Raw,
        expect: Expect::Same,
    },
    Fixture {
        name: "src-binary-licensed-bearer",
        service: Service::Source,
        method: Method::Get,
        path: "/releases/os-test-licensed/0.3.1/x86_64-linux",
        headers: &[("Authorization", "Bearer {TOKEN_VALID}")],
        body: None,
        normalize: Normalize::Raw,
        expect: Expect::Same,
    },
    Fixture {
        name: "src-binary-licensed-query",
        service: Service::Source,
        method: Method::Get,
        path: "/releases/os-test-licensed/0.3.1/x86_64-linux?token={TOKEN_VALID}",
        headers: NO_HDRS,
        body: None,
        normalize: Normalize::Raw,
        expect: Expect::Same,
    },
    Fixture {
        name: "src-binary-licensed-badsig",
        service: Service::Source,
        method: Method::Get,
        path: "/releases/os-test-licensed/0.3.1/x86_64-linux",
        headers: &[("Authorization", "Bearer {TOKEN_BADSIG}")],
        body: None,
        normalize: Normalize::Raw,
        expect: Expect::Same,
    },
    Fixture {
        name: "src-binary-licensed-expired",
        service: Service::Source,
        method: Method::Get,
        path: "/releases/os-test-licensed/0.3.1/x86_64-linux",
        headers: &[("Authorization", "Bearer {TOKEN_EXPIRED}")],
        body: None,
        normalize: Normalize::Raw,
        expect: Expect::Same,
    },
    Fixture {
        name: "src-binary-licensed-revoked",
        service: Service::Source,
        method: Method::Get,
        path: "/releases/os-test-licensed/0.3.1/x86_64-linux",
        headers: &[("Authorization", "Bearer {TOKEN_REVOKED}")],
        body: None,
        normalize: Normalize::Raw,
        expect: Expect::Same,
    },
    Fixture {
        name: "src-binary-licensed-wrong-product",
        service: Service::Source,
        method: Method::Get,
        path: "/releases/os-test-licensed/0.3.1/x86_64-linux",
        headers: &[("Authorization", "Bearer {TOKEN_WRONG_PRODUCT}")],
        body: None,
        normalize: Normalize::Raw,
        expect: Expect::Same,
    },
    Fixture {
        name: "src-binary-licensed-missing-file",
        service: Service::Source,
        method: Method::Get,
        path: "/releases/os-test-licensed/0.3.1/armv7-none",
        headers: &[("Authorization", "Bearer {TOKEN_VALID}")],
        body: None,
        normalize: Normalize::Raw,
        expect: Expect::Same,
    },
    Fixture {
        name: "src-git-stub-get",
        service: Service::Source,
        method: Method::Get,
        path: "/git/info/refs",
        headers: NO_HDRS,
        body: None,
        normalize: Normalize::Raw,
        expect: Expect::Same,
    },
    Fixture {
        name: "src-git-stub-post",
        service: Service::Source,
        method: Method::Post,
        path: "/git/git-upload-pack",
        headers: NO_HDRS,
        body: None,
        normalize: Normalize::Raw,
        expect: Expect::Same,
    },
    Fixture {
        name: "src-verify-key-valid",
        service: Service::Source,
        method: Method::Post,
        path: "/verify-key",
        headers: JSON_HDRS,
        body: Some(r#"{"license_key_b64":"{TOKEN_VALID}","product_id":"os-test-licensed"}"#),
        normalize: Normalize::Raw,
        expect: Expect::Same,
    },
    Fixture {
        name: "src-verify-key-malformed",
        service: Service::Source,
        method: Method::Post,
        path: "/verify-key",
        headers: JSON_HDRS,
        body: Some(r#"{"license_key_b64":"!!!not-base64!!!","product_id":"os-test-licensed"}"#),
        normalize: Normalize::Raw,
        expect: Expect::Same,
    },
    Fixture {
        name: "src-verify-key-short",
        service: Service::Source,
        method: Method::Post,
        path: "/verify-key",
        headers: JSON_HDRS,
        body: Some(r#"{"license_key_b64":"{TOKEN_SHORT}","product_id":"os-test-licensed"}"#),
        normalize: Normalize::Raw,
        expect: Expect::Same,
    },
    Fixture {
        name: "src-verify-key-badsig",
        service: Service::Source,
        method: Method::Post,
        path: "/verify-key",
        headers: JSON_HDRS,
        body: Some(r#"{"license_key_b64":"{TOKEN_BADSIG}","product_id":"os-test-licensed"}"#),
        normalize: Normalize::Raw,
        expect: Expect::Same,
    },
    Fixture {
        name: "src-verify-key-badpayload",
        service: Service::Source,
        method: Method::Post,
        path: "/verify-key",
        headers: JSON_HDRS,
        body: Some(r#"{"license_key_b64":"{TOKEN_BADPAYLOAD}","product_id":"os-test-licensed"}"#),
        normalize: Normalize::Raw,
        expect: Expect::Same,
    },
    Fixture {
        name: "src-verify-key-wrong-product",
        service: Service::Source,
        method: Method::Post,
        path: "/verify-key",
        headers: JSON_HDRS,
        body: Some(r#"{"license_key_b64":"{TOKEN_VALID}","product_id":"os-test-open"}"#),
        normalize: Normalize::Raw,
        expect: Expect::Same,
    },
    Fixture {
        name: "src-verify-key-expired",
        service: Service::Source,
        method: Method::Post,
        path: "/verify-key",
        headers: JSON_HDRS,
        body: Some(r#"{"license_key_b64":"{TOKEN_EXPIRED}","product_id":"os-test-licensed"}"#),
        normalize: Normalize::Raw,
        expect: Expect::Same,
    },
    Fixture {
        name: "src-verify-key-revoked",
        service: Service::Source,
        method: Method::Post,
        path: "/verify-key",
        headers: JSON_HDRS,
        body: Some(r#"{"license_key_b64":"{TOKEN_REVOKED}","product_id":"os-test-licensed"}"#),
        normalize: Normalize::Raw,
        expect: Expect::Same,
    },
    Fixture {
        name: "src-admin-reload-revocation",
        service: Service::Source,
        method: Method::Post,
        path: "/admin/reload-revocation-list",
        headers: NO_HDRS,
        body: None,
        normalize: Normalize::Raw,
        expect: Expect::Same,
    },
    Fixture {
        name: "src-verify-key-pub",
        service: Service::Source,
        method: Method::Get,
        path: "/verify-key.pub",
        headers: NO_HDRS,
        body: None,
        normalize: Normalize::Raw,
        expect: Expect::Same,
    },
    Fixture {
        name: "src-install-script",
        service: Service::Source,
        method: Method::Get,
        path: "/releases/os-test-open/install.sh",
        headers: NO_HDRS,
        body: None,
        normalize: Normalize::Raw,
        expect: Expect::Same,
    },
    Fixture {
        name: "src-install-script-missing",
        service: Service::Source,
        method: Method::Get,
        path: "/releases/nonexistent-product-xyz/install.sh",
        headers: NO_HDRS,
        body: None,
        normalize: Normalize::Raw,
        expect: Expect::Same,
    },
    // ── app-privategit-marketplace[-2] ── P1–P4 storefront + payment routes ──
    Fixture {
        name: "mkt-healthz",
        service: Service::Marketplace,
        method: Method::Get,
        path: "/healthz",
        headers: NO_HDRS,
        body: None,
        normalize: Normalize::Raw,
        expect: Expect::Same,
    },
    Fixture {
        name: "mkt-root-redirect",
        service: Service::Marketplace,
        method: Method::Get,
        path: "/",
        headers: NO_HDRS,
        body: None,
        normalize: Normalize::Raw,
        expect: Expect::RootRedirect,
    },
    Fixture {
        name: "mkt-software-page",
        service: Service::Marketplace,
        method: Method::Get,
        path: "/software",
        headers: NO_HDRS,
        body: None,
        normalize: Normalize::CatalogData,
        expect: Expect::ChromeDiff(
            "P2/P3: OLD serves stale compile-time static HTML (pre-chrome, frozen build, \
             already flagged to Command); NEW renders dynamically from products.yaml with \
             Sovereign Editorial chrome. Data view is compared; NEW must be complete.",
        ),
    },
    Fixture {
        name: "mkt-licensing-page",
        service: Service::Marketplace,
        method: Method::Get,
        path: "/licensing",
        headers: NO_HDRS,
        body: None,
        normalize: Normalize::LongTextRuns,
        expect: Expect::ChromeDiff(
            "P2: OLD serves the static licensing.html verbatim (old light chrome); NEW wraps \
             the same file in Sovereign Editorial chrome. Long-form text runs are compared; \
             every OLD run must survive in NEW.",
        ),
    },
    Fixture {
        name: "mkt-products-v1",
        service: Service::Marketplace,
        method: Method::Get,
        path: "/v1/products",
        headers: NO_HDRS,
        body: None,
        normalize: Normalize::Raw,
        expect: Expect::Same,
    },
    Fixture {
        name: "mkt-static-file",
        service: Service::Marketplace,
        method: Method::Get,
        path: "/static/licensing.html",
        headers: NO_HDRS,
        body: None,
        normalize: Normalize::Raw,
        expect: Expect::Same,
    },
    Fixture {
        name: "mkt-wallet-address",
        service: Service::Marketplace,
        method: Method::Get,
        path: "/v1/wallet/address",
        headers: NO_HDRS,
        body: None,
        normalize: Normalize::Raw,
        expect: Expect::Same,
    },
    Fixture {
        name: "mkt-license-confirmed-1usd",
        service: Service::Marketplace,
        method: Method::Get,
        path: "/v1/license/0xtestconfirmed1usd",
        headers: NO_HDRS,
        body: None,
        normalize: Normalize::MaskJson(VOLATILE_LICENSE),
        // Phase 1 catalog rebuild (BRIEF-software-hyperscaler-audit.md Licensing
        // Corrections): `apache`/`fsl` no longer exist as catalog ids — every os-*
        // product now carries its own `license_tier`/`price_usdc` fields directly,
        // and ALL of them ship at price_usdc: 0 during the active BETA gate (see
        // `Installer::price_usdc` doc comment in main.rs). A $1.00 test payment no
        // longer matches any installer on the NEW crate either — this is a second,
        // deliberate, approved divergence from the OLD crate's still-live
        // apache/fsl pricing, not a regression.
        expect: Expect::PricingFix {
            old_product: "unknown-1000000",
            new_product: "unknown-1000000",
        },
    },
    Fixture {
        name: "mkt-license-confirmed-19usd",
        service: Service::Marketplace,
        method: Method::Get,
        path: "/v1/license/0xtestconfirmed19usd",
        headers: NO_HDRS,
        body: None,
        normalize: Normalize::MaskJson(VOLATILE_LICENSE),
        // See the 1usd fixture's comment above — same rationale for the $19 tier.
        expect: Expect::PricingFix {
            old_product: "unknown-19000000",
            new_product: "unknown-19000000",
        },
    },
    Fixture {
        name: "mkt-license-confirmed-0usd",
        service: Service::Marketplace,
        method: Method::Get,
        path: "/v1/license/0xtestconfirmed0usd",
        headers: NO_HDRS,
        body: None,
        normalize: Normalize::MaskJson(VOLATILE_LICENSE),
        // $0.00 is the one price point where the OLD formula (0 * 1e6 == 0)
        // and the NEW formula (0 == 0) agree — byte parity expected.
        expect: Expect::Same,
    },
    Fixture {
        name: "mkt-license-confirmed-201cents",
        service: Service::Marketplace,
        method: Method::Get,
        path: "/v1/license/0xtestconfirmed201c",
        headers: NO_HDRS,
        body: None,
        normalize: Normalize::MaskJson(VOLATILE_LICENSE),
        // Checkpoint 2 latent float-precision finding: OLD trunc((2.01*1e6)) =
        // 2_009_999; NEW uses tool-wallet's exact amount_units = 2_010_000.
        // Neither matches a catalog price, so both fall through to unknown-N —
        // with different N. Approved fix, not a regression.
        expect: Expect::PricingFix {
            old_product: "unknown-2009999",
            new_product: "unknown-2010000",
        },
    },
    Fixture {
        name: "mkt-license-pending",
        service: Service::Marketplace,
        method: Method::Get,
        path: "/v1/license/0xtestpendingtx",
        headers: NO_HDRS,
        body: None,
        normalize: Normalize::Raw,
        expect: Expect::Same,
    },
    Fixture {
        name: "mkt-license-notfound",
        service: Service::Marketplace,
        method: Method::Get,
        path: "/v1/license/0xtestunknowntx",
        headers: NO_HDRS,
        body: None,
        normalize: Normalize::Raw,
        expect: Expect::Same,
    },
    Fixture {
        name: "mkt-license-receipt-cache",
        service: Service::Marketplace,
        method: Method::Get,
        path: "/v1/license/0xseededreceipttx",
        headers: NO_HDRS,
        body: None,
        // Replays the pre-seeded receipt verbatim (fixed confirmed_at) — no
        // masking, proving cross-binary receipt-file compatibility.
        normalize: Normalize::Raw,
        expect: Expect::Same,
    },
    Fixture {
        name: "mkt-claim",
        service: Service::Marketplace,
        method: Method::Post,
        path: "/v1/claim",
        headers: JSON_HDRS,
        body: Some(
            r#"{"binary_sha256":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef","wallet_address":"0xparitybuyer"}"#,
        ),
        normalize: Normalize::MaskJson(VOLATILE_CLAIM),
        expect: Expect::Same,
    },
];

/// Response headers excluded from snapshots (vary run to run).
const EXCLUDED_HEADERS: &[&str] = &["date", "server"];

/// Bodies larger than this that are not JSON are stored as sha256 + length.
const MAX_INLINE_BODY: usize = 65_536;

/// Minimum length of a text run kept by `Normalize::LongTextRuns`.
const LONG_RUN_MIN: usize = 80;

const MASKED: &str = "<masked>";

pub fn run(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("snapshot") => cmd_snapshot(&args[1..]),
        Some("diff") => cmd_diff(&args[1..]),
        Some("print-keys") => cmd_print_keys(),
        _ => {
            eprintln!(
                "usage: xtask characterize snapshot --out <dir> --source-port N --marketplace-port N"
            );
            eprintln!("       xtask characterize diff <old-dir> <new-dir>");
            eprintln!("       xtask characterize print-keys");
            Err("characterize: expected mode 'snapshot', 'diff', or 'print-keys'".to_string())
        }
    }
}

// ---------------------------------------------------------------------------
// Deterministic test tokens (fixed seed — identical across invocations)
// ---------------------------------------------------------------------------

/// Fixed, PUBLIC test seed. Not a secret and never a production key: it exists
/// so two separate `snapshot` invocations (old binaries, new binaries) sign
/// byte-identical license tokens and stay diffable.
const TEST_SEED: [u8; 32] = [0x5a; 32];

fn test_signing_key() -> SigningKey {
    SigningKey::from_bytes(&TEST_SEED)
}

fn sign_token(sk: &SigningKey, payload: &[u8]) -> String {
    let sig = sk.sign(payload);
    let mut token = Vec::with_capacity(64 + payload.len());
    token.extend_from_slice(&sig.to_bytes());
    token.extend_from_slice(payload);
    URL_SAFE_NO_PAD.encode(token)
}

/// `(placeholder, token)` pairs used to expand `{TOKEN_*}` in fixtures.
fn test_tokens() -> Vec<(&'static str, String)> {
    let sk = test_signing_key();
    let valid = sign_token(
        &sk,
        br#"{"product":"os-test-licensed","channel_expiry":"2099-12-31","entitlements":["download","source"],"version_floor":"0.1.0"}"#,
    );
    let expired = sign_token(
        &sk,
        br#"{"product":"os-test-licensed","channel_expiry":"2020-01-01","entitlements":["download"],"version_floor":null}"#,
    );
    let wrong_product = sign_token(
        &sk,
        br#"{"product":"os-test-other","channel_expiry":"2099-12-31","entitlements":["download"],"version_floor":null}"#,
    );
    let revoked = sign_token(
        &sk,
        br#"{"product":"os-test-licensed","channel_expiry":"2099-12-31","entitlements":["download","revocation-probe"],"version_floor":null}"#,
    );
    let bad_payload = sign_token(&sk, b"this is not a json license payload");

    // Corrupt one signature byte of an otherwise-valid token.
    let mut bad_sig_bytes = URL_SAFE_NO_PAD.decode(&valid).expect("own token decodes");
    bad_sig_bytes[3] ^= 0xff;
    let bad_sig = URL_SAFE_NO_PAD.encode(bad_sig_bytes);

    // <= 64 decoded bytes -> token-too-short.
    let short = URL_SAFE_NO_PAD.encode([0u8; 48]);

    vec![
        ("TOKEN_VALID", valid),
        ("TOKEN_EXPIRED", expired),
        ("TOKEN_WRONG_PRODUCT", wrong_product),
        ("TOKEN_REVOKED", revoked),
        ("TOKEN_BADPAYLOAD", bad_payload),
        ("TOKEN_BADSIG", bad_sig),
        ("TOKEN_SHORT", short),
    ]
}

fn expand(template: &str, tokens: &[(&'static str, String)]) -> String {
    let mut out = template.to_string();
    for (name, value) in tokens {
        out = out.replace(&format!("{{{name}}}"), value);
    }
    out
}

/// SHA256 hex of the raw base64url token string — matches the source crates'
/// `token_fingerprint` and the tool-wallet `fingerprint` subcommand.
fn token_fingerprint(raw_b64: &str) -> String {
    hex::encode(Sha256::digest(raw_b64.as_bytes()))
}

fn cmd_print_keys() -> Result<(), String> {
    let sk = test_signing_key();
    let vk_hex = hex::encode(sk.verifying_key().to_bytes());
    let revoked = test_tokens()
        .into_iter()
        .find(|(n, _)| *n == "TOKEN_REVOKED")
        .map(|(_, t)| t)
        .expect("TOKEN_REVOKED present");
    let out = json!({
        "verify_key_pub_hex": vk_hex,
        "revoked_token_fingerprint": token_fingerprint(&revoked),
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&out).map_err(|e| e.to_string())?
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// snapshot mode
// ---------------------------------------------------------------------------

fn cmd_snapshot(args: &[String]) -> Result<(), String> {
    let mut out_dir: Option<PathBuf> = None;
    let mut source_port: Option<u16> = None;
    let mut marketplace_port: Option<u16> = None;

    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--out" => {
                out_dir = Some(PathBuf::from(
                    it.next().ok_or("--out requires a directory argument")?,
                ));
            }
            "--source-port" => {
                source_port = Some(parse_port(it.next(), "--source-port")?);
            }
            "--marketplace-port" => {
                marketplace_port = Some(parse_port(it.next(), "--marketplace-port")?);
            }
            other => return Err(format!("snapshot: unknown argument '{other}'")),
        }
    }

    // Ports are REQUIRED — parity runs must never default onto the live
    // 9201/9202 services.
    let out_dir = out_dir.ok_or("snapshot: --out <dir> is required")?;
    let source_port = source_port.ok_or("snapshot: --source-port is required")?;
    let marketplace_port = marketplace_port.ok_or("snapshot: --marketplace-port is required")?;
    for p in [source_port, marketplace_port] {
        if p == 9201 || p == 9202 {
            return Err(format!(
                "snapshot: port {p} is a LIVE production service — use a scratch port"
            ));
        }
    }

    fs::create_dir_all(&out_dir)
        .map_err(|e| format!("cannot create {}: {e}", out_dir.display()))?;

    let tokens = test_tokens();
    let mut products_cache: HashMap<u16, Value> = HashMap::new();

    println!("[*] characterize snapshot -> {}", out_dir.display());
    println!(
        "    source port {source_port}, marketplace port {marketplace_port}, {} fixtures",
        FIXTURES.len()
    );

    let mut failures = 0usize;
    for fixture in FIXTURES {
        let port = match fixture.service {
            Service::Source => source_port,
            Service::Marketplace => marketplace_port,
        };
        let path = expand(fixture.path, &tokens);
        let headers: Vec<(String, String)> = fixture
            .headers
            .iter()
            .map(|(k, v)| (k.to_string(), expand(v, &tokens)))
            .collect();
        let body = fixture.body.map(|b| expand(b, &tokens));

        match http_request(fixture.method, port, &path, &headers, body.as_deref()) {
            Ok(resp) => match build_snapshot(fixture, port, &path, &resp, &mut products_cache) {
                Ok(snapshot) => {
                    let file = out_dir.join(format!("{}.json", fixture.name));
                    let mut text = serde_json::to_string_pretty(&snapshot)
                        .map_err(|e| format!("serialize {}: {e}", fixture.name))?;
                    text.push('\n');
                    fs::write(&file, text).map_err(|e| format!("write {}: {e}", file.display()))?;
                    println!(
                        "    [+] {}: HTTP {} ({} body bytes, {})",
                        fixture.name,
                        resp.status,
                        resp.body.len(),
                        fixture.normalize.label()
                    );
                }
                Err(e) => {
                    eprintln!("    [-] {}: normalize failed: {e}", fixture.name);
                    failures += 1;
                }
            },
            Err(e) => {
                eprintln!(
                    "    [-] {}: {} :{port}{path} failed: {e}",
                    fixture.name,
                    fixture.method.as_str()
                );
                failures += 1;
            }
        }
    }

    if failures > 0 {
        return Err(format!("snapshot: {failures} fixture(s) failed"));
    }
    println!("[+] snapshot complete: {} fixtures written", FIXTURES.len());
    Ok(())
}

fn parse_port(v: Option<&String>, flag: &str) -> Result<u16, String> {
    v.ok_or(format!("{flag} requires a port argument"))?
        .parse::<u16>()
        .map_err(|e| format!("{flag}: invalid port: {e}"))
}

struct HttpResponse {
    status: u16,
    /// Header name (lowercased) -> values in wire order.
    headers: BTreeMap<String, Vec<String>>,
    body: Vec<u8>,
}

fn build_snapshot(
    fixture: &Fixture,
    port: u16,
    path: &str,
    resp: &HttpResponse,
    products_cache: &mut HashMap<u16, Value>,
) -> Result<Value, String> {
    let drop_content_length = !matches!(fixture.normalize, Normalize::Raw);
    let mut headers = Map::new();
    for (k, vals) in &resp.headers {
        if EXCLUDED_HEADERS.contains(&k.as_str()) {
            continue;
        }
        if drop_content_length && (k == "content-length" || k == "transfer-encoding") {
            continue;
        }
        if vals.len() == 1 {
            headers.insert(k.clone(), Value::String(vals[0].clone()));
        } else {
            headers.insert(
                k.clone(),
                Value::Array(vals.iter().cloned().map(Value::String).collect()),
            );
        }
    }

    let content_type = resp
        .headers
        .get("content-type")
        .and_then(|v| v.first())
        .cloned()
        .unwrap_or_default();

    let body = match fixture.normalize {
        Normalize::Raw => classify_body(&resp.body, &content_type),
        Normalize::MaskJson(fields) => mask_json_body(&resp.body, fields)?,
        Normalize::CatalogData => {
            let products = fetch_products(port, products_cache)?;
            catalog_data_body(&resp.body, &products)
        }
        Normalize::LongTextRuns => long_text_runs_body(&resp.body),
    };

    Ok(json!({
        "fixture": fixture.name,
        "request": {
            "method": fixture.method.as_str(),
            "port": port,
            "path": path,
        },
        "normalize": fixture.normalize.label(),
        "status": resp.status,
        "headers": Value::Object(headers),
        "body": body,
    }))
}

fn classify_body(body: &[u8], content_type: &str) -> Value {
    if body.is_empty() {
        return json!({ "kind": "empty" });
    }
    if content_type.contains("json") {
        if let Ok(v) = serde_json::from_slice::<Value>(body) {
            return json!({ "kind": "json", "json": v });
        }
    }
    if body.len() <= MAX_INLINE_BODY {
        if let Ok(text) = std::str::from_utf8(body) {
            return json!({ "kind": "text", "text": text });
        }
    }
    json!({
        "kind": "bytes",
        "sha256": hex::encode(Sha256::digest(body)),
        "length": body.len(),
    })
}

fn mask_json_body(body: &[u8], fields: &[&str]) -> Result<Value, String> {
    let mut v: Value =
        serde_json::from_slice(body).map_err(|e| format!("mask-json: body is not JSON ({e})"))?;
    if let Some(obj) = v.as_object_mut() {
        for f in fields {
            if obj.contains_key(*f) {
                obj.insert(f.to_string(), Value::String(MASKED.to_string()));
            }
        }
    }
    Ok(json!({ "kind": "json", "json": v }))
}

/// Fetch `/v1/products` from the same marketplace instance (cached per port).
fn fetch_products(port: u16, cache: &mut HashMap<u16, Value>) -> Result<Value, String> {
    if let Some(v) = cache.get(&port) {
        return Ok(v.clone());
    }
    let resp = http_request(Method::Get, port, "/v1/products", &[], None)?;
    if resp.status != 200 {
        return Err(format!(
            "catalog-data: /v1/products returned {}",
            resp.status
        ));
    }
    let v: Value = serde_json::from_slice(&resp.body)
        .map_err(|e| format!("catalog-data: /v1/products unparseable: {e}"))?;
    cache.insert(port, v.clone());
    Ok(v)
}

/// Reduce a rendered catalog page to a presence map of the entries the SAME
/// service reports via `/v1/products`. This is the "diff the data, not the
/// bytes" mode for the P3 dynamic `/software` page.
///
/// Rebuilt for the Phase 1 catalog rebuild (BRIEF-software-hyperscaler-audit.md
/// Licensing Corrections): `v1_products` now emits a single unified `installers`
/// array (no more separate `licenses` key) — every entry carries its own
/// `license_tier`/`price_usdc` directly. `price_usdc == 0` is the active BETA gate,
/// not "this entry is a different kind of thing."
fn catalog_data_body(body: &[u8], products: &Value) -> Value {
    let html = String::from_utf8_lossy(body);
    let mut entries = Vec::new();

    for i in products
        .get("installers")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
    {
        let id = i.get("id").and_then(|v| v.as_str()).unwrap_or_default();
        let name = i.get("name").and_then(|v| v.as_str()).unwrap_or_default();
        let units = i.get("price_usdc").and_then(|v| v.as_u64()).unwrap_or(0);
        let tier = i.get("license_tier").and_then(|v| v.as_str()).unwrap_or("");
        let mut entry = json!({
            "kind": "installer",
            "id": id,
            "name": name,
            "license_tier": tier,
            "price_usdc": units,
            "id_present": html.contains(id),
            "name_present": html.contains(name),
        });
        if units > 0 {
            // The dynamic page renders a lifted-BETA (paid) product as "$X.YZ" (catalog.rs).
            let display = format!("${:.2}", units as f64 / 1_000_000.0);
            entry["price_display"] = Value::String(display.clone());
            entry["price_present"] = Value::Bool(html.contains(&display));
        }
        entries.push(entry);
    }

    json!({ "kind": "catalog-data", "entries": entries })
}

/// Reduce an HTML document to its sorted set of long text runs (>= LONG_RUN_MIN
/// chars after whitespace collapse), skipping style/script content AND the
/// page's `<header>`/`<footer>` elements. Header/footer are chrome on BOTH
/// sides of the comparison — the OLD page's light topnav + thin footer are
/// exactly what `wrap_static_html` strips, and the NEW Sovereign masthead +
/// footer are what it mounts — so "document substance" is everything else.
/// (The chrome swap itself is asserted separately via the `sw-masthead` /
/// trademark markers in the marketplace-2 unit tests, not here.)
fn long_text_runs_body(body: &[u8]) -> Value {
    let html = String::from_utf8_lossy(body);
    let runs = extract_long_text_runs(&html);
    json!({ "kind": "long-text-runs", "runs": runs })
}

fn extract_long_text_runs(html: &str) -> Vec<String> {
    let mut runs: BTreeSet<String> = BTreeSet::new();
    let mut rest = html;
    loop {
        let Some(lt) = rest.find('<') else {
            push_run(rest, &mut runs);
            break;
        };
        push_run(&rest[..lt], &mut runs);
        let after = &rest[lt..];
        let lower = after.to_ascii_lowercase();
        // Skip non-prose (style/script) and chrome (header/footer) elements
        // wholesale. `<header` cannot false-match `<head>` (longer prefix).
        let skipped = if lower.starts_with("<style") {
            skip_element(after, &lower, "</style>")
        } else if lower.starts_with("<script") {
            skip_element(after, &lower, "</script>")
        } else if lower.starts_with("<header") {
            skip_element(after, &lower, "</header>")
        } else if lower.starts_with("<footer") {
            skip_element(after, &lower, "</footer>")
        } else {
            None
        };
        if let Some(next) = skipped {
            rest = next;
            continue;
        }
        match after.find('>') {
            Some(gt) => rest = &after[gt + 1..],
            None => break, // truncated tag — stop
        }
    }
    runs.into_iter().collect()
}

fn skip_element<'a>(after: &'a str, lower: &str, close: &str) -> Option<&'a str> {
    lower.find(close).map(|i| &after[i + close.len()..])
}

fn push_run(text: &str, runs: &mut BTreeSet<String>) {
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.len() >= LONG_RUN_MIN {
        runs.insert(collapsed);
    }
}

// ---------------------------------------------------------------------------
// Minimal HTTP/1.1 client (localhost only; Connection: close; no redirects)
// ---------------------------------------------------------------------------

fn http_request(
    method: Method,
    port: u16,
    path: &str,
    headers: &[(String, String)],
    body: Option<&str>,
) -> Result<HttpResponse, String> {
    let addr = format!("127.0.0.1:{port}");
    let mut stream = TcpStream::connect(&addr).map_err(|e| format!("connect {addr}: {e}"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(30)))
        .map_err(|e| e.to_string())?;

    let mut request = format!(
        "{} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nUser-Agent: xtask-characterize/0.2\r\nAccept: */*\r\nConnection: close\r\n",
        method.as_str()
    );
    for (k, v) in headers {
        request.push_str(&format!("{k}: {v}\r\n"));
    }
    let body_bytes = body.map(str::as_bytes).unwrap_or_default();
    if matches!(method, Method::Post) {
        request.push_str(&format!("Content-Length: {}\r\n", body_bytes.len()));
    }
    request.push_str("\r\n");

    stream
        .write_all(request.as_bytes())
        .map_err(|e| format!("write request head: {e}"))?;
    if !body_bytes.is_empty() {
        stream
            .write_all(body_bytes)
            .map_err(|e| format!("write request body: {e}"))?;
    }

    let mut raw = Vec::new();
    stream
        .read_to_end(&mut raw)
        .map_err(|e| format!("read response: {e}"))?;

    parse_response(&raw)
}

fn parse_response(raw: &[u8]) -> Result<HttpResponse, String> {
    let header_end =
        find_subslice(raw, b"\r\n\r\n").ok_or("malformed response: no header terminator")?;
    let head = std::str::from_utf8(&raw[..header_end])
        .map_err(|e| format!("non-UTF-8 response head: {e}"))?;
    let mut lines = head.split("\r\n");

    let status_line = lines.next().ok_or("empty response")?;
    let mut parts = status_line.splitn(3, ' ');
    let _version = parts.next().ok_or("malformed status line")?;
    let status: u16 = parts
        .next()
        .ok_or("malformed status line: no code")?
        .parse()
        .map_err(|e| format!("malformed status code: {e}"))?;

    let mut headers: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for line in lines {
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| format!("malformed header line: {line}"))?;
        headers
            .entry(name.trim().to_ascii_lowercase())
            .or_default()
            .push(value.trim().to_string());
    }

    let rest = &raw[header_end + 4..];
    let body = if headers
        .get("transfer-encoding")
        .map(|v| v.iter().any(|s| s.to_ascii_lowercase().contains("chunked")))
        .unwrap_or(false)
    {
        decode_chunked(rest)?
    } else if let Some(cl) = headers.get("content-length").and_then(|v| v.first()) {
        let n: usize = cl.parse().map_err(|e| format!("bad content-length: {e}"))?;
        if rest.len() < n {
            return Err(format!(
                "truncated body: content-length {n}, got {}",
                rest.len()
            ));
        }
        rest[..n].to_vec()
    } else {
        // Connection: close — body runs to EOF.
        rest.to_vec()
    };

    Ok(HttpResponse {
        status,
        headers,
        body,
    })
}

fn decode_chunked(mut rest: &[u8]) -> Result<Vec<u8>, String> {
    let mut body = Vec::new();
    loop {
        let line_end = find_subslice(rest, b"\r\n").ok_or("chunked: missing size line")?;
        let size_line = std::str::from_utf8(&rest[..line_end])
            .map_err(|e| format!("chunked: bad size line: {e}"))?;
        let size_hex = size_line.split(';').next().unwrap_or("").trim();
        let size = usize::from_str_radix(size_hex, 16)
            .map_err(|e| format!("chunked: bad chunk size '{size_hex}': {e}"))?;
        rest = &rest[line_end + 2..];
        if size == 0 {
            break; // trailers (if any) ignored
        }
        if rest.len() < size + 2 {
            return Err("chunked: truncated chunk".to_string());
        }
        body.extend_from_slice(&rest[..size]);
        rest = &rest[size + 2..]; // skip chunk CRLF
    }
    Ok(body)
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

// ---------------------------------------------------------------------------
// diff mode — old-vs-new comparison with expectation classification
// ---------------------------------------------------------------------------

fn cmd_diff(args: &[String]) -> Result<(), String> {
    if args.len() != 2 {
        return Err("diff: expected exactly two snapshot directories (<old-dir> <new-dir>)".into());
    }
    let dir_old = Path::new(&args[0]);
    let dir_new = Path::new(&args[1]);

    let set_old = load_snapshot_dir(dir_old)?;
    let set_new = load_snapshot_dir(dir_new)?;

    let mut identical = 0usize;
    let mut expected = 0usize;
    let mut unexplained = 0usize;

    for name in set_old.keys() {
        if !set_new.contains_key(name) {
            println!("[-] UNEXPLAINED: only in {}: {name}", dir_old.display());
            unexplained += 1;
        }
    }
    for name in set_new.keys() {
        if !set_old.contains_key(name) {
            println!("[-] UNEXPLAINED: only in {}: {name}", dir_new.display());
            unexplained += 1;
        }
    }

    for (name, old) in &set_old {
        let Some(new) = set_new.get(name) else {
            continue;
        };
        let old_n = strip_port(old);
        let new_n = strip_port(new);
        if old_n == new_n {
            identical += 1;
            continue;
        }

        let fixture = FIXTURES.iter().find(|f| f.name == name.as_str());
        match fixture.map(|f| f.expect) {
            Some(Expect::RootRedirect) => match verify_root_redirect(&old_n, &new_n) {
                Ok(msg) => {
                    println!("[=] EXPECTED  {name}: {msg}");
                    expected += 1;
                }
                Err(e) => {
                    println!("[-] UNEXPLAINED {name}: RootRedirect verification FAILED: {e}");
                    print_field_diffs(&old_n, &new_n);
                    unexplained += 1;
                }
            },
            Some(Expect::PricingFix {
                old_product,
                new_product,
            }) => match verify_pricing_fix(&old_n, &new_n, old_product, new_product) {
                Ok(msg) => {
                    println!("[=] EXPECTED  {name}: {msg}");
                    expected += 1;
                }
                Err(e) => {
                    println!("[-] UNEXPLAINED {name}: PricingFix verification FAILED: {e}");
                    print_field_diffs(&old_n, &new_n);
                    unexplained += 1;
                }
            },
            Some(Expect::ChromeDiff(reason)) => {
                match verify_chrome_diff(fixture.unwrap(), &old_n, &new_n) {
                    Ok(details) => {
                        println!("[=] EXPECTED  {name}: {reason}");
                        for d in details {
                            println!("      {d}");
                        }
                        expected += 1;
                    }
                    Err(e) => {
                        println!("[-] UNEXPLAINED {name}: ChromeDiff data check FAILED: {e}");
                        print_field_diffs(&old_n, &new_n);
                        unexplained += 1;
                    }
                }
            }
            Some(Expect::Same) | None => {
                println!("[-] UNEXPLAINED {name} differs:");
                print_field_diffs(&old_n, &new_n);
                unexplained += 1;
            }
        }
    }

    println!(
        "\n[*] diff summary: {identical} identical, {expected} expected difference(s) verified, {unexplained} unexplained",
    );
    if unexplained == 0 {
        println!(
            "[+] parity holds between {} and {} (all differences are documented outcomes of the rewrite)",
            dir_old.display(),
            dir_new.display()
        );
        Ok(())
    } else {
        Err(format!(
            "diff: {unexplained} unexplained difference(s) found"
        ))
    }
}

/// Remove `request.port` (and legacy-only fields) so snapshots taken on
/// different scratch ports remain comparable.
fn strip_port(v: &Value) -> Value {
    let mut v = v.clone();
    if let Some(req) = v.get_mut("request").and_then(|r| r.as_object_mut()) {
        req.remove("port");
    }
    v
}

fn print_field_diffs(a: &Value, b: &Value) {
    for field in ["status", "headers", "body", "request", "normalize"] {
        let (fa, fb) = (a.get(field), b.get(field));
        if fa != fb {
            println!("      {field}:");
            println!("        old: {}", compact(fa));
            println!("        new: {}", compact(fb));
        }
    }
}

fn verify_root_redirect(old: &Value, new: &Value) -> Result<String, String> {
    let so = old.get("status").and_then(|v| v.as_u64()).unwrap_or(0);
    let sn = new.get("status").and_then(|v| v.as_u64()).unwrap_or(0);
    if so != 303 {
        return Err(format!(
            "old status is {so}, expected 303 (axum Redirect::to)"
        ));
    }
    if sn != 302 {
        return Err(format!("new status is {sn}, expected 302 (P1 contract)"));
    }
    let loc = |v: &Value| {
        v.get("headers")
            .and_then(|h| h.get("location"))
            .and_then(|l| l.as_str())
            .map(str::to_string)
    };
    let (lo, ln) = (loc(old), loc(new));
    if lo != ln || lo.is_none() {
        return Err(format!("Location mismatch: old={lo:?} new={ln:?}"));
    }
    // Beyond status + the volatile headers, nothing else may differ.
    let mut o = old.clone();
    let mut n = new.clone();
    for v in [&mut o, &mut n] {
        if let Some(obj) = v.as_object_mut() {
            obj.remove("status");
        }
    }
    if o != n {
        return Err("difference extends beyond the 303-vs-302 status code".into());
    }
    Ok(format!(
        "303 (old, axum Redirect::to) vs 302 (new, deliberate P1 contract), same Location {}",
        lo.unwrap_or_default()
    ))
}

/// Recompute a marketplace license key: first 32 hex chars of
/// SHA256("{product_id}:{tx_hash}:{customer_ref}") in four hyphenated groups.
/// Must stay byte-identical to both crates and tool-wallet.
fn derive_license_key(product_id: &str, tx_hash: &str, customer_ref: &str) -> String {
    let h = hex::encode(Sha256::digest(
        format!("{product_id}:{tx_hash}:{customer_ref}").as_bytes(),
    ));
    format!("{}-{}-{}-{}", &h[0..8], &h[8..16], &h[16..24], &h[24..32])
}

fn verify_pricing_fix(
    old: &Value,
    new: &Value,
    old_product: &str,
    new_product: &str,
) -> Result<String, String> {
    for (side, v) in [("old", old), ("new", new)] {
        let s = v.get("status").and_then(|x| x.as_u64()).unwrap_or(0);
        if s != 200 {
            return Err(format!("{side} status is {s}, expected 200 confirmed"));
        }
    }
    let body = |v: &Value| -> Result<Value, String> {
        v.get("body")
            .and_then(|b| b.get("json"))
            .cloned()
            .ok_or_else(|| "body is not a JSON snapshot".to_string())
    };
    let (bo, bn) = (body(old)?, body(new)?);
    let field = |b: &Value, f: &str| -> Result<String, String> {
        b.get(f)
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .ok_or_else(|| format!("missing field {f}"))
    };

    for (side, b) in [("old", &bo), ("new", &bn)] {
        let st = field(b, "status")?;
        if st != "confirmed" {
            return Err(format!(
                "{side} body status is '{st}', expected 'confirmed'"
            ));
        }
    }

    let po = field(&bo, "product_id")?;
    let pn = field(&bn, "product_id")?;
    if po != old_product {
        return Err(format!("old product_id '{po}' != expected '{old_product}'"));
    }
    if pn != new_product {
        return Err(format!("new product_id '{pn}' != expected '{new_product}'"));
    }

    let co = field(&bo, "customer_ref")?;
    let cn = field(&bn, "customer_ref")?;
    if co != cn {
        return Err(format!("customer_ref mismatch: old={co} new={cn}"));
    }

    let tx = old
        .get("request")
        .and_then(|r| r.get("path"))
        .and_then(|p| p.as_str())
        .and_then(|p| p.rsplit('/').next())
        .ok_or("cannot extract tx_hash from request path")?
        .to_lowercase();

    for (side, b, product) in [("old", &bo, &po), ("new", &bn, &pn)] {
        let key = field(b, "license_key")?;
        let want = derive_license_key(product, &tx, &co);
        if key != want {
            return Err(format!(
                "{side} license_key '{key}' does not re-derive from ('{product}', '{tx}', '{co}')"
            ));
        }
    }

    Ok(format!(
        "Checkpoint 2 pricing-unit fix confirmed: old matched '{po}', new matches '{pn}'; \
         both license keys re-derive correctly from their own product_id"
    ))
}

/// Data-completeness check for chrome-rewritten pages.
///
/// - `CatalogData`: the NEW page must render every catalog entry (id + name +
///   paid-tier price display all present). OLD-page gaps are reported as
///   details (stale-static findings) but are not fatal — that gap is a known,
///   separately-flagged condition.
/// - `LongTextRuns`: every long text run served by the OLD page must survive
///   in the NEW page (chrome may ADD runs, never lose document content).
fn verify_chrome_diff(fixture: &Fixture, old: &Value, new: &Value) -> Result<Vec<String>, String> {
    let so = old.get("status").and_then(|v| v.as_u64()).unwrap_or(0);
    let sn = new.get("status").and_then(|v| v.as_u64()).unwrap_or(0);
    if so != sn {
        return Err(format!("status differs: old={so} new={sn}"));
    }

    match fixture.normalize {
        Normalize::CatalogData => {
            let entries = |v: &Value| -> Vec<Value> {
                v.get("body")
                    .and_then(|b| b.get("entries"))
                    .and_then(|e| e.as_array())
                    .cloned()
                    .unwrap_or_default()
            };
            let (eo, en) = (entries(old), entries(new));
            let mut details = Vec::new();

            // NEW side must be complete.
            for e in &en {
                let id = e.get("id").and_then(|v| v.as_str()).unwrap_or("?");
                let ok = e
                    .get("id_present")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
                    && e.get("name_present")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false)
                    && e.get("price_present")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true);
                if !ok {
                    return Err(format!(
                        "NEW page is missing catalog entry data for '{id}': {e}"
                    ));
                }
            }
            details.push(format!(
                "NEW page renders all {} catalog entries (id + name + paid price displays)",
                en.len()
            ));

            // Report OLD-side gaps (stale static page) without failing.
            for e in &eo {
                let id = e.get("id").and_then(|v| v.as_str()).unwrap_or("?");
                let mut missing = Vec::new();
                for (flag, label) in [
                    ("id_present", "id"),
                    ("name_present", "name"),
                    ("price_present", "price"),
                ] {
                    if let Some(false) = e.get(flag).and_then(|v| v.as_bool()) {
                        missing.push(label);
                    }
                }
                if !missing.is_empty() {
                    details.push(format!(
                        "OLD stale static page lacks {} for '{id}' (known stale-static gap, flagged to Command)",
                        missing.join("+")
                    ));
                }
            }
            Ok(details)
        }
        Normalize::LongTextRuns => {
            let runs = |v: &Value| -> BTreeSet<String> {
                v.get("body")
                    .and_then(|b| b.get("runs"))
                    .and_then(|r| r.as_array())
                    .into_iter()
                    .flatten()
                    .filter_map(|s| s.as_str().map(str::to_string))
                    .collect()
            };
            let (ro, rn) = (runs(old), runs(new));
            let lost: Vec<&String> = ro.difference(&rn).collect();
            if !lost.is_empty() {
                return Err(format!(
                    "NEW page LOST {} long text run(s) from the OLD document, e.g.: {}",
                    lost.len(),
                    truncate(lost[0], 200)
                ));
            }
            let added: Vec<&String> = rn.difference(&ro).collect();
            let mut details = vec![format!(
                "all {} OLD long text runs preserved in NEW; {} chrome-added run(s)",
                ro.len(),
                added.len()
            )];
            for a in added.iter().take(5) {
                details.push(format!("chrome-added: {}", truncate(a, 160)));
            }
            Ok(details)
        }
        _ => Err("ChromeDiff expectation on a non-data-normalized fixture".into()),
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.len() <= n {
        s.to_string()
    } else {
        let mut end = n;
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}… ({} chars)", &s[..end], s.len())
    }
}

fn compact(v: Option<&Value>) -> String {
    match v {
        Some(v) => {
            let s = serde_json::to_string(v).unwrap_or_else(|_| "<unserializable>".into());
            truncate(&s, 400)
        }
        None => "<absent>".to_string(),
    }
}

fn load_snapshot_dir(dir: &Path) -> Result<BTreeMap<String, Value>, String> {
    let mut out = BTreeMap::new();
    let entries = fs::read_dir(dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| format!("bad filename: {}", path.display()))?
            .to_string();
        let text =
            fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
        let value: Value =
            serde_json::from_str(&text).map_err(|e| format!("parse {}: {e}", path.display()))?;
        out.insert(name, value);
    }
    if out.is_empty() {
        return Err(format!("no *.json snapshots found in {}", dir.display()));
    }
    Ok(out)
}
