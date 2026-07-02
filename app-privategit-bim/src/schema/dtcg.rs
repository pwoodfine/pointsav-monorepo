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
    pub uniclass: &'static str,
    pub ifc_hierarchy: &'static str,
    pub intro: &'static str,
    pub elements: &'static str,
    pub card_desc: &'static str,
    pub property_sets: &'static [(&'static str, &'static str, &'static str)],
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
    m.insert("spatial", CatMeta {
        display_name: "Spatial",
        ifc_anchor: "IfcSpatialElement",
        uniclass: "SL",
        ifc_hierarchy: "IfcRoot → IfcObjectDefinition → IfcObject → IfcSpatialElement",
        intro: "Spatial elements define the hierarchy of a building's geography: site, building, storeys, and individual spaces. They are the containers that built elements occupy, and the entities that jurisdictional and climate zone constraints apply to at a zone level.",
        elements: "IfcSite · IfcBuilding · IfcBuildingStorey · IfcSpace · IfcZone",
        card_desc: "Spaces, levels (IfcBuildingStorey), buildings, sites, and zones",
        property_sets: &[
            ("Pset_SpaceCommon", "IsExternal", "BOOLEAN"),
            ("Pset_SpaceCommon", "NetFloorArea", "IfcAreaMeasure"),
            ("Pset_BuildingCommon", "NumberOfStoreys", "INTEGER"),
            ("Pset_SiteCommon", "BuildableArea", "IfcAreaMeasure"),
        ],
    });
    m.insert("elements", CatMeta {
        display_name: "Elements",
        ifc_anchor: "IfcBuiltElement",
        uniclass: "EE",
        ifc_hierarchy: "IfcRoot → IfcObjectDefinition → IfcObject → IfcElement → IfcBuiltElement",
        intro: "Built elements are the physical components of a building: walls, slabs, columns, beams, doors, windows, roofs, and stairs. They carry fire rating, structural, and performance constraints.",
        elements: "IfcWall · IfcSlab · IfcColumn · IfcBeam · IfcDoor · IfcWindow · IfcRoof · IfcStair",
        card_desc: "Walls, slabs, columns, beams, doors, windows, and other built elements",
        property_sets: &[
            ("Pset_WallCommon", "FireRating", "IfcLabel"),
            ("Pset_SlabCommon", "LoadBearing", "BOOLEAN"),
            ("Pset_DoorCommon", "IsFireExit", "BOOLEAN"),
            ("Pset_WindowCommon", "ThermalTransmittance", "IfcThermalTransmittanceMeasure"),
        ],
    });
    m.insert("systems", CatMeta {
        display_name: "Systems",
        ifc_anchor: "IfcDistributionElement",
        uniclass: "SS",
        ifc_hierarchy: "IfcRoot → IfcObjectDefinition → IfcObject → IfcElement → IfcDistributionElement",
        intro: "Distribution elements are mechanical, electrical, and plumbing (MEP) systems: ducts, pipes, conduits, outlets, and equipment.",
        elements: "IfcDuctSegment · IfcPipeSegment · IfcCableSegment · IfcAirTerminal · IfcFan · IfcPump",
        card_desc: "HVAC, plumbing, electrical distribution, and fire protection systems",
        property_sets: &[
            ("Pset_DuctSegmentTypeCommon", "NominalDiameter", "IfcPositiveLengthMeasure"),
            ("Pset_ElectricMotorTypeCommon", "PowerNominal", "IfcPowerMeasure"),
        ],
    });
    m.insert("materials", CatMeta {
        display_name: "Materials",
        ifc_anchor: "IfcMaterial",
        uniclass: "Pr",
        ifc_hierarchy: "IfcMaterial",
        intro: "Material BIM Objects carry thermal, structural, acoustic, and environmental properties anchored to bSDD URI references and Pset_Material* property sets.",
        elements: "IfcMaterial · IfcMaterialLayer · IfcMaterialProfile · IfcMaterialConstituent",
        card_desc: "Material definitions with bSDD URI references and Pset_Material* property sets",
        property_sets: &[
            ("Pset_MaterialCommon", "MassDensity", "IfcMassDensityMeasure"),
            ("Pset_MaterialOptical", "VisibleTransmittance", "IfcNormalisedRatioMeasure"),
            ("Pset_MaterialThermal", "ThermalConductivity", "IfcThermalConductivityMeasure"),
        ],
    });
    m.insert("assemblies", CatMeta {
        display_name: "Assemblies",
        ifc_anchor: "IfcElementAssembly",
        uniclass: "Co",
        ifc_hierarchy: "IfcRoot → IfcObjectDefinition → IfcObject → IfcElement → IfcElementAssembly",
        intro: "Assemblies are hierarchical compositions of elements that function as a unit: curtain walls, stairs with landings, structural frames, and prefabricated panels.",
        elements: "IfcCurtainWall · IfcStairFlight · IfcRamp · IfcTruss · IfcElementAssembly",
        card_desc: "Composite element assemblies — curtain walls, stair assemblies, roof systems",
        property_sets: &[
            ("Pset_ElementAssemblyCommon", "AssemblyPlace", "IfcAssemblyPlaceEnum"),
        ],
    });
    m.insert("performance", CatMeta {
        display_name: "Performance",
        ifc_anchor: "IfcPropertySet",
        uniclass: "—",
        ifc_hierarchy: "IfcPropertySet · IfcQuantitySet",
        intro: "Performance tokens carry energy, thermal, acoustic, and fire properties as IfcPropertySet and IfcQuantitySet entries. These are the specification values that drive compliance checking.",
        elements: "Pset_ThermalLoad · Pset_SpaceThermalDesign · Pset_ZoneCommon · IfcQuantityArea",
        card_desc: "Property sets expressing thermal, acoustic, structural, and fire performance",
        property_sets: &[
            ("Pset_SpaceThermalDesign", "HeatingDesignLoad", "IfcPowerMeasure"),
            ("Pset_SpaceThermalDesign", "CoolingDesignLoad", "IfcPowerMeasure"),
            ("Pset_ZoneCommon", "IsExternal", "BOOLEAN"),
        ],
    });
    m.insert("identity-codes", CatMeta {
        display_name: "Identity + Codes",
        ifc_anchor: "IfcClassificationReference",
        uniclass: "—",
        ifc_hierarchy: "IfcClassificationReference · IfcConstraint",
        intro: "Identity and classification tokens anchor BIM Objects to external classification systems (Uniclass 2015, OmniClass, CAWS) and jurisdictional code references.",
        elements: "IfcClassificationReference · IfcClassification · IfcConstraint · IfcMetric",
        card_desc: "Uniclass, OmniClass, MasterFormat, and bSDD classification references",
        property_sets: &[],
    });
    m.insert("relationships", CatMeta {
        display_name: "Relationships",
        ifc_anchor: "IfcRel*",
        uniclass: "—",
        ifc_hierarchy: "IfcRelationship",
        intro: "Relationship tokens define how building elements connect, contain, aggregate, and interact with each other through the IFC IfcRel* relationship entity family.",
        elements: "IfcRelContainedInSpatialStructure · IfcRelAggregates · IfcRelConnects · IfcRelAssociates",
        card_desc: "Aggregation, containment, nesting, and constraint relationship templates",
        property_sets: &[],
    });
    m.insert("key-plans", CatMeta {
        display_name: "Key Plans",
        ifc_anchor: "IfcSpace",
        uniclass: "SL_25",
        ifc_hierarchy: "IfcRoot → IfcObjectDefinition → IfcObject → IfcSpatialElement → IfcSpace",
        intro: "Key Plans are the smallest BIM Object unit — spatial programs defined by real furniture placement, a three-zone cross-section (Zone 1 Habitat / Zone 2 Magazine / Zone 3 Corridor), net leasable area, and accessibility compliance. Authored by architects from Woodfine equipment programs; the tool-buildingwidth engine nests them into Tiles and Floor Plates.",
        elements: "Private Office · Medical · Business · Laboratory · Academic · Civic · Corporate Office",
        card_desc: "Leasable spatial programs with zone depths, furniture programs, and compliance data",
        property_sets: &[
            ("Pset_SpaceCommon", "NetFloorArea", "IfcAreaMeasure"),
            ("Pset_SpaceCommon", "IsExternal", "BOOLEAN"),
            ("Pset_OccupancyRequirements", "OccupancyNumber", "INTEGER"),
        ],
    });
    m.insert("amenity-key-plan", CatMeta {
        display_name: "Amenity Key Plans",
        ifc_anchor: "IfcSpace",
        uniclass: "SL_25",
        ifc_hierarchy: "IfcRoot → IfcObjectDefinition → IfcObject → IfcSpatialElement → IfcSpace",
        intro: "Non-leasable building amenity and service spaces for Professional Centre and Suburban Office development classes.",
        elements: "IfcSpace · IfcTransportElement · IfcAnnotation",
        card_desc: "Tenant Lounge · Lobby Atrium · Loading · Restrooms · Coffee/Bread · Building service rooms",
        property_sets: &[],
    });
    m.insert("retail-select", CatMeta {
        display_name: "Retail Select",
        ifc_anchor: "IfcSpace",
        uniclass: "—",
        ifc_hierarchy: "IfcRoot → IfcObjectDefinition → IfcObject → IfcSpatialElement → IfcSpace",
        intro: "Single-storey neighbourhood retail leaseholds where the Tile equals the Floor Plate — no building core deduction.",
        elements: "IfcSpace",
        card_desc: "RA-1 Small (4,500 SF) · RB-2 Medium (6,700 SF) · RC-3 Large (7,700 SF)",
        property_sets: &[],
    });
    m.insert("tech-industrial", CatMeta {
        display_name: "Tech Industrial",
        ifc_anchor: "IfcSpace",
        uniclass: "—",
        ifc_hierarchy: "IfcRoot → IfcObjectDefinition → IfcObject → IfcSpatialElement → IfcSpace",
        intro: "Single-storey light-industrial and R&D leaseholds where the Tile equals the Floor Plate — no building core deduction.",
        elements: "IfcSpace",
        card_desc: "TI-1 Medium (7,200 SF) · TI-2 Large (8,400 SF) · TI-3 Extra Large (9,600 SF)",
        property_sets: &[],
    });
    m
}
