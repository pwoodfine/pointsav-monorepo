//! `xtask characterize` — characterization-test harness for the
//! privategit NG rewrite (app-privategit-source-2 / app-privategit-marketplace-2).
//!
//! Two modes:
//!
//! ```text
//! cargo run -p xtask -- characterize snapshot [--out <dir>] [--source-port N] [--marketplace-port N]
//! cargo run -p xtask -- characterize diff <dir-a> <dir-b>
//! ```
//!
//! `snapshot` replays a FIXED set of read-only GET requests against the
//! running binaries (defaults: live ports 9201 / 9202) and writes one JSON
//! snapshot file per fixture. Only safe, non-mutating GETs are ever issued —
//! the fixture table below is the complete request set.
//!
//! `diff` compares two snapshot directories and reports differences; it is
//! used to compare old-binary baselines against new-binary output.
//!
//! Determinism: the volatile `date` and `server` response headers are
//! excluded; JSON bodies are parsed and re-serialized (BTreeMap key order)
//! for stable formatting; large/binary bodies are stored as sha256 + length.

use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

/// Which service a fixture targets. Resolved to a port at run time.
#[derive(Clone, Copy)]
enum Service {
    Source,      // app-privategit-source  (live: 9201)
    Marketplace, // app-privategit-marketplace (live: 9202)
}

/// The complete, fixed fixture set. READ-ONLY GET requests only.
const FIXTURES: &[(&str, Service, &str)] = &[
    // app-privategit-source
    ("healthz-source", Service::Source, "/healthz"),
    ("releases-index", Service::Source, "/releases/"),
    (
        "releases-os-network-admin",
        Service::Source,
        "/releases/os-network-admin/",
    ),
    (
        "releases-os-network-admin-latest-x86_64",
        Service::Source,
        "/releases/os-network-admin/latest/x86_64",
    ),
    (
        "manifest-app-privategit-source-0.1.0",
        Service::Source,
        "/releases/app-privategit-source/0.1.0/MANIFEST",
    ),
    (
        "releases-nonexistent-product",
        Service::Source,
        "/releases/nonexistent-product-xyz/",
    ),
    ("verify-key-pub", Service::Source, "/verify-key.pub"),
    // app-privategit-marketplace
    ("storefront-root", Service::Marketplace, "/"),
    ("storefront-software", Service::Marketplace, "/software"),
    ("healthz-marketplace", Service::Marketplace, "/healthz"),
    ("products-v1", Service::Marketplace, "/v1/products"),
];

/// Response headers excluded from snapshots (vary run to run).
const EXCLUDED_HEADERS: &[&str] = &["date", "server"];

/// Bodies larger than this that are not JSON are stored as sha256 + length.
const MAX_INLINE_BODY: usize = 65_536;

const DEFAULT_OUT_DIR: &str = "xtask/fixtures/ng-rewrite-baseline";

pub fn run(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("snapshot") => cmd_snapshot(&args[1..]),
        Some("diff") => cmd_diff(&args[1..]),
        _ => {
            eprintln!("usage: xtask characterize snapshot [--out <dir>] [--source-port N] [--marketplace-port N]");
            eprintln!("       xtask characterize diff <dir-a> <dir-b>");
            Err("characterize: expected mode 'snapshot' or 'diff'".to_string())
        }
    }
}

// ---------------------------------------------------------------------------
// snapshot mode
// ---------------------------------------------------------------------------

fn cmd_snapshot(args: &[String]) -> Result<(), String> {
    let mut out_dir = PathBuf::from(DEFAULT_OUT_DIR);
    let mut source_port: u16 = 9201;
    let mut marketplace_port: u16 = 9202;

    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--out" => {
                out_dir = PathBuf::from(it.next().ok_or("--out requires a directory argument")?);
            }
            "--source-port" => {
                source_port = parse_port(it.next(), "--source-port")?;
            }
            "--marketplace-port" => {
                marketplace_port = parse_port(it.next(), "--marketplace-port")?;
            }
            other => return Err(format!("snapshot: unknown argument '{other}'")),
        }
    }

    fs::create_dir_all(&out_dir)
        .map_err(|e| format!("cannot create {}: {e}", out_dir.display()))?;

    println!("[*] characterize snapshot -> {}", out_dir.display());
    println!(
        "    source port {source_port}, marketplace port {marketplace_port}, {} fixtures",
        FIXTURES.len()
    );

    let mut failures = 0usize;
    for (name, service, path) in FIXTURES {
        let port = match service {
            Service::Source => source_port,
            Service::Marketplace => marketplace_port,
        };
        match http_get(port, path) {
            Ok(resp) => {
                let snapshot = build_snapshot(name, port, path, &resp);
                let file = out_dir.join(format!("{name}.json"));
                let mut text = serde_json::to_string_pretty(&snapshot)
                    .map_err(|e| format!("serialize {name}: {e}"))?;
                text.push('\n');
                fs::write(&file, text).map_err(|e| format!("write {}: {e}", file.display()))?;
                println!(
                    "    [+] {name}: HTTP {} ({} body bytes)",
                    resp.status,
                    resp.body.len()
                );
            }
            Err(e) => {
                eprintln!("    [-] {name}: GET :{port}{path} failed: {e}");
                failures += 1;
            }
        }
    }

    if failures > 0 {
        return Err(format!("snapshot: {failures} fixture(s) failed"));
    }
    println!("[+] snapshot complete: {} fixtures written", FIXTURES.len());
    Ok(())
}

fn parse_port(v: Option<&String>, flag: &str) -> Result<u16, String> {
    v.ok_or(format!("{flag} requires a port argument"))?
        .parse::<u16>()
        .map_err(|e| format!("{flag}: invalid port: {e}"))
}

struct HttpResponse {
    status: u16,
    /// Header name (lowercased) -> values in wire order.
    headers: BTreeMap<String, Vec<String>>,
    body: Vec<u8>,
}

fn build_snapshot(name: &str, port: u16, path: &str, resp: &HttpResponse) -> Value {
    let mut headers = Map::new();
    for (k, vals) in &resp.headers {
        if EXCLUDED_HEADERS.contains(&k.as_str()) {
            continue;
        }
        if vals.len() == 1 {
            headers.insert(k.clone(), Value::String(vals[0].clone()));
        } else {
            headers.insert(
                k.clone(),
                Value::Array(vals.iter().cloned().map(Value::String).collect()),
            );
        }
    }

    let content_type = resp
        .headers
        .get("content-type")
        .and_then(|v| v.first())
        .cloned()
        .unwrap_or_default();

    let body = classify_body(&resp.body, &content_type);

    json!({
        "fixture": name,
        "request": {
            "method": "GET",
            "port": port,
            "path": path,
        },
        "status": resp.status,
        "headers": Value::Object(headers),
        "body": body,
    })
}

fn classify_body(body: &[u8], content_type: &str) -> Value {
    if body.is_empty() {
        return json!({ "kind": "empty" });
    }
    if content_type.contains("json") {
        if let Ok(v) = serde_json::from_slice::<Value>(body) {
            return json!({ "kind": "json", "json": v });
        }
    }
    if body.len() <= MAX_INLINE_BODY {
        if let Ok(text) = std::str::from_utf8(body) {
            return json!({ "kind": "text", "text": text });
        }
    }
    json!({
        "kind": "bytes",
        "sha256": hex::encode(Sha256::digest(body)),
        "length": body.len(),
    })
}

// ---------------------------------------------------------------------------
// Minimal HTTP/1.1 GET client (localhost only; Connection: close; no redirects)
// ---------------------------------------------------------------------------

fn http_get(port: u16, path: &str) -> Result<HttpResponse, String> {
    let addr = format!("127.0.0.1:{port}");
    let mut stream = TcpStream::connect(&addr).map_err(|e| format!("connect {addr}: {e}"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(30)))
        .map_err(|e| e.to_string())?;

    let request = format!(
        "GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nUser-Agent: xtask-characterize/0.1\r\nAccept: */*\r\nConnection: close\r\n\r\n"
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|e| format!("write request: {e}"))?;

    let mut raw = Vec::new();
    stream
        .read_to_end(&mut raw)
        .map_err(|e| format!("read response: {e}"))?;

    parse_response(&raw)
}

fn parse_response(raw: &[u8]) -> Result<HttpResponse, String> {
    let header_end =
        find_subslice(raw, b"\r\n\r\n").ok_or("malformed response: no header terminator")?;
    let head = std::str::from_utf8(&raw[..header_end])
        .map_err(|e| format!("non-UTF-8 response head: {e}"))?;
    let mut lines = head.split("\r\n");

    let status_line = lines.next().ok_or("empty response")?;
    let mut parts = status_line.splitn(3, ' ');
    let _version = parts.next().ok_or("malformed status line")?;
    let status: u16 = parts
        .next()
        .ok_or("malformed status line: no code")?
        .parse()
        .map_err(|e| format!("malformed status code: {e}"))?;

    let mut headers: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for line in lines {
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| format!("malformed header line: {line}"))?;
        headers
            .entry(name.trim().to_ascii_lowercase())
            .or_default()
            .push(value.trim().to_string());
    }

    let rest = &raw[header_end + 4..];
    let body = if headers
        .get("transfer-encoding")
        .map(|v| v.iter().any(|s| s.to_ascii_lowercase().contains("chunked")))
        .unwrap_or(false)
    {
        decode_chunked(rest)?
    } else if let Some(cl) = headers.get("content-length").and_then(|v| v.first()) {
        let n: usize = cl.parse().map_err(|e| format!("bad content-length: {e}"))?;
        if rest.len() < n {
            return Err(format!(
                "truncated body: content-length {n}, got {}",
                rest.len()
            ));
        }
        rest[..n].to_vec()
    } else {
        // Connection: close — body runs to EOF.
        rest.to_vec()
    };

    Ok(HttpResponse {
        status,
        headers,
        body,
    })
}

fn decode_chunked(mut rest: &[u8]) -> Result<Vec<u8>, String> {
    let mut body = Vec::new();
    loop {
        let line_end = find_subslice(rest, b"\r\n").ok_or("chunked: missing size line")?;
        let size_line = std::str::from_utf8(&rest[..line_end])
            .map_err(|e| format!("chunked: bad size line: {e}"))?;
        let size_hex = size_line.split(';').next().unwrap_or("").trim();
        let size = usize::from_str_radix(size_hex, 16)
            .map_err(|e| format!("chunked: bad chunk size '{size_hex}': {e}"))?;
        rest = &rest[line_end + 2..];
        if size == 0 {
            break; // trailers (if any) ignored
        }
        if rest.len() < size + 2 {
            return Err("chunked: truncated chunk".to_string());
        }
        body.extend_from_slice(&rest[..size]);
        rest = &rest[size + 2..]; // skip chunk CRLF
    }
    Ok(body)
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

// ---------------------------------------------------------------------------
// diff mode
// ---------------------------------------------------------------------------

fn cmd_diff(args: &[String]) -> Result<(), String> {
    if args.len() != 2 {
        return Err("diff: expected exactly two snapshot directories".to_string());
    }
    let dir_a = Path::new(&args[0]);
    let dir_b = Path::new(&args[1]);

    let set_a = load_snapshot_dir(dir_a)?;
    let set_b = load_snapshot_dir(dir_b)?;

    let mut differences = 0usize;

    for name in set_a.keys() {
        if !set_b.contains_key(name) {
            println!("[-] only in {}: {name}", dir_a.display());
            differences += 1;
        }
    }
    for name in set_b.keys() {
        if !set_a.contains_key(name) {
            println!("[-] only in {}: {name}", dir_b.display());
            differences += 1;
        }
    }

    for (name, a) in &set_a {
        let Some(b) = set_b.get(name) else { continue };
        if a == b {
            continue;
        }
        differences += 1;
        println!("[-] {name} differs:");
        for field in ["status", "headers", "body", "request"] {
            let (fa, fb) = (a.get(field), b.get(field));
            if fa != fb {
                println!("      {field}:");
                println!("        a: {}", compact(fa));
                println!("        b: {}", compact(fb));
            }
        }
    }

    if differences == 0 {
        println!(
            "[+] identical: {} fixtures match between {} and {}",
            set_a.len(),
            dir_a.display(),
            dir_b.display()
        );
        Ok(())
    } else {
        Err(format!("diff: {differences} difference(s) found"))
    }
}

fn compact(v: Option<&Value>) -> String {
    match v {
        Some(v) => {
            let s = serde_json::to_string(v).unwrap_or_else(|_| "<unserializable>".into());
            if s.len() > 400 {
                format!("{}… ({} chars)", &s[..400], s.len())
            } else {
                s
            }
        }
        None => "<absent>".to_string(),
    }
}

fn load_snapshot_dir(dir: &Path) -> Result<BTreeMap<String, Value>, String> {
    let mut out = BTreeMap::new();
    let entries = fs::read_dir(dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| format!("bad filename: {}", path.display()))?
            .to_string();
        let text =
            fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
        let value: Value =
            serde_json::from_str(&text).map_err(|e| format!("parse {}: {e}", path.display()))?;
        out.insert(name, value);
    }
    if out.is_empty() {
        return Err(format!("no *.json snapshots found in {}", dir.display()));
    }
    Ok(out)
}
