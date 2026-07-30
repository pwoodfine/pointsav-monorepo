//! `ssh-keygen -Y verify` wrapper for witness-record signatures.
//!
//! Per `~/Foundry/CLAUDE.md` §3 + `apprenticeship-substrate.md` §5:
//! the namespace tag for witness signatures is
//! [`WITNESS_NAMESPACE`] = `"capability-witness-v1"`. The signing
//! primitive is the same `ssh-keygen -Y sign / verify` workspace
//! infrastructure that backs commit signing and apprenticeship
//! verdict signing — different namespace tag prevents
//! cross-namespace replay attacks.
//!
//! Two implementations share this public interface:
//!
//! - **std path** (default): shells out to `/usr/bin/ssh-keygen -Y
//!   verify` via `std::process::Command`. Synchronous; wraps with
//!   `tokio::task::spawn_blocking` for async consumers.
//!
//! - **sel4 / no_std path**: in-process Ed25519 verification (W1
//!   approach). Parses the SSH-armored SSHSIG blob manually, decodes
//!   the OpenSSH public key, reconstructs the signed-data blob per
//!   PROTOCOL.sshsig §3, and calls `ed25519-dalek` directly. Wire
//!   format is identical to the std path — signatures produced by
//!   `ssh-keygen -Y sign` verify correctly on both paths.
//!
//! Signed-payload contract (per
//! `system-core::WitnessRecord` doc-comment): the bytes
//! that get signed are `(capability_hash || new_expiry_t.to_be_bytes())`.
//! The caller is responsible for assembling that payload before
//! invoking [`verify_witness_signature`].

// ── alloc imports (no_std path) ──────────────────────────────────────────────
#[cfg(not(feature = "std"))]
use alloc::{format, string::String, vec::Vec};

/// Namespace tag bound to the `-n` flag of `ssh-keygen -Y sign /
/// verify`. Cross-namespace replay against commit-signing or
/// apprenticeship-verdict signatures is the attack this discipline
/// prevents.
pub const WITNESS_NAMESPACE: &str = "capability-witness-v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WitnessVerifyError {
    /// Failed to write a tempfile (std path only).
    TempFileFailed(String),
    /// `ssh-keygen` not found, didn't start, or crashed (std path only).
    ShellOutFailed(String),
    /// Tempfile path was not valid UTF-8 (std path only).
    NonUtf8Path,
    /// Signature or public key could not be parsed (sel4 path).
    ParseError(String),
}

/// Verify a detached SSH signature over `signed_payload` under
/// `ssh_pubkey`, asserting namespace = `WITNESS_NAMESPACE` and
/// identity = `signer_identity`.
///
/// Returns `Ok(true)` if the signature is valid, `Ok(false)` if it
/// is not (wrong key, wrong payload, wrong namespace, truncated),
/// `Err(_)` only on infrastructure failure.
///
/// # Arguments
///
/// - `signature_armored`: `-----BEGIN SSH SIGNATURE-----` armored
///   form produced by `ssh-keygen -Y sign`; typically the bytes
///   from `WitnessRecord::signature`.
/// - `signed_payload`: bytes that were signed — per `WitnessRecord`
///   doc: `(capability_hash || new_expiry_t.to_be_bytes())`. Caller
///   assembles.
/// - `ssh_pubkey`: SSH-format public key on a single line, e.g.
///   `"ssh-ed25519 AAAA... [comment]"`.
/// - `signer_identity`: identity string for `ssh-keygen -Y verify
///   -I` (std path); ignored on sel4 path where the caller-supplied
///   public key is the authority.

// ── std path: unchanged ssh-keygen shellout ─────────────────────────────────
#[cfg(feature = "std")]
pub fn verify_witness_signature(
    signature_armored: &[u8],
    signed_payload: &[u8],
    ssh_pubkey: &str,
    signer_identity: &str,
) -> Result<bool, WitnessVerifyError> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    use tempfile::NamedTempFile;

    // Tempfile 1 — signature (.sig); ssh-keygen requires a file
    // path, not stdin.
    let mut sig_file = NamedTempFile::new()
        .map_err(|e| WitnessVerifyError::TempFileFailed(format!("sig tmp: {e}")))?;
    sig_file
        .write_all(signature_armored)
        .map_err(|e| WitnessVerifyError::TempFileFailed(format!("write sig: {e}")))?;
    sig_file
        .flush()
        .map_err(|e| WitnessVerifyError::TempFileFailed(format!("flush sig: {e}")))?;

    // Tempfile 2 — allowed_signers (single line).
    let allowed_line = format!("{} {}\n", signer_identity, ssh_pubkey.trim());
    let mut signers_file = NamedTempFile::new()
        .map_err(|e| WitnessVerifyError::TempFileFailed(format!("signers tmp: {e}")))?;
    signers_file
        .write_all(allowed_line.as_bytes())
        .map_err(|e| WitnessVerifyError::TempFileFailed(format!("write signers: {e}")))?;
    signers_file
        .flush()
        .map_err(|e| WitnessVerifyError::TempFileFailed(format!("flush signers: {e}")))?;

    let sig_path = sig_file
        .path()
        .to_str()
        .ok_or(WitnessVerifyError::NonUtf8Path)?;
    let signers_path = signers_file
        .path()
        .to_str()
        .ok_or(WitnessVerifyError::NonUtf8Path)?;

    // Spawn ssh-keygen -Y verify.
    let mut child = Command::new("ssh-keygen")
        .args([
            "-Y",
            "verify",
            "-f",
            signers_path,
            "-I",
            signer_identity,
            "-n",
            WITNESS_NAMESPACE,
            "-s",
            sig_path,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| WitnessVerifyError::ShellOutFailed(format!("spawn: {e}")))?;

    // Write payload to stdin.
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(signed_payload)
            .map_err(|e| WitnessVerifyError::ShellOutFailed(format!("stdin write: {e}")))?;
        // Drop closes stdin → signals EOF.
    }

    let output = child
        .wait_with_output()
        .map_err(|e| WitnessVerifyError::ShellOutFailed(format!("wait: {e}")))?;

    Ok(output.status.success())
}

// ── sel4 / no_std path: in-process W1 verifier ──────────────────────────────
#[cfg(not(feature = "std"))]
pub fn verify_witness_signature(
    signature_armored: &[u8],
    signed_payload: &[u8],
    ssh_pubkey: &str,
    _signer_identity: &str,
) -> Result<bool, WitnessVerifyError> {
    use base64::Engine as _;
    use ed25519_dalek::{Signature, VerifyingKey};
    use sha2::{Digest, Sha256, Sha512};

    // Step 1 — decode PEM armor → raw SSHSIG bytes.
    // Armored format: base64 content between "-----BEGIN/END SSH SIGNATURE-----" lines.
    let sig_str = core::str::from_utf8(signature_armored)
        .map_err(|_| WitnessVerifyError::ParseError("signature not UTF-8".into()))?;
    let b64: String = sig_str
        .lines()
        .filter(|l| !l.starts_with("-----"))
        .collect();
    let sshsig_bytes = base64::engine::general_purpose::STANDARD
        .decode(b64.as_bytes())
        .map_err(|e| WitnessVerifyError::ParseError(format!("base64 decode: {e}")))?;

    // Step 2 — parse SSHSIG binary wire format (PROTOCOL.sshsig §3):
    //   "SSHSIG"  6-byte magic preamble
    //   uint32    version (must be 1)
    //   string    public_key (SSH wire format — we use caller-supplied key)
    //   string    namespace
    //   string    reserved (empty)
    //   string    hash_alg ("sha256" or "sha512")
    //   string    sig_blob  (nested: string(algo_name) + string(raw_sig_64))
    let mut r = ByteReader::new(&sshsig_bytes);
    let magic = r.take(6)?;
    if magic != b"SSHSIG" {
        return Err(WitnessVerifyError::ParseError(
            "missing SSHSIG magic".into(),
        ));
    }
    let version = r.read_u32()?;
    if version != 1 {
        return Err(WitnessVerifyError::ParseError(format!(
            "SSHSIG version {version} unsupported"
        )));
    }
    r.skip_string()?; // embedded public key — authority is the caller-supplied key
    let namespace_bytes = r.read_string_owned()?;
    r.skip_string()?; // reserved
    let hash_alg_bytes = r.read_string_owned()?;
    let sig_blob = r.read_string_owned()?;

    // Step 3 — namespace guard (fail fast before any crypto).
    let namespace = core::str::from_utf8(&namespace_bytes)
        .map_err(|_| WitnessVerifyError::ParseError("namespace not UTF-8".into()))?;
    if namespace != WITNESS_NAMESPACE {
        return Ok(false);
    }

    // Step 4 — extract raw 64-byte Ed25519 signature from sig_blob.
    // sig_blob = string("ssh-ed25519") || string(raw_64_bytes)
    let mut sr = ByteReader::new(&sig_blob);
    sr.skip_string()?; // algorithm name
    let raw_sig_bytes = sr.read_string_owned()?;
    if raw_sig_bytes.len() != 64 {
        return Err(WitnessVerifyError::ParseError(format!(
            "ed25519 sig must be 64 bytes, got {}",
            raw_sig_bytes.len()
        )));
    }
    let mut sig_arr = [0u8; 64];
    sig_arr.copy_from_slice(&raw_sig_bytes);
    let ed_sig = Signature::from_bytes(&sig_arr);

    // Step 5 — parse caller-supplied OpenSSH public key.
    let key_bytes = parse_openssh_ed25519_key(ssh_pubkey)?;
    let verifying_key = VerifyingKey::from_bytes(&key_bytes)
        .map_err(|e| WitnessVerifyError::ParseError(format!("verifying key: {e}")))?;

    // Step 6 — hash the payload per the algorithm named in the signature.
    let hash_alg = core::str::from_utf8(&hash_alg_bytes)
        .map_err(|_| WitnessVerifyError::ParseError("hash_alg not UTF-8".into()))?;
    let msg_hash: Vec<u8> = match hash_alg {
        "sha256" => Sha256::digest(signed_payload).to_vec(),
        "sha512" => Sha512::digest(signed_payload).to_vec(),
        other => {
            return Err(WitnessVerifyError::ParseError(format!(
                "unknown hash_alg: {other}"
            )))
        }
    };

    // Step 7 — reconstruct SSHSIG signed-data blob (PROTOCOL.sshsig §3):
    //   "SSHSIG" || string(namespace) || string("") || string(hash_alg) || string(H(msg))
    let signed_data = build_sshsig_signed_data(namespace, hash_alg, &msg_hash);

    // Step 8 — verify; verify_strict rejects cofactor-malleable signatures.
    Ok(verifying_key.verify_strict(&signed_data, &ed_sig).is_ok())
}

// ── SSHSIG wire-format helpers (no_std path) ─────────────────────────────────

/// Build the SSHSIG signed-data blob per PROTOCOL.sshsig §3.
/// SSH "string" encoding = uint32_be(len) || bytes.
#[cfg(not(feature = "std"))]
fn build_sshsig_signed_data(namespace: &str, hash_alg: &str, msg_hash: &[u8]) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(b"SSHSIG");
    ssh_string(&mut buf, namespace.as_bytes());
    ssh_string(&mut buf, b""); // reserved
    ssh_string(&mut buf, hash_alg.as_bytes());
    ssh_string(&mut buf, msg_hash);
    buf
}

#[cfg(not(feature = "std"))]
fn ssh_string(buf: &mut Vec<u8>, s: &[u8]) {
    buf.extend_from_slice(&(s.len() as u32).to_be_bytes());
    buf.extend_from_slice(s);
}

/// Parse an `"ssh-ed25519 <base64> [comment]"` OpenSSH public key
/// string into the raw 32-byte Ed25519 key material.
#[cfg(not(feature = "std"))]
fn parse_openssh_ed25519_key(openssh: &str) -> Result<[u8; 32], WitnessVerifyError> {
    use base64::Engine as _;
    let mut parts = openssh.trim().splitn(3, ' ');
    let algo = parts.next().unwrap_or("");
    let b64 = parts
        .next()
        .ok_or_else(|| WitnessVerifyError::ParseError("invalid pubkey: missing key data".into()))?;
    if algo != "ssh-ed25519" {
        return Err(WitnessVerifyError::ParseError(format!(
            "expected ssh-ed25519, got {algo}"
        )));
    }
    // Decode wire format: string("ssh-ed25519") || string(key_bytes_32)
    let wire = base64::engine::general_purpose::STANDARD
        .decode(b64)
        .map_err(|e| WitnessVerifyError::ParseError(format!("pubkey base64: {e}")))?;
    let mut r = ByteReader::new(&wire);
    let inner_algo = r.read_string_owned()?;
    if &inner_algo != b"ssh-ed25519" {
        return Err(WitnessVerifyError::ParseError(
            "pubkey wire algo mismatch".into(),
        ));
    }
    let key_data = r.read_string_owned()?;
    if key_data.len() != 32 {
        return Err(WitnessVerifyError::ParseError(format!(
            "ed25519 key must be 32 bytes, got {}",
            key_data.len()
        )));
    }
    let mut k = [0u8; 32];
    k.copy_from_slice(&key_data);
    Ok(k)
}

/// Minimal zero-copy SSH binary reader (no_std path).
#[cfg(not(feature = "std"))]
struct ByteReader<'a> {
    data: &'a [u8],
    pos: usize,
}

#[cfg(not(feature = "std"))]
impl<'a> ByteReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], WitnessVerifyError> {
        if self.pos + n > self.data.len() {
            return Err(WitnessVerifyError::ParseError(
                "truncated SSHSIG data".into(),
            ));
        }
        let s = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }

    fn read_u32(&mut self) -> Result<u32, WitnessVerifyError> {
        let b = self.take(4)?;
        Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn read_string_owned(&mut self) -> Result<Vec<u8>, WitnessVerifyError> {
        let len = self.read_u32()? as usize;
        Ok(self.take(len)?.to_vec())
    }

    fn skip_string(&mut self) -> Result<(), WitnessVerifyError> {
        let len = self.read_u32()? as usize;
        self.take(len).map(|_| ())
    }
}

// ── tests (std path only) ─────────────────────────────────────────────────────
#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use std::process::{Command, Stdio};
    use tempfile::TempDir;

    /// Generate an ed25519 keypair via ssh-keygen. Returns
    /// `(privkey_path, pubkey_string)` where pubkey_string is the
    /// single-line SSH-format public key.
    fn make_keypair(dir: &TempDir, name: &str) -> (std::path::PathBuf, String) {
        let priv_path = dir.path().join(name);
        let pub_path = dir.path().join(format!("{name}.pub"));
        let status = Command::new("ssh-keygen")
            .args([
                "-t",
                "ed25519",
                "-f",
                priv_path.to_str().unwrap(),
                "-N",
                "",
                "-C",
                &format!("{name}@witness-test"),
                "-q",
            ])
            .status()
            .expect("ssh-keygen keygen runs");
        assert!(status.success(), "ssh-keygen keygen exit");
        let pubkey = fs::read_to_string(&pub_path)
            .expect("read .pub file")
            .trim()
            .to_string();
        (priv_path, pubkey)
    }

    /// Sign `payload` under `priv_path` with the given namespace.
    /// Returns the armored signature bytes.
    fn sign_payload(priv_path: &std::path::Path, payload: &[u8], namespace: &str) -> Vec<u8> {
        let mut child = Command::new("ssh-keygen")
            .args([
                "-Y",
                "sign",
                "-f",
                priv_path.to_str().unwrap(),
                "-n",
                namespace,
                "-q",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("ssh-keygen sign spawns");
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(payload).expect("write payload");
        }
        let output = child.wait_with_output().expect("wait sign");
        assert!(
            output.status.success(),
            "ssh-keygen sign failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        output.stdout
    }

    #[test]
    fn verify_accepts_valid_signature() {
        let dir = TempDir::new().expect("tmpdir");
        let (priv_path, pubkey) = make_keypair(&dir, "k1");

        let payload = b"capability_hash || new_expiry";
        let sig = sign_payload(&priv_path, payload, WITNESS_NAMESPACE);

        let r = verify_witness_signature(&sig, payload, &pubkey, "k1@witness-test")
            .expect("verify call ok");
        assert!(r, "valid signature should verify");
    }

    #[test]
    fn verify_rejects_tampered_payload() {
        let dir = TempDir::new().expect("tmpdir");
        let (priv_path, pubkey) = make_keypair(&dir, "k2");

        let payload = b"capability_hash || new_expiry";
        let sig = sign_payload(&priv_path, payload, WITNESS_NAMESPACE);

        let tampered = b"capability_hash || NEW_EXPIRY_TAMPERED";
        let r = verify_witness_signature(&sig, tampered, &pubkey, "k2@witness-test")
            .expect("verify call ok");
        assert!(!r, "tampered payload should NOT verify");
    }

    #[test]
    fn verify_rejects_wrong_pubkey() {
        let dir = TempDir::new().expect("tmpdir");
        let (priv1, _pub1) = make_keypair(&dir, "k3a");
        let (_priv2, pub2) = make_keypair(&dir, "k3b");

        let payload = b"some bytes";
        let sig = sign_payload(&priv1, payload, WITNESS_NAMESPACE);

        let r = verify_witness_signature(&sig, payload, &pub2, "k3a@witness-test")
            .expect("verify call ok");
        assert!(!r, "wrong pubkey should NOT verify");
    }

    #[test]
    fn verify_rejects_cross_namespace_signature() {
        let dir = TempDir::new().expect("tmpdir");
        let (priv_path, pubkey) = make_keypair(&dir, "k4");

        let payload = b"some bytes";
        let sig = sign_payload(&priv_path, payload, "git-commit-signing");

        let r = verify_witness_signature(&sig, payload, &pubkey, "k4@witness-test")
            .expect("verify call ok");
        assert!(
            !r,
            "signature in a different namespace must NOT verify under WITNESS_NAMESPACE"
        );
    }

    #[test]
    fn verify_rejects_truncated_signature() {
        let dir = TempDir::new().expect("tmpdir");
        let (priv_path, pubkey) = make_keypair(&dir, "k5");

        let payload = b"some bytes";
        let mut sig = sign_payload(&priv_path, payload, WITNESS_NAMESPACE);
        sig.truncate(sig.len() / 2);

        let r = verify_witness_signature(&sig, payload, &pubkey, "k5@witness-test");
        assert!(
            !matches!(r, Ok(true)),
            "truncated sig must not verify; got {r:?}"
        );
    }
}
