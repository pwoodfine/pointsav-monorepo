use serde_json::Value;
use std::{collections::HashMap, fs, path::Path};

pub fn load_tokens(
    design_system_dir: &Path,
) -> Result<HashMap<String, Value>, Box<dyn std::error::Error>> {
    let bim_dir = design_system_dir.join("tokens").join("bim");
    let mut map = HashMap::new();
    if !bim_dir.exists() {
        eprintln!("warn: BIM token dir not found: {}", bim_dir.display());
        return Ok(map);
    }
    for entry in fs::read_dir(&bim_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let stem = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .trim_end_matches(".dtcg.json")
            .to_string();
        let raw = fs::read_to_string(&path)?;
        match serde_json::from_str::<Value>(&raw) {
            Ok(v) => {
                map.insert(stem, v);
            }
            Err(e) => eprintln!("warn: failed to parse {}: {e}", path.display()),
        }
    }
    Ok(map)
}

pub struct CatMeta {
    pub display_name: &'static str,
    pub ifc_anchor: &'static str,
    pub intro: &'static str,
    pub elements: &'static str,
    pub card_desc: &'static str,
}

pub const SIDEBAR_ORDER: &[(&str, &str)] = &[
    ("spatial", "Spatial"),
    ("elements", "Elements"),
    ("systems", "Systems"),
    ("materials", "Materials"),
    ("assemblies", "Assemblies"),
    ("performance", "Performance"),
    ("identity-codes", "Identity + Codes"),
    ("relationships", "Relationships"),
    ("key-plans", "Key Plans"),
    ("amenity-key-plan", "Amenity Key Plans"),
    ("retail-select", "Retail Select"),
    ("tech-industrial", "Tech Industrial"),
];

pub fn known_categories() -> HashMap<&'static str, CatMeta> {
    let mut m = HashMap::new();
    m.insert(
        "spatial",
        CatMeta {
            display_name: "Spatial",
            ifc_anchor: "IfcSpatialElement",
            intro: "Spatial elements define the hierarchy of a building's geography.",
            elements: "IfcSite · IfcBuilding · IfcBuildingStorey · IfcSpace · IfcZone",
            card_desc: "Spaces, levels, buildings, sites, and zones",
        },
    );
    m.insert(
        "elements",
        CatMeta {
            display_name: "Elements",
            ifc_anchor: "IfcBuiltElement",
            intro: "Built elements are the physical components of a building.",
            elements: "IfcWall · IfcSlab · IfcColumn · IfcBeam · IfcDoor · IfcWindow",
            card_desc: "Walls, slabs, columns, beams, doors, windows",
        },
    );
    m.insert(
        "systems",
        CatMeta {
            display_name: "Systems",
            ifc_anchor: "IfcDistributionElement",
            intro: "Distribution elements are MEP systems.",
            elements: "IfcDuctSegment · IfcPipeSegment · IfcCableSegment · IfcAirTerminal",
            card_desc: "HVAC, plumbing, electrical distribution",
        },
    );
    m.insert(
        "materials",
        CatMeta {
            display_name: "Materials",
            ifc_anchor: "IfcMaterial",
            intro: "Material BIM Objects carry thermal, structural, and environmental properties.",
            elements: "IfcMaterial · IfcMaterialLayer · IfcMaterialProfile",
            card_desc: "Material definitions with bSDD URI references",
        },
    );
    m.insert(
        "assemblies",
        CatMeta {
            display_name: "Assemblies",
            ifc_anchor: "IfcElementAssembly",
            intro: "Assemblies are hierarchical compositions of elements.",
            elements: "IfcCurtainWall · IfcStairFlight · IfcRamp · IfcTruss",
            card_desc: "Curtain walls, stair assemblies, roof systems",
        },
    );
    m.insert(
        "performance",
        CatMeta {
            display_name: "Performance",
            ifc_anchor: "IfcPropertySet",
            intro: "Performance tokens carry energy, thermal, acoustic, and fire properties.",
            elements: "Pset_SpaceThermalDesign · Pset_ZoneCommon · IfcQuantityArea",
            card_desc: "Thermal, acoustic, structural, and fire performance",
        },
    );
    m.insert(
        "identity-codes",
        CatMeta {
            display_name: "Identity + Codes",
            ifc_anchor: "IfcClassificationReference",
            intro: "Identity tokens anchor BIM Objects to external classification systems.",
            elements: "IfcClassificationReference · IfcClassification · IfcConstraint",
            card_desc: "Uniclass, OmniClass, MasterFormat, bSDD references",
        },
    );
    m.insert(
        "relationships",
        CatMeta {
            display_name: "Relationships",
            ifc_anchor: "IfcRel*",
            intro: "Relationship tokens define how building elements connect and interact.",
            elements: "IfcRelContainedInSpatialStructure · IfcRelAggregates · IfcRelAssociates",
            card_desc: "Aggregation, containment, and constraint relationships",
        },
    );
    m.insert("key-plans", CatMeta {
        display_name: "Key Plans",
        ifc_anchor: "IfcSpace",
        intro: "Key Plans are the smallest BIM Object unit — spatial programs defined by furniture placement and three-zone cross-section.",
        elements: "Private Office · Medical · Business · Laboratory · Academic · Civic · Corporate Office",
        card_desc: "Spatial programs with zone depths, furniture programs, and compliance data",
    });
    m.insert("amenity-key-plan", CatMeta {
        display_name: "Amenity Key Plans",
        ifc_anchor: "IfcSpace",
        intro: "Non-leasable building amenity and service spaces for Professional Centre and Suburban Office development classes.",
        elements: "IfcSpace · IfcTransportElement · IfcAnnotation",
        card_desc: "Tenant Lounge · Lobby Atrium · Loading · Restrooms · Coffee/Bread · Building service rooms",
    });
    m.insert("retail-select", CatMeta {
        display_name: "Retail Select",
        ifc_anchor: "IfcSpace",
        intro: "Single-storey neighbourhood retail leaseholds where the Tile equals the Floor Plate — no building core deduction.",
        elements: "IfcSpace",
        card_desc: "RA-1 Small (4,500 SF) · RB-2 Medium (6,700 SF) · RC-3 Large (7,700 SF)",
    });
    m.insert("tech-industrial", CatMeta {
        display_name: "Tech Industrial",
        ifc_anchor: "IfcSpace",
        intro: "Single-storey light-industrial and R&D leaseholds where the Tile equals the Floor Plate — no building core deduction.",
        elements: "IfcSpace",
        card_desc: "TI-1 Medium (7,200 SF) · TI-2 Large (8,400 SF) · TI-3 Extra Large (9,600 SF)",
    });
    m
}
