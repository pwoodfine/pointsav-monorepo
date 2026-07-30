// SPDX-License-Identifier: LicenseRef-PointSav-Proprietary

//! Peer pairing for service-content — `POST /v1/pair` receiver side.
//!
//! Wire format matches project-orchestration's app-orchestration-command v0.0.1:
//!   token = `<base64url(payload_json)>.<base64url(ed25519_sig_over_payload_b64_bytes)>`
//!   public_key = base64url of the issuing node's Ed25519 verifying key (32 bytes)
//!
//! Totebox persists pairings to `$GRAPH_DIR/pairing-store.jsonl` (append-only)
//! and writes WORM audit entries to `$GRAPH_DIR/pair-audit.jsonl`.
//!
//! Totebox-side token issuance: `PairingKeypair::issue_token()`. The keypair
//! seed is persisted to `$GRAPH_DIR/totebox-pair.seed` (32 bytes, raw) so
//! the public key is stable across restarts.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use chrono::{DateTime, Utc};
use ed25519_dalek::{Signature, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

// ── token payload ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenPayload {
    pub issuer: String,
    pub role: String,
    pub nonce: String,
    pub expiry: String,
    #[serde(default)]
    pub archive_scope: Vec<String>,
    #[serde(default)]
    pub peer_type: String,
}

impl TokenPayload {
    pub fn is_expired(&self) -> bool {
        match self.expiry.parse::<DateTime<Utc>>() {
            Ok(exp) => Utc::now() > exp,
            Err(_) => true,
        }
    }
}

// ── pairing record ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingRecord {
    pub public_key: String,
    pub issuer: String,
    pub peer_type: String,
    pub role: String,
    pub archive_scope: Vec<String>,
    pub node_label: String,
    pub paired_on: String,
    pub nonce: String,
}

// ── error ─────────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum PairError {
    Malformed,
    BadSignature,
    Expired,
    #[allow(dead_code)]
    NonceReused,
}

impl std::fmt::Display for PairError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PairError::Malformed => write!(f, "malformed token"),
            PairError::BadSignature => write!(f, "invalid signature"),
            PairError::Expired => write!(f, "token expired"),
            PairError::NonceReused => write!(f, "nonce already used"),
        }
    }
}

// ── token verification ────────────────────────────────────────────────────────

/// Verify a pairing token against the caller-supplied public key.
///
/// Returns the embedded payload on success.
/// The caller must separately check nonce uniqueness.
pub fn verify_pair_token(token: &str, public_key_b64: &str) -> Result<TokenPayload, PairError> {
    let (payload_b64, sig_b64) = token.split_once('.').ok_or(PairError::Malformed)?;

    // Decode public key (32 bytes → VerifyingKey).
    let pk_bytes = URL_SAFE_NO_PAD
        .decode(public_key_b64)
        .map_err(|_| PairError::Malformed)?;
    let pk_arr: [u8; 32] = pk_bytes.try_into().map_err(|_| PairError::Malformed)?;
    let vk = VerifyingKey::from_bytes(&pk_arr).map_err(|_| PairError::Malformed)?;

    // Decode signature (64 bytes).
    let sig_bytes = URL_SAFE_NO_PAD
        .decode(sig_b64)
        .map_err(|_| PairError::Malformed)?;
    let sig_arr: [u8; 64] = sig_bytes.try_into().map_err(|_| PairError::Malformed)?;
    let sig = Signature::from_bytes(&sig_arr);

    // Verify: signature is over the payload_b64 bytes (same convention as membership.rs).
    use ed25519_dalek::Verifier as _;
    vk.verify(payload_b64.as_bytes(), &sig)
        .map_err(|_| PairError::BadSignature)?;

    // Decode and parse payload.
    let payload_bytes = URL_SAFE_NO_PAD
        .decode(payload_b64)
        .map_err(|_| PairError::Malformed)?;
    let payload: TokenPayload =
        serde_json::from_slice(&payload_bytes).map_err(|_| PairError::Malformed)?;

    if payload.is_expired() {
        return Err(PairError::Expired);
    }

    Ok(payload)
}

// ── pairing store ─────────────────────────────────────────────────────────────

/// In-memory pairing registry backed by an append-only JSONL file.
///
/// Keyed by `public_key` (base64url). A second pairing attempt with the same
/// public key returns `already_paired`.
pub struct PairingStore {
    store_path: PathBuf,
    audit_path: PathBuf,
    by_pubkey: HashMap<String, PairingRecord>,
}

impl PairingStore {
    /// Load existing pairings from disk. Creates the file if absent.
    pub fn load(graph_dir: &str) -> std::io::Result<Self> {
        let store_path = Path::new(graph_dir).join("pairing-store.jsonl");
        let audit_path = Path::new(graph_dir).join("pair-audit.jsonl");
        let mut by_pubkey = HashMap::new();

        if store_path.exists() {
            let content = std::fs::read_to_string(&store_path)?;
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                if let Ok(rec) = serde_json::from_str::<PairingRecord>(line) {
                    by_pubkey.insert(rec.public_key.clone(), rec);
                }
            }
        }

        Ok(Self {
            store_path,
            audit_path,
            by_pubkey,
        })
    }

    pub fn get(&self, public_key: &str) -> Option<&PairingRecord> {
        self.by_pubkey.get(public_key)
    }

    /// All paired records, for `GET /v1/pairs`.
    pub fn list(&self) -> Vec<PairingRecord> {
        self.by_pubkey.values().cloned().collect()
    }

    /// Resolve a peer's registered public key by the `from_instance` identifier
    /// presented in a forwarded `X-Foundry-Capability` header — matched against
    /// `issuer`, falling back to `node_label` (both populated at `/v1/pair` time).
    pub fn find_by_instance(&self, from_instance: &str) -> Option<&PairingRecord> {
        self.by_pubkey
            .values()
            .find(|r| r.issuer == from_instance || r.node_label == from_instance)
    }

    /// Persist a new pairing and return it.
    pub fn insert(&mut self, rec: PairingRecord) -> std::io::Result<()> {
        let line = serde_json::to_string(&rec).map_err(std::io::Error::other)?;

        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.store_path)?;
        writeln!(f, "{}", line)?;

        let audit = serde_json::json!({
            "event": "paired",
            "ts": Utc::now().to_rfc3339(),
            "issuer": rec.issuer,
            "peer_type": rec.peer_type,
            "role": rec.role,
            "node_label": rec.node_label,
            "nonce": rec.nonce,
        });
        let mut af = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.audit_path)?;
        writeln!(af, "{}", audit)?;

        self.by_pubkey.insert(rec.public_key.clone(), rec);
        Ok(())
    }
}

// ── totebox keypair (for token issuance) ──────────────────────────────────────

/// Persistent Ed25519 keypair for this Totebox instance.
///
/// The 32-byte seed is stored at `$GRAPH_DIR/totebox-pair.seed` so the public
/// key is stable across restarts (partners can cache it).
pub struct PairingKeypair {
    signing_key: SigningKey,
    pub verifying_key_b64: String,
}

impl PairingKeypair {
    /// Load from disk, or generate + save if not present.
    pub fn load_or_generate(graph_dir: &str) -> std::io::Result<Self> {
        let seed_path = Path::new(graph_dir).join("totebox-pair.seed");
        let seed: [u8; 32] = if seed_path.exists() {
            let bytes = std::fs::read(&seed_path)?;
            bytes.try_into().map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, "bad seed length")
            })?
        } else {
            let mut s = [0u8; 32];
            let mut f = std::fs::File::open("/dev/urandom")?;
            use std::io::Read as _;
            f.read_exact(&mut s)?;
            std::fs::write(&seed_path, s)?;
            s
        };

        let signing_key = SigningKey::from_bytes(&seed);
        let vk = signing_key.verifying_key();
        let verifying_key_b64 = URL_SAFE_NO_PAD.encode(vk.as_bytes());

        Ok(Self {
            signing_key,
            verifying_key_b64,
        })
    }

    /// Issue a signed invite token for the given role and archive scope.
    pub fn issue_token(&self, role: &str, archive_scope: Vec<String>, node_label: &str) -> String {
        let nonce = {
            let mut b = [0u8; 16];
            if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
                use std::io::Read as _;
                let _ = f.read_exact(&mut b);
            }
            format!(
                "{:x}{:x}",
                u64::from_le_bytes(b[..8].try_into().unwrap_or_default()),
                u64::from_le_bytes(b[8..].try_into().unwrap_or_default())
            )
        };

        let payload = TokenPayload {
            issuer: node_label.to_string(),
            role: role.to_string(),
            nonce,
            expiry: (Utc::now() + chrono::Duration::hours(24)).to_rfc3339(),
            archive_scope,
            peer_type: "totebox".to_string(),
        };

        let payload_json = serde_json::to_string(&payload).expect("always serializable");
        let payload_b64 = URL_SAFE_NO_PAD.encode(payload_json.as_bytes());

        use ed25519_dalek::Signer as _;
        let sig: Signature = self.signing_key.sign(payload_b64.as_bytes());
        let sig_b64 = URL_SAFE_NO_PAD.encode(sig.to_bytes());

        format!("{}.{}", payload_b64, sig_b64)
    }
}

// ── capability assertions (Model B forwarded trust) ────────────────────────────
//
// Wire format agreed with project-orchestration 2026-06-30 (outbox
// project-totebox-20260630-design-ack-pairing-protocol):
//   X-Foundry-Capability: <base64url(payload_json)>.<base64url(sig)>
//   payload = { from_instance, user_scope, archive_scope, nonce, expiry, peer_type }
// Signed with the forwarding peer's own Ed25519 key — the same key registered
// in PairingStore at `/v1/pair` time. Model B: os-totebox verifies independently
// rather than trusting the forwarding instance's pairing alone (DOCTRINE: holds
// no archive keys).

/// Payload of a forwarded `X-Foundry-Capability` header.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityPayload {
    pub from_instance: String,
    pub user_scope: String,
    #[serde(default)]
    pub archive_scope: Vec<String>,
    pub nonce: String,
    pub expiry: String,
    #[serde(default)]
    pub peer_type: String,
}

impl CapabilityPayload {
    pub fn is_expired(&self) -> bool {
        match self.expiry.parse::<DateTime<Utc>>() {
            Ok(exp) => Utc::now() > exp,
            Err(_) => true,
        }
    }
}

#[derive(Debug)]
pub enum CapabilityError {
    Malformed,
    BadSignature,
    Expired,
}

impl std::fmt::Display for CapabilityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CapabilityError::Malformed => write!(f, "malformed capability header"),
            CapabilityError::BadSignature => write!(f, "invalid capability signature"),
            CapabilityError::Expired => write!(f, "capability expired"),
        }
    }
}

/// Verify an `X-Foundry-Capability` header value against a known public key.
///
/// Caller resolves `from_instance` → `public_key` via
/// [`PairingStore::find_by_instance`] before calling this, and separately
/// checks nonce uniqueness via [`NonceCache`].
pub fn verify_capability(
    token: &str,
    public_key_b64: &str,
) -> Result<CapabilityPayload, CapabilityError> {
    let (payload_b64, sig_b64) = token.split_once('.').ok_or(CapabilityError::Malformed)?;

    let pk_bytes = URL_SAFE_NO_PAD
        .decode(public_key_b64)
        .map_err(|_| CapabilityError::Malformed)?;
    let pk_arr: [u8; 32] = pk_bytes
        .try_into()
        .map_err(|_| CapabilityError::Malformed)?;
    let vk = VerifyingKey::from_bytes(&pk_arr).map_err(|_| CapabilityError::Malformed)?;

    let sig_bytes = URL_SAFE_NO_PAD
        .decode(sig_b64)
        .map_err(|_| CapabilityError::Malformed)?;
    let sig_arr: [u8; 64] = sig_bytes
        .try_into()
        .map_err(|_| CapabilityError::Malformed)?;
    let sig = Signature::from_bytes(&sig_arr);

    use ed25519_dalek::Verifier as _;
    vk.verify(payload_b64.as_bytes(), &sig)
        .map_err(|_| CapabilityError::BadSignature)?;

    let payload_bytes = URL_SAFE_NO_PAD
        .decode(payload_b64)
        .map_err(|_| CapabilityError::Malformed)?;
    let payload: CapabilityPayload =
        serde_json::from_slice(&payload_bytes).map_err(|_| CapabilityError::Malformed)?;

    if payload.is_expired() {
        return Err(CapabilityError::Expired);
    }

    Ok(payload)
}

/// Append-only WORM audit log for capability-gated INTERFACE-peer requests.
///
/// Interim sidecar until the service-fs WORM drop dir is wired
/// (BRIEF-flow-build-plan.md §INTERFACE middleware).
pub struct InterfaceAuditLog {
    path: PathBuf,
}

impl InterfaceAuditLog {
    pub fn new(graph_dir: &str) -> Self {
        Self {
            path: Path::new(graph_dir).join("interface-audit.jsonl"),
        }
    }

    pub fn record(&self, endpoint: &str, payload: &CapabilityPayload) -> std::io::Result<()> {
        let entry = serde_json::json!({
            "event": "capability_verified",
            "ts": Utc::now().to_rfc3339(),
            "endpoint": endpoint,
            "from_instance": payload.from_instance,
            "user_scope": payload.user_scope,
            "archive_scope": payload.archive_scope,
            "peer_type": payload.peer_type,
            "nonce": payload.nonce,
        });
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        writeln!(f, "{}", entry)
    }
}

// ── nonce cache ───────────────────────────────────────────────────────────────

/// In-memory nonce deduplication. Prevents replay within the process lifetime.
///
/// Not persisted — nonces are tied to short-lived tokens (24h default).
/// After restart the window is narrow enough to be acceptable.
pub struct NonceCache(pub Mutex<HashSet<String>>);

impl NonceCache {
    pub fn new() -> Self {
        Self(Mutex::new(HashSet::new()))
    }

    /// Returns false if the nonce was already seen.
    pub fn try_insert(&self, nonce: &str) -> bool {
        self.0.lock().unwrap().insert(nonce.to_string())
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_dir() -> tempfile::TempDir {
        tempfile::TempDir::new().expect("tmpdir")
    }

    fn make_keypair(dir: &str) -> PairingKeypair {
        PairingKeypair::load_or_generate(dir).expect("keypair")
    }

    #[test]
    fn issue_and_verify_roundtrip() {
        let d = tmp_dir();
        let kp = make_keypair(d.path().to_str().unwrap());
        let token = kp.issue_token("INTERFACE", vec!["project-totebox".into()], "test-node");
        let payload = verify_pair_token(&token, &kp.verifying_key_b64).expect("valid");
        assert_eq!(payload.role, "INTERFACE");
        assert_eq!(payload.peer_type, "totebox");
        assert!(!payload.is_expired());
    }

    #[test]
    fn tampered_payload_rejected() {
        let d = tmp_dir();
        let kp = make_keypair(d.path().to_str().unwrap());
        let token = kp.issue_token("USER", vec![], "node");
        let tampered = token.replacen('a', "b", 1);
        assert!(verify_pair_token(&tampered, &kp.verifying_key_b64).is_err());
    }

    #[test]
    fn wrong_key_rejected() {
        let d = tmp_dir();
        let kp1 = make_keypair(d.path().to_str().unwrap());
        let d2 = tmp_dir();
        let kp2 = make_keypair(d2.path().to_str().unwrap());
        let token = kp1.issue_token("ADMIN", vec![], "node");
        assert!(verify_pair_token(&token, &kp2.verifying_key_b64).is_err());
    }

    #[test]
    fn expired_token_rejected() {
        let d = tmp_dir();
        let kp = make_keypair(d.path().to_str().unwrap());
        // Manually craft a token with past expiry.
        let payload = TokenPayload {
            issuer: "test".into(),
            role: "USER".into(),
            nonce: "abc".into(),
            expiry: "2020-01-01T00:00:00Z".into(),
            archive_scope: vec![],
            peer_type: "orchestration".into(),
        };
        let pj = serde_json::to_string(&payload).unwrap();
        let pb64 = URL_SAFE_NO_PAD.encode(pj.as_bytes());
        use ed25519_dalek::Signer as _;
        let sig: Signature = kp.signing_key.sign(pb64.as_bytes());
        let sb64 = URL_SAFE_NO_PAD.encode(sig.to_bytes());
        let token = format!("{}.{}", pb64, sb64);
        assert!(matches!(
            verify_pair_token(&token, &kp.verifying_key_b64),
            Err(PairError::Expired)
        ));
    }

    #[test]
    fn nonce_cache_deduplicates() {
        let cache = NonceCache::new();
        assert!(cache.try_insert("nonce-1"));
        assert!(!cache.try_insert("nonce-1"));
        assert!(cache.try_insert("nonce-2"));
    }

    #[test]
    fn pairing_store_roundtrip() {
        let d = tmp_dir();
        let dir = d.path().to_str().unwrap();
        let mut store = PairingStore::load(dir).expect("load");
        assert!(store.get("pk1").is_none());

        let rec = PairingRecord {
            public_key: "pk1".into(),
            issuer: "test-issuer".into(),
            peer_type: "orchestration".into(),
            role: "INTERFACE".into(),
            archive_scope: vec!["project-totebox".into()],
            node_label: "test-node".into(),
            paired_on: Utc::now().to_rfc3339(),
            nonce: "n1".into(),
        };
        store.insert(rec.clone()).expect("insert");
        assert!(store.get("pk1").is_some());

        // Reload from disk — record must survive.
        let store2 = PairingStore::load(dir).expect("reload");
        let loaded = store2.get("pk1").expect("persisted");
        assert_eq!(loaded.role, "INTERFACE");
    }

    #[test]
    fn pairing_store_list_returns_all_records() {
        let d = tmp_dir();
        let dir = d.path().to_str().unwrap();
        let mut store = PairingStore::load(dir).expect("load");
        assert!(store.list().is_empty());

        for i in 0..3 {
            let rec = PairingRecord {
                public_key: format!("pk{i}"),
                issuer: "test-issuer".into(),
                peer_type: "orchestration".into(),
                role: "INTERFACE".into(),
                archive_scope: vec!["project-totebox".into()],
                node_label: format!("node-{i}"),
                paired_on: Utc::now().to_rfc3339(),
                nonce: format!("n{i}"),
            };
            store.insert(rec).expect("insert");
        }

        let listed = store.list();
        assert_eq!(listed.len(), 3);
    }

    fn make_capability_token(
        kp: &PairingKeypair,
        from_instance: &str,
        expiry: DateTime<Utc>,
    ) -> String {
        let payload = CapabilityPayload {
            from_instance: from_instance.into(),
            user_scope: "jennifer".into(),
            archive_scope: vec!["project-orchestration".into()],
            nonce: "cap-nonce-1".into(),
            expiry: expiry.to_rfc3339(),
            peer_type: "orchestration".into(),
        };
        let pj = serde_json::to_string(&payload).unwrap();
        let pb64 = URL_SAFE_NO_PAD.encode(pj.as_bytes());
        use ed25519_dalek::Signer as _;
        let sig: Signature = kp.signing_key.sign(pb64.as_bytes());
        let sb64 = URL_SAFE_NO_PAD.encode(sig.to_bytes());
        format!("{}.{}", pb64, sb64)
    }

    #[test]
    fn capability_roundtrip_verifies() {
        let d = tmp_dir();
        let kp = make_keypair(d.path().to_str().unwrap());
        let token = make_capability_token(
            &kp,
            "project-orchestration",
            Utc::now() + chrono::Duration::hours(1),
        );
        let payload = verify_capability(&token, &kp.verifying_key_b64).expect("valid capability");
        assert_eq!(payload.from_instance, "project-orchestration");
        assert_eq!(payload.user_scope, "jennifer");
    }

    #[test]
    fn capability_expired_rejected() {
        let d = tmp_dir();
        let kp = make_keypair(d.path().to_str().unwrap());
        let token = make_capability_token(
            &kp,
            "project-orchestration",
            Utc::now() - chrono::Duration::hours(1),
        );
        assert!(matches!(
            verify_capability(&token, &kp.verifying_key_b64),
            Err(CapabilityError::Expired)
        ));
    }

    #[test]
    fn capability_wrong_key_rejected() {
        let d = tmp_dir();
        let kp1 = make_keypair(d.path().to_str().unwrap());
        let d2 = tmp_dir();
        let kp2 = make_keypair(d2.path().to_str().unwrap());
        let token = make_capability_token(
            &kp1,
            "project-orchestration",
            Utc::now() + chrono::Duration::hours(1),
        );
        assert!(verify_capability(&token, &kp2.verifying_key_b64).is_err());
    }

    #[test]
    fn find_by_instance_matches_issuer_or_label() {
        let d = tmp_dir();
        let dir = d.path().to_str().unwrap();
        let mut store = PairingStore::load(dir).expect("load");
        let rec = PairingRecord {
            public_key: "pk-orch".into(),
            issuer: "project-orchestration".into(),
            peer_type: "orchestration".into(),
            role: "INTERFACE".into(),
            archive_scope: vec![],
            node_label: "orch-node-1".into(),
            paired_on: Utc::now().to_rfc3339(),
            nonce: "n1".into(),
        };
        store.insert(rec).expect("insert");

        assert_eq!(
            store
                .find_by_instance("project-orchestration")
                .unwrap()
                .public_key,
            "pk-orch"
        );
        assert_eq!(
            store.find_by_instance("orch-node-1").unwrap().public_key,
            "pk-orch"
        );
        assert!(store.find_by_instance("unknown-instance").is_none());
    }

    #[test]
    fn interface_audit_log_appends_jsonl() {
        let d = tmp_dir();
        let log = InterfaceAuditLog::new(d.path().to_str().unwrap());
        let payload = CapabilityPayload {
            from_instance: "project-orchestration".into(),
            user_scope: "jennifer".into(),
            archive_scope: vec!["project-totebox".into()],
            nonce: "n1".into(),
            expiry: (Utc::now() + chrono::Duration::hours(1)).to_rfc3339(),
            peer_type: "orchestration".into(),
        };
        log.record("/v1/graph/mutate", &payload).expect("record");
        let content = std::fs::read_to_string(d.path().join("interface-audit.jsonl")).unwrap();
        assert_eq!(content.lines().count(), 1);
        assert!(content.contains("capability_verified"));
        assert!(content.contains("project-orchestration"));
    }

    #[test]
    fn keypair_seed_stable_across_reload() {
        let d = tmp_dir();
        let dir = d.path().to_str().unwrap();
        let kp1 = make_keypair(dir);
        let kp2 = PairingKeypair::load_or_generate(dir).expect("reload");
        assert_eq!(kp1.verifying_key_b64, kp2.verifying_key_b64);
    }
}
