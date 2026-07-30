//! moonshot-bim-engine — IFC (ISO 16739) STEP-file structural parser.
//!
//! Replaces the structural-parse role of web-ifc (MPL) and xeokit (commercial) — the
//! dependency recorded as the `app-workplace-bim` licensing gate. v0 parses the
//! IFC-SPF container (STEP Physical File, ISO 10303-21): the HEADER records and the
//! DATA instances (`#id = ENTITY(params)`), string- and paren-aware so commas and
//! parentheses inside string literals or nested lists never mis-split. The full STEP
//! value grammar (typed values, enums, refs) and geometry tessellation are the
//! documented next layers.
//!
//! **SYS-ADR-07:** IFC is structured geometric/property data — it must never transit
//! any AI inference layer. This parser is deterministic Rust only. Zero dependencies,
//! WASM-ready; the IFC fiduciary record stays plain, open, and 50-year-readable.

/// One `#id = ENTITY(params)` record from the DATA section. `params` is the raw,
/// unsplit inner text (use [`split_params`] for top-level fields).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instance {
    pub id: u64,
    pub entity: String,
    pub params: String,
}

/// One `KEYWORD(params)` record from the HEADER section (FILE_DESCRIPTION, etc.).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub keyword: String,
    pub params: String,
}

/// A parsed IFC-SPF file: header records and data instances.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StepFile {
    pub header: Vec<Record>,
    pub data: Vec<Instance>,
}

impl StepFile {
    /// The instance with the given `#id`, if present.
    pub fn instance(&self, id: u64) -> Option<&Instance> {
        self.data.iter().find(|i| i.id == id)
    }

    /// Every instance of a given IFC entity type (e.g. `"IFCWALL"`).
    pub fn instances_of<'a>(&'a self, entity: &str) -> Vec<&'a Instance> {
        self.data.iter().filter(|i| i.entity == entity).collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// No `DATA;` section was found.
    MissingData,
    /// Structural break at the given byte offset.
    Malformed(usize),
}

/// Parse an IFC-SPF source into its header records and data instances.
pub fn parse(source: &str) -> Result<StepFile, ParseError> {
    let data_range = section_body(source, "DATA;").ok_or(ParseError::MissingData)?;
    let data = parse_instances(source, data_range)?;
    let header = match section_body(source, "HEADER;") {
        Some(r) => parse_records(source, r)?,
        None => Vec::new(),
    };
    Ok(StepFile { header, data })
}

/// Split a raw `params` string into its top-level fields, respecting STEP string
/// literals (`'...'` with `''` escape) and nested parentheses. Empty params -> `[]`.
pub fn split_params(params: &str) -> Vec<String> {
    if params.trim().is_empty() {
        return Vec::new();
    }
    let b = params.as_bytes();
    let n = b.len();
    let mut out = Vec::new();
    let mut i = 0;
    let mut field_start = 0;
    let mut depth = 0i32;
    while i < n {
        match b[i] {
            b'\'' => i = scan_string(b, i, n),
            b'(' => {
                depth += 1;
                i += 1;
            }
            b')' => {
                depth -= 1;
                i += 1;
            }
            b',' if depth == 0 => {
                out.push(params[field_start..i].trim().to_string());
                i += 1;
                field_start = i;
            }
            _ => i += 1,
        }
    }
    out.push(params[field_start..].trim().to_string());
    out
}

// ---- typed values -----------------------------------------------------------

/// A parsed STEP parameter value (ISO 10303-21). Reals carry `f64`, so `Value` is
/// `PartialEq` but not `Eq`. The grammar covered: references, integers, reals,
/// strings, enumerations, the unset (`$`) and derived (`*`) markers, lists, and
/// typed values (`KEYWORD(value)`). STEP `\X\`-style string encodings are kept
/// verbatim in v0; decoding them is a documented next layer.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// `#42` — a reference to another instance.
    Ref(u64),
    Integer(i64),
    Real(f64),
    /// A string literal with `''` escapes already decoded to `'`.
    Str(String),
    /// `.ADDED.` -> `Enum("ADDED")` (dots stripped).
    Enum(String),
    /// `$` — an unset optional attribute.
    Null,
    /// `*` — a derived attribute.
    Derived,
    List(Vec<Value>),
    /// `IFCBOOLEAN(.T.)` -> `Typed("IFCBOOLEAN", Enum("T"))`.
    Typed(String, Box<Value>),
    /// `"00AB"` — a binary/hex literal, kept verbatim.
    Binary(String),
}

impl Instance {
    /// Parse this instance's raw params into typed [`Value`]s (top-level fields).
    pub fn values(&self) -> Result<Vec<Value>, ParseError> {
        parse_value_list(&self.params)
    }
}

/// Parse a single STEP value from `s` (leading whitespace/comments allowed).
pub fn parse_value(s: &str) -> Result<Value, ParseError> {
    let b = s.as_bytes();
    let (v, _) = value_at(s, b, 0, b.len())?;
    Ok(v)
}

/// Parse a comma-separated list of top-level STEP values (an instance's params).
pub fn parse_value_list(params: &str) -> Result<Vec<Value>, ParseError> {
    if params.trim().is_empty() {
        return Ok(Vec::new());
    }
    let b = params.as_bytes();
    let end = b.len();
    let mut i = 0;
    let mut out = Vec::new();
    loop {
        let (v, ni) = value_at(params, b, i, end)?;
        out.push(v);
        i = skip_ws_comments(b, ni, end);
        if i >= end {
            break;
        }
        if b[i] == b',' {
            i += 1;
        } else {
            return Err(ParseError::Malformed(i));
        }
    }
    Ok(out)
}

/// Parse one value starting at `i`; returns the value and the index just past it.
fn value_at(s: &str, b: &[u8], i: usize, end: usize) -> Result<(Value, usize), ParseError> {
    let i = skip_ws_comments(b, i, end);
    if i >= end {
        return Err(ParseError::Malformed(i));
    }
    match b[i] {
        b'$' => Ok((Value::Null, i + 1)),
        b'*' => Ok((Value::Derived, i + 1)),
        b'#' => {
            let st = i + 1;
            let mut j = st;
            while j < end && b[j].is_ascii_digit() {
                j += 1;
            }
            if j == st {
                return Err(ParseError::Malformed(i));
            }
            let id = s[st..j].parse().map_err(|_| ParseError::Malformed(st))?;
            Ok((Value::Ref(id), j))
        }
        b'\'' => {
            let e = scan_string(b, i, end);
            let inner = &s[i + 1..e - 1];
            Ok((Value::Str(inner.replace("''", "'")), e))
        }
        b'"' => {
            let mut j = i + 1;
            while j < end && b[j] != b'"' {
                j += 1;
            }
            let inner = s[i + 1..j.min(end)].to_string();
            Ok((Value::Binary(inner), (j + 1).min(end)))
        }
        b'.' => {
            let st = i + 1;
            let mut j = st;
            while j < end && b[j] != b'.' {
                j += 1;
            }
            Ok((Value::Enum(s[st..j].to_string()), (j + 1).min(end)))
        }
        b'(' => {
            let (items, ni) = list_at(s, b, i + 1, end)?;
            Ok((Value::List(items), ni))
        }
        c if c == b'+' || c == b'-' || c.is_ascii_digit() => {
            let st = i;
            let mut j = i + 1;
            let mut real = false;
            while j < end {
                let d = b[j];
                if d.is_ascii_digit() {
                    j += 1;
                } else if d == b'.' || d == b'e' || d == b'E' {
                    real = true;
                    j += 1;
                } else if (d == b'+' || d == b'-') && (b[j - 1] == b'e' || b[j - 1] == b'E') {
                    j += 1;
                } else {
                    break;
                }
            }
            let tok = &s[st..j];
            if real {
                let r: f64 = tok.parse().map_err(|_| ParseError::Malformed(st))?;
                Ok((Value::Real(r), j))
            } else {
                let n: i64 = tok.parse().map_err(|_| ParseError::Malformed(st))?;
                Ok((Value::Integer(n), j))
            }
        }
        c if c.is_ascii_alphabetic() || c == b'_' => {
            let st = i;
            let mut j = i;
            while j < end && (b[j].is_ascii_alphanumeric() || b[j] == b'_') {
                j += 1;
            }
            let kw = s[st..j].to_string();
            let j2 = skip_ws_comments(b, j, end);
            if j2 < end && b[j2] == b'(' {
                let (inner, nj) = value_at(s, b, j2 + 1, end)?;
                let nj = skip_ws_comments(b, nj, end);
                if nj >= end || b[nj] != b')' {
                    return Err(ParseError::Malformed(nj));
                }
                Ok((Value::Typed(kw, Box::new(inner)), nj + 1))
            } else {
                Ok((Value::Enum(kw), j))
            }
        }
        _ => Err(ParseError::Malformed(i)),
    }
}

/// Parse list items starting just past `(`; returns items and the index past `)`.
fn list_at(s: &str, b: &[u8], mut i: usize, end: usize) -> Result<(Vec<Value>, usize), ParseError> {
    let mut items = Vec::new();
    loop {
        i = skip_ws_comments(b, i, end);
        if i >= end {
            return Err(ParseError::Malformed(i));
        }
        if b[i] == b')' {
            return Ok((items, i + 1));
        }
        let (v, ni) = value_at(s, b, i, end)?;
        items.push(v);
        i = skip_ws_comments(b, ni, end);
        if i < end && b[i] == b',' {
            i += 1;
        } else if i < end && b[i] == b')' {
            return Ok((items, i + 1));
        } else {
            return Err(ParseError::Malformed(i));
        }
    }
}

// ---- internals --------------------------------------------------------------

/// Byte range between a section keyword (e.g. `"DATA;"`) and its following `ENDSEC;`.
fn section_body(source: &str, kw: &str) -> Option<(usize, usize)> {
    let k = source.find(kw)?;
    let start = k + kw.len();
    let end_rel = source[start..].find("ENDSEC;")?;
    Some((start, start + end_rel))
}

fn skip_ws_comments(b: &[u8], mut i: usize, end: usize) -> usize {
    loop {
        while i < end && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if i + 1 < end && b[i] == b'/' && b[i + 1] == b'*' {
            i += 2;
            while i + 1 < end && !(b[i] == b'*' && b[i + 1] == b'/') {
                i += 1;
            }
            i = if i + 1 < end { i + 2 } else { end };
        } else {
            break;
        }
    }
    i
}

/// `i` points at the opening `'`. Returns the index just past the closing quote,
/// treating `''` as an escaped quote.
fn scan_string(b: &[u8], mut i: usize, end: usize) -> usize {
    i += 1;
    while i < end {
        if b[i] == b'\'' {
            if i + 1 < end && b[i + 1] == b'\'' {
                i += 2;
            } else {
                return i + 1;
            }
        } else {
            i += 1;
        }
    }
    i
}

/// `i` points at `(`. Returns `(inner_start, inner_end, after_close)` for the
/// balanced group, string-aware.
fn scan_group(b: &[u8], mut i: usize, end: usize) -> Result<(usize, usize, usize), ParseError> {
    let open = i;
    i += 1;
    let inner_start = i;
    let mut depth = 1i32;
    while i < end && depth > 0 {
        match b[i] {
            b'\'' => i = scan_string(b, i, end),
            b'(' => {
                depth += 1;
                i += 1;
            }
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Ok((inner_start, i, i + 1));
                }
                i += 1;
            }
            _ => i += 1,
        }
    }
    Err(ParseError::Malformed(open))
}

fn parse_instances(
    source: &str,
    (start, end): (usize, usize),
) -> Result<Vec<Instance>, ParseError> {
    let b = source.as_bytes();
    let mut cur = start;
    let mut out = Vec::new();
    loop {
        cur = skip_ws_comments(b, cur, end);
        if cur >= end {
            break;
        }
        if b[cur] != b'#' {
            // Resync to the next instance rather than failing the whole parse.
            match (cur + 1..end).find(|&j| b[j] == b'#') {
                Some(p) => {
                    cur = p;
                    continue;
                }
                None => break,
            }
        }
        cur += 1;
        let id_start = cur;
        while cur < end && b[cur].is_ascii_digit() {
            cur += 1;
        }
        if cur == id_start {
            return Err(ParseError::Malformed(cur));
        }
        let id: u64 = source[id_start..cur]
            .parse()
            .map_err(|_| ParseError::Malformed(id_start))?;
        cur = skip_ws_comments(b, cur, end);
        if cur >= end || b[cur] != b'=' {
            return Err(ParseError::Malformed(cur));
        }
        cur = skip_ws_comments(b, cur + 1, end);
        let kw_start = cur;
        while cur < end && (b[cur].is_ascii_alphanumeric() || b[cur] == b'_') {
            cur += 1;
        }
        let entity = source[kw_start..cur].to_string();
        cur = skip_ws_comments(b, cur, end);
        if cur >= end || b[cur] != b'(' {
            return Err(ParseError::Malformed(cur));
        }
        let (ps, pe, after) = scan_group(b, cur, end)?;
        out.push(Instance {
            id,
            entity,
            params: source[ps..pe].to_string(),
        });
        cur = skip_ws_comments(b, after, end);
        if cur < end && b[cur] == b';' {
            cur += 1;
        }
    }
    Ok(out)
}

fn parse_records(source: &str, (start, end): (usize, usize)) -> Result<Vec<Record>, ParseError> {
    let b = source.as_bytes();
    let mut cur = start;
    let mut out = Vec::new();
    loop {
        cur = skip_ws_comments(b, cur, end);
        if cur >= end {
            break;
        }
        let kw_start = cur;
        while cur < end && (b[cur].is_ascii_alphanumeric() || b[cur] == b'_') {
            cur += 1;
        }
        if cur == kw_start {
            break; // not a record keyword
        }
        let keyword = source[kw_start..cur].to_string();
        cur = skip_ws_comments(b, cur, end);
        if cur >= end || b[cur] != b'(' {
            return Err(ParseError::Malformed(cur));
        }
        let (ps, pe, after) = scan_group(b, cur, end)?;
        out.push(Record {
            keyword,
            params: source[ps..pe].to_string(),
        });
        cur = skip_ws_comments(b, after, end);
        if cur < end && b[cur] == b';' {
            cur += 1;
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "ISO-10303-21;
HEADER;
FILE_DESCRIPTION(('ViewDefinition [CoordinationView]'),'2;1');
FILE_NAME('example.ifc','2024-01-01T00:00:00',(''),(''),'','','');
FILE_SCHEMA(('IFC4'));
ENDSEC;
DATA;
#1=IFCPROJECT('0YvhhKNkr0kugbFTf53O9L',#2,'My Project',$,$,$,$,(#11),#7);
#2=IFCOWNERHISTORY(#3,#6,$,.ADDED.,$,$,$,1234);
#11=IFCGEOMETRICREPRESENTATIONCONTEXT($,'Model',3,1.0E-5,#12,$);
ENDSEC;
END-ISO-10303-21;
";

    #[test]
    fn parses_data_instances() {
        let f = parse(SAMPLE).unwrap();
        assert_eq!(f.data.len(), 3);
        assert_eq!(f.data[0].id, 1);
        assert_eq!(f.data[0].entity, "IFCPROJECT");
        assert_eq!(f.instance(2).unwrap().entity, "IFCOWNERHISTORY");
        assert_eq!(
            f.instance(11).unwrap().entity,
            "IFCGEOMETRICREPRESENTATIONCONTEXT"
        );
        assert!(f.instance(99).is_none());
    }

    #[test]
    fn parses_header_records() {
        let f = parse(SAMPLE).unwrap();
        let kws: Vec<&str> = f.header.iter().map(|r| r.keyword.as_str()).collect();
        assert_eq!(kws, ["FILE_DESCRIPTION", "FILE_NAME", "FILE_SCHEMA"]);
        assert_eq!(f.header[2].params, "('IFC4')");
    }

    #[test]
    fn instances_of_filters_by_entity() {
        let f = parse(SAMPLE).unwrap();
        let projects = f.instances_of("IFCPROJECT");
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].id, 1);
    }

    #[test]
    fn split_params_is_string_and_paren_aware() {
        let f = parse(SAMPLE).unwrap();
        let fields = split_params(&f.instance(1).unwrap().params);
        // 9 fields; the list (#11) and the strings stay intact.
        assert_eq!(fields.len(), 9);
        assert_eq!(fields[0], "'0YvhhKNkr0kugbFTf53O9L'");
        assert_eq!(fields[1], "#2");
        assert_eq!(fields[2], "'My Project'");
        assert_eq!(fields[3], "$");
        assert_eq!(fields[7], "(#11)"); // nested list not split
    }

    #[test]
    fn commas_and_parens_inside_strings_do_not_split() {
        // The string field contains a comma and parens — must remain one field.
        let p = "'a, b (c)',#5,'x'";
        let fields = split_params(p);
        assert_eq!(fields, vec!["'a, b (c)'", "#5", "'x'"]);
    }

    #[test]
    fn escaped_quotes_in_strings_are_handled() {
        let src = "DATA;\n#1=IFCLABEL('it''s a wall',#2);\nENDSEC;\n";
        let f = parse(src).unwrap();
        let fields = split_params(&f.instance(1).unwrap().params);
        assert_eq!(fields[0], "'it''s a wall'");
        assert_eq!(fields[1], "#2");
    }

    #[test]
    fn missing_data_section_is_an_error() {
        assert_eq!(
            parse("ISO-10303-21;\nHEADER;\nENDSEC;\n"),
            Err(ParseError::MissingData)
        );
    }

    #[test]
    fn split_params_empty_is_no_fields() {
        assert!(split_params("").is_empty());
        assert!(split_params("   ").is_empty());
    }

    #[test]
    fn parse_value_scalars() {
        assert_eq!(parse_value("#42").unwrap(), Value::Ref(42));
        assert_eq!(parse_value("$").unwrap(), Value::Null);
        assert_eq!(parse_value("*").unwrap(), Value::Derived);
        assert_eq!(parse_value("1234").unwrap(), Value::Integer(1234));
        assert_eq!(parse_value("-3").unwrap(), Value::Integer(-3));
        assert_eq!(parse_value(".ADDED.").unwrap(), Value::Enum("ADDED".into()));
        assert_eq!(
            parse_value("'My Project'").unwrap(),
            Value::Str("My Project".into())
        );
        assert_eq!(parse_value("'it''s'").unwrap(), Value::Str("it's".into()));
    }

    #[test]
    fn parse_value_real() {
        match parse_value("1.0E-5").unwrap() {
            Value::Real(r) => assert!((r - 1.0e-5).abs() < 1e-12),
            other => panic!("expected Real, got {other:?}"),
        }
    }

    #[test]
    fn parse_value_lists_and_typed() {
        assert_eq!(
            parse_value("(#11,#12)").unwrap(),
            Value::List(vec![Value::Ref(11), Value::Ref(12)])
        );
        assert_eq!(
            parse_value("IFCBOOLEAN(.T.)").unwrap(),
            Value::Typed("IFCBOOLEAN".into(), Box::new(Value::Enum("T".into())))
        );
        // Nested lists.
        assert_eq!(
            parse_value("((1,2),(3,4))").unwrap(),
            Value::List(vec![
                Value::List(vec![Value::Integer(1), Value::Integer(2)]),
                Value::List(vec![Value::Integer(3), Value::Integer(4)]),
            ])
        );
    }

    #[test]
    fn instance_values_decode_full_attribute_lists() {
        let f = parse(SAMPLE).unwrap();
        // #2 IFCOWNERHISTORY(#3,#6,$,.ADDED.,$,$,$,1234)
        let v = f.instance(2).unwrap().values().unwrap();
        assert_eq!(
            v,
            vec![
                Value::Ref(3),
                Value::Ref(6),
                Value::Null,
                Value::Enum("ADDED".into()),
                Value::Null,
                Value::Null,
                Value::Null,
                Value::Integer(1234),
            ]
        );
        // #1 IFCPROJECT — the (#11) attribute decodes as a single-element list.
        let p = f.instance(1).unwrap().values().unwrap();
        assert_eq!(p[2], Value::Str("My Project".into()));
        assert_eq!(p[7], Value::List(vec![Value::Ref(11)]));
        assert_eq!(p[8], Value::Ref(7));
    }
}
