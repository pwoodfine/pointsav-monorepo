use jsonschema::Validator;
use serde_json::{json, Value};
use std::sync::OnceLock;

// PBS-1: PointSav BIM Schema v1 — structural envelope (compiled once at first validation)
static PBS1_COMPILED: OnceLock<Validator> = OnceLock::new();

fn pbs1_schema() -> &'static Validator {
    PBS1_COMPILED.get_or_init(|| {
        let schema = json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "title": "PBS-1 BIM Object",
            "type": "object",
            "required": ["bim"],
            "properties": {
                "bim": {
                    "type": "object",
                    "additionalProperties": {
                        "type": "object",
                        "additionalProperties": {
                            "type": "object",
                            "required": ["$type", "$value"],
                            "properties": {
                                "$type": { "type": "string", "const": "bim.entity" },
                                "$value": { "type": "object" }
                            }
                        }
                    }
                }
            }
        });
        Validator::new(&schema).expect("PBS-1 schema compilation failed")
    })
}

/// Validate a DTCG document against PBS-1.
/// Returns Ok(()) on success, Err(Vec<String>) with all error messages on failure.
pub fn validate_dtcg(doc: &Value) -> Result<(), Vec<String>> {
    let mut errors: Vec<String> = Vec::new();

    // Layer 1: JSON Schema structural check
    let compiled = pbs1_schema();
    if let Err(iter) = compiled.validate(doc) {
        for err in iter {
            errors.push(format!("{}: {}", err.instance_path, err));
        }
    }

    // Layer 2: PBS-1 semantic rules not expressible in JSON Schema v1
    if let Some(bim) = doc.get("bim").and_then(|v| v.as_object()) {
        for (cat, cat_val) in bim {
            let entities = match cat_val.as_object() {
                Some(o) => o,
                None => continue,
            };
            for (slug, entity) in entities {
                if slug.starts_with('$') {
                    continue;
                }
                let path = format!("bim.{cat}.{slug}");
                if let Some(v) = entity.get("$value") {
                    if let Some(obj) = v.as_object() {
                        let has_ifc =
                            obj.contains_key("ifc_class") || obj.contains_key("ifc_anchor");
                        let has_zone = obj.contains_key("zone1_depth_m");
                        let has_name = obj.contains_key("display_name");
                        if !has_ifc && !has_zone && !has_name {
                            errors.push(format!(
                                "{path}.$value: must have at least one of ifc_class, ifc_anchor, zone1_depth_m, or display_name"
                            ));
                        }
                    }
                }
            }
        }
    } else if errors.is_empty() {
        errors.push("missing required top-level 'bim' object".into());
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
