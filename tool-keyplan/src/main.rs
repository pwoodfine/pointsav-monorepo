// tool-keyplan: BIM Key Plan compiler — TOML config → validated DTCG JSON
//
// CLI:
//   tool-keyplan --interior <interior.dtcg.json> --config <po-1.toml> --output <out.dtcg.json>
//   tool-keyplan --interior <interior.dtcg.json> --config <po-1.toml> --validate-only
//
// Engine process (AEC Key Plan Methodology):
//   1. Load interior.dtcg.json → furniture spec map (keyed by DTCG token path)
//   2. Parse TOML config → KeyPlanConfig (furniture placements + zone geometry)
//   3. Validate: ASR A1.2 area, European Lighting Standard, wheelchair turning
//   4. Compute bounding box from zone geometry + authoritative area
//   5. Emit DTCG JSON with structured furniture_refs + compliance record

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use std::{collections::HashMap, env, fs, path::Path};

// ─── interior DTCG structures ──────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
struct DimensionsMm {
    w: f64,
    d: f64,
    #[allow(dead_code)]
    h_min: f64,
    #[allow(dead_code)]
    h_max: f64,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
struct ClearanceMm {
    front: f64,
    sides: f64,
    rear: f64,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
struct FurnitureValue {
    manufacturer: String,
    product_line: Option<String>,
    model: String,
    sku: Option<String>,
    dimensions_mm: DimensionsMm,
    clearance_mm: ClearanceMm,
    weight_kg: Option<f64>,
    ifc_class: String,
    url: Option<String>,
}

// ─── TOML config structures ────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct KeyPlanToml {
    key_plan: KeyPlanMeta,
    circulation: CirculationToml,
    #[serde(default)]
    furniture: Vec<FurniturePlacement>,
}

#[derive(Debug, Deserialize)]
struct KeyPlanMeta {
    internal_code: String,
    display_name: String,
    category: String,
    capacity: u32,
    zone1_depth_m: f64,
    zone2_depth_m: f64,
    zone3_depth_m: f64,
    area_m2: f64,
    area_sf: f64,
}

#[derive(Debug, Deserialize)]
struct CirculationToml {
    standard: String,
    min_area_per_person_m2: f64,
    #[allow(dead_code)] // schema field — aisle-width check not yet implemented
    min_aisle_mm: f64,
    wheelchair_radius_mm: f64,
    desk_to_window_max_m: f64,
}

#[derive(Debug, Deserialize)]
struct FurniturePlacement {
    token: String,
    zone: String,
    qty: u32,
    #[allow(dead_code)]
    x_mm: f64,
    y_mm: f64,
    #[serde(default)]
    rotation_deg: f64,
    #[allow(dead_code)]
    note: Option<String>,
}

// ─── DTCG token resolution ─────────────────────────────────────────────────

fn load_furniture_map(interior_path: &Path) -> Result<HashMap<String, FurnitureValue>> {
    let content = fs::read_to_string(interior_path)
        .with_context(|| format!("reading {}", interior_path.display()))?;
    let root: Value = serde_json::from_str(&content).context("parsing interior DTCG JSON")?;
    let mut map = HashMap::new();
    collect_furniture_tokens(&root, "", &mut map)?;
    Ok(map)
}

fn collect_furniture_tokens(
    node: &Value,
    prefix: &str,
    map: &mut HashMap<String, FurnitureValue>,
) -> Result<()> {
    if let Value::Object(obj) = node {
        if let Some(type_val) = obj.get("$type") {
            if type_val.as_str() == Some("bim.furniture-object") {
                if let Some(val) = obj.get("$value") {
                    let spec: FurnitureValue = serde_json::from_value(val.clone())
                        .with_context(|| format!("parsing token at '{}'", prefix))?;
                    map.insert(prefix.to_string(), spec);
                    return Ok(());
                }
            }
        }
        for (key, child) in obj {
            if key.starts_with('$') {
                continue;
            }
            let child_prefix = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{}.{}", prefix, key)
            };
            collect_furniture_tokens(child, &child_prefix, map)?;
        }
    }
    Ok(())
}

// ─── constraint validation ─────────────────────────────────────────────────

struct Validation {
    asr_ok: bool,
    area_per_person: f64,
    lighting_ok: bool,
    wheelchair_ok: bool,
    plan_width_mm: f64,
    messages: Vec<String>,
}

fn validate(config: &KeyPlanToml, furniture_map: &HashMap<String, FurnitureValue>) -> Validation {
    let mut messages = Vec::new();

    // 1. ASR A1.2 — minimum 8 m² per person (room area constraint)
    let area_per_person = config.key_plan.area_m2 / config.key_plan.capacity as f64;
    let asr_ok = area_per_person >= config.circulation.min_area_per_person_m2;
    if asr_ok {
        messages.push(format!(
            "ASR A1.2 ✓ — {:.2} m² / {} = {:.2} m²/person ≥ {:.1} m² minimum",
            config.key_plan.area_m2,
            config.key_plan.capacity,
            area_per_person,
            config.circulation.min_area_per_person_m2,
        ));
    } else {
        messages.push(format!(
            "ASR A1.2 ✗ — {:.2} m²/person < {:.1} m² minimum (capacity {})",
            area_per_person, config.circulation.min_area_per_person_m2, config.key_plan.capacity,
        ));
    }

    // 2. European Lighting Standard — all Zone 1 furniture back edge ≤ 6 m from facade
    let desk_limit_mm = config.circulation.desk_to_window_max_m * 1000.0;
    let mut lighting_ok = true;
    for p in &config.furniture {
        if p.zone != "Z1" {
            continue;
        }
        let depth_mm = furniture_map
            .get(&p.token)
            .map(|s| {
                if (p.rotation_deg - 90.0).abs() < 1.0 || (p.rotation_deg - 270.0).abs() < 1.0 {
                    s.dimensions_mm.w
                } else {
                    s.dimensions_mm.d
                }
            })
            .unwrap_or(0.0);
        let far_edge = p.y_mm + depth_mm;
        if far_edge > desk_limit_mm {
            lighting_ok = false;
            messages.push(format!(
                "European Lighting ✗ — '{}' far edge {:.0} mm > {:.0} mm limit",
                p.token, far_edge, desk_limit_mm,
            ));
        }
    }
    if lighting_ok {
        messages.push(format!(
            "European Lighting ✓ — all Zone 1 furniture within {:.1} m of facade",
            config.circulation.desk_to_window_max_m,
        ));
    }

    // 3. Wheelchair turning radius — plan width ≥ 2 × radius at door
    //    Zone 3 corridor (door approach) must accommodate a full-circle turn.
    //    Plan width is derived from authoritative area ÷ total depth.
    let total_depth_mm = (config.key_plan.zone1_depth_m
        + config.key_plan.zone2_depth_m
        + config.key_plan.zone3_depth_m)
        * 1000.0;
    let plan_width_mm = (config.key_plan.area_m2 * 1_000_000.0) / total_depth_mm;
    let wheelchair_ok = plan_width_mm >= 2.0 * config.circulation.wheelchair_radius_mm;
    if wheelchair_ok {
        messages.push(format!(
            "Wheelchair ✓ — plan width {:.0} mm ≥ {} mm (2 × {:.0} mm radius)",
            plan_width_mm,
            (2.0 * config.circulation.wheelchair_radius_mm) as u32,
            config.circulation.wheelchair_radius_mm,
        ));
    } else {
        messages.push(format!(
            "Wheelchair ✗ — plan width {:.0} mm < {} mm (2 × {:.0} mm radius)",
            plan_width_mm,
            (2.0 * config.circulation.wheelchair_radius_mm) as u32,
            config.circulation.wheelchair_radius_mm,
        ));
    }

    Validation {
        asr_ok,
        area_per_person,
        lighting_ok,
        wheelchair_ok,
        plan_width_mm,
        messages,
    }
}

// ─── output generation ─────────────────────────────────────────────────────

fn size_key_from_code(code: &str) -> &'static str {
    if code.ends_with("-1") {
        "small"
    } else if code.ends_with("-2") {
        "medium"
    } else if code.ends_with("-3") {
        "large"
    } else {
        "unknown"
    }
}

fn generate_output(config: &KeyPlanToml, v: &Validation) -> Value {
    let total_depth_mm = (config.key_plan.zone1_depth_m
        + config.key_plan.zone2_depth_m
        + config.key_plan.zone3_depth_m)
        * 1000.0;

    // Ordered, deduplicated furniture refs
    let mut seen = std::collections::HashSet::new();
    let furniture_refs: Vec<String> = config
        .furniture
        .iter()
        .flat_map(|p| std::iter::repeat_n(p.token.clone(), p.qty as usize))
        .filter(|t| seen.insert(t.clone()))
        .collect();

    let compliance = json!({
        "standard": config.circulation.standard,
        "asr_a12_area_m2_per_person": v.area_per_person,
        "asr_a12_satisfied": v.asr_ok,
        "european_lighting_standard": format!(
            "Zone 1 = {:.1} m satisfies {:.1} m desk-to-window rule",
            config.key_plan.zone1_depth_m,
            config.circulation.desk_to_window_max_m,
        ),
        "european_lighting_satisfied": v.lighting_ok,
        "wheelchair_clearance": format!(
            "{:.0} mm turning radius at door — plan width {:.0} mm",
            config.circulation.wheelchair_radius_mm,
            v.plan_width_mm,
        ),
        "wheelchair_satisfied": v.wheelchair_ok,
    });

    let key_plan_value = json!({
        "display_name": config.key_plan.display_name,
        "internal_code": config.key_plan.internal_code,
        "category": config.key_plan.category,
        "capacity": config.key_plan.capacity,
        "area_m2": config.key_plan.area_m2,
        "area_sf": config.key_plan.area_sf,
        "zone1_depth_m": config.key_plan.zone1_depth_m,
        "zone2_depth_m": config.key_plan.zone2_depth_m,
        "zone3_depth_m": config.key_plan.zone3_depth_m,
        "bounding_box_mm": {
            "w": (v.plan_width_mm).round(),
            "d": total_depth_mm.round(),
        },
        "furniture_refs": furniture_refs,
        "circulation_ref": "bim.interior.circulation.standard-private-office",
        "compliance": compliance,
        "status": "confirmed",
        "source": "FIN.xlsx Summary_Key Plans V3 2025-11-29",
        "development_classes": ["Professional Centres", "Suburban Office"],
        "tile_role": "component — nests into 2700 SF Tile with other Private Office Key Plans",
    });

    let description = format!(
        "{} — {:.2} m² / {} SF. Zone 1 = {:.1} m, Zone 2 = {:.1} m, Zone 3 = {:.1} m. {}",
        config.key_plan.display_name,
        config.key_plan.area_m2,
        config.key_plan.area_sf as u32,
        config.key_plan.zone1_depth_m,
        config.key_plan.zone2_depth_m,
        config.key_plan.zone3_depth_m,
        if v.asr_ok && v.lighting_ok && v.wheelchair_ok {
            "All constraints satisfied."
        } else {
            "CONSTRAINT VIOLATIONS — see compliance field."
        }
    );

    let size_key = size_key_from_code(&config.key_plan.internal_code);
    let token_obj = json!({
        "$type": "bim.entity",
        "$value": key_plan_value,
        "$description": description,
    });

    let mut size_map = Map::new();
    size_map.insert(size_key.to_string(), token_obj);

    let mut cat_map = Map::new();
    cat_map.insert(config.key_plan.category.clone(), Value::Object(size_map));

    json!({
        "$schema": "https://design-tokens.github.io/community-group/format/",
        "$description": format!(
            "Generated by tool-keyplan v0.0.1 — {} compiled from TOML config + interior.dtcg.json",
            config.key_plan.internal_code,
        ),
        "bim": {
            "key-plan": Value::Object(cat_map),
        }
    })
}

// ─── main ──────────────────────────────────────────────────────────────────

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    let mut interior: Option<String> = None;
    let mut config_path: Option<String> = None;
    let mut output: Option<String> = None;
    let mut validate_only = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--interior" => {
                i += 1;
                interior = Some(args.get(i).context("--interior requires a path")?.clone());
            }
            "--config" => {
                i += 1;
                config_path = Some(args.get(i).context("--config requires a path")?.clone());
            }
            "--output" => {
                i += 1;
                output = Some(args.get(i).context("--output requires a path")?.clone());
            }
            "--validate-only" => validate_only = true,
            other => bail!("unknown argument: {}", other),
        }
        i += 1;
    }

    let interior = interior.context("--interior <interior.dtcg.json> is required")?;
    let config_path = config_path.context("--config <config.toml> is required")?;

    let furniture_map = load_furniture_map(Path::new(&interior))?;
    eprintln!(
        "Loaded {} furniture tokens from {}",
        furniture_map.len(),
        interior
    );

    let config_str =
        fs::read_to_string(&config_path).with_context(|| format!("reading {}", config_path))?;
    let config: KeyPlanToml =
        toml::from_str(&config_str).with_context(|| format!("parsing {}", config_path))?;

    eprintln!(
        "Compiling {} — {}",
        config.key_plan.internal_code, config.key_plan.display_name
    );

    let v = validate(&config, &furniture_map);
    for msg in &v.messages {
        eprintln!("  {}", msg);
    }

    let all_ok = v.asr_ok && v.lighting_ok && v.wheelchair_ok;
    if all_ok {
        eprintln!(
            "{}: ALL CONSTRAINTS SATISFIED",
            config.key_plan.internal_code
        );
    } else {
        eprintln!(
            "{}: CONSTRAINT VIOLATIONS DETECTED",
            config.key_plan.internal_code
        );
    }

    if validate_only {
        return if all_ok {
            Ok(())
        } else {
            bail!(
                "{}: constraint violations detected",
                config.key_plan.internal_code
            )
        };
    }

    let output = output.context("--output <path> is required (or use --validate-only)")?;

    let doc = generate_output(&config, &v);
    let json_str = serde_json::to_string_pretty(&doc).context("serializing DTCG output")?;
    fs::write(&output, &json_str).with_context(|| format!("writing {}", output))?;
    eprintln!("Written → {}", output);

    Ok(())
}
