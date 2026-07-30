// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

//! `xtask fsl-clock <products.yaml>` — Phase 5 (BRIEF-software-hyperscaler-audit.md
//! Pricing and Licensing decision framework, criterion 4: "track the two-year
//! clock per binary and price before conversion").
//!
//! Lists every FSL-tier product's `fsl_conversion_date` (when its FSL term
//! automatically converts to Apache 2.0), sorted soonest-first, flagging any
//! that have already passed. Products with no conversion date set yet (the
//! normal case until a release date and legal team commit one) are reported
//! separately, not silently dropped.
//!
//! Deliberately a hand-rolled line scanner, not a YAML library dependency: xtask
//! has no `serde`/`serde_yaml` today, and `products.yaml`'s structure (one
//! `- id: <x>` per installer, flat 2-space-indented `key: value` fields under it)
//! is simple and regular enough that adding a full parser dependency for this one
//! small subcommand isn't warranted.

use std::fs;

#[derive(Debug)]
struct FslEntry {
    id: String,
    conversion_date: Option<String>,
}

fn parse_fsl_entries(raw: &str) -> Vec<FslEntry> {
    let mut entries = Vec::new();
    let mut current_id: Option<String> = None;
    let mut current_tier: Option<String> = None;
    let mut current_conversion: Option<String> = None;

    let flush = |id: &Option<String>,
                 tier: &Option<String>,
                 conv: &Option<String>,
                 out: &mut Vec<FslEntry>| {
        if let (Some(id), Some(tier)) = (id, tier) {
            if tier == "fsl" {
                out.push(FslEntry {
                    id: id.clone(),
                    conversion_date: conv.clone(),
                });
            }
        }
    };

    for line in raw.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("- id:") {
            // New installer entry starts — flush the previous one first.
            flush(
                &current_id,
                &current_tier,
                &current_conversion,
                &mut entries,
            );
            current_id = Some(rest.trim().trim_matches('"').to_string());
            current_tier = None;
            current_conversion = None;
        } else if let Some(rest) = trimmed.strip_prefix("license_tier:") {
            current_tier = Some(rest.trim().trim_matches('"').to_string());
        } else if let Some(rest) = trimmed.strip_prefix("fsl_conversion_date:") {
            let v = rest.trim().trim_matches('"');
            current_conversion = if v.is_empty() || v == "null" || v == "~" {
                None
            } else {
                Some(v.to_string())
            };
        }
    }
    flush(
        &current_id,
        &current_tier,
        &current_conversion,
        &mut entries,
    );
    entries
}

pub fn run(args: &[String]) -> Result<(), String> {
    let path = args
        .first()
        .ok_or("fsl-clock: usage: xtask fsl-clock <products.yaml>")?;
    let raw = fs::read_to_string(path).map_err(|e| format!("fsl-clock: read {path}: {e}"))?;
    let mut entries = parse_fsl_entries(&raw);

    let (mut dated, undated): (Vec<_>, Vec<_>) =
        entries.drain(..).partition(|e| e.conversion_date.is_some());
    dated.sort_by(|a, b| a.conversion_date.cmp(&b.conversion_date));

    println!("FSL two-year conversion clock — {path}");
    println!();
    if dated.is_empty() {
        println!("No FSL product has a conversion date set yet.");
    } else {
        println!("Sorted soonest first:");
        for e in &dated {
            println!("  {} -> {}", e.conversion_date.as_deref().unwrap(), e.id);
        }
    }
    if !undated.is_empty() {
        println!();
        println!(
            "{} FSL product(s) with no conversion date set yet (populate manually once a \
             release date is committed):",
            undated.len()
        );
        for e in &undated {
            println!("  {}", e.id);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"installers:
  - id: os-console
    name: Console
    license_tier: commercial
    price_usdc: 0
  - id: os-mediakit
    name: MediaKit
    license_tier: fsl
    price_usdc: 0
    fsl_conversion_date: "2028-05-14"
  - id: os-infrastructure
    name: Infra
    license_tier: fsl
    price_usdc: 0
  - id: os-network-admin
    name: NetAdmin
    license_tier: fsl
    price_usdc: 0
    fsl_conversion_date: "2027-01-01"
"#;

    #[test]
    fn parses_only_fsl_entries_ignoring_commercial() {
        let entries = parse_fsl_entries(FIXTURE);
        assert_eq!(entries.len(), 3, "3 FSL entries, os-console excluded");
        assert!(entries.iter().all(|e| e.id != "os-console"));
    }

    #[test]
    fn dated_entries_sort_soonest_first_undated_excluded_not_dropped() {
        let entries = parse_fsl_entries(FIXTURE);
        let (mut dated, undated): (Vec<_>, Vec<_>) = entries
            .into_iter()
            .partition(|e| e.conversion_date.is_some());
        dated.sort_by(|a, b| a.conversion_date.cmp(&b.conversion_date));

        assert_eq!(dated.len(), 2);
        assert_eq!(dated[0].id, "os-network-admin"); // 2027-01-01, soonest
        assert_eq!(dated[1].id, "os-mediakit"); // 2028-05-14

        assert_eq!(undated.len(), 1);
        assert_eq!(undated[0].id, "os-infrastructure");
    }

    #[test]
    fn past_conversion_date_still_sorts_first_flagged_by_position_not_dropped() {
        let past_fixture = r#"installers:
  - id: os-mediakit
    license_tier: fsl
    fsl_conversion_date: "2020-01-01"
"#;
        let entries = parse_fsl_entries(past_fixture);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].conversion_date.as_deref(), Some("2020-01-01"));
    }
}
