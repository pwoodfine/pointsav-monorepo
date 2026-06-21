use crate::{parse, StepFile};
use wasm_bindgen::prelude::*;

fn escape_json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str(r#"\""#),
            '\\' => out.push_str(r#"\\"#),
            '\n' => out.push_str(r#"\n"#),
            '\r' => out.push_str(r#"\r"#),
            '\t' => out.push_str(r#"\t"#),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn step_file_to_json(sf: &StepFile) -> String {
    let mut out = String::from(r#"{"header":["#);
    for (i, r) in sf.header.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&format!(
            r#"{{"keyword":{},"params":{}}}"#,
            escape_json_str(&r.keyword),
            escape_json_str(&r.params)
        ));
    }
    out.push_str(r#"],"data":["#);
    for (i, inst) in sf.data.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&format!(
            r#"{{"id":{},"entity":{},"params":{}}}"#,
            inst.id,
            escape_json_str(&inst.entity),
            escape_json_str(&inst.params)
        ));
    }
    out.push_str("]}");
    out
}

/// Parse an IFC-SPF source string.
/// Returns a JSON string: `{header:[...], data:[...]}` on success,
/// or `{error: "..."}` on failure. Call `JSON.parse()` in JS.
#[wasm_bindgen]
pub fn wasm_parse(src: &str) -> String {
    match parse(src) {
        Ok(sf) => step_file_to_json(&sf),
        Err(e) => {
            let msg = match e {
                crate::ParseError::MissingData => "MissingData".to_string(),
                crate::ParseError::Malformed(offset) => format!("Malformed at byte {}", offset),
            };
            format!(r#"{{"error":{}}}"#, escape_json_str(&msg))
        }
    }
}

/// WASM-exposed wrapper holding a parsed IFC file for repeated queries.
#[wasm_bindgen]
pub struct WasmStepFile {
    inner: StepFile,
}

#[wasm_bindgen]
impl WasmStepFile {
    /// Parse an IFC-SPF source. Returns `null` on parse error.
    pub fn parse(src: &str) -> Option<WasmStepFile> {
        parse(src).ok().map(|inner| WasmStepFile { inner })
    }

    /// All instances of the given IFC entity type (e.g. `"IFCWALL"`).
    /// Returns a JSON string: `[{id, entity, params}, ...]`.
    pub fn instances_of(&self, entity: &str) -> String {
        let matches: Vec<_> = self.inner.instances_of(entity);
        let mut out = String::from("[");
        for (i, inst) in matches.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            out.push_str(&format!(
                r#"{{"id":{},"entity":{},"params":{}}}"#,
                inst.id,
                escape_json_str(&inst.entity),
                escape_json_str(&inst.params)
            ));
        }
        out.push(']');
        out
    }

    /// Total data instances in the file.
    pub fn instance_count(&self) -> u32 {
        self.inner.data.len() as u32
    }
}
