use anyhow::Result;
use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderName, HeaderValue, StatusCode},
    response::{IntoResponse, Json, Redirect, Response},
    routing::{get, post},
    Router,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::Utc;
use ed25519_dalek::{Signer, SigningKey};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf, process::Command, sync::Arc};
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;

// Security headers applied to every response. CSP allows 'unsafe-inline' for
// script-src/style-src because this crate's own architecture serves CSS and a
// couple of small interactive scripts (SHA256 verification fetch, install-command
// copy) as inline <style>/<script> blocks rather than external assets (see
// ui::layout::head, ui::product_detail, ui::catalog) — there are no nonces or
// hashes wired through the render pipeline to tighten this further, and no
// external CDN/font dependency that would need its own allowance.
const HSTS_VALUE: &str = "max-age=63072000; includeSubDomains";
const CSP_VALUE: &str = "default-src 'self'; script-src 'self' 'unsafe-inline'; \
    style-src 'self' 'unsafe-inline'; img-src 'self' data:; frame-ancestors 'none'; \
    base-uri 'self'";

mod ui;
use ui::SoftwareSurface;

// ── State ─────────────────────────────────────────────────────────────────────
//
// The payment-config fields (`polygon_wallet_address`, `receipts_dir`,
// `claims_dir`, `polygon_rpc_url`, `tool_wallet_bin`) were wired as placeholders
// in P1 and are consumed by the P4 license/claim/wallet handlers below.
// `bind_addr` remains stored for symmetry but is only read via the local in
// `main`; it is retained behind `#[allow(dead_code)]`.
#[derive(Clone)]
#[allow(dead_code)]
struct AppState {
    catalog_path: PathBuf,
    bind_addr: String,
    // Static-HTML source of truth (single source: the on-disk directory). Both
    // /software and /licensing read from this directory at request time, and
    // /static/* mounts the same directory via ServeDir. Nothing is baked with
    // include_str! — see BRIEF/report for the rationale.
    static_dir: PathBuf,
    // ── P4 payment config ───────────────────────────────────────────────────
    polygon_wallet_address: String,
    receipts_dir: PathBuf,
    claims_dir: PathBuf,
    source_base_url: String,
    polygon_rpc_url: String,
    // Name (or absolute path) of the `tool-wallet` binary shelled out to by
    // `v1_license`. Defaults to `"tool-wallet"` (resolved via PATH, matching the
    // OLD crate's `Command::new("tool-wallet")`). Overridable via the
    // `TOOL_WALLET_BIN` env var so tests can inject a JSON test-double without a
    // real Polygon RPC call. Production behaviour is unchanged.
    tool_wallet_bin: String,
    // ── Phase 2: real download-token minting ────────────────────────────────
    // The private counterpart to `app-privategit-source-2`'s `VERIFY_KEY_PUB` —
    // loaded from `SIGNING_KEY_SECRET` (same hex-seed-or-file-path convention as
    // that crate's `load_verify_key`). `None` until a production key is
    // provisioned (a deployment-time secrets step, not decided by this code).
    signing_key: Option<SigningKey>,
    // ── Phase 5: CRA barter-transaction log ─────────────────────────────────
    // `data/software-catalog/tx-log.jsonl`'s local-crate counterpart, per
    // `BRIEF-software-distribution-substrate.md` §8 — required from the first
    // real sale.
    tx_log_path: PathBuf,
    // Placeholder USDC->CAD spot rate (no live FX feed is wired into this crate;
    // building one is out of scope for this cleanup). Configurable via
    // `USDC_CAD_SPOT_RATE` so production can update it without a code change —
    // see `append_tx_log`'s doc comment.
    usdc_cad_spot_rate: f64,
}

// ── Catalog types ─────────────────────────────────────────────────────────────
//
// Rebuilt per `BRIEF-software-hyperscaler-audit.md`'s Licensing Corrections section
// (factory-release-engineering/LICENSE-MATRIX.md is authoritative). The prior
// `licenses:` list conflated two unrelated things — two license *terms* (mislabeled
// "Apache 2.0"/"FSL") and five unrelated free *products* — with an `installers:` list
// that was already correctly modeled. Each os-* product has exactly ONE fixed tier
// (not a customer choice), so the fix is additive fields on `Installer`, not a new
// products/terms-with-references model.

/// The ratified license tiers.
///
/// **Rebuilt 2026-07-07** to match `BRIEF-software-licensing-structure.md`
/// (Command Session, ratified 2026-07-07) — the authoritative per-product tier
/// review that superseded the three-tier `Commercial`/`Fsl`/`OpenSource` model
/// this enum used to carry. That BRIEF's own per-product table never uses
/// "PointSav Commercial" as a license category — it reclassifies every `os-*`
/// product into one of exactly four real tiers, each a real license identifier:
///
/// - `Proprietary` — permanent, no source grant. `os-orchestration` +
///   `app-orchestration-*` only (the company's stated commercial moat).
/// - `Fsl` — FSL-1.1-ALv2. Source-readable now, converts to Apache-2.0 two years
///   after each release. `os-infrastructure`, `os-network-admin`, `os-mediakit`,
///   `os-totebox`, the `os-privategit` engine, and the `os-workplace` `moonshot-*`
///   engine crates.
/// - `Agpl` — AGPL-3.0-or-later, this workspace's deliberate backend default
///   (kept, not replaced — see that BRIEF's Decision 1). Includes `os-console` +
///   `app-console-*`, the flagship buy-to-own product: its BRIEF explicitly
///   calls "Commercial" a *pricing* state ("Commercial tier when priced"), not a
///   separate license — the license underneath is AGPL.
/// - `Apache` — a genuine, unconditional Apache-2.0 grant. `tool-wallet` (a named
///   exception to the `tool-*` AGPL default, seeding the Binary Library's Open
///   Source/Community shelf), `pointsav-design-system`, `woodfine-bim-library`.
///
/// Per that BRIEF §4, **every catalogued product is priced at $0 USDC (BETA) for
/// now, regardless of tier** — pricing and licensing are independent decisions,
/// and no non-zero future price has been ratified for any tier (see
/// `Installer::price_usdc`, not this enum, for the one number that's real).
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum LicenseTier {
    Proprietary,
    Fsl,
    Agpl,
    Apache,
}

impl LicenseTier {
    /// Display label — the real SPDX-recognizable identifier for every tier
    /// except `Proprietary` (which has none). `Apache` carries a shelf-clarifying
    /// suffix since it's the one tier that's permanently, unconditionally free.
    fn label(self) -> &'static str {
        match self {
            LicenseTier::Proprietary => "Proprietary",
            LicenseTier::Fsl => "FSL-1.1-ALv2",
            LicenseTier::Agpl => "AGPL-3.0-or-later",
            LicenseTier::Apache => "Apache-2.0 (Open Source)",
        }
    }

    /// Which Binary Library shelf this tier belongs to
    /// (`BRIEF-binary-library-repositioning.md`'s two-shelf model, operator-approved
    /// 2026-07-07). `Proprietary`/`Fsl`/`Agpl` are all the existing, ratified
    /// os-*-only public catalog — unchanged. `Apache` is the Open Source shelf,
    /// populated only as individual crates are actually relicensed (Phase 2),
    /// never inferred. Read by `v1_products` (JSON `shelf` field) and
    /// `ui::catalog::catalog_markup` (the two-shelf HTML grouping) — kept
    /// alongside the tier it derives from rather than a separate,
    /// independently-settable field, so shelf membership can never drift out of
    /// sync with `license_tier` itself.
    fn shelf(self) -> Shelf {
        match self {
            LicenseTier::Proprietary | LicenseTier::Fsl | LicenseTier::Agpl => Shelf::Commercial,
            LicenseTier::Apache => Shelf::OpenSource,
        }
    }
}

/// The two Binary Library shelves. See [`LicenseTier::shelf`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shelf {
    Commercial,
    OpenSource,
}

impl Shelf {
    /// Wire-format value for the `/v1/products` JSON `shelf` field.
    fn as_str(self) -> &'static str {
        match self {
            Shelf::Commercial => "commercial",
            Shelf::OpenSource => "open-source",
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Installer {
    id: String,
    name: String,
    description: String,
    edition: String,
    platform: String,
    size_mb: u64,
    path: String,
    /// Ratified tier label — always set correctly regardless of BETA status; this is
    /// legal/display metadata, not a payment gate. See `price_usdc` for the gate.
    license_tier: LicenseTier,
    /// The active price. `0` = active BETA gate (no payment/license flow triggered,
    /// same curl-pipe-sh pattern as any other free product — no separate `beta: true`
    /// flag needed, matching this workspace's established BETA convention). Every
    /// product ships at `0` initially per an explicit, current operator/Command
    /// directive (os-console, os-mediakit, and the orchestration-command binary all
    /// carry an active "stay free during BETA" instruction in `.agent/inbox.md` as of
    /// 2026-07-01/02, and none has been lifted yet). Flipping a specific product to
    /// its real tier price (1_000_000 for `commercial`, 19_000_000 for `fsl`) is a
    /// one-line data change once Command sends an explicit per-product BETA-lifted
    /// message — not a code change.
    price_usdc: u64,
    /// Phase 5: the date (`YYYY-MM-DD`) this specific version's FSL term
    /// automatically converts to Apache 2.0 (two years after release, per
    /// FSL-1.1-ALv2 — see `LICENSE-MATRIX.md`). Populated manually per release;
    /// no release-date data exists in this catalog to derive it automatically.
    /// Meaningless for `commercial`-tier products (always `None` there).
    /// Read by `xtask fsl-clock`, not by any route in this crate.
    #[serde(default)]
    fsl_conversion_date: Option<String>,
    /// S136: link to the product's operational GUIDE, when one exists.
    /// Genuinely optional — the original request's own wording was "GUIDE link
    /// (when available)". No `products.yaml` entry populates this yet; the
    /// product detail page (`ui::product_detail`) simply omits the row when
    /// `None` rather than fabricating a URL.
    #[serde(default)]
    guide_url: Option<String>,
}

// `deny_unknown_fields`: a stray legacy `licenses:` key (from a pre-migration
// products.yaml) must fail to parse loudly (500 on every catalog-backed route),
// not silently produce an empty-but-successfully-parsed catalog.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Catalog {
    installers: Vec<Installer>,
}

fn load_catalog(catalog_path: &PathBuf) -> Result<Catalog> {
    let raw = fs::read_to_string(catalog_path)?;
    let catalog: Catalog = serde_yaml::from_str(&raw)?;
    for i in &catalog.installers {
        // `Apache` is a genuine, unconditional Apache-2.0 grant, not a BETA gate —
        // it has no "real price to flip to later." A nonzero price here is always a
        // data-entry mistake (e.g. a stray FSL/AGPL price copied onto a relicensed
        // entry), not a valid pricing choice. Fail loudly, matching this catalog's
        // established pattern for the retired `licenses:` key above.
        if i.license_tier == LicenseTier::Apache && i.price_usdc != 0 {
            anyhow::bail!(
                "installer '{}' is license_tier: apache but has a nonzero \
                 price_usdc ({}) — apache entries must always be price_usdc: 0",
                i.id,
                i.price_usdc
            );
        }
    }
    Ok(catalog)
}

// ── Receipt (mirrors tool-wallet's LicenseReceipt) ────────────────────────────
//
// Field-for-field port of the OLD crate's `LicenseReceipt`. tool-wallet writes
// receipt files that carry ONE extra field — `license_tier` — which serde
// silently ignores here on read (no `deny_unknown_fields`), so files written by
// either binary deserialize cleanly. When THIS crate writes a receipt (the
// fresh-check path in `v1_license`), it omits `license_tier`, exactly as the OLD
// crate did. `price_usdc` here stores `price_units` (micro-USDC), NOT the
// catalog's dollar-labelled value — a pre-existing field-name quirk, not renamed
// in this phase. See tool-wallet/src/main.rs for the writer side.
#[derive(Debug, Serialize, Deserialize)]
struct LicenseReceipt {
    product_id: String,
    version: String,
    customer_ref: String,
    price_usdc: u64,
    tx_hash: String,
    chain: String,
    confirmed_at: String,
    block_number: u64,
    license_key: String,
}

// ── Payment helpers ───────────────────────────────────────────────────────────

/// Deterministic license key: first 32 hex chars of SHA256("{product_id}:{tx_hash}:{customer_ref}"),
/// split into four hyphen-joined 8-char groups. EXACT construction — must stay
/// byte-identical to the OLD crate and tool-wallet so already-issued keys remain
/// reproducible.
fn generate_license_key(product_id: &str, tx_hash: &str, customer_ref: &str) -> String {
    let h = hex::encode(Sha256::digest(
        format!("{product_id}:{tx_hash}:{customer_ref}").as_bytes(),
    ));
    format!("{}-{}-{}-{}", &h[0..8], &h[8..16], &h[16..24], &h[24..32])
}

/// Receipt file path: `<receipts_dir>/<current-UTC-year>/<current-UTC-month>/<tx_hash>.json`.
///
/// NOTE (carried forward, NOT fixed in this phase): the year/month are TODAY's at
/// request time, not the transaction's confirmation date. A receipt written near a
/// month boundary and re-read the next month misses the cache and re-verifies. This
/// is a known, pre-existing gap shared with the OLD crate and tool-wallet's own
/// writer; it was not named as an in-scope fix for P4. Left as-is deliberately.
fn receipt_path(receipts_dir: &std::path::Path, tx_hash: &str) -> PathBuf {
    let now = Utc::now();
    receipts_dir
        .join(now.format("%Y").to_string())
        .join(now.format("%m").to_string())
        .join(format!("{tx_hash}.json"))
}

/// Marker path for [`flag_if_first_live_transaction`] — deliberately at the
/// receipts-dir root, not inside a year/month subdirectory, so it survives
/// month rollover and is trivially findable.
fn first_live_transaction_marker_path(receipts_dir: &std::path::Path) -> PathBuf {
    receipts_dir.join(".first-live-transaction-marker.json")
}

/// Checkpoint 3b (real on-chain confirmation) could not run before this crate
/// went live — operator decision 2026-07-02, given real-transaction testing
/// isn't feasible for some time and the site needs to launch. In place of a
/// pre-launch gate, this flags the FIRST real (subprocess-confirmed, not
/// receipt-cache-replayed) transaction distinctly, so the operator can review
/// that one transaction closely after the fact rather than blind. Purely
/// observational — never affects the response, never blocks or slows the
/// request, and does nothing after the first transaction (every subsequent
/// one is silent).
fn flag_if_first_live_transaction(receipts_dir: &std::path::Path, receipt: &LicenseReceipt) {
    let marker = first_live_transaction_marker_path(receipts_dir);
    if marker.exists() {
        return;
    }
    if let Ok(raw) = serde_json::to_string_pretty(receipt) {
        let _ = fs::write(&marker, raw);
    }
    tracing::warn!(
        tx_hash = %receipt.tx_hash,
        product_id = %receipt.product_id,
        license_key = %receipt.license_key,
        confirmed_at = %receipt.confirmed_at,
        "FIRST-LIVE-TRANSACTION: the first real on-chain payment has been confirmed through \
         this crate. Checkpoint 3b was deferred at launch (2026-07-02) -- review this specific \
         transaction and receipt now to close it out."
    );
}

/// Append one JSONL row to the CRA barter-transaction log
/// (`BRIEF-software-distribution-substrate.md` §8 — required from the first real
/// sale; a crypto payment is a CRA barter transaction, recorded at CAD fair-market
/// value at settlement time). Schema matches that BRIEF's own example exactly:
/// `date`, `sku` (`<product_id>@<edition>`), `license_tier`, `crypto_received`,
/// `polygon_tx`, `spot_rate_cad`, `cad_equivalent`.
///
/// Only called from the fresh-confirmation path in `resolve_license`, never the
/// receipt-cache-replay path — a cached replay is not a new sale and must not be
/// logged twice.
///
/// **Known simplification**: `spot_rate_cad` is a placeholder (`AppState::
/// usdc_cad_spot_rate`, configurable via `USDC_CAD_SPOT_RATE`, no live FX feed is
/// wired into this crate). Sourcing a real rate at settlement time is future work,
/// plausibly a `project-bookkeeping` integration per that BRIEF's own §9
/// cross-cluster dependency table — not decided here.
fn append_tx_log(
    tx_log_path: &std::path::Path,
    receipt: &LicenseReceipt,
    edition: &str,
    license_tier: &str,
    spot_rate_cad: f64,
) {
    let usd = receipt.price_usdc as f64 / 1_000_000.0;
    let cad_equivalent = usd * spot_rate_cad;
    let row = json!({
        "date": receipt.confirmed_at,
        "sku": format!("{}@{}", receipt.product_id, edition),
        "license_tier": license_tier,
        "crypto_received": format!("{usd:.2} USDC"),
        "polygon_tx": receipt.tx_hash,
        "spot_rate_cad": format!("{spot_rate_cad:.2}"),
        "cad_equivalent": format!("{cad_equivalent:.2}"),
    });
    if let Some(parent) = tx_log_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(mut line) = serde_json::to_string(&row) {
        line.push('\n');
        use std::io::Write;
        match fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(tx_log_path)
        {
            Ok(mut file) => {
                if let Err(e) = file.write_all(line.as_bytes()) {
                    tracing::warn!("tx-log append failed: {e}");
                }
            }
            Err(e) => tracing::warn!("tx-log open failed: {e}"),
        }
    }
}

/// Convert a dollars-denominated USDC amount (as reported by `tool-wallet check`'s
/// `amount_usdc` float) into integer micro-USDC base units (6 decimals).
/// Correct for whole-dollar amounts ($1.00 → 1_000_000), but **lossy for many
/// cent-level values** due to the `f64` round-trip (Checkpoint 2 review finding —
/// see `price_units_from_check_json`, which prefers an exact integer and uses
/// this only as a defensive fallback). Do not call this directly on a
/// `tool-wallet check` response; go through `price_units_from_check_json`.
fn price_units_from_amount(amount_usdc: f64) -> u64 {
    (amount_usdc * 1_000_000.0) as u64
}

/// Extract the confirmed payment amount, in exact micro-USDC base units, from a
/// `tool-wallet check` confirmation JSON.
///
/// # Checkpoint 2 review finding
///
/// Prefers `amount_units` — the exact source integer `tool-wallet` already emits
/// (`tool-wallet/src/main.rs:568`) — over `price_units_from_amount(amount_usdc)`,
/// which round-trips through a lossy `f64` and is off-by-one for roughly 2% of
/// whole-cent prices (e.g. a genuine $2.01 arrives as 2,009,999, one micro-unit
/// short, and would silently fail to match — the same failure class this phase's
/// P4 pricing-unit fix exists to eliminate, just latent rather than fixed). The
/// float path is kept only as a defensive fallback for a `tool-wallet` response
/// that, contrary to its current and expected behavior, omits `amount_units`.
fn price_units_from_check_json(check_json: &Value) -> u64 {
    check_json
        .get("amount_units")
        .and_then(|v| v.as_u64())
        .unwrap_or_else(|| {
            let amount_usdc = check_json
                .get("amount_usdc")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            price_units_from_amount(amount_usdc)
        })
}

/// Resolve the catalog `product_id` whose price equals an on-chain payment of
/// `price_units` (micro-USDC base units).
///
/// # P4 pricing-unit fix — the one deliberate behavioural change in this rewrite
///
/// The OLD crate (`app-privategit-marketplace/src/main.rs`) matched with:
///
/// ```text
/// c.licenses.iter().find(|l| l.price_usdc * 1_000_000 == price_units)
/// ```
///
/// That is WRONG. `products.yaml` already stores `price_usdc` in micro-USDC base
/// units (`apache: 1000000` = $1.00, `fsl: 19000000` = $19.00 — the field is
/// misleadingly *named* dollars but its *value* is micro-units). Re-multiplying
/// the catalog side by 1_000_000 double-counts the unit conversion, so a genuine
/// $1.00 payment (`price_units == 1_000_000`) was compared against
/// `1_000_000 * 1_000_000` and could never match a real payment except by
/// coincidence.
///
/// **Correction (Checkpoint 2 review):** `apache` and `fsl` are live, non-zero
/// priced entries — NOT every catalog price is `0`. The actual safety argument
/// is: the OLD formula never matched a real payment (any historical paid tx would
/// have fallen through to `unknown-<price_units>`), so switching to the correct
/// comparison cannot invalidate any previously-issued license key — receipts are
/// read from disk verbatim and replay identically regardless of this fix.
///
/// THE FIX: compare `l.price_usdc == price_units` directly — both operands are
/// already micro-USDC units, so no multiplication belongs on the catalog side.
/// Full rationale: `docs/P4-PRICING-FIX.md`.
///
/// **Known limitation, not fixed in this phase (data-model rebuild):** now that
/// multiple os-* products can share the same tier price (up to 4 at `commercial`/
/// $1, up to 4 at `fsl`/$19, once BETA lifts for more than one product per tier),
/// price alone cannot disambiguate WHICH product was purchased — `.find()` returns
/// the first match. This is a legacy/compatibility path for the raw `/v1/license/
/// :tx_hash` JSON endpoint only. The new `/checkout/:product_id` → `/order/:tx_hash`
/// flow (Phase 2) carries `product_id` explicitly from the moment the customer picks
/// a product, so it does not depend on this inference at all — this function is not
/// the long-term mechanism, just the pre-existing one kept working during the
/// transition.
fn match_license_product_id(catalog: &Catalog, price_units: u64) -> Option<String> {
    catalog
        .installers
        .iter()
        // FIX: direct micro-unit equality. The OLD crate wrote `l.price_usdc *
        // 1_000_000 == price_units`, which re-applied the dollars→micro-units
        // scale to a value already in micro-units. No re-multiplication here.
        .find(|i| i.price_usdc == price_units)
        .map(|i| i.id.clone())
}

/// Outcome of shelling out to `tool-wallet check`.
enum WalletCheck {
    /// Subprocess exited 0 and its JSON reported `confirmed: true`.
    Confirmed(Value),
    /// Subprocess exited 0 and its JSON reported `confirmed: false`.
    Pending,
    /// Subprocess failed to spawn, exited non-zero, or emitted unparseable stdout.
    NotFound,
}

/// Run `tool-wallet check <tx_hash> --rpc-url <url> --wallet-address <addr>` as an
/// external subprocess and classify the result. Consumes tool-wallet's CLI
/// contract exactly as-is; never modifies it. `bin` is normally `"tool-wallet"`
/// (PATH-resolved); tests inject a JSON test-double path via it.
fn run_tool_wallet_check(
    bin: &str,
    tx_hash: &str,
    rpc_url: &str,
    wallet_addr: &str,
) -> WalletCheck {
    let result = Command::new(bin)
        .args([
            "check",
            tx_hash,
            "--rpc-url",
            rpc_url,
            "--wallet-address",
            wallet_addr,
        ])
        .output();

    match result {
        Ok(out) if out.status.success() => match serde_json::from_slice::<Value>(&out.stdout) {
            Ok(check_json) => {
                let confirmed = check_json
                    .get("confirmed")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                if confirmed {
                    WalletCheck::Confirmed(check_json)
                } else {
                    WalletCheck::Pending
                }
            }
            Err(e) => {
                tracing::warn!("tool-wallet check: unparseable stdout: {e}");
                WalletCheck::NotFound
            }
        },
        Ok(out) => {
            tracing::warn!(
                "tool-wallet check exit {:?}: {}",
                out.status,
                String::from_utf8_lossy(&out.stderr)
            );
            WalletCheck::NotFound
        }
        Err(e) => {
            tracing::error!("tool-wallet not available: {e}");
            WalletCheck::NotFound
        }
    }
}

/// The shared `200 OK` confirmed-license JSON shape (receipt-cache path and
/// fresh-check path return identical bodies).
fn confirmed_response(
    license_key: &str,
    product_id: &str,
    confirmed_at: &str,
    customer_ref: &str,
) -> (StatusCode, Json<Value>) {
    (
        StatusCode::OK,
        Json(json!({
            "status": "confirmed",
            "license_key": license_key,
            "product_id": product_id,
            "confirmed_at": confirmed_at,
            "customer_ref": customer_ref
        })),
    )
}

// ── Result of resolving a tx_hash — shared by the JSON and HTML order endpoints ──
//
// Extracted from `v1_license`'s body (Phase 2) so the new `/order/:tx_hash` HTML
// page and the existing `/v1/license/:tx_hash` JSON endpoint agree on state by
// construction — one code path, two renderings, not two parallel state machines.
enum LicenseOutcome {
    Confirmed {
        license_key: String,
        product_id: String,
        confirmed_at: String,
        customer_ref: String,
    },
    Pending {
        retry_after: u64,
    },
    NotFound,
}

async fn resolve_license(state: &AppState, tx_hash: &str) -> LicenseOutcome {
    // 1. Check local receipt file (idempotent replay of a prior confirmation).
    let rpath = receipt_path(&state.receipts_dir, tx_hash);
    if rpath.exists() {
        if let Ok(raw) = fs::read_to_string(&rpath) {
            if let Ok(receipt) = serde_json::from_str::<LicenseReceipt>(&raw) {
                return LicenseOutcome::Confirmed {
                    license_key: receipt.license_key,
                    product_id: receipt.product_id,
                    confirmed_at: receipt.confirmed_at,
                    customer_ref: receipt.customer_ref,
                };
            }
        }
    }

    // 2. No receipt on file — verify on-chain via the tool-wallet subprocess.
    match run_tool_wallet_check(
        &state.tool_wallet_bin,
        tx_hash,
        &state.polygon_rpc_url,
        &state.polygon_wallet_address,
    ) {
        WalletCheck::Confirmed(check_json) => {
            let customer_ref = check_json
                .get("from")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let block_number = check_json
                .get("block")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);

            let price_units = price_units_from_check_json(&check_json);
            let catalog = load_catalog(&state.catalog_path).ok();
            let product_id = catalog
                .as_ref()
                .and_then(|c| match_license_product_id(c, price_units))
                .unwrap_or_else(|| format!("unknown-{price_units}"));

            let license_key = generate_license_key(&product_id, tx_hash, &customer_ref);
            let confirmed_at = Utc::now().to_rfc3339();

            let receipt = LicenseReceipt {
                product_id: product_id.clone(),
                version: "0.0.1".into(),
                customer_ref: customer_ref.clone(),
                price_usdc: price_units,
                tx_hash: tx_hash.to_string(),
                chain: "polygon-pos".into(),
                confirmed_at: confirmed_at.clone(),
                block_number,
                license_key: license_key.clone(),
            };

            if let Some(parent) = rpath.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if let Ok(raw) = serde_json::to_string_pretty(&receipt) {
                let _ = fs::write(&rpath, raw);
            }

            flag_if_first_live_transaction(&state.receipts_dir, &receipt);

            // Phase 5: CRA barter-transaction log — only on a genuinely fresh
            // confirmation (this branch), never a receipt-cache replay.
            if let Some(installer) = catalog
                .as_ref()
                .and_then(|c| c.installers.iter().find(|i| i.id == product_id))
            {
                append_tx_log(
                    &state.tx_log_path,
                    &receipt,
                    &installer.edition,
                    installer.license_tier.label(),
                    state.usdc_cad_spot_rate,
                );
            }

            LicenseOutcome::Confirmed {
                license_key,
                product_id,
                confirmed_at,
                customer_ref,
            }
        }
        WalletCheck::Pending => LicenseOutcome::Pending { retry_after: 30 },
        WalletCheck::NotFound => LicenseOutcome::NotFound,
    }
}

// ── Download-token minting (Phase 2) ─────────────────────────────────────────
//
// Mints a real, Ed25519-signed download-auth token matching
// `app-privategit-source-2`'s `LicensePayload` shape byte-for-byte (same field
// names/types, same `base64url_no_pad(sig[64] || payload_json)` wire format) —
// closes the previously-undiscovered gap where nothing in production ever
// actually minted one. Uses the mechanism `BRIEF-software-distribution-
// substrate.md` already specifies ("download delivery: time-limited URL"), not
// an invented single-use/revocation scheme: `channel_expiry` is set to TODAY, so
// the link is valid through the end of the day it was minted. A later visit to
// `/order/:tx_hash` mints a fresh token with that day's date — no source-2
// changes, no new persistent state.
#[derive(Serialize)]
struct LicensePayload {
    product: String,
    channel_expiry: String,
    entitlements: Vec<String>,
    version_floor: Option<String>,
}

/// Load the marketplace's private signing key from `SIGNING_KEY_SECRET` — same
/// hex-seed-or-file-path convention as `app-privategit-source-2`'s
/// `load_verify_key`, so the two crates' key-provisioning stories match.
fn load_signing_key(val: &str) -> Option<SigningKey> {
    let hex_str = if val.len() == 64 && val.chars().all(|c| c.is_ascii_hexdigit()) {
        val.to_string()
    } else {
        fs::read_to_string(val).ok()?.trim().to_string()
    };
    let bytes = hex::decode(&hex_str).ok()?;
    let arr: [u8; 32] = bytes.try_into().ok()?;
    Some(SigningKey::from_bytes(&arr))
}

/// Mint a fresh download-auth token for `product_id`, valid through the end of
/// today (see module doc above). The hardcoded `"linux-x86_64"` platform and
/// single `"binary"` entitlement are known simplifications for this phase — real
/// platform selection is bigger scope than this cleanup.
fn mint_license_token(signing_key: &SigningKey, product_id: &str) -> String {
    let today = Utc::now().format("%Y-%m-%d").to_string();
    let payload = LicensePayload {
        product: product_id.to_string(),
        channel_expiry: today,
        entitlements: vec!["binary".to_string()],
        version_floor: None,
    };
    let payload_json = serde_json::to_string(&payload).expect("LicensePayload always serializes");
    let sig = signing_key.sign(payload_json.as_bytes());
    let mut bytes = sig.to_bytes().to_vec();
    bytes.extend_from_slice(payload_json.as_bytes());
    URL_SAFE_NO_PAD.encode(bytes)
}

// ── Handlers ──────────────────────────────────────────────────────────────────

// GET / -> 302 Found redirect to /software.
//
// Note: axum's `Redirect::to` emits 303 See Other, not 302. The P1 contract
// specifies 302, so we build the response explicitly with StatusCode::FOUND.
async fn root() -> Response {
    (StatusCode::FOUND, [(header::LOCATION, "/software")]).into_response()
}

// GET /software — dynamic product catalog.
//
// Replaces the P1 static-HTML read (`software.html` + `wrap_static_html`). The page is
// now rendered from the SAME `Catalog` that `v1_products` loads, so the product cards
// can never drift from `products.yaml` again (the bug this phase fixes). The Sovereign
// Editorial chrome is supplied by `ui::render_page`.
async fn software_page(State(state): State<Arc<AppState>>) -> Response {
    match load_catalog(&state.catalog_path) {
        Ok(catalog) => {
            let content = ui::catalog_markup(&catalog, &state.source_base_url);
            let body = ui::render_page(
                SoftwareSurface::Marketplace,
                "Products — PointSav Software",
                content,
            )
            .into_string();
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
                body,
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!("catalog load failed for /software: {e:#}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                "catalog unavailable",
            )
                .into_response()
        }
    }
}

// GET /licensing — UNCHANGED. Static legal/terms document (not catalog data): keeps the
// P1 static-file read + P2 chrome-wrap exactly as-is. Phase 4 rewrote the file's
// CONTENT (dropping fictional wallet-connect/tax/fake-product copy) but not this
// handler's logic.
async fn licensing_page(State(state): State<Arc<AppState>>) -> Response {
    serve_chrome_page(&state.static_dir.join("licensing.html"))
}

// GET /pricing — Phase 4. Catalog-driven (like `software_page`), not a static file,
// so it can never drift into fiction the way `licensing.html` had.
async fn pricing_page(State(state): State<Arc<AppState>>) -> Response {
    match load_catalog(&state.catalog_path) {
        Ok(catalog) => {
            let content = ui::pricing_markup(&catalog);
            let body = ui::render_page(
                SoftwareSurface::Marketplace,
                "Pricing — PointSav Software",
                content,
            )
            .into_string();
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
                body,
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!("catalog load failed for /pricing: {e:#}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                "catalog unavailable",
            )
                .into_response()
        }
    }
}

// GET /page/disclaimer — self-contained disclaimer page (operator instruction 2026-07-02:
// this site has its own content, no cross-site links out to the wiki/marketing sites'
// disclaimers). Content is a compile-time constant (`ui::disclaimer::disclaimer_markup`),
// not read from disk, so there is no "file missing" error path to handle here.
async fn disclaimer_page() -> Response {
    let body = ui::render_page(
        SoftwareSurface::Marketplace,
        "Disclaimer — PointSav Software",
        ui::disclaimer_markup(),
    )
    .into_string();
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        body,
    )
        .into_response()
}

// GET /page/privacy — self-contained privacy page, same pattern as disclaimer_page.
async fn privacy_page() -> Response {
    let body = ui::render_page(
        SoftwareSurface::Marketplace,
        "Privacy — PointSav Software",
        ui::privacy_markup(),
    )
    .into_string();
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        body,
    )
        .into_response()
}

// GET /page/accessibility — self-contained accessibility page, same pattern as disclaimer_page.
async fn accessibility_page() -> Response {
    let body = ui::render_page(
        SoftwareSurface::Marketplace,
        "Accessibility — PointSav Software",
        ui::accessibility_markup(),
    )
    .into_string();
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        body,
    )
        .into_response()
}

// GET /page/contact — self-contained contact page, same pattern as disclaimer_page.
// Closes the highest-priority finding from the original Sovereign Editorial audit
// (this route previously did not exist at all — HTTP 0, flagged twice).
async fn contact_page() -> Response {
    let body = ui::render_page(
        SoftwareSurface::Marketplace,
        "Contact us — PointSav Software",
        ui::contact_markup(),
    )
    .into_string();
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        body,
    )
        .into_response()
}

// GET /software/:product_id — S136 product detail page. Mirrors checkout_page's
// catalog-lookup + 404/500 shape exactly.
async fn product_detail_page(
    State(state): State<Arc<AppState>>,
    Path(product_id): Path<String>,
) -> Response {
    match load_catalog(&state.catalog_path) {
        Ok(catalog) => match catalog.installers.iter().find(|i| i.id == product_id) {
            Some(installer) => {
                let content = ui::product_detail_markup(installer, &state.source_base_url);
                let body = ui::render_page(
                    SoftwareSurface::Marketplace,
                    &format!("{} — PointSav Software", installer.name),
                    content,
                )
                .into_string();
                (
                    StatusCode::OK,
                    [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
                    body,
                )
                    .into_response()
            }
            None => (
                StatusCode::NOT_FOUND,
                [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                "product not found",
            )
                .into_response(),
        },
        Err(e) => {
            tracing::error!("catalog load failed for /software/:id: {e:#}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                "catalog unavailable",
            )
                .into_response()
        }
    }
}

// Read the prerendered static page from disk (P1 logic, unchanged) and wrap it in
// the Sovereign Editorial chrome (navy masthead + near-black footer) before serving.
fn serve_chrome_page(path: &PathBuf) -> Response {
    match fs::read_to_string(path) {
        Ok(raw) => {
            let body = ui::wrap_static_html(&raw, SoftwareSurface::Marketplace);
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
                body,
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!("failed to read static page {}: {e}", path.display());
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                "page unavailable",
            )
                .into_response()
        }
    }
}

async fn healthz() -> Json<Value> {
    Json(json!({"status": "ok", "service": "app-privategit-marketplace"}))
}

async fn v1_products(State(state): State<Arc<AppState>>) -> (StatusCode, Json<Value>) {
    match load_catalog(&state.catalog_path) {
        Ok(catalog) => {
            // No compatibility view needed: this crate has not launched (P8 pending),
            // so there are no external JSON-API consumers of the old `installers`/
            // `licenses` split to protect. One unified shape, reflecting the
            // corrected data model directly.
            let installers: Vec<Value> = catalog
                .installers
                .iter()
                .map(|i| {
                    json!({
                        "id": i.id,
                        "name": i.name,
                        "description": i.description,
                        "edition": i.edition,
                        "platform": i.platform,
                        "size_mb": i.size_mb,
                        "download_url": format!("{}/{}", state.source_base_url, i.path),
                        "manifest_url": format!("{}/{}/MANIFEST", state.source_base_url, i.path),
                        "license_tier": i.license_tier.label(),
                        "price_usdc": i.price_usdc,
                        "cost": if i.price_usdc == 0 { "free" } else { "paid" },
                        "payment_address": state.polygon_wallet_address,
                        "payment_chain": "polygon-pos",
                        "payment_token": "USDC"
                    })
                })
                .collect();
            (StatusCode::OK, Json(json!({"installers": installers})))
        }
        Err(e) => {
            tracing::error!("catalog load failed: {e:#}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "catalog unavailable"})),
            )
        }
    }
}

// GET /v1/license/:tx_hash — payment state machine (receipt cache → tool-wallet check).
//
// Phase 2: the state machine itself now lives in `resolve_license`, shared with the
// new `/order/:tx_hash` HTML page — this handler is just the JSON rendering of that
// shared result. Behavior/response shapes are unchanged from before the refactor.
async fn v1_license(
    State(state): State<Arc<AppState>>,
    Path(tx_hash): Path<String>,
) -> (StatusCode, Json<Value>) {
    let tx_hash = tx_hash.to_lowercase();
    match resolve_license(&state, &tx_hash).await {
        LicenseOutcome::Confirmed {
            license_key,
            product_id,
            confirmed_at,
            customer_ref,
        } => confirmed_response(&license_key, &product_id, &confirmed_at, &customer_ref),
        LicenseOutcome::Pending { retry_after } => (
            StatusCode::ACCEPTED,
            Json(json!({
                "status": "pending",
                "retry_after": retry_after,
                "message": "Transaction not yet confirmed on Polygon. Retry in 30 seconds."
            })),
        ),
        LicenseOutcome::NotFound => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "status": "not_found",
                "message": "Transaction not found or not a recognised USDC payment to this address."
            })),
        ),
    }
}

#[derive(Debug, Deserialize)]
struct ClaimRequest {
    binary_sha256: String,
    wallet_address: String,
}

// POST /v1/claim — placeholder token issuance (on-chain mint arrives v0.0.2). Ported
// as-is from the OLD crate; not made "more real" in this phase.
async fn v1_claim(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ClaimRequest>,
) -> (StatusCode, Json<Value>) {
    let claimed_at = Utc::now().to_rfc3339();
    let token = hex::encode(Sha256::digest(
        format!(
            "{}|{}|{}",
            req.binary_sha256, req.wallet_address, claimed_at
        )
        .as_bytes(),
    ));

    let claim_dir = state
        .claims_dir
        .join(req.wallet_address.trim_start_matches("0x"));
    let _ = fs::create_dir_all(&claim_dir);
    let short = &req.binary_sha256[..16.min(req.binary_sha256.len())];
    let claim_file = claim_dir.join(format!("{short}.json"));
    let payload = json!({
        "token": token,
        "binary_sha256": req.binary_sha256,
        "wallet_address": req.wallet_address,
        "claimed_at": claimed_at
    });
    let _ = fs::write(
        claim_file,
        serde_json::to_string_pretty(&payload).unwrap_or_default(),
    );

    (
        StatusCode::OK,
        Json(json!({
            "token": token,
            "claimed_at": claimed_at,
            "status": "ok",
            "note": "on-chain mint arrives v0.0.2"
        })),
    )
}

// GET /v1/wallet/address — the receiving wallet + chain/token/contract descriptor.
// The USDC contract is a hardcoded public constant (native USDC on Polygon PoS).
async fn v1_wallet_address(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({
        "address": state.polygon_wallet_address,
        "chain": "polygon-pos",
        "token": "USDC",
        "contract": "0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359"
    }))
}

// ── Phase 2: checkout / order handlers ───────────────────────────────────────

// GET /checkout/:product_id — invoice page for one product, one fixed price.
async fn checkout_page(
    State(state): State<Arc<AppState>>,
    Path(product_id): Path<String>,
) -> Response {
    match load_catalog(&state.catalog_path) {
        Ok(catalog) => match catalog.installers.iter().find(|i| i.id == product_id) {
            Some(installer) => {
                let content = ui::checkout_markup(installer, &state.polygon_wallet_address);
                let body = ui::render_page(
                    SoftwareSurface::Marketplace,
                    &format!("Checkout — {} — PointSav Software", installer.name),
                    content,
                )
                .into_string();
                (
                    StatusCode::OK,
                    [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
                    body,
                )
                    .into_response()
            }
            None => (
                StatusCode::NOT_FOUND,
                [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                "product not found",
            )
                .into_response(),
        },
        Err(e) => {
            tracing::error!("catalog load failed for /checkout: {e:#}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                "catalog unavailable",
            )
                .into_response()
        }
    }
}

#[derive(Deserialize)]
struct OrderRedirectQuery {
    product: String,
    tx_hash: String,
}

// GET /order?product=<id>&tx_hash=<value> — the checkout form's submit target.
// Redirects to the canonical, bookmarkable /order/:tx_hash?product=<id> so the
// order page has one durable URL regardless of how the customer arrived at it.
async fn order_redirect(Query(q): Query<OrderRedirectQuery>) -> Response {
    let tx_hash = q.tx_hash.trim().to_lowercase();
    Redirect::to(&format!("/order/{tx_hash}?product={}", q.product)).into_response()
}

#[derive(Deserialize, Default)]
struct OrderQuery {
    product: Option<String>,
}

// GET /order/:tx_hash — status/entitlement page. Renders whichever of the three
// `LicenseOutcome` states `resolve_license` returns; the confirmed state shows
// the receipt and a Download link. `?product=` (carried from checkout) is used
// only for the not-found state's "back to checkout" link — the confirmed state
// already knows its own product_id from the resolved receipt.
async fn order_status_page(
    State(state): State<Arc<AppState>>,
    Path(tx_hash): Path<String>,
    Query(q): Query<OrderQuery>,
) -> Response {
    let tx_hash = tx_hash.to_lowercase();
    let content = match resolve_license(&state, &tx_hash).await {
        LicenseOutcome::Confirmed {
            license_key,
            product_id,
            ..
        } => ui::order_confirmed_markup(&tx_hash, &license_key, &product_id),
        LicenseOutcome::Pending { retry_after } => ui::order_pending_markup(&tx_hash, retry_after),
        LicenseOutcome::NotFound => ui::order_not_found_markup(&tx_hash, q.product.as_deref()),
    };
    let body = ui::render_page(
        SoftwareSurface::Marketplace,
        "Order status — PointSav Software",
        content,
    )
    .into_string();
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        body,
    )
        .into_response()
}

// GET /order/:tx_hash/download?product=<id> — mints a fresh download-auth token
// (see `mint_license_token` doc comment) and redirects to the authenticated
// app-privategit-source-2 URL. Re-resolves the tx_hash rather than trusting the
// query param alone, so a customer can't mint a token for a product they didn't
// actually pay for by editing the URL.
async fn order_download(
    State(state): State<Arc<AppState>>,
    Path(tx_hash): Path<String>,
    Query(q): Query<OrderQuery>,
) -> Response {
    let tx_hash = tx_hash.to_lowercase();
    let product_id = match resolve_license(&state, &tx_hash).await {
        LicenseOutcome::Confirmed { product_id, .. } => product_id,
        _ => {
            return (
                StatusCode::FORBIDDEN,
                [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                "payment not confirmed for this order",
            )
                .into_response();
        }
    };
    // Defensive check: if the caller supplied ?product=, it must match the
    // resolved receipt's product — catches a stale/copy-pasted link, doesn't
    // trust the query param as the source of truth.
    if let Some(claimed) = &q.product {
        if claimed != &product_id {
            return (
                StatusCode::BAD_REQUEST,
                [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                "product mismatch for this order",
            )
                .into_response();
        }
    }
    let Some(signing_key) = &state.signing_key else {
        tracing::warn!(product_id = %product_id, "order-download: SIGNING_KEY_SECRET not set");
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            "download signing not configured",
        )
            .into_response();
    };
    let edition = load_catalog(&state.catalog_path)
        .ok()
        .and_then(|c| {
            c.installers
                .iter()
                .find(|i| i.id == product_id)
                .map(|i| i.edition.clone())
        })
        .unwrap_or_else(|| "latest".to_string());
    let token = mint_license_token(signing_key, &product_id);
    // Platform hardcoded to linux-x86_64 — see `mint_license_token` doc comment.
    let target = format!(
        "{}/{}/{}/linux-x86_64?token={}",
        state.source_base_url, product_id, edition, token
    );
    Redirect::to(&target).into_response()
}

// ── Main ──────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()))
        .init();

    // P1 testing binds to a test port. Production port (9202) is owned by the
    // live app-privategit-marketplace service and must not be touched here.
    let bind_addr = std::env::var("MARKETPLACE_BIND").unwrap_or_else(|_| "127.0.0.1:9202".into());
    let catalog_path = PathBuf::from(
        std::env::var("CATALOG_PATH")
            .unwrap_or_else(|_| "/var/lib/local-software/catalog/products.yaml".into()),
    );
    let static_dir = PathBuf::from(std::env::var("STATIC_DIR").unwrap_or_else(|_| "static".into()));
    let polygon_wallet_address = std::env::var("POLYGON_WALLET_ADDRESS").unwrap_or_default();
    let receipts_dir = PathBuf::from(
        std::env::var("RECEIPTS_DIR").unwrap_or_else(|_| "/var/lib/local-software/receipts".into()),
    );
    let claims_dir = PathBuf::from(
        std::env::var("CLAIMS_DIR").unwrap_or_else(|_| "/var/lib/local-software/claims".into()),
    );
    let source_base_url = std::env::var("SOURCE_BASE_URL")
        .unwrap_or_else(|_| "https://software.pointsav.com/releases".into());
    let polygon_rpc_url =
        std::env::var("POLYGON_RPC_URL").unwrap_or_else(|_| "https://polygon-rpc.com".into());
    let tool_wallet_bin = std::env::var("TOOL_WALLET_BIN").unwrap_or_else(|_| "tool-wallet".into());
    let signing_key = std::env::var("SIGNING_KEY_SECRET")
        .ok()
        .and_then(|path| load_signing_key(&path));
    if signing_key.is_none() {
        tracing::warn!("SIGNING_KEY_SECRET not set — /order/:tx_hash/download will return 503");
    }
    let tx_log_path = PathBuf::from(
        std::env::var("TX_LOG_PATH")
            .unwrap_or_else(|_| "/var/lib/local-software/tx-log.jsonl".into()),
    );
    let usdc_cad_spot_rate = std::env::var("USDC_CAD_SPOT_RATE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1.37);

    let state = Arc::new(AppState {
        catalog_path,
        bind_addr: bind_addr.clone(),
        static_dir: static_dir.clone(),
        polygon_wallet_address,
        receipts_dir,
        claims_dir,
        source_base_url,
        polygon_rpc_url,
        tool_wallet_bin,
        signing_key,
        tx_log_path,
        usdc_cad_spot_rate,
    });

    let app = Router::new()
        .route("/", get(root))
        .route("/software", get(software_page))
        .route("/software/:product_id", get(product_detail_page))
        .route("/licensing", get(licensing_page))
        .route("/pricing", get(pricing_page))
        .route("/page/disclaimer", get(disclaimer_page))
        .route("/page/privacy", get(privacy_page))
        .route("/page/accessibility", get(accessibility_page))
        .route("/page/contact", get(contact_page))
        .route("/healthz", get(healthz))
        .route("/v1/products", get(v1_products))
        .route("/v1/license/:tx_hash", get(v1_license))
        .route("/v1/claim", post(v1_claim))
        .route("/v1/wallet/address", get(v1_wallet_address))
        .route("/checkout/:product_id", get(checkout_page))
        .route("/order", get(order_redirect))
        .route("/order/:tx_hash", get(order_status_page))
        .route("/order/:tx_hash/download", get(order_download))
        .nest_service("/static", ServeDir::new(static_dir))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("strict-transport-security"),
            HeaderValue::from_static(HSTS_VALUE),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("x-content-type-options"),
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("x-frame-options"),
            HeaderValue::from_static("DENY"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("referrer-policy"),
            HeaderValue::from_static("strict-origin-when-cross-origin"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("content-security-policy"),
            HeaderValue::from_static(CSP_VALUE),
        ))
        .with_state(state);

    tracing::info!("app-privategit-marketplace-2 listening on {bind_addr}");
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────
//
// SAFETY: no test binds a TCP port (handlers are called directly) and every test
// writes ONLY under a unique scratch dir inside `std::env::temp_dir()` (`/tmp`).
// Nothing here touches `/var/lib/local-software/` or ports 9201/9202. The
// subprocess path is exercised via a locally-written JSON test-double — no real
// Polygon RPC call is ever made.
#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static SEQ: AtomicUsize = AtomicUsize::new(0);

    /// Fresh, unique scratch directory under /tmp for one test.
    fn scratch_dir(tag: &str) -> PathBuf {
        let n = SEQ.fetch_add(1, Ordering::SeqCst);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "mkt2-test-{tag}-{}-{n}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Minimal products.yaml with a realistic paid tier (os-console = $1.00 =
    /// 1_000_000 micro-USDC, os-mediakit = $19.00 = 19_000_000 micro-USDC), written
    /// into `dir`. Returns the catalog path. Prices are ACTIVE (nonzero) here
    /// deliberately, to preserve test coverage of the paid path — unlike the real
    /// products.yaml, which ships all products at price_usdc: 0 during the current
    /// BETA gate (see the `Installer::price_usdc` doc comment).
    fn write_catalog(dir: &std::path::Path) -> PathBuf {
        let path = dir.join("products.yaml");
        fs::write(
            &path,
            r#"installers:
  - id: os-console
    name: PointSav Console OS
    description: Operator Terminal Surface.
    edition: "2026.05.144"
    platform: "macOS · Win · Linux"
    size_mb: 412
    path: os-console/2026.05.144
    license_tier: agpl
    price_usdc: 1000000
  - id: os-mediakit
    name: PointSav MediaKit OS
    description: MediaKit Surface.
    edition: "2026.05.142"
    platform: "Linux server"
    size_mb: 96
    path: os-mediakit/2026.05.142
    license_tier: fsl
    price_usdc: 19000000
"#,
        )
        .unwrap();
        path
    }

    /// Write an executable bash test-double that mimics `tool-wallet check`.
    /// Branches on the (lowercased) tx_hash argument:
    ///   *confirmed* -> confirmed:true, $1.00 payment, exit 0
    ///   *pending*   -> confirmed:false, exit 0
    ///   otherwise   -> confirmed:false + exit 1 (mirrors real tool-wallet's not-found)
    fn write_tool_wallet_double(dir: &std::path::Path) -> PathBuf {
        let path = dir.join("tool-wallet-double.sh");
        fs::write(
            &path,
            r#"#!/usr/bin/env bash
# args: check <tx_hash> --rpc-url <url> --wallet-address <addr>
tx="$2"
case "$tx" in
  *confirmed*) echo '{"confirmed":true,"amount_usdc":1.00,"amount_units":1000000,"from":"0xcaffee","block":123,"tx_hash":"'"$tx"'"}'; exit 0;;
  *pending*)   echo '{"confirmed":false,"reason":"not yet mined"}'; exit 0;;
  *)           echo '{"confirmed":false,"reason":"transaction not found"}'; exit 1;;
esac
"#,
        )
        .unwrap();
        let mut perms = fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).unwrap();
        path
    }

    /// Deterministic in-test Ed25519 signing key. Never a production key — matches
    /// `app-privategit-source-2`'s own `test_signing_key()` seed convention so the
    /// two crates' test fixtures could interoperate if ever cross-checked.
    fn test_signing_key() -> SigningKey {
        SigningKey::from_bytes(&[42u8; 32])
    }

    fn test_state(scratch: &std::path::Path, tool_wallet_bin: String) -> Arc<AppState> {
        Arc::new(AppState {
            catalog_path: write_catalog(scratch),
            bind_addr: "127.0.0.1:0".into(),
            static_dir: scratch.to_path_buf(),
            polygon_wallet_address: "0xTESTWALLET".into(),
            receipts_dir: scratch.join("receipts"),
            claims_dir: scratch.join("claims"),
            source_base_url: "https://example.invalid/releases".into(),
            polygon_rpc_url: "https://rpc.invalid".into(),
            tool_wallet_bin,
            signing_key: Some(test_signing_key()),
            tx_log_path: scratch.join("tx-log.jsonl"),
            usdc_cad_spot_rate: 1.37,
        })
    }

    /// Richer products.yaml fixture for the P1–P3 tests: one BETA/free product per
    /// tier plus one actively-priced product — the full shape production can serve
    /// once a BETA gate lifts, even though the real catalog ships all-zero today.
    fn write_full_catalog(dir: &std::path::Path) -> PathBuf {
        let path = dir.join("products.yaml");
        fs::write(
            &path,
            r#"installers:
  - id: os-mediakit
    name: MediaKit OS
    description: Sovereign media workstation image.
    edition: "1.2.0"
    platform: linux-x86_64
    size_mb: 812
    path: os-mediakit/1.2.0/installer.run
    license_tier: fsl
    price_usdc: 0
  - id: os-console
    name: Console OS
    description: Operator Terminal Surface, free during BETA.
    edition: "1.0.0"
    platform: linux-x86_64
    size_mb: 300
    path: os-console/1.0.0/installer.run
    license_tier: agpl
    price_usdc: 0
  - id: os-privategit
    name: PrivateGit OS
    description: Independent code repository, priced tier (test fixture only).
    edition: "1.0.0"
    platform: linux-x86_64
    size_mb: 150
    path: os-privategit/1.0.0/installer.run
    license_tier: fsl
    price_usdc: 1000000
"#,
        )
        .unwrap();
        path
    }

    /// State whose catalog path is caller-supplied (P1–P3 tests; never shells out,
    /// so `tool_wallet_bin` is the PATH default and irrelevant).
    fn test_state_at(scratch: &std::path::Path, catalog_path: PathBuf) -> Arc<AppState> {
        Arc::new(AppState {
            catalog_path,
            bind_addr: "127.0.0.1:0".into(),
            static_dir: scratch.to_path_buf(),
            polygon_wallet_address: "0xTESTWALLET".into(),
            receipts_dir: scratch.join("receipts"),
            claims_dir: scratch.join("claims"),
            source_base_url: "https://example.invalid/releases".into(),
            polygon_rpc_url: "https://rpc.invalid".into(),
            tool_wallet_bin: "tool-wallet".into(),
            signing_key: Some(test_signing_key()),
            tx_log_path: scratch.join("tx-log.jsonl"),
            usdc_cad_spot_rate: 1.37,
        })
    }

    fn test_state_full(scratch: &std::path::Path) -> Arc<AppState> {
        let catalog_path = write_full_catalog(scratch);
        test_state_at(scratch, catalog_path)
    }

    /// Collect a handler `Response` body into a String (handlers are called
    /// directly — no TCP port is ever bound).
    async fn body_text(body: axum::body::Body) -> String {
        let bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();
        String::from_utf8(bytes.to_vec()).unwrap()
    }

    // ── Pure functions ────────────────────────────────────────────────────────

    #[test]
    fn license_key_construction_is_exact() {
        // Must stay byte-identical to the OLD binary + tool-wallet.
        let key = generate_license_key("apache", "0xabc", "0xcaffee");
        let full = hex::encode(Sha256::digest(b"apache:0xabc:0xcaffee"));
        let expected = format!(
            "{}-{}-{}-{}",
            &full[0..8],
            &full[8..16],
            &full[16..24],
            &full[24..32]
        );
        assert_eq!(key, expected);
        // Shape: four hyphen-joined 8-hex-char groups.
        let parts: Vec<&str> = key.split('-').collect();
        assert_eq!(parts.len(), 4);
        assert!(parts
            .iter()
            .all(|p| p.len() == 8 && p.chars().all(|c| c.is_ascii_hexdigit())));
    }

    #[test]
    fn price_units_conversion_is_unchanged() {
        // Dollars -> micro-USDC. THIS conversion is correct and not the bug.
        assert_eq!(price_units_from_amount(1.00), 1_000_000);
        assert_eq!(price_units_from_amount(19.00), 19_000_000);
    }

    /// THE reviewable proof: for a realistic $1.00 os-console-tier payment, the OLD
    /// formula fails to match and the NEW (fixed) formula matches. Catalog
    /// `price_usdc` is already micro-USDC (os-console = 1_000_000).
    #[test]
    fn pricing_fix_old_formula_fails_new_formula_matches() {
        let scratch = scratch_dir("pricematch");
        let catalog = load_catalog(&write_catalog(&scratch)).unwrap();

        // A confirmed $1.00 payment -> 1_000_000 micro-USDC.
        let price_units = price_units_from_amount(1.00);
        assert_eq!(price_units, 1_000_000);

        // OLD (buggy) matcher, reproduced verbatim for the before/after proof:
        //   i.price_usdc * 1_000_000 == price_units
        // os-console.price_usdc (1_000_000) * 1_000_000 = 1_000_000_000_000 != 1_000_000.
        let old_match: Option<String> = catalog
            .installers
            .iter()
            .find(|i| i.price_usdc * 1_000_000 == price_units)
            .map(|i| i.id.clone());
        assert_eq!(
            old_match, None,
            "OLD formula must FAIL to match a real $1.00 payment (this was the bug)"
        );

        // NEW (fixed) matcher: direct micro-unit equality.
        let new_match = match_license_product_id(&catalog, price_units);
        assert_eq!(
            new_match,
            Some("os-console".to_string()),
            "NEW formula must match os-console for a $1.00 payment"
        );

        // Sanity: $19.00 -> os-mediakit under the fix as well.
        assert_eq!(
            match_license_product_id(&catalog, price_units_from_amount(19.00)),
            Some("os-mediakit".to_string())
        );

        let _ = fs::remove_dir_all(&scratch);
    }

    /// Checkpoint 2 review finding: the float round-trip in
    /// `price_units_from_amount` is lossy for many whole-cent prices. $2.01 is a
    /// concrete failure case — verify it, then verify `price_units_from_check_json`
    /// avoids it by preferring the exact `amount_units` integer.
    #[test]
    fn amount_units_precision_fix() {
        // The lossy float path: tool-wallet's own display conversion
        // (amount_units as f64 / 1_000_000.0) fed back through
        // price_units_from_amount does NOT reliably round-trip.
        let amount_units: u64 = 2_010_000; // a genuine $2.01 payment
        let amount_usdc = amount_units as f64 / 1_000_000.0;
        let float_roundtrip = price_units_from_amount(amount_usdc);
        assert_ne!(
            float_roundtrip, amount_units,
            "float round-trip must reproduce the known $2.01 precision loss \
             (if this now passes, tool-wallet's amount_usdc formatting or Rust's \
             float behavior changed — re-verify the exact-integer path is still \
             the one actually used in production before relaxing this test)"
        );

        // The fixed path: given tool-wallet's real response shape (both fields
        // present, as it always emits), the exact integer wins.
        let check_json = json!({
            "confirmed": true,
            "amount_usdc": amount_usdc,
            "amount_units": amount_units,
            "from": "0xbuyer",
            "block": 1
        });
        assert_eq!(
            price_units_from_check_json(&check_json),
            amount_units,
            "must use the exact amount_units field, not the lossy float"
        );

        // Defensive fallback: if amount_units is ever absent, the float path is
        // still exercised (documented limitation, not silently broken).
        let check_json_no_units = json!({
            "confirmed": true,
            "amount_usdc": 1.00,
            "from": "0xbuyer",
            "block": 1
        });
        assert_eq!(price_units_from_check_json(&check_json_no_units), 1_000_000);
    }

    // ── Handler: receipt-cache path ───────────────────────────────────────────

    #[tokio::test]
    async fn license_confirmed_via_existing_receipt() {
        let scratch = scratch_dir("receipt");
        // tool_wallet_bin points at a non-existent binary: this path must NOT shell out.
        let state = test_state(&scratch, "/nonexistent/tool-wallet".into());

        let tx = "0xdeadbeefreceipt";
        let rpath = receipt_path(&state.receipts_dir, tx);
        fs::create_dir_all(rpath.parent().unwrap()).unwrap();
        // Fixture INCLUDES `license_tier` — the extra field tool-wallet writes.
        // It must be ignored on read (proves cross-binary receipt compatibility).
        fs::write(
            &rpath,
            r#"{
  "product_id": "apache",
  "license_tier": "apache",
  "version": "0.0.1",
  "customer_ref": "0xcaffee",
  "price_usdc": 1000000,
  "tx_hash": "0xdeadbeefreceipt",
  "chain": "polygon-pos",
  "confirmed_at": "2026-07-01T00:00:00+00:00",
  "block_number": 123,
  "license_key": "aaaaaaaa-bbbbbbbb-cccccccc-dddddddd"
}"#,
        )
        .unwrap();

        let (status, Json(body)) = v1_license(State(state.clone()), Path(tx.to_string())).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["status"], "confirmed");
        assert_eq!(body["product_id"], "apache");
        assert_eq!(body["license_key"], "aaaaaaaa-bbbbbbbb-cccccccc-dddddddd");
        assert_eq!(body["customer_ref"], "0xcaffee");

        // Phase 5: a receipt-cache replay is NOT a new sale — must not append a
        // tx-log row (only the fresh-confirmation path does).
        assert!(
            !state.tx_log_path.exists(),
            "receipt-cache replay must not write to the tx log"
        );

        let _ = fs::remove_dir_all(&scratch);
    }

    // ── Handler: fresh tool-wallet check (mocked) ─────────────────────────────

    #[tokio::test]
    async fn license_confirmed_via_fresh_check_matches_apache_and_writes_receipt() {
        let scratch = scratch_dir("fresh");
        let double = write_tool_wallet_double(&scratch);
        let state = test_state(&scratch, double.to_string_lossy().into_owned());

        let tx = "0xconfirmedpayment01";
        let (status, Json(body)) = v1_license(State(state.clone()), Path(tx.to_string())).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["status"], "confirmed");
        // The FIX in action end-to-end: $1.00 -> 1_000_000 -> catalog "os-console".
        assert_eq!(body["product_id"], "os-console");
        assert_eq!(body["customer_ref"], "0xcaffee");
        let expected_key = generate_license_key("os-console", tx, "0xcaffee");
        assert_eq!(body["license_key"], expected_key);

        // A receipt must have been written to the SCRATCH dir (never /var/lib).
        let rpath = receipt_path(&state.receipts_dir, tx);
        assert!(
            rpath.exists(),
            "receipt should be persisted to scratch receipts dir"
        );
        assert!(rpath.starts_with(&scratch));
        let written: LicenseReceipt =
            serde_json::from_str(&fs::read_to_string(&rpath).unwrap()).unwrap();
        assert_eq!(written.product_id, "os-console");
        assert_eq!(written.price_usdc, 1_000_000); // micro-USDC, not dollars

        // Phase 5: a fresh confirmation must append exactly one correctly-shaped
        // tx-log.jsonl row (the CRA barter-transaction record).
        let log_contents = fs::read_to_string(&state.tx_log_path).unwrap();
        let lines: Vec<&str> = log_contents.lines().collect();
        assert_eq!(lines.len(), 1, "exactly one row for one fresh confirmation");
        let row: Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(row["sku"], "os-console@2026.05.144");
        assert_eq!(row["license_tier"], "AGPL-3.0-or-later");
        assert_eq!(row["crypto_received"], "1.00 USDC");
        assert_eq!(row["polygon_tx"], tx);
        assert_eq!(row["spot_rate_cad"], "1.37");
        assert_eq!(row["cad_equivalent"], "1.37");

        let _ = fs::remove_dir_all(&scratch);
    }

    // Checkpoint 3b deferral (operator decision 2026-07-02): the first real transaction
    // through this crate must be flagged distinctly for after-the-fact manual review.
    #[tokio::test]
    async fn first_real_confirmation_writes_marker_second_does_not() {
        let scratch = scratch_dir("firstlive");
        let double = write_tool_wallet_double(&scratch);
        let state = test_state(&scratch, double.to_string_lossy().into_owned());

        let marker = first_live_transaction_marker_path(&state.receipts_dir);
        assert!(!marker.exists(), "no marker before any transaction");

        // First confirmed transaction -> marker written.
        let (status, _) = v1_license(
            State(state.clone()),
            Path("0xconfirmedpayment01".to_string()),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(
            marker.exists(),
            "marker must exist after the first confirmation"
        );
        let recorded: LicenseReceipt =
            serde_json::from_str(&fs::read_to_string(&marker).unwrap()).unwrap();
        assert_eq!(recorded.tx_hash, "0xconfirmedpayment01");
        let first_marker_mtime = fs::metadata(&marker).unwrap().modified().unwrap();

        // A second, DIFFERENT confirmed transaction must NOT overwrite the marker --
        // it stays pointing at the first one.
        let (status2, _) = v1_license(
            State(state.clone()),
            Path("0xconfirmedpayment19".to_string()),
        )
        .await;
        assert_eq!(status2, StatusCode::OK);
        let still_recorded: LicenseReceipt =
            serde_json::from_str(&fs::read_to_string(&marker).unwrap()).unwrap();
        assert_eq!(
            still_recorded.tx_hash, "0xconfirmedpayment01",
            "marker must still point at the FIRST transaction, not be overwritten by the second"
        );
        assert_eq!(
            fs::metadata(&marker).unwrap().modified().unwrap(),
            first_marker_mtime,
            "marker file must not be rewritten on the second transaction"
        );

        let _ = fs::remove_dir_all(&scratch);
    }

    #[tokio::test]
    async fn license_pending_when_check_unconfirmed() {
        let scratch = scratch_dir("pending");
        let double = write_tool_wallet_double(&scratch);
        let state = test_state(&scratch, double.to_string_lossy().into_owned());

        let (status, Json(body)) =
            v1_license(State(state), Path("0xpendingtx01".to_string())).await;
        assert_eq!(status, StatusCode::ACCEPTED);
        assert_eq!(body["status"], "pending");
        assert_eq!(body["retry_after"], 30);

        let _ = fs::remove_dir_all(&scratch);
    }

    #[tokio::test]
    async fn license_not_found_when_check_fails() {
        let scratch = scratch_dir("notfound");
        let double = write_tool_wallet_double(&scratch);
        let state = test_state(&scratch, double.to_string_lossy().into_owned());

        // Neither *confirmed* nor *pending* -> test-double exits 1 -> NotFound.
        let (status, Json(body)) =
            v1_license(State(state), Path("0xunknowntx01".to_string())).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["status"], "not_found");

        let _ = fs::remove_dir_all(&scratch);
    }

    #[tokio::test]
    async fn not_found_when_tool_wallet_binary_missing() {
        let scratch = scratch_dir("nobin");
        let state = test_state(&scratch, "/nonexistent/tool-wallet".into());

        let (status, Json(body)) = v1_license(State(state), Path("0xanytx01".to_string())).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["status"], "not_found");

        let _ = fs::remove_dir_all(&scratch);
    }

    // ── Handler: wallet address + claim ───────────────────────────────────────

    #[tokio::test]
    async fn wallet_address_shape_and_hardcoded_contract() {
        let scratch = scratch_dir("wallet");
        let state = test_state(&scratch, "tool-wallet".into());
        let Json(body) = v1_wallet_address(State(state)).await;
        assert_eq!(body["address"], "0xTESTWALLET");
        assert_eq!(body["chain"], "polygon-pos");
        assert_eq!(body["token"], "USDC");
        assert_eq!(
            body["contract"],
            "0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359"
        );
        let _ = fs::remove_dir_all(&scratch);
    }

    #[tokio::test]
    async fn claim_writes_placeholder_and_returns_ok() {
        let scratch = scratch_dir("claim");
        let state = test_state(&scratch, "tool-wallet".into());
        let req = ClaimRequest {
            binary_sha256: "0123456789abcdef0123456789abcdef".into(),
            wallet_address: "0xWALLET123".into(),
        };
        let (status, Json(body)) = v1_claim(State(state.clone()), Json(req)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["status"], "ok");
        assert_eq!(body["note"], "on-chain mint arrives v0.0.2");
        assert!(body["token"].as_str().unwrap().len() == 64);
        // File written under claims/<addr-without-0x>/<first16>.json in scratch.
        let claim_file = state
            .claims_dir
            .join("WALLET123")
            .join("0123456789abcdef.json");
        assert!(claim_file.exists());
        let _ = fs::remove_dir_all(&scratch);
    }

    // ── P1: catalog loading ───────────────────────────────────────────────────

    #[test]
    fn load_catalog_parses_realistic_fixture() {
        let scratch = scratch_dir("loadcat");
        let catalog = load_catalog(&write_full_catalog(&scratch)).unwrap();

        assert_eq!(catalog.installers.len(), 3);
        let i = &catalog.installers[0];
        assert_eq!(i.id, "os-mediakit");
        assert_eq!(i.name, "MediaKit OS");
        assert_eq!(i.edition, "1.2.0");
        assert_eq!(i.platform, "linux-x86_64");
        assert_eq!(i.size_mb, 812);
        assert_eq!(i.path, "os-mediakit/1.2.0/installer.run");
        assert_eq!(i.license_tier, LicenseTier::Fsl);
        assert_eq!(i.price_usdc, 0); // active BETA gate

        assert_eq!(catalog.installers[1].id, "os-console");
        assert_eq!(catalog.installers[1].license_tier, LicenseTier::Agpl);
        assert_eq!(catalog.installers[1].price_usdc, 0); // active BETA gate

        assert_eq!(catalog.installers[2].id, "os-privategit");
        assert_eq!(catalog.installers[2].license_tier, LicenseTier::Fsl);
        assert_eq!(catalog.installers[2].price_usdc, 1_000_000); // micro-USDC

        let _ = fs::remove_dir_all(&scratch);
    }

    // ── Phase 1: Binary Library two-shelf model ───────────────────────────────
    // (BRIEF-binary-library-repositioning.md, operator-approved 2026-07-07)

    #[test]
    fn license_tier_apache_round_trips_as_apache() {
        let s = serde_yaml::to_string(&LicenseTier::Apache).unwrap();
        assert_eq!(s.trim(), "apache");
        let back: LicenseTier = serde_yaml::from_str("apache").unwrap();
        assert_eq!(back, LicenseTier::Apache);
    }

    #[test]
    fn license_tier_apache_label() {
        assert_eq!(LicenseTier::Apache.label(), "Apache-2.0 (Open Source)");
    }

    #[test]
    fn shelf_mapping_matches_two_shelf_model() {
        // Rebuilt 2026-07-07 for BRIEF-software-licensing-structure.md's four-tier
        // model: Proprietary/Fsl/Agpl are all the Commercial shelf; Apache alone is
        // the Open Source shelf. See `LicenseTier::shelf`'s doc comment.
        assert_eq!(LicenseTier::Proprietary.shelf(), Shelf::Commercial);
        assert_eq!(LicenseTier::Fsl.shelf(), Shelf::Commercial);
        assert_eq!(LicenseTier::Agpl.shelf(), Shelf::Commercial);
        assert_eq!(LicenseTier::Apache.shelf(), Shelf::OpenSource);
    }

    #[test]
    fn load_catalog_accepts_apache_installer_at_zero_price() {
        let scratch = scratch_dir("oss-ok");
        let path = scratch.join("products.yaml");
        fs::write(
            &path,
            r#"installers:
  - id: tool-wallet
    name: PointSav Wallet
    description: Polygon USDC watcher, relicensed Apache-2.0.
    edition: "1.0.0"
    platform: linux-x86_64
    size_mb: 12
    path: tool-wallet/1.0.0/installer.run
    license_tier: apache
    price_usdc: 0
"#,
        )
        .unwrap();

        let catalog = load_catalog(&path).unwrap();
        assert_eq!(catalog.installers[0].license_tier, LicenseTier::Apache);
        assert_eq!(
            catalog.installers[0].license_tier.shelf(),
            Shelf::OpenSource
        );
        assert_eq!(catalog.installers[0].price_usdc, 0);

        let _ = fs::remove_dir_all(&scratch);
    }

    #[test]
    fn load_catalog_rejects_apache_installer_with_nonzero_price() {
        let scratch = scratch_dir("oss-bad-price");
        let path = scratch.join("products.yaml");
        fs::write(
            &path,
            r#"installers:
  - id: tool-wallet
    name: PointSav Wallet
    description: Polygon USDC watcher, relicensed Apache-2.0.
    edition: "1.0.0"
    platform: linux-x86_64
    size_mb: 12
    path: tool-wallet/1.0.0/installer.run
    license_tier: apache
    price_usdc: 1000000
"#,
        )
        .unwrap();

        let err = load_catalog(&path);
        assert!(
            err.is_err(),
            "an apache-tier installer with a nonzero price must fail to load loudly, \
             not silently accept an invalid price"
        );
        assert!(err.unwrap_err().to_string().contains("tool-wallet"));

        let _ = fs::remove_dir_all(&scratch);
    }

    #[test]
    fn load_catalog_missing_file_is_err() {
        let missing = PathBuf::from("/nonexistent/mkt2-test/products.yaml");
        assert!(load_catalog(&missing).is_err());
    }

    #[test]
    fn load_catalog_malformed_or_incomplete_yaml_is_err() {
        let scratch = scratch_dir("badcat");
        let path = scratch.join("products.yaml");

        // Syntactically invalid YAML (unterminated flow sequence).
        fs::write(&path, "installers: [this is: not, valid yaml").unwrap();
        assert!(load_catalog(&path).is_err());

        // Well-formed YAML entirely missing the required `installers` field must
        // also fail (an empty `installers: []` list is now valid, not an error).
        fs::write(&path, "not_installers: []\n").unwrap();
        assert!(load_catalog(&path).is_err());

        // A legacy file still carrying the retired `licenses:` key must fail to
        // load LOUDLY (`#[serde(deny_unknown_fields)]` on `Catalog`) — not silently
        // produce an empty-but-successfully-parsed catalog. Protects against a
        // stale prod file surviving the Phase 1 migration undetected.
        fs::write(
            &path,
            "installers: []\nlicenses:\n  - id: apache\n    price_usdc: 1000000\n",
        )
        .unwrap();
        assert!(
            load_catalog(&path).is_err(),
            "a stray legacy licenses: key must fail to parse loudly, not silently \
             produce an empty installers list"
        );

        let _ = fs::remove_dir_all(&scratch);
    }

    // ── P1: /v1/products JSON shape ───────────────────────────────────────────

    /// Regression guard for the Checkpoint 1 finding: every documented field must
    /// be present on every entry — a missing field here is exactly the class of
    /// gap that diff caught.
    #[tokio::test]
    async fn v1_products_documented_field_shape() {
        let scratch = scratch_dir("products");
        let state = test_state_full(&scratch);

        let (status, Json(body)) = v1_products(State(state)).await;
        assert_eq!(status, StatusCode::OK);

        let installers = body["installers"].as_array().unwrap();
        assert_eq!(installers.len(), 3);
        for i in installers {
            for field in [
                "id",
                "name",
                "description",
                "edition",
                "platform",
                "size_mb",
                "download_url",
                "manifest_url",
                "license_tier",
                "price_usdc",
                "cost",
                "payment_address",
                "payment_chain",
                "payment_token",
            ] {
                assert!(
                    i.get(field).is_some(),
                    "installer entry missing documented field `{field}`"
                );
            }
            assert_eq!(i["payment_address"], "0xTESTWALLET");
            assert_eq!(i["payment_chain"], "polygon-pos");
            assert_eq!(i["payment_token"], "USDC");
        }

        let mediakit = &installers[0];
        assert_eq!(mediakit["id"], "os-mediakit");
        assert_eq!(mediakit["edition"], "1.2.0");
        assert_eq!(mediakit["platform"], "linux-x86_64");
        assert_eq!(mediakit["size_mb"], 812);
        assert_eq!(mediakit["license_tier"], "FSL-1.1-ALv2");
        assert_eq!(mediakit["shelf"], "commercial"); // Fsl tier -> Commercial shelf
        assert_eq!(mediakit["price_usdc"], 0);
        assert_eq!(mediakit["cost"], "free"); // active BETA gate
        assert_eq!(
            mediakit["download_url"],
            "https://example.invalid/releases/os-mediakit/1.2.0/installer.run"
        );
        assert_eq!(
            mediakit["manifest_url"],
            "https://example.invalid/releases/os-mediakit/1.2.0/installer.run/MANIFEST"
        );

        let privategit = &installers[2];
        assert_eq!(privategit["id"], "os-privategit");
        assert_eq!(privategit["license_tier"], "FSL-1.1-ALv2");
        assert_eq!(privategit["shelf"], "commercial");
        assert_eq!(privategit["price_usdc"], 1_000_000); // micro-USDC passthrough
        assert_eq!(privategit["cost"], "paid");

        let _ = fs::remove_dir_all(&scratch);
    }

    /// Binary Library Phase 3: an `apache`-tier installer must report
    /// `shelf: "open-source"` distinctly from `agpl`/`fsl`'s `shelf: "commercial"`
    /// — the JSON API's grouping must never drift from the HTML catalog's.
    #[tokio::test]
    async fn v1_products_apache_tier_reports_open_source_shelf() {
        let scratch = scratch_dir("shelf");
        let path = scratch.join("products.yaml");
        fs::write(
            &path,
            r#"installers:
  - id: tool-wallet
    name: PointSav Wallet
    description: Polygon USDC watcher, relicensed Apache-2.0.
    edition: "1.0.0"
    platform: linux-x86_64
    size_mb: 12
    path: tool-wallet/1.0.0/installer.run
    license_tier: apache
    price_usdc: 0
  - id: os-console
    name: Console OS
    description: Operator Terminal Surface.
    edition: "1.0.0"
    platform: linux-x86_64
    size_mb: 300
    path: os-console/1.0.0/installer.run
    license_tier: agpl
    price_usdc: 0
"#,
        )
        .unwrap();
        let state = test_state_at(&scratch, path);

        let (status, Json(body)) = v1_products(State(state)).await;
        assert_eq!(status, StatusCode::OK);
        let installers = body["installers"].as_array().unwrap();

        let wallet = &installers[0];
        assert_eq!(wallet["id"], "tool-wallet");
        assert_eq!(wallet["license_tier"], "Apache-2.0 (Open Source)");
        assert_eq!(wallet["shelf"], "open-source");

        let console = &installers[1];
        assert_eq!(console["id"], "os-console");
        assert_eq!(console["shelf"], "commercial");

        let _ = fs::remove_dir_all(&scratch);
    }

    #[tokio::test]
    async fn v1_products_500_when_catalog_unavailable() {
        let scratch = scratch_dir("products500");
        let state = test_state_at(&scratch, scratch.join("no-such-products.yaml"));

        let (status, Json(body)) = v1_products(State(state)).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body, json!({"error": "catalog unavailable"}));

        let _ = fs::remove_dir_all(&scratch);
    }

    // ── P3: dynamic /software catalog page ────────────────────────────────────

    #[tokio::test]
    async fn software_page_renders_dynamic_catalog_with_chrome() {
        let scratch = scratch_dir("swpage");
        let state = test_state_full(&scratch);

        let (parts, body) = software_page(State(state)).await.into_parts();
        assert_eq!(parts.status, StatusCode::OK);
        assert_eq!(
            parts.headers.get(header::CONTENT_TYPE).unwrap(),
            "text/html; charset=utf-8"
        );
        let html = body_text(body).await;

        // Catalog data actually drives the page (the drift bug this phase fixed).
        assert!(html.contains("os-mediakit"));
        assert!(html.contains("MediaKit OS"));
        assert!(html.contains("os-console"));
        assert!(html.contains("os-privategit"));
        assert!(html.contains("AGPL-3.0-or-later"));
        assert!(html.contains("FSL-1.1-ALv2"));
        assert!(
            !html.contains("Apache 2.0"),
            "must not use the factually wrong tier label"
        );
        assert!(html.contains("<title>Products — PointSav Software</title>"));

        // P2 chrome wraps the dynamic content.
        assert!(html.contains("sw-masthead"));
        assert!(html.contains(SoftwareSurface::Marketplace.trademark_line()));
        assert!(html.contains(SoftwareSurface::Marketplace.copyright_holder()));

        let _ = fs::remove_dir_all(&scratch);
    }

    #[tokio::test]
    async fn software_page_500_when_catalog_unavailable() {
        let scratch = scratch_dir("swpage500");
        let state = test_state_at(&scratch, scratch.join("no-such-products.yaml"));

        let (parts, body) = software_page(State(state)).await.into_parts();
        assert_eq!(parts.status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body_text(body).await, "catalog unavailable");

        let _ = fs::remove_dir_all(&scratch);
    }

    // ── P1/P2: /licensing static page (UNCHANGED by the P3 dynamic-catalog work) ──

    const LICENSING_FIXTURE: &str = r#"<!doctype html>
<html><head><title>Licensing</title></head>
<body>
<header class="topnav">OLD LIGHT NAV</header>
<main><h1>Licensing terms</h1><p>Static legal content, verbatim.</p></main>
<footer>OLD THIN FOOTER</footer>
</body></html>"#;

    #[tokio::test]
    async fn licensing_page_serves_static_content_with_chrome() {
        let scratch = scratch_dir("licensing");
        let state = test_state_full(&scratch);
        fs::write(state.static_dir.join("licensing.html"), LICENSING_FIXTURE).unwrap();

        let (parts, body) = licensing_page(State(state)).await.into_parts();
        assert_eq!(parts.status, StatusCode::OK);
        assert_eq!(
            parts.headers.get(header::CONTENT_TYPE).unwrap(),
            "text/html; charset=utf-8"
        );
        let html = body_text(body).await;

        // Static document content served unchanged.
        assert!(html.contains("Licensing terms"));
        assert!(html.contains("Static legal content, verbatim."));

        // Old light chrome stripped; Sovereign chrome mounted.
        assert!(!html.contains("OLD LIGHT NAV"));
        assert!(!html.contains("OLD THIN FOOTER"));
        assert!(html.contains("sw-masthead"));
        assert!(html.contains(SoftwareSurface::Marketplace.trademark_line()));

        // NOT affected by P3: no dynamic catalog cards on /licensing.
        assert!(!html.contains("sw-cat-card"));

        let _ = fs::remove_dir_all(&scratch);
    }

    #[tokio::test]
    async fn licensing_page_500_when_static_file_missing() {
        let scratch = scratch_dir("licensing500");
        let state = test_state_full(&scratch); // no licensing.html written

        let (parts, body) = licensing_page(State(state)).await.into_parts();
        assert_eq!(parts.status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body_text(body).await, "page unavailable");

        let _ = fs::remove_dir_all(&scratch);
    }

    // Self-contained disclaimer page (operator instruction 2026-07-02): no static file,
    // no disk read, no cross-site links — a compile-time constant every time.
    #[tokio::test]
    async fn disclaimer_page_serves_self_contained_content_with_chrome() {
        let (parts, body) = disclaimer_page().await.into_parts();
        assert_eq!(parts.status, StatusCode::OK);
        assert_eq!(
            parts.headers.get(header::CONTENT_TYPE).unwrap(),
            "text/html; charset=utf-8"
        );
        let html = body_text(body).await;

        // The genuinely new section (Checkpoint 3a follow-up, 2026-07-02): on-chain
        // USDC payment risk, with no precedent elsewhere in the corpus.
        assert!(html.contains("Polygon"));
        assert!(html.contains("irreversible"));

        // Not an LP-investment disclaimer -- confirms the DISCLAIMER.md securities
        // -offering sections (accredited investor exemptions, PPM references) were
        // correctly dropped, not carried forward unmodified.
        assert!(!html.contains("Private Placement Memorandum"));
        assert!(!html.contains("Accredited Investor"));

        // Sovereign chrome present (same page shell as /software and /licensing).
        assert!(html.contains("sw-masthead"));
        assert!(html.contains(SoftwareSurface::Marketplace.trademark_line()));

        // `home.pointsav.com` and `home.woodfinegroup.com` legitimately appear now
        // (2026-07-07 footer redesign, operator-approved) — the footer's "Network"
        // column links out to "PointSav Digital Systems" and "Woodfine Capital
        // Projects" on every page, including this one. That's site navigation, not
        // a legal-content cross-reference (the original concern this test's name
        // refers to — this page's own legal *text* must not duplicate or point at
        // another site's legal text, which it still doesn't), so it doesn't violate
        // the "self-contained" property.
        assert!(html.contains("home.pointsav.com"));
        assert!(html.contains("home.woodfinegroup.com"));
    }

    // Three footer pages closing the long-standing dead-link gap
    // (BRIEF-sovereign-editorial-software.md audit finding #1: /page/contact
    // returning HTTP 0). Same self-contained, no-disk-read pattern as disclaimer_page.

    #[tokio::test]
    async fn privacy_page_serves_self_contained_content_with_chrome() {
        let (parts, body) = privacy_page().await.into_parts();
        assert_eq!(parts.status, StatusCode::OK);
        assert_eq!(
            parts.headers.get(header::CONTENT_TYPE).unwrap(),
            "text/html; charset=utf-8"
        );
        let html = body_text(body).await;
        assert!(html.contains("Polygon"));
        assert!(html.contains("open.source@pointsav.com"));
        assert!(html.contains("sw-masthead"));
        assert!(html.contains(SoftwareSurface::Marketplace.trademark_line()));
    }

    #[tokio::test]
    async fn accessibility_page_serves_self_contained_content_with_chrome() {
        let (parts, body) = accessibility_page().await.into_parts();
        assert_eq!(parts.status, StatusCode::OK);
        assert_eq!(
            parts.headers.get(header::CONTENT_TYPE).unwrap(),
            "text/html; charset=utf-8"
        );
        let html = body_text(body).await;
        assert!(html.contains("WCAG 2.1"));
        assert!(html.contains("open.source@pointsav.com"));
        assert!(html.contains("sw-masthead"));
        assert!(html.contains(SoftwareSurface::Marketplace.trademark_line()));
    }

    #[tokio::test]
    async fn contact_page_serves_self_contained_content_with_chrome() {
        let (parts, body) = contact_page().await.into_parts();
        assert_eq!(parts.status, StatusCode::OK);
        assert_eq!(
            parts.headers.get(header::CONTENT_TYPE).unwrap(),
            "text/html; charset=utf-8"
        );
        let html = body_text(body).await;
        assert!(html.contains("open.source@pointsav.com"));
        assert!(html.contains("sw-masthead"));
        assert!(html.contains(SoftwareSurface::Marketplace.trademark_line()));
    }

    // ── Phase 4: GET /pricing ─────────────────────────────────────────────────

    #[tokio::test]
    async fn pricing_page_renders_catalog_driven_content_with_chrome() {
        let scratch = scratch_dir("pricing-ok");
        let state = test_state_full(&scratch);
        let resp = pricing_page(State(state)).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let html = body_text(resp.into_body()).await;
        assert!(html.contains("AGPL-3.0-or-later"));
        assert!(html.contains("FSL-1.1-ALv2"));
        assert!(html.contains("currently free during BETA"));
        assert!(html.contains("No tax collected"));
        assert!(html.contains("github.com/pointsav/pointsav-monorepo"));
        assert!(html.contains("sw-masthead"));
        let _ = fs::remove_dir_all(&scratch);
    }

    #[tokio::test]
    async fn pricing_page_500_when_catalog_unavailable() {
        let scratch = scratch_dir("pricing-500");
        let state = test_state_at(&scratch, scratch.join("no-such-products.yaml"));
        let resp = pricing_page(State(state)).await;
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let _ = fs::remove_dir_all(&scratch);
    }

    // Regression guard: the real, committed static/licensing.html must never regress
    // to the fictional wallet-connect/multi-chain/tax/fake-product content this
    // phase replaced (a real prior finding, not a hypothetical). `include_str!` is
    // compile-time and path-stable regardless of test-runner CWD.
    #[test]
    fn real_licensing_html_is_free_of_fictional_content() {
        let real = include_str!("../static/licensing.html");
        for fictional in [
            "F*KEYS CONSOLE",
            "Command Centre",
            "Business Applications",
            "BMS Bridge",
            "Peak-Load Forecast",
            "HST 13%",
            "wallet-connect",
            "auto-detected from wallet billing region",
        ] {
            assert!(
                !real.contains(fictional),
                "static/licensing.html regressed: contains fictional content `{fictional}`"
            );
        }
        // Rebuilt 2026-07-07 to match BRIEF-software-licensing-structure.md's real
        // four-tier model — "PointSav Commercial" is not a license category in that
        // BRIEF's per-product table (see `LicenseTier`'s doc comment in this file).
        assert!(real.contains("Proprietary"));
        assert!(real.contains("FSL-1.1-ALv2"));
        assert!(real.contains("AGPL-3.0-or-later"));
        assert!(real.contains("Apache-2.0"));
        // The exact bug that BRIEF explicitly flags as corrected elsewhere
        // (`os-orchestration` was wrongly FSL) must not reappear here.
        assert!(!real.contains("<code>os-orchestration</code>. These are distributed under FSL"));
        assert!(real.contains("github.com/pointsav/pointsav-monorepo"));
    }

    // ── Phase 2: GET /checkout/:product_id ────────────────────────────────────

    #[tokio::test]
    async fn checkout_page_renders_known_product() {
        let scratch = scratch_dir("checkout-ok");
        let state = test_state_full(&scratch);
        let resp = checkout_page(State(state), Path("os-privategit".to_string())).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let html = body_text(resp.into_body()).await;
        assert!(html.contains("PrivateGit OS"));
        assert!(html.contains("$1.00"));
        assert!(html.contains("0xTESTWALLET"));
        let _ = fs::remove_dir_all(&scratch);
    }

    #[tokio::test]
    async fn checkout_page_404_for_unknown_product() {
        let scratch = scratch_dir("checkout-404");
        let state = test_state_full(&scratch);
        let resp = checkout_page(State(state), Path("no-such-product".to_string())).await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        let _ = fs::remove_dir_all(&scratch);
    }

    #[tokio::test]
    async fn checkout_page_500_when_catalog_unavailable() {
        let scratch = scratch_dir("checkout-500");
        let state = test_state_at(&scratch, scratch.join("no-such-products.yaml"));
        let resp = checkout_page(State(state), Path("os-console".to_string())).await;
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let _ = fs::remove_dir_all(&scratch);
    }

    // ── S136: GET /software/:product_id ───────────────────────────────────────

    #[tokio::test]
    async fn product_detail_page_renders_known_product() {
        let scratch = scratch_dir("product-detail-ok");
        let state = test_state_full(&scratch);
        let resp = product_detail_page(State(state), Path("os-mediakit".to_string())).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let html = body_text(resp.into_body()).await;
        assert!(html.contains("MediaKit OS"));
        assert!(html.contains("BETA \u{00b7} free"));
        assert!(html.contains("v1.2.0"));
        assert!(html.contains("sw-masthead"));
        let _ = fs::remove_dir_all(&scratch);
    }

    #[tokio::test]
    async fn product_detail_page_404_for_unknown_product() {
        let scratch = scratch_dir("product-detail-404");
        let state = test_state_full(&scratch);
        let resp = product_detail_page(State(state), Path("no-such-product".to_string())).await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        let _ = fs::remove_dir_all(&scratch);
    }

    #[tokio::test]
    async fn product_detail_page_500_when_catalog_unavailable() {
        let scratch = scratch_dir("product-detail-500");
        let state = test_state_at(&scratch, scratch.join("no-such-products.yaml"));
        let resp = product_detail_page(State(state), Path("os-console".to_string())).await;
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let _ = fs::remove_dir_all(&scratch);
    }

    // ── Phase 2: GET /order redirect ──────────────────────────────────────────

    #[tokio::test]
    async fn order_redirect_builds_canonical_url_and_lowercases_tx_hash() {
        let resp = order_redirect(Query(OrderRedirectQuery {
            product: "os-console".to_string(),
            tx_hash: "0xABCDEF".to_string(),
        }))
        .await;
        // axum's Redirect::to emits 303 See Other (matches the / -> /software note above).
        assert_eq!(resp.status(), StatusCode::SEE_OTHER);
        assert_eq!(
            resp.headers().get(header::LOCATION).unwrap(),
            "/order/0xabcdef?product=os-console"
        );
    }

    // ── Phase 2: GET /order/:tx_hash status page ──────────────────────────────

    #[tokio::test]
    async fn order_status_page_confirmed_shows_receipt_and_download_link() {
        let scratch = scratch_dir("order-confirmed");
        let double = write_tool_wallet_double(&scratch);
        let state = test_state(&scratch, double.to_string_lossy().into_owned());
        let resp = order_status_page(
            State(state),
            Path("0xconfirmedpayment01".to_string()),
            Query(OrderQuery::default()),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let html = body_text(resp.into_body()).await;
        assert!(html.contains("Confirmed"));
        // A $1.00 payment matches os-console in this test's write_catalog fixture.
        assert!(html.contains("os-console"));
        assert!(html.contains("href=\"/order/0xconfirmedpayment01/download?product=os-console\""));
        let _ = fs::remove_dir_all(&scratch);
    }

    #[tokio::test]
    async fn order_status_page_pending_shows_retry_hint() {
        let scratch = scratch_dir("order-pending");
        let double = write_tool_wallet_double(&scratch);
        let state = test_state(&scratch, double.to_string_lossy().into_owned());
        let resp = order_status_page(
            State(state),
            Path("0xpendingtx01".to_string()),
            Query(OrderQuery::default()),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let html = body_text(resp.into_body()).await;
        assert!(html.contains("Pending"));
        let _ = fs::remove_dir_all(&scratch);
    }

    #[tokio::test]
    async fn order_status_page_not_found_links_back_to_checkout() {
        let scratch = scratch_dir("order-notfound");
        let double = write_tool_wallet_double(&scratch);
        let state = test_state(&scratch, double.to_string_lossy().into_owned());
        let resp = order_status_page(
            State(state),
            Path("0xunknowntx01".to_string()),
            Query(OrderQuery {
                product: Some("os-console".to_string()),
            }),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let html = body_text(resp.into_body()).await;
        assert!(html.contains("Not found"));
        assert!(html.contains("href=\"/checkout/os-console\""));
        let _ = fs::remove_dir_all(&scratch);
    }

    // ── Phase 2: GET /order/:tx_hash/download — real token minting ───────────

    #[tokio::test]
    async fn order_download_confirmed_mints_token_and_redirects() {
        let scratch = scratch_dir("order-download-ok");
        let double = write_tool_wallet_double(&scratch);
        let state = test_state(&scratch, double.to_string_lossy().into_owned());
        let resp = order_download(
            State(state.clone()),
            Path("0xconfirmedpayment01".to_string()),
            Query(OrderQuery::default()),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::SEE_OTHER);
        let location = resp
            .headers()
            .get(header::LOCATION)
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        assert!(location.starts_with(
            "https://example.invalid/releases/os-console/2026.05.144/linux-x86_64?token="
        ));

        // The minted token must actually verify against the signing key's public
        // counterpart, matching app-privategit-source-2's exact wire format
        // (base64url_no_pad(sig[64] || payload_json)).
        use ed25519_dalek::Verifier;
        let token = location.split("token=").nth(1).unwrap();
        let bytes = URL_SAFE_NO_PAD.decode(token).unwrap();
        let (sig_bytes, payload_bytes) = bytes.split_at(64);
        let sig = ed25519_dalek::Signature::from_bytes(sig_bytes.try_into().unwrap());
        let vk = test_signing_key().verifying_key();
        assert!(vk.verify(payload_bytes, &sig).is_ok());
        let payload: Value = serde_json::from_slice(payload_bytes).unwrap();
        assert_eq!(payload["product"], "os-console");
        assert_eq!(
            payload["channel_expiry"],
            Utc::now().format("%Y-%m-%d").to_string()
        );
        assert_eq!(payload["entitlements"], json!(["binary"]));

        let _ = fs::remove_dir_all(&scratch);
    }

    #[tokio::test]
    async fn order_download_403_when_not_confirmed() {
        let scratch = scratch_dir("order-download-403");
        let double = write_tool_wallet_double(&scratch);
        let state = test_state(&scratch, double.to_string_lossy().into_owned());
        let resp = order_download(
            State(state),
            Path("0xpendingtx01".to_string()),
            Query(OrderQuery::default()),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
        let _ = fs::remove_dir_all(&scratch);
    }

    #[tokio::test]
    async fn order_download_400_on_product_mismatch() {
        let scratch = scratch_dir("order-download-mismatch");
        let double = write_tool_wallet_double(&scratch);
        let state = test_state(&scratch, double.to_string_lossy().into_owned());
        // 0xconfirmedpayment01 is a $1.00 payment -> resolves to os-console; claiming
        // os-mediakit here must be rejected rather than trusted.
        let resp = order_download(
            State(state),
            Path("0xconfirmedpayment01".to_string()),
            Query(OrderQuery {
                product: Some("os-mediakit".to_string()),
            }),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let _ = fs::remove_dir_all(&scratch);
    }

    #[tokio::test]
    async fn order_download_503_when_signing_key_unconfigured() {
        let scratch = scratch_dir("order-download-503");
        let double = write_tool_wallet_double(&scratch);
        let base_state = test_state(&scratch, double.to_string_lossy().into_owned());
        let state = Arc::new(AppState {
            signing_key: None,
            ..(*base_state).clone()
        });
        let resp = order_download(
            State(state),
            Path("0xconfirmedpayment01".to_string()),
            Query(OrderQuery::default()),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
        let _ = fs::remove_dir_all(&scratch);
    }

    // ── P1: /healthz + / redirect ─────────────────────────────────────────────

    #[tokio::test]
    async fn healthz_shape() {
        let Json(body) = healthz().await;
        assert_eq!(
            body,
            json!({"status": "ok", "service": "app-privategit-marketplace"})
        );
    }

    #[tokio::test]
    async fn root_redirects_302_to_software() {
        let resp = root().await;
        // P1 contract: 302 Found (NOT axum Redirect::to's 303).
        assert_eq!(resp.status(), StatusCode::FOUND);
        assert_eq!(resp.headers().get(header::LOCATION).unwrap(), "/software");
    }
}
