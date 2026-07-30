use clap::{Parser, ValueEnum};
use serde_json::json;

#[derive(Parser)]
#[command(
    name = "tool-floorplate",
    version,
    about = "Floor plate composition from Woodfine Key Plan Tile algebra"
)]
struct Cli {
    /// Development class
    #[arg(long)]
    class: DevelopmentClass,

    /// Gross floor plate area in SF (uses class midpoint if omitted)
    #[arg(long)]
    area: Option<f64>,

    /// Number of floors — appends building totals
    #[arg(long)]
    floors: Option<u32>,

    /// Output format
    #[arg(long, default_value = "text")]
    format: Format,
}

#[derive(ValueEnum, Clone, Debug)]
enum DevelopmentClass {
    #[value(name = "professional-centre")]
    ProfessionalCentre,
    #[value(name = "suburban-office")]
    SuburbanOffice,
    #[value(name = "retail-select")]
    RetailSelect,
    #[value(name = "tech-industrial")]
    TechIndustrial,
}

#[derive(ValueEnum, Clone, Debug)]
enum Format {
    Text,
    Json,
}

// ── Constants ─────────────────────────────────────────────────────────────────

/// CO-1/8 = 1 tile = 2,500 SF leasable.
/// Derived: CO-FF = 20,000 SF / 8 = 2,500 SF
const TILE_SF: f64 = 2_500.0;

/// Building core + vertical circulation as share of gross floor area.
/// Derived: PC gross max 23,000 SF − CO-FF 20,000 SF = 3,000 SF = 13%
const CORE_FACTOR: f64 = 0.13;

/// Private Office Key Plan areas (source: FFE xlsx + DISCOVERY summary V3)
const PO_S_SF: f64 = 325.0;
const PO_M_SF: f64 = 465.0;
const PO_L_SF: f64 = 685.0;

/// CO Key Plan fractions: (label, leasable_sf, tile_count_label, note)
const CO_FRACTIONS: &[(&str, f64, &str, &str)] = &[
    ("CO-FF", 20_000.0, "8 tiles", "full leasable floor"),
    ("CO-1/2", 10_000.0, "4 tiles", ""),
    ("CO-1/3", 6_667.0, "2-3 tiles", ""),
    ("CO-1/4", 5_000.0, "2 tiles", ""),
    ("CO-1/8", 2_500.0, "1 tile", "smallest unit"),
];

// ── Class specification ───────────────────────────────────────────────────────

struct ClassSpec {
    display_name: &'static str,
    floors_min: u32,
    floors_max: u32,
    area_min_sf: f64,
    area_max_sf: f64,
    /// If true: Tile = Floor Plate (Retail Select, Tech Industrial)
    tile_is_floor: bool,
    tile_sizes: &'static [(&'static str, f64)],
}

fn class_spec(class: &DevelopmentClass) -> ClassSpec {
    match class {
        DevelopmentClass::ProfessionalCentre => ClassSpec {
            display_name: "Professional Centre",
            floors_min: 3,
            floors_max: 5,
            area_min_sf: 19_000.0,
            area_max_sf: 23_000.0,
            tile_is_floor: false,
            tile_sizes: &[],
        },
        DevelopmentClass::SuburbanOffice => ClassSpec {
            display_name: "Suburban Office",
            floors_min: 6,
            floors_max: 9,
            area_min_sf: 17_000.0,
            area_max_sf: 21_000.0,
            tile_is_floor: false,
            tile_sizes: &[],
        },
        DevelopmentClass::RetailSelect => ClassSpec {
            display_name: "Retail Select",
            floors_min: 1,
            floors_max: 1,
            area_min_sf: 4_500.0,
            area_max_sf: 7_700.0,
            tile_is_floor: true,
            tile_sizes: &[("Small", 4_500.0), ("Medium", 6_700.0), ("Large", 7_700.0)],
        },
        DevelopmentClass::TechIndustrial => ClassSpec {
            display_name: "Tech Industrial",
            floors_min: 1,
            floors_max: 1,
            area_min_sf: 7_200.0,
            area_max_sf: 8_400.0,
            tile_is_floor: true,
            tile_sizes: &[("Medium", 7_200.0), ("Large", 8_400.0)],
        },
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn sf_to_m2(sf: f64) -> f64 {
    sf / 10.763_9
}

fn ft(sf: f64) -> String {
    format!("{:.0} SF", sf)
}

fn m2(sf: f64) -> String {
    format!("{:.0} m²", sf_to_m2(sf))
}

// ── Rendering — multi-tile classes ───────────────────────────────────────────

fn print_multi_tile(cli: &Cli, s: &ClassSpec) {
    let gross = cli.area.unwrap_or((s.area_min_sf + s.area_max_sf) / 2.0);
    let default_note = if cli.area.is_none() {
        "  (class midpoint)"
    } else {
        ""
    };
    let core_sf = (gross * CORE_FACTOR).round();
    let net_sf = gross - core_sf;
    let tiles_full = (net_sf / TILE_SF).floor() as u32;
    let remainder = net_sf - tiles_full as f64 * TILE_SF;

    println!("Floor Plate — {}", s.display_name);
    println!(
        "Development class   {}  ({}-{} floors)",
        s.display_name, s.floors_min, s.floors_max
    );
    println!(
        "Gross floor area    {:>9}  ({}){}",
        ft(gross),
        m2(gross),
        default_note
    );
    println!();
    println!(
        "  Building core       {:>7}  ({:.0}%)",
        ft(core_sf),
        CORE_FACTOR * 100.0
    );
    println!(
        "  Net leasable area   {:>7}  ({:.0}%)",
        ft(net_sf),
        (1.0 - CORE_FACTOR) * 100.0
    );
    if remainder > 1.0 {
        println!(
            "  Tiles               {}  ×  CO-1/8 ({} SF each)  +  {:.0} SF remainder",
            tiles_full, TILE_SF as u32, remainder
        );
    } else {
        println!(
            "  Tiles               {}  ×  CO-1/8 ({} SF each)",
            tiles_full, TILE_SF as u32
        );
    }

    println!();
    println!("CO Key Plan fractions");
    println!(
        "  {:<8}  {:>9}   {:>6}   {:>8}   Note",
        "Label", "Leasable", "m²", "Tiles"
    );
    println!(
        "  {:<8}  {:>9}   {:>6}   {:>8}   ----",
        "-----", "--------", "--", "-----"
    );
    for (label, lsf, tile_label, note) in CO_FRACTIONS {
        println!(
            "  {:<8}  {:>9}   {:>6}   {:>8}   {}",
            label,
            ft(*lsf),
            m2(*lsf),
            tile_label,
            note
        );
    }

    println!();
    println!(
        "T_Basic composition per CO-1/8 tile  ({} SF leasable)",
        TILE_SF as u32
    );
    println!("  PO-S  {}  each", ft(PO_S_SF));
    println!("  PO-M  {}  each", ft(PO_M_SF));
    println!("  PO-L  {}  each", ft(PO_L_SF));
    println!("  T_Basic    =  n(PO-S)  +  p(PO-M)  +  q(PO-L)");
    println!("  T_Compound =  T_Basic  +  amenity Key Plans");
    println!("  T_Special  =  T_Basic  +  corner / elevator lobby Key Plans");

    println!();
    match cli.floors {
        Some(f) => {
            println!("Building totals  ({} floors)", f);
            println!(
                "  Gross       {:>10}  ({})",
                ft(gross * f as f64),
                m2(gross * f as f64)
            );
            println!(
                "  Leasable    {:>10}  ({})",
                ft(net_sf * f as f64),
                m2(net_sf * f as f64)
            );
        }
        None => {
            println!(
                "Building totals  ({}-{} floors  ×  {:.0} SF/floor gross)",
                s.floors_min, s.floors_max, gross
            );
            for f in [s.floors_min, s.floors_max] {
                println!(
                    "  {} floors   {:>10} gross   ({} leasable)",
                    f,
                    ft(gross * f as f64),
                    ft(net_sf * f as f64)
                );
            }
        }
    }
}

fn json_multi_tile(cli: &Cli, s: &ClassSpec) {
    let gross = cli.area.unwrap_or((s.area_min_sf + s.area_max_sf) / 2.0);
    let core_sf = (gross * CORE_FACTOR).round();
    let net_sf = gross - core_sf;
    let tiles_full = (net_sf / TILE_SF).floor() as u32;
    let remainder = net_sf - tiles_full as f64 * TILE_SF;
    let floors = cli.floors.unwrap_or(s.floors_min);

    let co_fractions: Vec<_> = CO_FRACTIONS
        .iter()
        .map(|(label, lsf, tile_label, note)| {
            json!({
                "label": label,
                "leasable_sf": lsf,
                "leasable_m2": (sf_to_m2(*lsf) * 10.0).round() / 10.0,
                "tiles": tile_label,
                "note": note,
            })
        })
        .collect();

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "class": s.display_name,
            "floors_range": { "min": s.floors_min, "max": s.floors_max },
            "floor_plate": {
                "gross_sf": gross,
                "gross_m2": (sf_to_m2(gross) * 10.0).round() / 10.0,
                "core_sf": core_sf,
                "core_pct": CORE_FACTOR * 100.0,
                "net_leasable_sf": net_sf,
                "net_leasable_m2": (sf_to_m2(net_sf) * 10.0).round() / 10.0,
                "tile_count": tiles_full,
                "tile_sf": TILE_SF,
                "tile_remainder_sf": remainder,
            },
            "co_fractions": co_fractions,
            "t_basic": {
                "po_s_sf": PO_S_SF,
                "po_m_sf": PO_M_SF,
                "po_l_sf": PO_L_SF,
                "formula": "n × PO-S + p × PO-M + q × PO-L ≈ 2500 SF per tile",
            },
            "building_totals": {
                "floors": floors,
                "gross_sf": gross * floors as f64,
                "leasable_sf": net_sf * floors as f64,
            },
        }))
        .unwrap()
    );
}

// ── Rendering — tile-is-floor classes ────────────────────────────────────────

fn print_tile_floor(cli: &Cli, s: &ClassSpec) {
    println!("Floor Plate — {}", s.display_name);
    println!("(Tile = Floor Plate for this class; no separate building core)");
    println!();
    println!("Available tile sizes");
    println!("  {:<10}  {:>9}   {:>6}", "Size", "SF", "m²");
    println!("  {:<10}  {:>9}   {:>6}", "----", "--", "--");
    for (label, sf) in s.tile_sizes {
        println!("  {:<10}  {:>9}   {:>6}", label, ft(*sf), m2(*sf));
    }
    if let Some(area) = cli.area {
        println!();
        let nearest = s
            .tile_sizes
            .iter()
            .min_by_key(|(_, sf)| ((sf - area).abs() * 100.0) as u64);
        if let Some((label, nearest_sf)) = nearest {
            println!(
                "Given area  {}  →  nearest: {} ({})",
                ft(area),
                label,
                ft(*nearest_sf)
            );
        }
    }
    if let Some(f) = cli.floors {
        let sf = cli.area.unwrap_or(s.area_min_sf);
        println!();
        println!("Building totals  ({} floors  ×  {} floor)", f, ft(sf));
        println!(
            "  Gross = Leasable  {:>9}  ({})",
            ft(sf * f as f64),
            m2(sf * f as f64)
        );
    }
}

fn json_tile_floor(cli: &Cli, s: &ClassSpec) {
    let selected_sf = cli.area.unwrap_or(s.area_min_sf);
    let floors = cli.floors.unwrap_or(1);
    let sizes: Vec<_> = s
        .tile_sizes
        .iter()
        .map(|(label, sf)| {
            json!({ "label": label, "sf": sf, "m2": (sf_to_m2(*sf) * 10.0).round() / 10.0 })
        })
        .collect();
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "class": s.display_name,
            "tile_is_floor": true,
            "available_sizes": sizes,
            "selected_sf": selected_sf,
            "selected_m2": (sf_to_m2(selected_sf) * 10.0).round() / 10.0,
            "building_totals": {
                "floors": floors,
                "gross_sf": selected_sf * floors as f64,
                "leasable_sf": selected_sf * floors as f64,
            },
        }))
        .unwrap()
    );
}

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() {
    let cli = Cli::parse();
    let s = class_spec(&cli.class);
    if s.tile_is_floor {
        match cli.format {
            Format::Text => print_tile_floor(&cli, &s),
            Format::Json => json_tile_floor(&cli, &s),
        }
    } else {
        match cli.format {
            Format::Text => print_multi_tile(&cli, &s),
            Format::Json => json_multi_tile(&cli, &s),
        }
    }
}
