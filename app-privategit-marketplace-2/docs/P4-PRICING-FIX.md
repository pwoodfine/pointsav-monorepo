# P4 pricing-unit fix — license price matching

**Phase:** P4 (license + payment flow) of the `app-privategit-marketplace-2` rewrite.
**Status:** the single deliberate, reviewable behavioural change this rewrite makes.
**Scope of this change:** one comparison in `match_license_product_id` (`src/main.rs`).
Everything else in P4 is a faithful port of the OLD crate's contract.

---

## Summary for the reviewer

When a customer pays for a license on-chain, the marketplace must map the amount
they paid to a catalog product. The OLD crate computed that mapping with a
comparison that multiplied the catalog price by 1,000,000 — a unit conversion that
had **already been applied** to the catalog value. That double-counts the
conversion, so a real payment would never match the correct product. This phase
replaces the comparison with a direct equality.

**Correction (Checkpoint 2 review, 2026-07-02):** an earlier version of this doc
claimed "every live catalog price is 0" as the safety basis — that is false.
`apache` ($1.00) and `fsl` ($19.00) are live, non-zero, purchasable license tiers,
and this same phase adds a working "Pay with Polygon USDC" CTA for both. The real
safety argument (see §d) is that the OLD formula's bug means no historical paid
transaction could ever have matched correctly — any real payment would have fallen
through to the `unknown-*` fallback — so switching to the correct comparison
cannot invalidate any previously-issued license key. That is a structural
guarantee (receipts replay verbatim from disk, independent of this matcher), not
a claim about catalog pricing.

---

## (a) The exact old formula and where it came from

`app-privategit-marketplace/src/main.rs` (the OLD crate), in the `/v1/license/:tx_hash`
handler, resolving `product_id` from the confirmed on-chain amount:

```rust
let price_units = (amount_usdc * 1_000_000.0) as u64;   // dollars -> micro-USDC (CORRECT)
let product_id = catalog
    .as_ref()
    .and_then(|c| {
        c.licenses
            .iter()
            .find(|l| l.price_usdc * 1_000_000 == price_units)   // <-- WRONG
    })
    .map(|l| l.id.clone())
    .unwrap_or_else(|| format!("unknown-{price_units}"));
```

- `amount_usdc` is the dollars-denominated amount reported by
  `tool-wallet check` (its JSON field `amount_usdc`, e.g. `1.00`).
- `price_units = (amount_usdc * 1_000_000.0) as u64` converts dollars to
  micro-USDC base units (USDC has 6 decimals). For a $1.00 payment this is
  `1_000_000`. **Correction (Checkpoint 2 review): this float round-trip is
  lossy for ~2% of whole-cent prices — see §(e) below. This handler no longer
  relies on it as the primary path; `amount_units` (an exact integer
  `tool-wallet` already provides) is preferred, with this float conversion kept
  only as a defensive fallback.**
- The bug is on the **catalog side** of the comparison: `l.price_usdc * 1_000_000`.

## (b) Why it is wrong (unit double-counting)

The catalog field is *named* `price_usdc`, which reads like "price in dollars" —
but the stored **value is already in micro-USDC base units**, not dollars. From
`products.yaml`:

```yaml
- id: apache
  price_usdc: 1000000     # = $1.00  (1,000,000 micro-USDC)
- id: fsl
  price_usdc: 19000000    # = $19.00 (19,000,000 micro-USDC)
```

So both sides of the comparison should already be in the same unit (micro-USDC).
Multiplying the catalog side by another `1_000_000` applies the
dollars-to-micro-units scale **a second time**:

| Scenario ($1.00 apache payment) | Value |
|---|---|
| On-chain amount reported by tool-wallet | `amount_usdc = 1.00` |
| `price_units` (dollars → micro-USDC) | `1_000_000` |
| Catalog `apache.price_usdc` (already micro-USDC) | `1_000_000` |
| OLD LHS `l.price_usdc * 1_000_000` | `1_000_000_000_000` |
| OLD comparison `1_000_000_000_000 == 1_000_000` | **false → no match** |

Result: the payment falls through to the `unknown-{price_units}` fallback
(`unknown-1000000`) instead of resolving to `apache`. A real paid transaction
would essentially never match the intended product — it could only match by
numeric coincidence.

## (c) The exact fix

Compare the two micro-USDC quantities directly, with **no** multiplication on the
catalog side:

```rust
fn match_license_product_id(catalog: &Catalog, price_units: u64) -> Option<String> {
    catalog
        .licenses
        .iter()
        .find(|l| l.price_usdc == price_units)   // direct micro-unit equality
        .map(|l| l.id.clone())
}
```

The dollars → micro-USDC conversion for the incoming amount
(`price_units_from_amount`) is untouched; only the catalog-side comparison changed.
For the $1.00 apache case: `1_000_000 == 1_000_000` → matches `apache`. For $19.00:
`19_000_000 == 19_000_000` → matches `fsl`.

The field name (`price_usdc`, misleadingly dollar-labelled) is a pre-existing quirk
shared across the catalog, the receipt struct, and tool-wallet; renaming it is out
of scope for this phase and is deliberately **not** done here to keep the change
minimal and reviewable.

## (d) Why it is safe to change now specifically

**Correction (Checkpoint 2 review):** the argument below does NOT rest on "every
catalog price is 0" — `apache` and `fsl` are real, non-zero, live prices. The
safety argument is structural, independent of what any price happens to be:

- **The OLD formula could never have matched a real paid transaction, for any
  non-zero price.** `l.price_usdc * 1_000_000 == price_units` only holds when
  `price_units` is itself a multiple of `1_000_000 * l.price_usdc` — for the live
  prices (1,000,000 and 19,000,000), that requires an absurd payment size (e.g.
  $1,000,000 to match `apache`). Any real-sized payment against `apache` or `fsl`
  would have fallen through to the `unknown-{price_units}` fallback under the OLD
  code, every time, with no exception. This is a structural guarantee from reading
  the formula, not an empirical claim about what has or hasn't happened on
  software.pointsav.com (a different machine from this development session, whose
  actual payment history was not and could not be checked from here).
- **No receipts depend on the old matching.** `product_id` is recomputed per
  request from the catalog; it is not a stored key that older receipts index on.
  Existing receipt files are replayed verbatim from disk (the receipt-cache path),
  independent of this matcher.
- **License-key derivation is unchanged.** `generate_license_key` is byte-identical
  to the OLD crate and tool-wallet, so any key that *was* issued remains
  reproducible. The fix changes only which `product_id` a *future* paid
  transaction resolves to — from a wrong `unknown-*` fallback to the correct tier.

Because the OLD formula was structurally incapable of correctly matching any
realistic non-zero payment, and no receipt or issued key depends on that matcher's
output, there is nothing to migrate or regress — the fix can land immediately.

## (e) Related precision issue found and fixed in the same review pass

The dollars→micro-USDC conversion (`price_units_from_amount`, using
`amount_usdc: f64`) is a **lossy float round-trip**: `tool-wallet` computes
`amount_usdc = amount_units as f64 / 1_000_000.0` for display, and this handler
then computes `(amount_usdc * 1_000_000.0) as u64` to get back to integer
micro-units. That round-trip is off-by-one for roughly 2% of whole-cent prices
(e.g. $2.01), which would silently reintroduce the exact failure class this fix
exists to eliminate — just latent, since today's live prices ($1.00, $19.00)
happen to round-trip exactly.

**Fixed:** `tool-wallet` already emits the exact source integer as
`amount_units` in its JSON output (`tool-wallet/src/main.rs:568`). The handler
now reads `amount_units` directly when present, falling back to the float
conversion only if that field is absent. This removes the precision-loss class
entirely rather than certifying a conversion that was still silently wrong for
some inputs.

---

## Test evidence

`src/main.rs` `#[cfg(test)]` module, `pricing_fix_old_formula_fails_new_formula_matches`,
proves the before/after directly for a realistic $1.00 apache-tier payment:

- Reproduces the OLD formula verbatim (`l.price_usdc * 1_000_000 == price_units`)
  and asserts it returns `None` (no match — the bug).
- Asserts the NEW `match_license_product_id` returns `Some("apache")`.
- Cross-checks $19.00 → `fsl` under the fix.

The end-to-end handler test
`license_confirmed_via_fresh_check_matches_apache_and_writes_receipt` drives the
whole `/v1/license/:tx_hash` path through a mocked `tool-wallet check` (no real
Polygon RPC) and confirms a $1.00 payment now resolves to `product_id: "apache"`.
