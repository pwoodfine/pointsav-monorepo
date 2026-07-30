//! C2SP signed-note checkpoint primitive — the apex-cosigning substrate.
//!
//! Per `~/Foundry/conventions/system-substrate-doctrine.md` §4: the
//! deed-transfer ceremony is one signed ledger entry. C2SP signed-note
//! explicitly supports multi-signature on the same checkpoint, enabling
//! "previous apex revokes; new apex co-signs" without state migration.
//!
//! Format reference: <https://github.com/C2SP/C2SP/blob/main/signed-note.md>
//! and tlog-checkpoint.md. This module implements the body+signature
//! wire format and ed25519 verification; signing is performed by the
//! apex via its own keying surface (Sigstore Cosign, ssh-keygen, or
//! direct ed25519 — the format is signer-agnostic).
//!
//! # Wire format
//!
//! ```text
//! <origin>
//! <tree-size>
//! <base64-root-hash>
//! [<extension>...]
//!
//! — <signer-name> <base64(key-hash[4] || ed25519-sig[64])>
//! [— <signer-name> ...additional signatures...]
//! ```
//!
//! Body and signature block are separated by a blank line. The body
//! ends with `\n`; signature lines each end with `\n`.
//!
//! # Multi-signature (apex co-signing)
//!
//! Multiple signature lines on the same body realise the apex-rotation
//! primitive (convention §4): at the handover checkpoint, both P-old
//! and P-new sign the same body. The kernel verifier accepts the
//! handover when both signatures verify; subsequent checkpoints
//! require only P-new.

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use ed25519_dalek::{Signature as EdSignature, Verifier, VerifyingKey, SIGNATURE_LENGTH};
use sha2::{Digest, Sha256};

use crate::consistency_proof::{ConsistencyProof, ConsistencyVerifyError};
use crate::inclusion_proof::{InclusionProof, InclusionVerifyError};
use crate::Hash256;

#[cfg(not(feature = "std"))]
use alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};

/// One C2SP signed-note checkpoint body. Maps to a Merkle log state
/// at a specific tree size.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Checkpoint {
    /// Origin string identifying the log (e.g.,
    /// `foundry.<module-id>.capability-ledger`).
    pub origin: String,
    /// Number of entries committed to the log at this checkpoint.
    pub tree_size: u64,
    /// Merkle root hash at `tree_size`.
    pub root_hash: Hash256,
    /// Optional extension lines per the signed-note spec (e.g.,
    /// timestamp, doctrine version). Each line MUST NOT contain `\n`
    /// or start with `—`. Empty = no extensions.
    pub extensions: Vec<String>,
}

impl Checkpoint {
    /// Canonical body bytes per the spec — what gets signed.
    /// Lines: origin, tree_size (decimal), base64(root_hash),
    /// extensions...; each terminated by `\n`.
    pub fn body_bytes(&self) -> Vec<u8> {
        let mut s = String::new();
        s.push_str(&self.origin);
        s.push('\n');
        s.push_str(&self.tree_size.to_string());
        s.push('\n');
        s.push_str(&BASE64.encode(self.root_hash));
        s.push('\n');
        for ext in &self.extensions {
            s.push_str(ext);
            s.push('\n');
        }
        s.into_bytes()
    }

    /// Parse a checkpoint body from its canonical bytes.
    pub fn parse_body(body: &[u8]) -> Result<Self, ParseError> {
        let text = core::str::from_utf8(body).map_err(|_| ParseError::NotUtf8)?;
        let mut lines = text.split_inclusive('\n');

        let origin = lines
            .next()
            .ok_or(ParseError::Truncated)?
            .strip_suffix('\n')
            .ok_or(ParseError::MissingNewline)?
            .to_string();
        let tree_size_line = lines
            .next()
            .ok_or(ParseError::Truncated)?
            .strip_suffix('\n')
            .ok_or(ParseError::MissingNewline)?;
        let tree_size: u64 = tree_size_line
            .parse()
            .map_err(|_| ParseError::BadTreeSize)?;
        let root_hash_line = lines
            .next()
            .ok_or(ParseError::Truncated)?
            .strip_suffix('\n')
            .ok_or(ParseError::MissingNewline)?;
        let root_bytes = BASE64
            .decode(root_hash_line)
            .map_err(|_| ParseError::BadRootHash)?;
        if root_bytes.len() != 32 {
            return Err(ParseError::BadRootHashLength);
        }
        let mut root_hash = [0u8; 32];
        root_hash.copy_from_slice(&root_bytes);

        let extensions: Vec<String> = lines
            .filter_map(|l| l.strip_suffix('\n').map(|s| s.to_string()))
            .filter(|s| !s.is_empty())
            .collect();

        Ok(Checkpoint {
            origin,
            tree_size,
            root_hash,
            extensions,
        })
    }
}

/// One signature line on a signed-note. Multiple lines = multi-sig
/// (apex co-signing).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NoteSignature {
    /// Human-readable signer identity (e.g., `apex-acme-corp`).
    pub signer_name: String,
    /// First 4 bytes of `SHA-256("<signer-name>\nED25519\n<32-byte-pubkey>")`.
    pub key_hash: [u8; 4],
    /// 64-byte ed25519 signature over `Checkpoint::body_bytes()`.
    pub signature: [u8; SIGNATURE_LENGTH],
}

impl NoteSignature {
    /// Compute the 4-byte key hash that prefixes a signature line per
    /// the C2SP signed-note spec.
    pub fn derive_key_hash(signer_name: &str, pubkey: &[u8; 32]) -> [u8; 4] {
        let mut hasher = Sha256::new();
        hasher.update(signer_name.as_bytes());
        hasher.update(b"\n");
        hasher.update(b"ED25519\n");
        hasher.update(pubkey);
        let full = hasher.finalize();
        let mut out = [0u8; 4];
        out.copy_from_slice(&full[..4]);
        out
    }

    /// Render this signature as a single signed-note line (terminating
    /// `\n` included).
    pub fn to_line(&self) -> String {
        let mut payload = Vec::with_capacity(4 + SIGNATURE_LENGTH);
        payload.extend_from_slice(&self.key_hash);
        payload.extend_from_slice(&self.signature);
        format!("\u{2014} {} {}\n", self.signer_name, BASE64.encode(payload))
    }

    /// Parse a single signature line.
    pub fn parse_line(line: &str) -> Result<Self, ParseError> {
        let line = line.strip_suffix('\n').unwrap_or(line);
        let after = line
            .strip_prefix("\u{2014} ")
            .ok_or(ParseError::MissingEmDash)?;
        let (name, b64) = after
            .split_once(' ')
            .ok_or(ParseError::MalformedSignature)?;
        let bytes = BASE64
            .decode(b64)
            .map_err(|_| ParseError::MalformedSignature)?;
        if bytes.len() != 4 + SIGNATURE_LENGTH {
            return Err(ParseError::MalformedSignature);
        }
        let mut key_hash = [0u8; 4];
        key_hash.copy_from_slice(&bytes[..4]);
        let mut signature = [0u8; SIGNATURE_LENGTH];
        signature.copy_from_slice(&bytes[4..]);
        Ok(NoteSignature {
            signer_name: name.to_string(),
            key_hash,
            signature,
        })
    }
}

/// A signed checkpoint — body + ≥ 1 signature line. Multi-sig
/// realises the apex co-signing primitive (convention §4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedCheckpoint {
    /// The checkpoint body (origin, tree size, root hash, extensions).
    pub checkpoint: Checkpoint,
    /// One or more signatures over `checkpoint.body_bytes()`.
    pub signatures: Vec<NoteSignature>,
}

impl SignedCheckpoint {
    /// Render full wire format: body + blank line separator +
    /// signature lines. Per spec, the body ends with `\n`; an
    /// additional `\n` separates body from signature block.
    pub fn to_wire(&self) -> String {
        let mut out = String::from_utf8(self.checkpoint.body_bytes())
            .expect("body is valid UTF-8 by construction");
        out.push('\n');
        for sig in &self.signatures {
            out.push_str(&sig.to_line());
        }
        out
    }

    /// Parse a wire-format signed-note. Body is everything before the
    /// blank-line separator; signature block follows.
    pub fn parse(wire: &str) -> Result<Self, ParseError> {
        let sep_idx = wire
            .find("\n\n")
            .ok_or(ParseError::MissingSignatureSeparator)?;
        let body = &wire[..=sep_idx]; // include trailing \n
        let sig_block = &wire[sep_idx + 2..];
        let checkpoint = Checkpoint::parse_body(body.as_bytes())?;
        let mut signatures = Vec::new();
        for line in sig_block.split_inclusive('\n') {
            if line.trim().is_empty() {
                continue;
            }
            signatures.push(NoteSignature::parse_line(line)?);
        }
        if signatures.is_empty() {
            return Err(ParseError::NoSignatures);
        }
        Ok(SignedCheckpoint {
            checkpoint,
            signatures,
        })
    }

    /// Verify a specific signer's signature over the checkpoint body.
    /// Returns `Ok(true)` if any signature line matches `(signer_name,
    /// pubkey)` and verifies; `Ok(false)` if no matching key-hash
    /// found; `Err(_)` on cryptographic failure.
    pub fn verify_signer(&self, signer_name: &str, pubkey: &[u8; 32]) -> Result<bool, VerifyError> {
        let expected_kh = NoteSignature::derive_key_hash(signer_name, pubkey);
        let body = self.checkpoint.body_bytes();
        let vk = VerifyingKey::from_bytes(pubkey).map_err(|_| VerifyError::BadPublicKey)?;
        for sig in &self.signatures {
            if sig.signer_name == signer_name && sig.key_hash == expected_kh {
                let edsig = EdSignature::from_bytes(&sig.signature);
                return Ok(vk.verify(&body, &edsig).is_ok());
            }
        }
        Ok(false)
    }

    /// Verify that BOTH (P-old, P-new) signatures appear and verify on
    /// the same body — the apex-rotation handover predicate per
    /// convention §4.
    pub fn verify_apex_handover(
        &self,
        old_apex_name: &str,
        old_apex_pubkey: &[u8; 32],
        new_apex_name: &str,
        new_apex_pubkey: &[u8; 32],
    ) -> Result<bool, VerifyError> {
        Ok(self.verify_signer(old_apex_name, old_apex_pubkey)?
            && self.verify_signer(new_apex_name, new_apex_pubkey)?)
    }

    /// Composed kernel-facing primitive (Master directive
    /// 2026-04-27): verify both that this checkpoint is validly
    /// signed by `(signer_name, signer_pubkey)` AND that `leaf_hash`
    /// is included in the checkpoint's Merkle root via `proof`.
    ///
    /// Use this instead of calling [`InclusionProof::verify`]
    /// directly. Treating signature + inclusion as a single
    /// load-bearing primitive avoids "verified-inclusion-against-
    /// untrusted-root" footguns.
    ///
    /// Order: tree-size match → signature verification → inclusion
    /// verification. Returns the first-encountered failure; on
    /// success returns `Ok(())`.
    pub fn verify_inclusion_proof(
        &self,
        proof: &InclusionProof,
        leaf_hash: &Hash256,
        signer_name: &str,
        signer_pubkey: &[u8; 32],
    ) -> Result<(), CheckpointInclusionError> {
        if proof.tree_size != self.checkpoint.tree_size {
            return Err(CheckpointInclusionError::TreeSizeMismatch);
        }
        let sig_ok = self
            .verify_signer(signer_name, signer_pubkey)
            .map_err(|_| CheckpointInclusionError::BadSignerPublicKey)?;
        if !sig_ok {
            return Err(CheckpointInclusionError::SignatureInvalid);
        }
        proof
            .verify(leaf_hash, &self.checkpoint.root_hash)
            .map_err(CheckpointInclusionError::Inclusion)
    }

    /// Composed kernel-facing primitive: verify that
    /// `old_signed_checkpoint` is a consistent prefix of `self`,
    /// with both signed by `(signer_name, signer_pubkey)`.
    ///
    /// Use this instead of calling [`ConsistencyProof::verify`]
    /// directly. Treating signature + consistency as a single
    /// primitive avoids "verified-consistency-against-untrusted-
    /// root" footguns — both roots are authenticated before the
    /// proof is checked.
    ///
    /// Check order (first failure returns):
    /// 1. `old_signed_checkpoint.tree_size == proof.old_size` — [`CheckpointConsistencyError::OldTreeSizeMismatch`]
    /// 2. `self.tree_size == proof.new_size` — [`CheckpointConsistencyError::NewTreeSizeMismatch`]
    /// 3. old checkpoint signature valid — [`CheckpointConsistencyError::BadSignerPublicKey`] / [`CheckpointConsistencyError::OldSignatureInvalid`]
    /// 4. new checkpoint (self) signature valid — [`CheckpointConsistencyError::BadSignerPublicKey`] / [`CheckpointConsistencyError::NewSignatureInvalid`]
    /// 5. `proof.verify(old_root, old_size, new_root, new_size)` — [`CheckpointConsistencyError::Consistency`]
    pub fn verify_consistency_proof(
        &self,
        proof: &ConsistencyProof,
        old_size: u64,
        new_size: u64,
        old_signed_checkpoint: &SignedCheckpoint,
        signer_name: &str,
        signer_pubkey: &[u8; 32],
    ) -> Result<(), CheckpointConsistencyError> {
        // Step 1 — old tree-size match.
        if old_signed_checkpoint.checkpoint.tree_size != old_size {
            return Err(CheckpointConsistencyError::OldTreeSizeMismatch);
        }
        // Step 2 — new tree-size match.
        if self.checkpoint.tree_size != new_size {
            return Err(CheckpointConsistencyError::NewTreeSizeMismatch);
        }
        // Step 3 — old checkpoint signature.
        let old_sig_ok = old_signed_checkpoint
            .verify_signer(signer_name, signer_pubkey)
            .map_err(|_| CheckpointConsistencyError::BadSignerPublicKey)?;
        if !old_sig_ok {
            return Err(CheckpointConsistencyError::OldSignatureInvalid);
        }
        // Step 4 — new checkpoint (self) signature.
        let new_sig_ok = self
            .verify_signer(signer_name, signer_pubkey)
            .map_err(|_| CheckpointConsistencyError::BadSignerPublicKey)?;
        if !new_sig_ok {
            return Err(CheckpointConsistencyError::NewSignatureInvalid);
        }
        // Step 5 — raw consistency proof.
        proof
            .verify(
                old_signed_checkpoint.checkpoint.root_hash,
                old_size,
                self.checkpoint.root_hash,
                new_size,
            )
            .map_err(CheckpointConsistencyError::Consistency)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckpointInclusionError {
    /// Proof's `tree_size` doesn't match this checkpoint's `tree_size`.
    TreeSizeMismatch,
    /// `signer_pubkey` couldn't be parsed as an ed25519 public key.
    BadSignerPublicKey,
    /// No signature line under `signer_name` matched, or the
    /// signature failed cryptographic verification.
    SignatureInvalid,
    /// The Merkle inclusion proof failed against the checkpoint's
    /// root.
    Inclusion(InclusionVerifyError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckpointConsistencyError {
    /// `old_signed_checkpoint.tree_size` does not match `old_size`.
    OldTreeSizeMismatch,
    /// `self.checkpoint.tree_size` does not match `new_size`.
    NewTreeSizeMismatch,
    /// `signer_pubkey` couldn't be parsed as an ed25519 public key.
    BadSignerPublicKey,
    /// Old checkpoint signature did not verify under
    /// `(signer_name, signer_pubkey)`.
    OldSignatureInvalid,
    /// New checkpoint (self) signature did not verify.
    NewSignatureInvalid,
    /// Raw consistency proof verification failed.
    Consistency(ConsistencyVerifyError),
}

/// Errors returned when parsing a signed-note checkpoint from wire bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// Input bytes are not valid UTF-8.
    NotUtf8,
    /// Input ended before all required fields were parsed.
    Truncated,
    /// A required line was not terminated by `\n`.
    MissingNewline,
    /// Tree-size line could not be parsed as a decimal `u64`.
    BadTreeSize,
    /// Root-hash line is not valid base64.
    BadRootHash,
    /// Root-hash decoded to a length other than 32 bytes.
    BadRootHashLength,
    /// No `\n\n` separator between body and signature block found.
    MissingSignatureSeparator,
    /// A signature line did not start with the C2SP em-dash (`—`) prefix.
    MissingEmDash,
    /// A signature line's payload could not be base64-decoded, or decoded
    /// to a length other than 68 bytes (4-byte key-hash + 64-byte sig).
    MalformedSignature,
    /// Signature block contained no parseable signature lines.
    NoSignatures,
}

/// Errors returned by ed25519 signature verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyError {
    /// The provided public-key bytes are not a valid ed25519 point.
    BadPublicKey,
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    fn fixture_checkpoint() -> Checkpoint {
        Checkpoint {
            origin: "foundry.test.capability-ledger".to_string(),
            tree_size: 100,
            root_hash: [0xCC; 32],
            extensions: vec![],
        }
    }

    fn fixed_keypair(seed: u8) -> (SigningKey, [u8; 32]) {
        let sk = SigningKey::from_bytes(&[seed; 32]);
        let pk_bytes = sk.verifying_key().to_bytes();
        (sk, pk_bytes)
    }

    fn sign(checkpoint: &Checkpoint, signer_name: &str, sk: &SigningKey) -> NoteSignature {
        let pk = sk.verifying_key().to_bytes();
        let key_hash = NoteSignature::derive_key_hash(signer_name, &pk);
        let sig = sk.sign(&checkpoint.body_bytes());
        NoteSignature {
            signer_name: signer_name.to_string(),
            key_hash,
            signature: sig.to_bytes(),
        }
    }

    #[test]
    fn checkpoint_body_round_trip() {
        let cp = fixture_checkpoint();
        let bytes = cp.body_bytes();
        let restored = Checkpoint::parse_body(&bytes).unwrap();
        assert_eq!(cp, restored);
    }

    #[test]
    fn checkpoint_with_extensions_round_trip() {
        let cp = Checkpoint {
            origin: "foundry.test.x".to_string(),
            tree_size: 42,
            root_hash: [0xDD; 32],
            extensions: vec!["timestamp 1730000000".to_string()],
        };
        let bytes = cp.body_bytes();
        let restored = Checkpoint::parse_body(&bytes).unwrap();
        assert_eq!(cp, restored);
    }

    #[test]
    fn key_hash_derivation_is_deterministic() {
        let pk = [0xEE; 32];
        let h1 = NoteSignature::derive_key_hash("apex-acme", &pk);
        let h2 = NoteSignature::derive_key_hash("apex-acme", &pk);
        assert_eq!(h1, h2);
    }

    #[test]
    fn key_hash_changes_with_name() {
        let pk = [0xEE; 32];
        let h1 = NoteSignature::derive_key_hash("apex-acme", &pk);
        let h2 = NoteSignature::derive_key_hash("apex-other", &pk);
        assert_ne!(h1, h2);
    }

    #[test]
    fn signed_checkpoint_wire_round_trip_single_sig() {
        let (sk, _pk) = fixed_keypair(7);
        let cp = fixture_checkpoint();
        let sig = sign(&cp, "apex-test", &sk);
        let signed = SignedCheckpoint {
            checkpoint: cp,
            signatures: vec![sig],
        };
        let wire = signed.to_wire();
        let restored = SignedCheckpoint::parse(&wire).unwrap();
        assert_eq!(signed, restored);
    }

    #[test]
    fn single_signature_verifies() {
        let (sk, pk) = fixed_keypair(11);
        let cp = fixture_checkpoint();
        let sig = sign(&cp, "apex-acme", &sk);
        let signed = SignedCheckpoint {
            checkpoint: cp,
            signatures: vec![sig],
        };
        assert!(signed.verify_signer("apex-acme", &pk).unwrap());
    }

    #[test]
    fn signature_fails_under_wrong_pubkey() {
        let (sk, _pk_correct) = fixed_keypair(11);
        let (_, pk_wrong) = fixed_keypair(13);
        let cp = fixture_checkpoint();
        let sig = sign(&cp, "apex-acme", &sk);
        let signed = SignedCheckpoint {
            checkpoint: cp,
            signatures: vec![sig],
        };
        // Wrong pubkey — key_hash won't match, so we get false (no
        // matching signer), not an error.
        assert!(!signed.verify_signer("apex-acme", &pk_wrong).unwrap());
    }

    #[test]
    fn multi_sig_apex_handover_round_trip() {
        let (sk_old, pk_old) = fixed_keypair(17);
        let (sk_new, pk_new) = fixed_keypair(19);
        let cp = fixture_checkpoint();
        let sig_old = sign(&cp, "apex-old", &sk_old);
        let sig_new = sign(&cp, "apex-new", &sk_new);
        let signed = SignedCheckpoint {
            checkpoint: cp,
            signatures: vec![sig_old, sig_new],
        };
        let wire = signed.to_wire();
        let restored = SignedCheckpoint::parse(&wire).unwrap();
        assert_eq!(signed, restored);
        assert!(restored
            .verify_apex_handover("apex-old", &pk_old, "apex-new", &pk_new)
            .unwrap());
    }

    #[test]
    fn handover_fails_if_only_one_signs() {
        let (sk_old, pk_old) = fixed_keypair(17);
        let (_, pk_new) = fixed_keypair(19);
        let cp = fixture_checkpoint();
        let sig_old = sign(&cp, "apex-old", &sk_old);
        // P-new did NOT sign.
        let signed = SignedCheckpoint {
            checkpoint: cp,
            signatures: vec![sig_old],
        };
        assert!(!signed
            .verify_apex_handover("apex-old", &pk_old, "apex-new", &pk_new)
            .unwrap());
    }

    #[test]
    fn body_tampering_breaks_signature() {
        let (sk, pk) = fixed_keypair(23);
        let cp = fixture_checkpoint();
        let sig = sign(&cp, "apex-test", &sk);
        // Tamper with the checkpoint after signing.
        let mut tampered = cp.clone();
        tampered.tree_size = 999;
        let signed = SignedCheckpoint {
            checkpoint: tampered,
            signatures: vec![sig],
        };
        assert!(!signed.verify_signer("apex-test", &pk).unwrap());
    }

    // ---------- verify_inclusion_proof composed-primitive tests ----------

    use crate::inclusion_proof::{rfc9162_internal_hash, rfc9162_leaf_hash, InclusionProof};

    fn build_root(leaf_hashes: &[Hash256]) -> Hash256 {
        let mut layer = leaf_hashes.to_vec();
        while layer.len() > 1 {
            let mut next = Vec::with_capacity(layer.len().div_ceil(2));
            let mut i = 0;
            while i < layer.len() {
                if i + 1 < layer.len() {
                    next.push(rfc9162_internal_hash(&layer[i], &layer[i + 1]));
                } else {
                    next.push(layer[i]);
                }
                i += 2;
            }
            layer = next;
        }
        layer.into_iter().next().expect("≥ 1 leaf")
    }

    fn make_proof(leaf_hashes: &[Hash256], leaf_index: u64) -> InclusionProof {
        let mut path = Vec::new();
        let mut layer = leaf_hashes.to_vec();
        let mut idx = leaf_index as usize;
        while layer.len() > 1 {
            let sibling_idx = idx ^ 1;
            if sibling_idx < layer.len() {
                path.push(layer[sibling_idx]);
            }
            let mut next = Vec::with_capacity(layer.len().div_ceil(2));
            let mut i = 0;
            while i < layer.len() {
                if i + 1 < layer.len() {
                    next.push(rfc9162_internal_hash(&layer[i], &layer[i + 1]));
                } else {
                    next.push(layer[i]);
                }
                i += 2;
            }
            idx /= 2;
            layer = next;
        }
        InclusionProof {
            leaf_index,
            tree_size: leaf_hashes.len() as u64,
            sibling_hashes: path,
        }
    }

    fn signed_checkpoint_with_root(
        tree_size: u64,
        root_hash: Hash256,
        signer_name: &str,
        sk: &ed25519_dalek::SigningKey,
    ) -> SignedCheckpoint {
        let cp = Checkpoint {
            origin: "foundry.test.cap-ledger".to_string(),
            tree_size,
            root_hash,
            extensions: vec![],
        };
        let pk = sk.verifying_key().to_bytes();
        let key_hash = NoteSignature::derive_key_hash(signer_name, &pk);
        let sig = sk.sign(&cp.body_bytes()).to_bytes();
        SignedCheckpoint {
            checkpoint: cp,
            signatures: vec![NoteSignature {
                signer_name: signer_name.to_string(),
                key_hash,
                signature: sig,
            }],
        }
    }

    #[test]
    fn inclusion_proof_with_valid_sig_and_proof_accepts() {
        let (sk, pk) = fixed_keypair(31);
        let leaves: Vec<Hash256> = (0..4u64)
            .map(|i| rfc9162_leaf_hash(format!("witness-{i}").as_bytes()))
            .collect();
        let root = build_root(&leaves);
        let signed = signed_checkpoint_with_root(4, root, "apex", &sk);

        for i in 0..4u64 {
            let proof = make_proof(&leaves, i);
            let r = signed.verify_inclusion_proof(&proof, &leaves[i as usize], "apex", &pk);
            assert!(r.is_ok(), "leaf {i}: should accept; got {r:?}");
        }
    }

    #[test]
    fn inclusion_proof_with_tampered_signature_rejects() {
        let (sk, _pk_correct) = fixed_keypair(31);
        let (_sk_other, pk_wrong) = fixed_keypair(33);
        let leaves: Vec<Hash256> = (0..4u64)
            .map(|i| rfc9162_leaf_hash(format!("w{i}").as_bytes()))
            .collect();
        let root = build_root(&leaves);
        let signed = signed_checkpoint_with_root(4, root, "apex", &sk);
        let proof = make_proof(&leaves, 1);

        // Wrong pubkey → key_hash mismatch → SignatureInvalid.
        let r = signed.verify_inclusion_proof(&proof, &leaves[1], "apex", &pk_wrong);
        assert_eq!(r, Err(CheckpointInclusionError::SignatureInvalid));
    }

    #[test]
    fn inclusion_proof_with_tampered_proof_rejects() {
        let (sk, pk) = fixed_keypair(31);
        let leaves: Vec<Hash256> = (0..4u64)
            .map(|i| rfc9162_leaf_hash(format!("w{i}").as_bytes()))
            .collect();
        let root = build_root(&leaves);
        let signed = signed_checkpoint_with_root(4, root, "apex", &sk);
        let mut proof = make_proof(&leaves, 1);
        proof.sibling_hashes[0] = [0xFF; 32]; // tamper

        let r = signed.verify_inclusion_proof(&proof, &leaves[1], "apex", &pk);
        assert!(
            matches!(r, Err(CheckpointInclusionError::Inclusion(_))),
            "expected Inclusion error, got {r:?}"
        );
    }

    #[test]
    fn inclusion_proof_with_tree_size_mismatch_rejects() {
        let (sk, pk) = fixed_keypair(31);
        let leaves: Vec<Hash256> = (0..4u64)
            .map(|i| rfc9162_leaf_hash(format!("w{i}").as_bytes()))
            .collect();
        let root = build_root(&leaves);
        // Checkpoint claims tree_size = 8; proof is for tree of 4.
        let signed = signed_checkpoint_with_root(8, root, "apex", &sk);
        let proof = make_proof(&leaves, 1);

        let r = signed.verify_inclusion_proof(&proof, &leaves[1], "apex", &pk);
        assert_eq!(r, Err(CheckpointInclusionError::TreeSizeMismatch));
    }

    #[test]
    fn inclusion_proof_with_wrong_leaf_hash_rejects() {
        let (sk, pk) = fixed_keypair(31);
        let leaves: Vec<Hash256> = (0..4u64)
            .map(|i| rfc9162_leaf_hash(format!("w{i}").as_bytes()))
            .collect();
        let root = build_root(&leaves);
        let signed = signed_checkpoint_with_root(4, root, "apex", &sk);
        let proof = make_proof(&leaves, 1);
        let wrong_leaf = [0xCC; 32];

        let r = signed.verify_inclusion_proof(&proof, &wrong_leaf, "apex", &pk);
        assert!(
            matches!(r, Err(CheckpointInclusionError::Inclusion(_))),
            "expected Inclusion error, got {r:?}"
        );
    }

    // ---------- verify_consistency_proof composed-primitive tests ----------

    use crate::consistency_proof::ConsistencyProof;

    /// Build a consistency proof from old_leaves → new_leaves using the
    /// same oracle pattern as `consistency_proof::tests::make_consistency_proof`.
    /// Simplified inline version to keep the test module self-contained.
    fn make_consistency_proof_for_test(old_n: usize, new_n: usize) -> ConsistencyProof {
        if old_n == new_n {
            return ConsistencyProof { hashes: vec![] };
        }
        let new_leaves: Vec<Hash256> = (0..new_n as u64)
            .map(|i| rfc9162_leaf_hash(format!("leaf-{i}").as_bytes()))
            .collect();
        let old_leaves: Vec<Hash256> = new_leaves[..old_n].to_vec();

        // Build all layers for old and new trees.
        let build_layers_local = |leaves: &[Hash256]| -> Vec<Vec<Hash256>> {
            let mut layers: Vec<Vec<Hash256>> = vec![leaves.to_vec()];
            while layers.last().unwrap().len() > 1 {
                let prev = layers.last().unwrap().clone();
                let mut next = Vec::with_capacity(prev.len().div_ceil(2));
                let mut i = 0;
                while i < prev.len() {
                    if i + 1 < prev.len() {
                        next.push(rfc9162_internal_hash(&prev[i], &prev[i + 1]));
                    } else {
                        next.push(prev[i]);
                    }
                    i += 2;
                }
                layers.push(next);
            }
            layers
        };

        let new_layers = build_layers_local(&new_leaves);
        let old_layers = build_layers_local(&old_leaves);

        let get_new = |lv: usize, idx: usize| -> Option<Hash256> {
            new_layers.get(lv).and_then(|l| l.get(idx)).copied()
        };
        let get_old = |lv: usize, idx: usize| -> Option<Hash256> {
            old_layers.get(lv).and_then(|l| l.get(idx)).copied()
        };

        let mut path: Vec<Hash256> = Vec::new();
        path.push(get_old(0, old_n - 1).expect("old tree anchor"));

        let mut n_loop = (old_n - 1) as u64;
        let mut ln_loop = (new_n - 1) as u64;
        let mut lv: usize = 0;

        while ln_loop != 0 {
            if n_loop & 1 == 1 || n_loop == ln_loop {
                let mut n_stripped = n_loop;
                let mut _ln_stripped = ln_loop;
                let mut lv_stripped = lv;
                while n_stripped & 1 == 0 && n_stripped != 0 {
                    n_stripped >>= 1;
                    _ln_stripped >>= 1;
                    lv_stripped += 1;
                }
                let sibling_idx = (n_stripped ^ 1) as usize;
                let p = get_new(lv_stripped, sibling_idx)
                    .or_else(|| get_old(lv_stripped, sibling_idx))
                    .expect("sibling exists");
                path.push(p);
                while n_loop & 1 == 0 && n_loop != 0 {
                    n_loop >>= 1;
                    ln_loop >>= 1;
                    lv += 1;
                }
            } else {
                let sibling_idx = (n_loop ^ 1) as usize;
                let p = get_new(lv, sibling_idx).expect("sibling exists");
                path.push(p);
            }
            n_loop >>= 1;
            ln_loop >>= 1;
            lv += 1;
        }
        ConsistencyProof { hashes: path }
    }

    fn signed_checkpoint_n_leaves(
        n: u64,
        signer_name: &str,
        sk: &ed25519_dalek::SigningKey,
    ) -> (SignedCheckpoint, Vec<Hash256>) {
        let leaves: Vec<Hash256> = (0..n)
            .map(|i| rfc9162_leaf_hash(format!("leaf-{i}").as_bytes()))
            .collect();
        let root = build_root(&leaves);
        let sc = signed_checkpoint_with_root(n, root, signer_name, sk);
        (sc, leaves)
    }

    #[test]
    fn consistency_proof_valid_accepts() {
        let (sk, pk) = fixed_keypair(41);
        let (old_sc, _old_leaves) = signed_checkpoint_n_leaves(4, "apex", &sk);
        let (new_sc, _new_leaves) = signed_checkpoint_n_leaves(7, "apex", &sk);
        let proof = make_consistency_proof_for_test(4, 7);

        let r = new_sc.verify_consistency_proof(&proof, 4, 7, &old_sc, "apex", &pk);
        assert_eq!(r, Ok(()), "4→7 consistency proof should accept; got {r:?}");
    }

    #[test]
    fn consistency_proof_old_tree_size_mismatch_rejects() {
        let (sk, pk) = fixed_keypair(41);
        let (old_sc, _) = signed_checkpoint_n_leaves(4, "apex", &sk);
        let (new_sc, _) = signed_checkpoint_n_leaves(7, "apex", &sk);
        let proof = make_consistency_proof_for_test(4, 7);

        // Pass old_size=5 but old_sc.tree_size=4 → OldTreeSizeMismatch.
        let r = new_sc.verify_consistency_proof(&proof, 5, 7, &old_sc, "apex", &pk);
        assert_eq!(r, Err(CheckpointConsistencyError::OldTreeSizeMismatch));
    }

    #[test]
    fn consistency_proof_new_tree_size_mismatch_rejects() {
        let (sk, pk) = fixed_keypair(41);
        let (old_sc, _) = signed_checkpoint_n_leaves(4, "apex", &sk);
        let (new_sc, _) = signed_checkpoint_n_leaves(7, "apex", &sk);
        let proof = make_consistency_proof_for_test(4, 7);

        // Pass new_size=8 but new_sc.tree_size=7 → NewTreeSizeMismatch.
        let r = new_sc.verify_consistency_proof(&proof, 4, 8, &old_sc, "apex", &pk);
        assert_eq!(r, Err(CheckpointConsistencyError::NewTreeSizeMismatch));
    }

    #[test]
    fn consistency_proof_wrong_signer_pubkey_rejects() {
        let (sk, _pk_correct) = fixed_keypair(41);
        let (_, pk_wrong) = fixed_keypair(43);
        let (old_sc, _) = signed_checkpoint_n_leaves(4, "apex", &sk);
        let (new_sc, _) = signed_checkpoint_n_leaves(7, "apex", &sk);
        let proof = make_consistency_proof_for_test(4, 7);

        // Wrong pubkey → key_hash mismatch → OldSignatureInvalid
        // (verify_signer returns Ok(false), not Err).
        let r = new_sc.verify_consistency_proof(&proof, 4, 7, &old_sc, "apex", &pk_wrong);
        assert_eq!(r, Err(CheckpointConsistencyError::OldSignatureInvalid));
    }

    #[test]
    fn consistency_proof_corrupt_hash_rejects() {
        let (sk, pk) = fixed_keypair(41);
        let (old_sc, _) = signed_checkpoint_n_leaves(4, "apex", &sk);
        let (new_sc, _) = signed_checkpoint_n_leaves(7, "apex", &sk);
        let mut proof = make_consistency_proof_for_test(4, 7);
        assert!(!proof.hashes.is_empty());
        proof.hashes[0] = [0xFFu8; 32]; // corrupt

        let r = new_sc.verify_consistency_proof(&proof, 4, 7, &old_sc, "apex", &pk);
        assert!(
            matches!(r, Err(CheckpointConsistencyError::Consistency(_))),
            "expected Consistency error, got {r:?}"
        );
    }

    // ---------- ParseError variant tests ----------

    #[test]
    fn parse_error_not_utf8() {
        let bad_bytes = b"\xFF\xFE invalid utf-8";
        assert_eq!(Checkpoint::parse_body(bad_bytes), Err(ParseError::NotUtf8));
    }

    #[test]
    fn parse_error_truncated() {
        assert_eq!(Checkpoint::parse_body(b""), Err(ParseError::Truncated));
    }

    #[test]
    fn parse_error_missing_newline() {
        // First line has no trailing \n — stripped_suffix returns None.
        assert_eq!(
            Checkpoint::parse_body(b"foundry.test.no-newline"),
            Err(ParseError::MissingNewline)
        );
    }

    #[test]
    fn parse_error_bad_root_hash_length() {
        // "AAAA" decodes to 3 bytes — not the required 32 → BadRootHashLength.
        let body = b"foundry.test.cap-ledger\n100\nAAAA\n";
        assert_eq!(
            Checkpoint::parse_body(body),
            Err(ParseError::BadRootHashLength)
        );
    }

    #[test]
    fn parse_error_missing_signature_separator() {
        // Wire text with no \n\n between body and signature block.
        let wire = "foundry.test.cap-ledger\n100\nAAAA\n— apex AAAA\n";
        assert_eq!(
            SignedCheckpoint::parse(wire),
            Err(ParseError::MissingSignatureSeparator)
        );
    }

    // ---------- VerifyError::BadPublicKey ----------

    #[test]
    fn verify_error_bad_public_key() {
        // y=2 is a quadratic non-residue on Ed25519 (verified: Legendre(u/v, p)=-1
        // for y=2 where u=y^2-1, v=d*y^2+1). CompressedEdwardsY::decompress()
        // returns None → VerifyError::BadPublicKey.
        let (sk, _) = fixed_keypair(7);
        let cp = fixture_checkpoint();
        let sig = sign(&cp, "apex-test", &sk);
        let signed = SignedCheckpoint {
            checkpoint: cp,
            signatures: vec![sig],
        };
        let mut bad_pubkey = [0u8; 32];
        bad_pubkey[0] = 2; // y=2, no corresponding x exists on Ed25519
        assert_eq!(
            signed.verify_signer("apex-test", &bad_pubkey),
            Err(VerifyError::BadPublicKey)
        );
    }

    // ---------- NewSignatureInvalid coverage ----------

    #[test]
    fn consistency_proof_new_signature_invalid_rejects() {
        // Old checkpoint signed by sk1 (correct); new checkpoint signed by sk2
        // (different key). Passing pk1 → old sig verifies, new sig doesn't →
        // NewSignatureInvalid.
        let (sk1, pk1) = fixed_keypair(41);
        let (sk2, _pk2) = fixed_keypair(43);
        let (old_sc, _) = signed_checkpoint_n_leaves(4, "apex", &sk1);
        let (new_sc, _) = signed_checkpoint_n_leaves(7, "apex", &sk2);
        let proof = make_consistency_proof_for_test(4, 7);

        let r = new_sc.verify_consistency_proof(&proof, 4, 7, &old_sc, "apex", &pk1);
        assert_eq!(r, Err(CheckpointConsistencyError::NewSignatureInvalid));
    }
}
