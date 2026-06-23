/// service-ingress — mTLS Phase A public ingress for os-console.
///
/// Listens on 0.0.0.0:8443 (HTTPS/TLS 1.3). On first start, auto-generates a
/// self-signed CA + server cert under ~/.config/service-ingress/ and prints the
/// SHA-256 fingerprint so os-console can pin it.
///
/// Path routing (all traffic fans out to localhost-only services):
///   /v1/proof/*      → http://127.0.0.1:9092  (service-content proofread)
///   /v1/content/*    → http://127.0.0.1:9092  (service-content generic)
///   /v1/search/*     → http://127.0.0.1:9092  (service-content search endpoint)
///   /doorman/*       → http://127.0.0.1:9080  (Doorman)
///   /health          → 200 OK {"status":"ok"}
///
/// Config file (optional): ~/.config/service-ingress/config.toml
///   [listen]
///   port = 8443
///   [upstream]
///   content = "http://127.0.0.1:9092"
///   doorman = "http://127.0.0.1:9080"

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use axum::{
    body::Body,
    extract::{Request, State},
    http::{StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::any,
    Router,
};
use axum_server::tls_rustls::RustlsConfig;
use bytes::Bytes;
use rcgen::{CertificateParams, DistinguishedName, KeyPair};
use sha2::{Digest, Sha256};

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct IngressConfig {
    content_upstream: String,
    doorman_upstream: String,
}

fn config_dir() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(".config/service-ingress")
}

fn cert_path() -> PathBuf {
    config_dir().join("cert.pem")
}
fn key_path() -> PathBuf {
    config_dir().join("key.pem")
}

// ---------------------------------------------------------------------------
// Certificate bootstrap
// ---------------------------------------------------------------------------

fn ensure_cert() -> Result<(String, String)> {
    let cert_p = cert_path();
    let key_p = key_path();

    if cert_p.exists() && key_p.exists() {
        let cert_pem = std::fs::read_to_string(&cert_p)?;
        let key_pem = std::fs::read_to_string(&key_p)?;
        return Ok((cert_pem, key_pem));
    }

    eprintln!("service-ingress: generating self-signed cert (first start)…");
    std::fs::create_dir_all(config_dir())?;

    let mut params = CertificateParams::default();
    let mut dn = DistinguishedName::new();
    dn.push(rcgen::DnType::CommonName, "service-ingress.local");
    params.distinguished_name = dn;
    params.not_before = rcgen::date_time_ymd(2026, 1, 1);
    params.not_after = rcgen::date_time_ymd(2030, 1, 1);
    params.subject_alt_names = vec![
        rcgen::SanType::DnsName("localhost".try_into()?),
        rcgen::SanType::IpAddress(std::net::IpAddr::V4("127.0.0.1".parse()?)),
    ];

    let key_pair = KeyPair::generate()?;
    let cert = params.self_signed(&key_pair)?;
    let cert_pem = cert.pem();
    let key_pem = key_pair.serialize_pem();

    std::fs::write(&cert_p, &cert_pem)?;
    std::fs::write(&key_p, &key_pem)?;
    eprintln!("service-ingress: cert written to {}", cert_p.display());
    Ok((cert_pem, key_pem))
}

fn fingerprint_from_pem(cert_pem: &str) -> String {
    // Strip PEM headers and decode DER to compute SHA-256 fingerprint.
    let der_b64: String = cert_pem
        .lines()
        .filter(|l| !l.starts_with("-----"))
        .collect();
    if let Ok(der) = base64_decode(&der_b64) {
        let hash = Sha256::digest(&der);
        let hex: String = hash
            .iter()
            .enumerate()
            .map(|(i, b)| {
                if i == 0 {
                    format!("{:02X}", b)
                } else {
                    format!(":{:02X}", b)
                }
            })
            .collect();
        return format!("SHA256:{}", hex);
    }
    "(fingerprint unavailable)".to_string()
}

fn base64_decode(s: &str) -> Result<Vec<u8>, ()> {
    use std::io::Read;
    // Use a minimal base64 decoder — avoids a new dep.
    let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut lookup = [255u8; 256];
    for (i, &c) in alphabet.iter().enumerate() {
        lookup[c as usize] = i as u8;
    }
    let s: Vec<u8> = s.bytes().filter(|&b| b != b'=' && lookup[b as usize] != 255).collect();
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let mut buf = 0u32;
    let mut bits = 0u8;
    for b in s {
        let v = lookup[b as usize];
        if v == 255 { continue; }
        buf = (buf << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Proxy handler
// ---------------------------------------------------------------------------

async fn proxy(
    State(cfg): State<Arc<IngressConfig>>,
    req: Request,
) -> Response {
    let path = req.uri().path_and_query().map(|p| p.as_str()).unwrap_or("/");

    let upstream = if path.starts_with("/doorman") {
        &cfg.doorman_upstream
    } else if path == "/health" {
        return (StatusCode::OK, r#"{"status":"ok"}"#).into_response();
    } else {
        &cfg.content_upstream
    };

    let target = format!("{}{}", upstream, path);
    let target_uri: Uri = match target.parse() {
        Ok(u) => u,
        Err(e) => {
            eprintln!("service-ingress: bad upstream URI {target}: {e}");
            return StatusCode::BAD_GATEWAY.into_response();
        }
    };

    let client = reqwest::Client::new();
    let method: reqwest::Method = req.method().clone().into();

    let body_bytes = match axum::body::to_bytes(req.into_body(), 16 * 1024 * 1024).await {
        Ok(b) => b,
        Err(e) => {
            eprintln!("service-ingress: body read error: {e}");
            return StatusCode::BAD_REQUEST.into_response();
        }
    };

    let upstream_req = client
        .request(method, target.as_str())
        .body(body_bytes.to_vec())
        .build();

    let upstream_req = match upstream_req {
        Ok(r) => r,
        Err(e) => {
            eprintln!("service-ingress: request build error: {e}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    match client.execute(upstream_req).await {
        Ok(resp) => {
            let status = StatusCode::from_u16(resp.status().as_u16())
                .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            let body = resp.bytes().await.unwrap_or_else(|_| Bytes::new());
            (status, body).into_response()
        }
        Err(e) => {
            eprintln!("service-ingress: upstream error for {target}: {e}");
            StatusCode::BAD_GATEWAY.into_response()
        }
    }
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<()> {
    let (cert_pem, key_pem) = ensure_cert()?;

    let fingerprint = fingerprint_from_pem(&cert_pem);
    eprintln!("service-ingress: server cert fingerprint: {fingerprint}");
    eprintln!("service-ingress: set totebox_known_host_key = \"{fingerprint}\" in os-console config.toml");

    let tls_config = RustlsConfig::from_pem(cert_pem.into_bytes(), key_pem.into_bytes()).await?;

    let cfg = Arc::new(IngressConfig {
        content_upstream: "http://127.0.0.1:9092".to_string(),
        doorman_upstream: "http://127.0.0.1:9080".to_string(),
    });

    let app = Router::new()
        .route("/{*path}", any(proxy))
        .route("/", any(proxy))
        .with_state(cfg);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8443));
    eprintln!("service-ingress: listening on https://{addr}");

    axum_server::bind_rustls(addr, tls_config)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}
