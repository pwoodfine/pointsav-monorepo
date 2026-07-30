// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// fs-anchor-emitter — Sigstore Rekor v2 monthly anchoring for service-fs checkpoints.
// Doctrine Invention #7.  ADR-07: no AI path anywhere in this binary.
//
// Flow:
//   1. GET /v1/checkpoint from service-fs
//   2. Build a Sigstore hashedrekord entry wrapping the checkpoint JSON
//      (ephemeral Ed25519 keypair per run; value is the Rekor timestamp +
//      inclusion proof, not key identity)
//   3. POST to rekor.sigstore.dev/api/v2/log/entries
//   4. POST the tlog entry back to service-fs /v1/append
//
// Exit codes:
//   0 — success
//   1 — env/config error
//   2 — checkpoint fetch failed
//   3 — Rekor submission failed
//   4 — service-fs append of anchor record failed

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use ed25519_dalek::{Signer, SigningKey};
use rand_core::OsRng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

// Rekor v2 default endpoint. Sigstore deploys log shards on a yearly
// rotation (`logYEAR-rev.rekor.sigstore.dev`) and explicitly warns
// against hardcoding any single URL — the 2025 instance will be turned
// down when the 2026 instance lands. The long-term-correct path is
// TUF-based SigningConfig discovery; until that is wired in, the
// `REKOR_URL` env var lets the operator pin the active shard without
// rebuilding the binary.
//
// The legacy v1 host `rekor.sigstore.dev/api/v2/log/entries` returns
// 404 (no v2 path on that host); v1 is live there at /api/v1/log/entries
// but uses a different response shape. Use a v2 shard host explicitly.
const DEFAULT_REKOR_URL: &str = "https://log2025-1.rekor.sigstore.dev/api/v2/log/entries";

// ── Config ────────────────────────────────────────────────────────────────────

#[derive(Debug)]
struct Config {
    fs_endpoint: String,
    module_id: String,
    rekor_url: String,
}

impl Config {
    fn from_env() -> Result<Self, String> {
        let fs_endpoint = std::env::var("FS_ENDPOINT")
            .map_err(|_| "FS_ENDPOINT not set".to_string())?;
        let module_id = std::env::var("FS_MODULE_ID")
            .map_err(|_| "FS_MODULE_ID not set".to_string())?;
        let rekor_url = std::env::var("REKOR_URL")
            .unwrap_or_else(|_| DEFAULT_REKOR_URL.to_string());
        Ok(Self { fs_endpoint, module_id, rekor_url })
    }
}

// ── Checkpoint (from service-fs /v1/checkpoint) ───────────────────────────────

#[derive(Debug, Deserialize, Serialize, Clone)]
struct Checkpoint {
    origin: String,
    tree_size: u64,
    root_hash: String,
    algorithm: Option<String>,
    timestamp: i64,
    signature: Option<String>,
    public_key: Option<String>,
}

fn fetch_checkpoint(
    client: &reqwest::blocking::Client,
    endpoint: &str,
    module_id: &str,
) -> Result<Checkpoint, String> {
    let url = format!("{}/v1/checkpoint", endpoint.trim_end_matches('/'));
    let resp = client
        .get(&url)
        .header("X-Foundry-Module-ID", module_id)
        .send()
        .map_err(|e| format!("checkpoint GET failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("checkpoint GET returned {}", resp.status()));
    }
    resp.json::<Checkpoint>()
        .map_err(|e| format!("checkpoint JSON parse failed: {e}"))
}

// ── SPKI DER encoding for Ed25519 verifying key ────────────────────────────────
//
// Ed25519 SubjectPublicKeyInfo (SPKI) DER structure — 44 bytes total:
//   30 2a                  SEQUENCE (42 bytes)
//     30 05                SEQUENCE (5 bytes) — AlgorithmIdentifier
//       06 03 2b 65 70     OID 1.3.101.112 (id-Ed25519)
//     03 21 00             BIT STRING (33 bytes, 0 unused bits)
//       <32-byte pub key>
//
// Rekor v2 (`hashedRekordRequestV002`) wants the raw DER bytes
// base64-encoded as `verifier.publicKey.rawBytes` — NOT a PEM string.
// The PEM wrapper here is retained for tests and for any future
// consumer that wants the conventional PEM textual form.

fn ed25519_spki_der(pub_key_bytes: &[u8; 32]) -> Vec<u8> {
    let mut der = Vec::with_capacity(44);
    der.extend_from_slice(&[0x30, 0x2a]);          // SEQUENCE
    der.extend_from_slice(&[0x30, 0x05]);          // AlgorithmIdentifier SEQUENCE
    der.extend_from_slice(&[0x06, 0x03, 0x2b, 0x65, 0x70]); // OID id-Ed25519
    der.extend_from_slice(&[0x03, 0x21, 0x00]);    // BIT STRING
    der.extend_from_slice(pub_key_bytes);
    der
}

#[allow(dead_code)] // Retained as a primitive for future consumers; tests exercise it.
fn ed25519_spki_pem(pub_key_bytes: &[u8; 32]) -> String {
    let der = ed25519_spki_der(pub_key_bytes);
    let b64_der = B64.encode(&der);
    format!(
        "-----BEGIN PUBLIC KEY-----\n{}\n-----END PUBLIC KEY-----\n",
        b64_der
    )
}

// ── Rekor v0.0.2 hashedRekordRequestV002 entry types ───────────────────────────
//
// The v0.0.2 body shape per rekor-tiles `api/proto/rekor/v2/`:
//
//   {
//     "hashedRekordRequestV002": {
//       "digest": "<base64 raw SHA-256 bytes>",
//       "signature": {
//         "content": "<base64 raw Ed25519 signature bytes>",
//         "verifier": {
//           "publicKey": { "rawBytes": "<base64 raw 44-byte SPKI DER>" },
//           "keyDetails": "PKIX_ED25519"
//         }
//       }
//     }
//   }
//
// Three breaking changes from v0.0.1: top-level envelope removed
// (no `kind`/`apiVersion`); `digest` is base64 raw bytes at the
// top of the request (not a hex string under `data.hash.value`);
// `signature.format` removed (verifier kind is communicated via
// the `verifier` oneof — `publicKey` here, alternatively
// `x509Certificate`).
//
// `keyDetails` is the PublicKeyDetails enum from sigstore_common;
// `PKIX_ED25519` (= 7) is the correct value for our ephemeral
// Ed25519 keys signing arbitrary artifact bytes (not pre-hashed,
// so NOT `PKIX_ED25519_PH`).

const KEY_DETAILS_ED25519: &str = "PKIX_ED25519";

#[derive(Serialize)]
struct RekorPublicKey {
    #[serde(rename = "rawBytes")]
    raw_bytes: String, // base64-encoded raw DER (NOT PEM)
}

#[derive(Serialize)]
struct RekorVerifier {
    #[serde(rename = "publicKey")]
    public_key: RekorPublicKey,
    #[serde(rename = "keyDetails")]
    key_details: String,
}

#[derive(Serialize)]
struct RekorSignature {
    content: String, // base64-encoded raw Ed25519 signature
    verifier: RekorVerifier,
}

#[derive(Serialize)]
struct HashedRekordRequestV002 {
    digest: String, // base64-encoded raw SHA-256 bytes (NOT hex)
    signature: RekorSignature,
}

#[derive(Serialize)]
struct RekorRequest {
    #[serde(rename = "hashedRekordRequestV002")]
    request: HashedRekordRequestV002,
}

// ── Rekor submission ───────────────────────────────────────────────────────────

fn post_to_rekor(
    client: &reqwest::blocking::Client,
    rekor_url: &str,
    checkpoint: &Checkpoint,
) -> Result<serde_json::Value, String> {
    // Serialize checkpoint as the artifact being anchored.
    let artifact_json = serde_json::to_vec(checkpoint)
        .map_err(|e| format!("checkpoint serialisation failed: {e}"))?;

    // SHA-256 of the artifact. v0.0.2 sends the raw 32-byte digest
    // base64-encoded — NOT hex.
    let digest_bytes = Sha256::digest(&artifact_json);
    let digest_b64 = B64.encode(digest_bytes);

    // Ephemeral Ed25519 keypair — value is the Rekor timestamp + inclusion proof.
    let signing_key = SigningKey::generate(&mut OsRng);
    let verifying_key = signing_key.verifying_key();

    // Sign the artifact bytes.
    let signature = signing_key.sign(&artifact_json);
    let sig_b64 = B64.encode(signature.to_bytes());

    // Raw 44-byte SPKI DER for the verifying key, base64-encoded.
    // v0.0.2 wants `verifier.publicKey.rawBytes` as base64-of-DER,
    // NOT base64-of-PEM-string (the v0.0.1 shape was the latter).
    let der = ed25519_spki_der(verifying_key.as_bytes());
    let der_b64 = B64.encode(&der);

    let request = RekorRequest {
        request: HashedRekordRequestV002 {
            digest: digest_b64,
            signature: RekorSignature {
                content: sig_b64,
                verifier: RekorVerifier {
                    public_key: RekorPublicKey { raw_bytes: der_b64 },
                    key_details: KEY_DETAILS_ED25519.to_string(),
                },
            },
        },
    };

    let resp = client
        .post(rekor_url)
        .json(&request)
        .send()
        .map_err(|e| format!("Rekor POST failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().unwrap_or_default();
        return Err(format!("Rekor returned {status}: {body}"));
    }

    resp.json::<serde_json::Value>()
        .map_err(|e| format!("Rekor response JSON parse failed: {e}"))
}

// ── Write anchor record back to service-fs ─────────────────────────────────────

#[derive(Serialize)]
struct AppendRequest {
    module_id: String,
    payload: serde_json::Value,
    payload_id: String,
}

fn write_anchor(
    client: &reqwest::blocking::Client,
    endpoint: &str,
    module_id: &str,
    tlog_entry: serde_json::Value,
) -> Result<(), String> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let payload_id = format!("anchor-rekor-{now}");

    let url = format!("{}/v1/append", endpoint.trim_end_matches('/'));
    let body = AppendRequest {
        module_id: module_id.to_string(),
        payload: tlog_entry,
        payload_id,
    };

    let resp = client
        .post(&url)
        .header("X-Foundry-Module-ID", module_id)
        .json(&body)
        .send()
        .map_err(|e| format!("service-fs append POST failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("service-fs append returned {}", resp.status()));
    }
    Ok(())
}

// ── Entry point ────────────────────────────────────────────────────────────────

fn main() {
    let cfg = match Config::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("config error: {e}");
            std::process::exit(1);
        }
    };

    let client = reqwest::blocking::Client::builder()
        .use_rustls_tls()
        .build()
        .expect("reqwest client build failed");

    let checkpoint = match fetch_checkpoint(&client, &cfg.fs_endpoint, &cfg.module_id) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("checkpoint fetch failed: {e}");
            std::process::exit(2);
        }
    };

    let tlog_entry = match post_to_rekor(&client, &cfg.rekor_url, &checkpoint) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Rekor submission failed: {e}");
            std::process::exit(3);
        }
    };

    if let Err(e) = write_anchor(&client, &cfg.fs_endpoint, &cfg.module_id, tlog_entry) {
        eprintln!("service-fs anchor append failed: {e}");
        std::process::exit(4);
    }

    println!("anchor emitted successfully");
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_from_env_missing_fs_endpoint() {
        std::env::remove_var("FS_ENDPOINT");
        std::env::remove_var("FS_MODULE_ID");
        let result = Config::from_env();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("FS_ENDPOINT"));
    }

    #[test]
    fn config_from_env_missing_module_id() {
        std::env::set_var("FS_ENDPOINT", "http://localhost:9100");
        std::env::remove_var("FS_MODULE_ID");
        let result = Config::from_env();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("FS_MODULE_ID"));
        std::env::remove_var("FS_ENDPOINT");
    }

    #[test]
    fn config_rekor_url_defaults_to_log2025_shard() {
        std::env::set_var("FS_ENDPOINT", "http://localhost:9100");
        std::env::set_var("FS_MODULE_ID", "test");
        std::env::remove_var("REKOR_URL");
        let cfg = Config::from_env().unwrap();
        assert_eq!(cfg.rekor_url, DEFAULT_REKOR_URL);
        assert!(
            cfg.rekor_url.contains("log2025-1.rekor.sigstore.dev"),
            "default must point at the active 2025 v2 shard host"
        );
        assert!(
            cfg.rekor_url.ends_with("/api/v2/log/entries"),
            "default must hit the v2 entries endpoint"
        );
        std::env::remove_var("FS_ENDPOINT");
        std::env::remove_var("FS_MODULE_ID");
    }

    #[test]
    fn config_rekor_url_overridable_via_env() {
        std::env::set_var("FS_ENDPOINT", "http://localhost:9100");
        std::env::set_var("FS_MODULE_ID", "test");
        std::env::set_var(
            "REKOR_URL",
            "https://log2026-1.rekor.sigstore.dev/api/v2/log/entries",
        );
        let cfg = Config::from_env().unwrap();
        assert_eq!(
            cfg.rekor_url,
            "https://log2026-1.rekor.sigstore.dev/api/v2/log/entries"
        );
        std::env::remove_var("FS_ENDPOINT");
        std::env::remove_var("FS_MODULE_ID");
        std::env::remove_var("REKOR_URL");
    }

    #[test]
    fn spki_pem_has_correct_headers() {
        let pub_key = [0u8; 32];
        let pem = ed25519_spki_pem(&pub_key);
        assert!(pem.starts_with("-----BEGIN PUBLIC KEY-----"));
        assert!(pem.contains("-----END PUBLIC KEY-----"));
    }

    #[test]
    fn spki_der_is_44_bytes_with_correct_oid() {
        let pub_key = [0xABu8; 32];
        let der = ed25519_spki_der(&pub_key);
        assert_eq!(der.len(), 44, "SPKI DER must be exactly 44 bytes");
        // OID bytes for id-Ed25519 (1.3.101.112): 2b 65 70
        assert!(
            der.windows(3).any(|w| w == [0x2b, 0x65, 0x70]),
            "OID id-Ed25519 must be present"
        );
        // Last 32 bytes must be the supplied public key
        assert_eq!(&der[12..44], &pub_key);
    }

    #[test]
    fn spki_der_round_trips_through_pem() {
        let pub_key = [0xABu8; 32];
        let pem = ed25519_spki_pem(&pub_key);
        let b64_content: String = pem.lines().filter(|l| !l.starts_with("-----")).collect();
        let der_from_pem = B64.decode(b64_content.trim()).expect("base64 decode");
        assert_eq!(der_from_pem, ed25519_spki_der(&pub_key));
    }

    // ── v0.0.2 body shape ─────────────────────────────────────────────

    fn synthetic_v002_request() -> RekorRequest {
        RekorRequest {
            request: HashedRekordRequestV002 {
                digest: B64.encode([0xAB; 32]),
                signature: RekorSignature {
                    content: B64.encode([0xCD; 64]),
                    verifier: RekorVerifier {
                        public_key: RekorPublicKey {
                            raw_bytes: B64.encode(ed25519_spki_der(&[0xEF; 32])),
                        },
                        key_details: KEY_DETAILS_ED25519.to_string(),
                    },
                },
            },
        }
    }

    #[test]
    fn v002_top_level_wrapper_key_is_named() {
        let req = synthetic_v002_request();
        let v: serde_json::Value = serde_json::to_value(&req).unwrap();
        assert!(
            v.get("hashedRekordRequestV002").is_some(),
            "top-level wrapper key must be 'hashedRekordRequestV002': got {v}"
        );
    }

    #[test]
    fn v002_has_no_v001_envelope_fields() {
        // v0.0.1 wrapped the body in {kind, apiVersion, spec}; v0.0.2 must NOT
        // surface those fields at any level — they're gone entirely from the
        // request shape.
        let req = synthetic_v002_request();
        let json_text = serde_json::to_string(&req).unwrap();
        assert!(
            !json_text.contains("\"kind\""),
            "v0.0.2 body must not contain 'kind' (v0.0.1 envelope removed)"
        );
        assert!(
            !json_text.contains("\"apiVersion\""),
            "v0.0.2 body must not contain 'apiVersion'"
        );
        assert!(
            !json_text.contains("\"spec\""),
            "v0.0.2 body must not contain 'spec'"
        );
    }

    #[test]
    fn v002_signature_has_no_format_field() {
        // v0.0.1 had `signature.format = "x509"`; v0.0.2 removes it
        // entirely — the verifier kind is communicated through the
        // `verifier` oneof.
        let req = synthetic_v002_request();
        let v: serde_json::Value = serde_json::to_value(&req).unwrap();
        let sig = &v["hashedRekordRequestV002"]["signature"];
        assert!(sig.is_object(), "signature must be an object");
        assert!(
            sig.get("format").is_none(),
            "v0.0.2 signature must not have 'format' field"
        );
        assert!(sig.get("content").is_some());
        assert!(sig.get("verifier").is_some());
    }

    #[test]
    fn v002_digest_is_base64_raw_bytes_not_hex() {
        let req = synthetic_v002_request();
        let v: serde_json::Value = serde_json::to_value(&req).unwrap();
        let digest = v["hashedRekordRequestV002"]["digest"].as_str().unwrap();
        let decoded = B64.decode(digest).expect("digest must be valid base64");
        assert_eq!(
            decoded.len(),
            32,
            "decoded digest must be 32 raw bytes (SHA-256 output length); got {} bytes",
            decoded.len()
        );
        // Hex would be 64 ASCII chars; base64 of 32 bytes is 44 chars (or 43 + padding).
        assert!(digest.len() <= 44, "base64 of 32 bytes is at most 44 chars");
    }

    #[test]
    fn v002_public_key_raw_bytes_decodes_to_44_byte_der() {
        let req = synthetic_v002_request();
        let v: serde_json::Value = serde_json::to_value(&req).unwrap();
        let raw = v["hashedRekordRequestV002"]["signature"]["verifier"]["publicKey"]["rawBytes"]
            .as_str()
            .unwrap();
        let der = B64.decode(raw).expect("rawBytes must be valid base64");
        assert_eq!(
            der.len(),
            44,
            "decoded publicKey.rawBytes must be exactly 44 bytes (SPKI DER for Ed25519)"
        );
        // Must NOT be a PEM string masquerading as base64-of-PEM (the v0.0.1 mistake)
        assert!(
            !raw.contains("BEGIN") && !raw.contains("PUBLIC"),
            "rawBytes must be base64 of DER, NOT base64 of a PEM-wrapped string"
        );
    }

    #[test]
    fn v002_key_details_is_pkix_ed25519() {
        let req = synthetic_v002_request();
        let v: serde_json::Value = serde_json::to_value(&req).unwrap();
        let kd = v["hashedRekordRequestV002"]["signature"]["verifier"]["keyDetails"]
            .as_str()
            .unwrap();
        assert_eq!(
            kd, "PKIX_ED25519",
            "keyDetails must be the sigstore_common PublicKeyDetails enum string PKIX_ED25519 \
             (= 7); NOT the prehash variant PKIX_ED25519_PH (= 8) since we sign arbitrary \
             artifact bytes, not pre-hashed messages"
        );
    }

    #[test]
    fn v002_serialisation_field_naming() {
        // Spot-check: serde rename attributes correctly map snake_case Rust
        // identifiers to camelCase JSON keys per the Rekor v2 wire spec.
        let req = synthetic_v002_request();
        let json_text = serde_json::to_string(&req).unwrap();
        assert!(json_text.contains("\"rawBytes\""));
        assert!(json_text.contains("\"keyDetails\""));
        assert!(json_text.contains("\"publicKey\""));
        assert!(!json_text.contains("\"raw_bytes\""));
        assert!(!json_text.contains("\"key_details\""));
        assert!(!json_text.contains("\"public_key\""));
    }

    #[test]
    fn fetch_checkpoint_fails_on_connection_refused() {
        let client = reqwest::blocking::Client::builder()
            .use_rustls_tls()
            .timeout(std::time::Duration::from_millis(200))
            .build()
            .unwrap();
        let result = fetch_checkpoint(&client, "http://127.0.0.1:19999", "test");
        assert!(result.is_err());
    }

    #[test]
    fn write_anchor_fails_on_connection_refused() {
        let client = reqwest::blocking::Client::builder()
            .use_rustls_tls()
            .timeout(std::time::Duration::from_millis(200))
            .build()
            .unwrap();
        let result = write_anchor(
            &client,
            "http://127.0.0.1:19999",
            "test",
            serde_json::json!({"test": true}),
        );
        assert!(result.is_err());
    }
}
