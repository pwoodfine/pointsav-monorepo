// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

// P0.2 — visual token gallery. Flattens tokens.full.json's primitive/theme/extension
// tiers into renderable entries: color tokens get a swatch + contrast ratio, everything
// else gets a plain name/value row.
//
// Phase 5 fix (2026-07-04): this previously read `tokens/dtcg-bundle.json` — a
// knowledge-wiki-specific additive bundle (own $description: "PointSav design system —
// knowledge-wiki baseline... Adds knowledge-wiki primitives"), master-cosigned for that
// product, not this substrate's own generic primitives. `sync-design-tokens.sh` explicitly
// SKIPs dtcg-bundle.json from its canonical merge step — it was never meant to be the
// gallery's source. The actual merged, canonical bundle
// (`sync-design-tokens.sh`'s own output, matching what nginx serves at
// `/tokens.full.json`) lives at `exports/tokens.full.json`. Its `primitive` tier's
// `primary-60: #234ed8` matches the ratified PointSav blue from `primitive.json` /
// `pointsav-brand.json` — confirming this is the correct source.
use serde_json::Value;
use std::path::Path;

#[derive(serde::Serialize)]
pub struct TokenEntry {
    pub path: String,
    pub css_var: String,
    pub value: String,
    pub kind: String,
    pub description: Option<String>,
    pub contrast_vs_white: Option<String>,
}

#[derive(serde::Serialize)]
pub struct TokenGroup {
    pub name: String,
    pub entries: Vec<TokenEntry>,
}

#[derive(serde::Serialize)]
pub struct TokenTier {
    pub name: String,
    pub groups: Vec<TokenGroup>,
}

pub fn load_and_flatten(vault: &Path) -> Vec<TokenTier> {
    let path = vault.join("exports").join("tokens.full.json");
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    let Ok(Value::Object(root)) = serde_json::from_str::<Value>(&raw) else {
        return Vec::new();
    };

    let mut tiers = Vec::new();
    for tier_name in ["primitive", "theme", "ibm-carbon-org-chart", "org-chart-extended"] {
        let Some(Value::Object(tier_map)) = root.get(tier_name) else {
            continue;
        };
        let mut groups = Vec::new();
        for (group_name, group_val) in tier_map {
            if group_name.starts_with('$') {
                continue;
            }
            let mut entries = Vec::new();
            flatten(group_val, group_name.clone(), None, &mut entries);
            if !entries.is_empty() {
                groups.push(TokenGroup {
                    name: group_name.clone(),
                    entries,
                });
            }
        }
        if !groups.is_empty() {
            tiers.push(TokenTier {
                name: tier_name.to_string(),
                groups,
            });
        }
    }
    tiers
}

fn flatten(val: &Value, path: String, inherited_type: Option<String>, out: &mut Vec<TokenEntry>) {
    let Value::Object(map) = val else {
        return;
    };
    let own_type = map
        .get("$type")
        .and_then(|v| v.as_str())
        .map(String::from)
        .or(inherited_type.clone());

    if let Some(v) = map.get("$value") {
        let value = match v {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        let description = map
            .get("$description")
            .and_then(|v| v.as_str())
            .map(String::from);
        let kind = own_type.unwrap_or_else(|| "unknown".to_string());
        let contrast_vs_white = if kind == "color" {
            contrast_ratio_vs_white(&value)
        } else {
            None
        };
        out.push(TokenEntry {
            css_var: format!("--{}", path.replace('.', "-")),
            path,
            value,
            kind,
            description,
            contrast_vs_white,
        });
        return;
    }

    for (k, v) in map {
        if k.starts_with('$') {
            continue;
        }
        flatten(v, format!("{path}.{k}"), own_type.clone(), out);
    }
}

fn contrast_ratio_vs_white(hex: &str) -> Option<String> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    let lum = relative_luminance(r, g, b);
    let ratio = (1.0 + 0.05) / (lum + 0.05);
    Some(format!("{ratio:.2}:1"))
}

fn relative_luminance(r: u8, g: u8, b: u8) -> f64 {
    fn chan(c: u8) -> f64 {
        let c = c as f64 / 255.0;
        if c <= 0.03928 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }
    0.2126 * chan(r) + 0.7152 * chan(g) + 0.0722 * chan(b)
}
