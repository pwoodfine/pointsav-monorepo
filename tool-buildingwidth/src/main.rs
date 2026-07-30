use clap::{Parser, ValueEnum};
use serde_json::json;

/// Building Width Calculator — Woodfine Key Plans V3 (Jan 2026).
///
/// Computes double-loaded floor plate width from zone depths.
/// Formula: full_width = 2 × (Z1 + Z2) + Z3
///
/// Zone semantics:
///   Z1 – Habitat   : workstation zone; max 6 m from façade (European Lighting Standard)
///   Z2 – Magazine  : storage zone beyond Z1
///   Z3 – Corridor  : central corridor shared by both sides of the floor plate
#[derive(Parser)]
#[command(name = "tool-buildingwidth", version)]
struct Cli {
    /// Key Plan category — uses V3 zone depths from DISCOVERY_MCorp_Sketches_Key_Plans_Summary
    #[arg(long, conflicts_with_all = ["z1", "z2", "z3"])]
    category: Option<Category>,

    /// Net leasable area (m²) for frontage calculation (optional)
    #[arg(long)]
    area: Option<f64>,

    /// Override Z1 – Habitat depth (m)
    #[arg(long, requires_all = ["z2", "z3"])]
    z1: Option<f64>,

    /// Override Z2 – Magazine depth (m)
    #[arg(long, requires_all = ["z1", "z3"])]
    z2: Option<f64>,

    /// Override Z3 – Corridor depth (m)
    #[arg(long, requires_all = ["z1", "z2"])]
    z3: Option<f64>,

    /// Output format
    #[arg(long, default_value = "text")]
    format: Format,
}

#[derive(Clone, ValueEnum)]
enum Category {
    PrivateOffice,
    Medical,
    Business,
    Laboratory,
    Academic,
    Civic,
}

#[derive(Clone, ValueEnum)]
enum Format {
    Text,
    Json,
}

struct ZoneDepths {
    z1: f64,
    z2: f64,
    z3: f64,
}

impl ZoneDepths {
    /// V3 Jan 2026 — source: DISCOVERY_MCorp_Sketches_Key_Plans_Summary.pdf
    /// Confirmed against CONSTRUCTION_2026_01_06_Key_Plan_Professional_Office_FFE_FIN.xlsx
    fn from_category(cat: &Category) -> Self {
        match cat {
            Category::PrivateOffice => Self {
                z1: 6.0,
                z2: 3.8,
                z3: 2.0,
            },
            Category::Medical => Self {
                z1: 7.2,
                z2: 4.9,
                z3: 2.9,
            },
            Category::Business => Self {
                z1: 6.0,
                z2: 7.3,
                z3: 2.7,
            },
            Category::Laboratory => Self {
                z1: 6.8,
                z2: 4.8,
                z3: 3.0,
            },
            Category::Academic => Self {
                z1: 4.7,
                z2: 3.0,
                z3: 0.0,
            },
            Category::Civic => Self {
                z1: 6.0,
                z2: 7.2,
                z3: 3.6,
            },
        }
    }

    fn category_name(cat: &Category) -> &'static str {
        match cat {
            Category::PrivateOffice => "private-office",
            Category::Medical => "medical",
            Category::Business => "business",
            Category::Laboratory => "laboratory",
            Category::Academic => "academic",
            Category::Civic => "civic",
        }
    }

    /// Half-width: one side of the floor plate (habitat + magazine, no corridor)
    fn half_width(&self) -> f64 {
        self.z1 + self.z2
    }

    /// Full double-loaded width: both sides + central corridor
    fn full_width(&self) -> f64 {
        2.0 * self.half_width() + self.z3
    }

    /// Frontage (m) = net leasable area divided by the half-width depth
    /// (area_m2 per side = area / 2 for a double-loaded plate; frontage = that ÷ half_width)
    fn frontage(&self, area_m2: f64) -> f64 {
        area_m2 / self.half_width()
    }
}

fn m_to_ft(m: f64) -> f64 {
    m * 3.28084
}

fn main() {
    let cli = Cli::parse();

    let (depths, cat_label) = match (&cli.category, cli.z1, cli.z2, cli.z3) {
        (Some(cat), None, None, None) => {
            let d = ZoneDepths::from_category(cat);
            let label = ZoneDepths::category_name(cat).to_string();
            (d, label)
        }
        (None, Some(z1), Some(z2), Some(z3)) => (ZoneDepths { z1, z2, z3 }, "custom".to_string()),
        _ => {
            eprintln!("error: provide either --category or all three of --z1 --z2 --z3");
            std::process::exit(1);
        }
    };

    let half_w = depths.half_width();
    let full_w = depths.full_width();
    let frontage = cli.area.map(|a| depths.frontage(a));

    match cli.format {
        Format::Json => {
            let mut obj = json!({
                "category": cat_label,
                "zone_depths_m": {
                    "z1_habitat":  depths.z1,
                    "z2_magazine": depths.z2,
                    "z3_corridor": depths.z3,
                },
                "half_width_m":  half_w,
                "full_width_m":  full_w,
                "half_width_ft": m_to_ft(half_w),
                "full_width_ft": m_to_ft(full_w),
            });
            if let (Some(area), Some(fr)) = (cli.area, frontage) {
                obj["area_m2"] = json!(area);
                obj["frontage_m"] = json!(fr);
                obj["frontage_ft"] = json!(m_to_ft(fr));
            }
            println!("{}", serde_json::to_string_pretty(&obj).unwrap());
        }
        Format::Text => {
            println!("Building Width Calculator — {cat_label}");
            println!();
            println!("Zone depths");
            println!(
                "  Z1 – Habitat   {:5.2} m  ({:.1} ft)",
                depths.z1,
                m_to_ft(depths.z1)
            );
            println!(
                "  Z2 – Magazine  {:5.2} m  ({:.1} ft)",
                depths.z2,
                m_to_ft(depths.z2)
            );
            println!(
                "  Z3 – Corridor  {:5.2} m  ({:.1} ft)",
                depths.z3,
                m_to_ft(depths.z3)
            );
            println!();
            println!("Floor plate (double-loaded)");
            println!(
                "  Half-width     {:5.2} m  ({:.1} ft)  [Z1 + Z2, per side]",
                half_w,
                m_to_ft(half_w)
            );
            println!(
                "  Full width     {:5.2} m  ({:.1} ft)  [2 × (Z1 + Z2) + Z3]",
                full_w,
                m_to_ft(full_w)
            );
            if let (Some(area), Some(fr)) = (cli.area, frontage) {
                println!();
                println!(
                    "Leasable area    {:6.2} m²  ({:.1} SF)",
                    area,
                    area * 10.7639
                );
                println!("Façade frontage  {:5.2} m  ({:.1} ft)", fr, m_to_ft(fr));
            }
        }
    }
}
