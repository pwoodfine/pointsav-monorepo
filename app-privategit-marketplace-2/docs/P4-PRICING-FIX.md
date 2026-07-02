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
replaces the comparison with a direct equality. The change is safe to make now
because there is currently **zero paid traffic**: every live catalog price is `0`.

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
  `1_000_000`. **This conversion is correct and is not being changed.**
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

- **Zero paid traffic today.** Every license in the live `products.yaml` that is
  actually purchasable is priced `price_usdc: 0`. The only non-zero entries
  (`apache` = 1,000,000; `fsl` = 19,000,000) are licensing *labels*; no paid
  purchase has ever flowed through this matcher.
- **The bug was masked, never exercised.** With a `0` price, both formulas agree:
  `0 * 1_000_000 == 0` and `0 == 0` are both true. So the OLD formula produced
  correct results for every transaction that has ever occurred. There is no
  historical behaviour to preserve and nothing to migrate.
- **No receipts depend on the old matching.** `product_id` is recomputed per
  request from the catalog; it is not a stored key that older receipts index on.
  Existing receipt files are replayed verbatim from disk (the receipt-cache path),
  independent of this matcher.
- **License-key derivation is unchanged.** `generate_license_key` is byte-identical
  to the OLD crate and tool-wallet, so any key that *was* issued remains
  reproducible. The fix changes only which `product_id` a *future* paid
  transaction resolves to — from a wrong `unknown-*` fallback to the correct tier.

Because the change strictly turns a latent wrong answer (that has never been hit)
into a correct answer, and there is no paid transaction history to regress, it is
safe to land now rather than deferring to the first real payment.

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
