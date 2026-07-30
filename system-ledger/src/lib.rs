//! system-ledger — the substrate-tier consumer of the Capability
//! Ledger Substrate primitives in `system-core`.
//!
//! Per `~/Foundry/conventions/system-substrate-doctrine.md` §3.1:
//! "Before the kernel honors any capability invocation, it consults
//! the ledger for: current revocation status of the invoking
//! capability; time-bound expiry per Mechanism A; apex root validity."
//!
//! This crate owns the **state machine** that performs that
//! consultation. The cryptographic primitives (Capability,
//! WitnessRecord, SignedCheckpoint) live in `system-core`; this
//! crate composes them into a kernel-side verifier.
//!
//! # Module layout
//!
//! - [`cache`] — recent-N checkpoint cache (LRU)
//! - [`revocation`] — revoked-capability set
//! - [`apex`] — apex history + post-handover invariant
//! - [`witness`] — `ssh-keygen -Y verify` wrapper for witness records
//!
//! # Public API
//!
//! [`LedgerConsumer`] is the kernel-facing trait. v0.1.x ships
//! [`InMemoryLedger`] as the concrete impl; future MINOR may add
//! `MoonshotDatabaseLedger` once `moonshot-database` ships.

#![cfg_attr(feature = "sel4", no_std)]
#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::{
    collections::BTreeSet,
    format,
    string::{String, ToString},
    vec::Vec,
};
#[cfg(feature = "std")]
use std::collections::HashSet;

pub mod apex;
pub mod cache;
pub mod revocation;
pub mod witness;

#[cfg(feature = "std")]
type WitnessedSet = HashSet<system_core::Hash256>;
#[cfg(not(feature = "std"))]
type WitnessedSet = BTreeSet<system_core::Hash256>;

use system_core::{
    Capability, CheckpointInclusionError, Hash256, InclusionProof, SignedCheckpoint, WitnessRecord,
};

/// Kernel verifier verdict on a capability invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Honor the invocation. Capability is current and unexpired.
    Allow,
    /// Refuse with structured reason.
    Refuse(RefuseReason),
    /// Honor the invocation AND log the witness extension into the
    /// ledger so future invocations see the new expiry. Caller MUST
    /// append the witness record to the ledger before honoring.
    ExtendThenAllow { new_expiry_t: u64 },
}

/// Why an invocation was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefuseReason {
    /// Capability is in the revocation set.
    Revoked,
    /// `expiry_t` reached and no valid witness extension presented.
    Expired,
    /// Witness signature failed verification.
    WitnessSignatureInvalid,
    /// Witness record's hash is not in the current Merkle root.
    WitnessNotInLedger,
    /// Witness presented but capability has no `witness_pubkey`
    /// (non-extensible by construction).
    NotExtensible,
    /// `current_root` is not signed by the current apex.
    ApexInvalid,
    /// Post-handover invariant: only P-new accepted from N+3+;
    /// presented checkpoint signed only by P-old.
    StaleApex,
}

/// Errors that prevent the verifier from rendering a verdict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConsultError {
    /// Internal cache or state inconsistency.
    InconsistentState(String),
    /// `ssh-keygen -Y verify` invocation failed.
    WitnessVerifyFailed(String),
}

/// Errors applying state changes to the ledger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LedgerError {
    /// Apex handover refused — handover checkpoint malformed or
    /// fails the both-signatures-required predicate.
    InvalidHandover(String),
    /// Revocation event references an unknown capability.
    UnknownCapability,
    /// `apply_witness_record` was called but the ledger has no
    /// current checkpoint set — the consumer must call
    /// [`InMemoryLedger::set_current_checkpoint`] (or equivalent on
    /// other implementors) before applying witness records.
    NoCurrentCheckpoint,
    /// `apply_witness_record` proof failed to verify against the
    /// current checkpoint's root.
    WitnessNotInRoot(CheckpointInclusionError),
    /// The current checkpoint's apex pubkey hasn't been recorded —
    /// the consumer must `apply_apex_handover` (or `record_genesis`)
    /// before applying witness records.
    NoApexForCheckpoint,
}

/// Kernel-facing consumer of the Capability Ledger Substrate. The
/// kernel calls into this trait to decide whether to honor a
/// capability invocation; the concrete impl owns the cache /
/// revocation / apex / witness state.
pub trait LedgerConsumer {
    /// Consult the ledger about a capability invocation. Returns a
    /// [`Verdict`] or a [`ConsultError`] if the consultation itself
    /// fails (cache miss without on-disk fallback, witness-verify
    /// shellout failure, etc).
    ///
    /// `now` is unix seconds (UTC). `witness` is optional; if the
    /// capability is past `expiry_t` and no witness is supplied,
    /// the verdict is [`Verdict::Refuse`] with [`RefuseReason::Expired`].
    fn consult_capability(
        &self,
        cap: &Capability,
        current_root: &SignedCheckpoint,
        now: u64,
        witness: Option<&WitnessRecord>,
    ) -> Result<Verdict, ConsultError>;

    /// Apply a revocation event to the ledger state. After this
    /// call, subsequent [`LedgerConsumer::consult_capability`] for the revoked
    /// capability returns [`Verdict::Refuse`] with [`RefuseReason::Revoked`].
    fn apply_revocation(&mut self, event: revocation::RevocationEvent) -> Result<(), LedgerError>;

    /// Apply an apex handover. After this call, only `new_apex`
    /// signatures are accepted on subsequent checkpoints
    /// (post-handover invariant per convention §4 height-N+3+).
    fn apply_apex_handover(
        &mut self,
        old_apex_name: &str,
        old_apex_pubkey: &[u8; 32],
        new_apex_name: &str,
        new_apex_pubkey: &[u8; 32],
        handover_checkpoint: &SignedCheckpoint,
    ) -> Result<(), LedgerError>;

    /// Record a witness record as having been logged in the ledger.
    ///
    /// Per Master directive 2026-04-27 (Phase 1A.4): the production
    /// path takes an [`InclusionProof`] proving the record's leaf
    /// hash is in the implementor's current ledger root. The
    /// implementor uses the composed
    /// [`SignedCheckpoint::verify_inclusion_proof`] primitive to
    /// validate before recording.
    ///
    /// Returns:
    /// - `Ok(())` on successful verification + insert (or idempotent
    ///   replay; replay tolerance is implementor-defined).
    /// - `Err(LedgerError::NoCurrentCheckpoint)` if the implementor
    ///   has no current root to validate against.
    /// - `Err(LedgerError::WitnessNotInRoot(_))` if the proof fails
    ///   against the current root.
    ///
    /// # Handover-height policy
    ///
    /// At a handover height (per `system-substrate-doctrine.md` §4),
    /// **either apex's signature is sufficient for inclusion-proof
    /// verification**. The check this method performs is structural
    /// (chain-state attestation), not governance: a checkpoint at
    /// the handover height is a valid attestation of the ledger
    /// state regardless of which of the two valid apexes signed it.
    ///
    /// Strict "both-signatures-required-at-handover" is a separate
    /// consumer-side check via
    /// [`SignedCheckpoint::verify_apex_handover`] before the
    /// inclusion call. Layered policies (auditor-grade evidence
    /// that the handover ceremony completed correctly, not just
    /// that one party is willing to sign) belong above this method,
    /// not buried inside it.
    ///
    /// The §4 N+3+ post-handover invariant — "P-old's signature on
    /// checkpoints at heights AFTER the handover MUST be refused" —
    /// is a separate property, covered end-to-end by
    /// [`LedgerConsumer::apply_apex_handover`] + [`LedgerConsumer::consult_capability`] together.
    /// It is verified at the integration-test level
    /// (`full_handover_ceremony_end_to_end`).
    fn apply_witness_record(
        &mut self,
        record: WitnessRecord,
        proof: InclusionProof,
    ) -> Result<(), LedgerError>;
}

/// In-memory [`LedgerConsumer`] for v0.1.x. Single-writer; not
/// thread-safe — matches the kernel-side single-threaded substrate
/// model. Future MINOR may add Mutex wrapping for shared deployments
/// or a `MoonshotDatabaseLedger` impl backed by `moonshot-database`.
pub struct InMemoryLedger {
    pub cache: cache::CheckpointCache,
    pub revocations: revocation::RevocationSet,
    pub apex: apex::ApexHistory,
    /// Hashes of witness records known to be in the current ledger
    /// Merkle root. Per Mechanism A, the kernel verifier honors a
    /// witness extension only if the record's hash appears in the
    /// current root. v0.1.4+: the production path verifies an
    /// [`InclusionProof`] against [`current_checkpoint`] before
    /// inserting; the `#[cfg(test)]`
    /// [`InMemoryLedger::apply_witness_record_unchecked`] shortcut
    /// preserves backward compatibility for tests that don't
    /// construct full proofs.
    witnessed: WitnessedSet,
    /// The current ledger root checkpoint. Set by
    /// [`set_current_checkpoint`] when a normal checkpoint lands;
    /// also set by [`apply_apex_handover`] when a handover lands.
    /// `None` means no checkpoint has been observed yet — calls
    /// to [`apply_witness_record`] return
    /// [`LedgerError::NoCurrentCheckpoint`].
    current_checkpoint: Option<SignedCheckpoint>,
    /// Identity used in the `allowed_signers` lookup for witness
    /// signature verification. v0.1.x: a fixed `"witness"` label;
    /// the actual binding is via the SSH-format pubkey carried on
    /// the [`Capability`].
    witness_identity: String,
}

impl InMemoryLedger {
    pub fn new() -> Self {
        Self {
            cache: cache::CheckpointCache::with_capacity(64),
            revocations: revocation::RevocationSet::new(),
            apex: apex::ApexHistory::new(),
            witnessed: WitnessedSet::new(),
            current_checkpoint: None,
            witness_identity: "witness".to_string(),
        }
    }

    /// Set the current ledger root checkpoint. The consumer calls
    /// this when a new checkpoint arrives via the WORM ledger
    /// stream. Subsequent [`LedgerConsumer::apply_witness_record`] calls verify
    /// inclusion proofs against this checkpoint's Merkle root.
    pub fn set_current_checkpoint(&mut self, checkpoint: SignedCheckpoint) {
        self.cache.insert(checkpoint.clone());
        self.current_checkpoint = Some(checkpoint);
    }

    /// Compute the canonical leaf hash for a witness record per
    /// RFC 9162 §2.1 (SHA-256(0x00 || serialized_bytes)). This is
    /// the value that gets committed to the ledger's Merkle tree
    /// AND used as the lookup key in [`witnessed`].
    fn witness_record_leaf_hash(record: &WitnessRecord) -> Hash256 {
        let mut buf = Vec::new();
        ciborium::into_writer(record, &mut buf).expect("WitnessRecord serializable");
        system_core::rfc9162_leaf_hash(&buf)
    }

    fn is_witness_logged(&self, record: &WitnessRecord) -> bool {
        self.witnessed
            .contains(&Self::witness_record_leaf_hash(record))
    }

    /// Test-only shortcut: insert a witness record into the
    /// `witnessed` set WITHOUT verifying an inclusion proof. Used
    /// by tests that exercise the consult-side decision logic
    /// without constructing full Merkle fixtures. Production code
    /// must use [`LedgerConsumer::apply_witness_record`] which
    /// validates against the current root.
    #[cfg(test)]
    pub fn apply_witness_record_unchecked(&mut self, record: WitnessRecord) {
        let h = Self::witness_record_leaf_hash(&record);
        self.witnessed.insert(h);
    }
}

impl Default for InMemoryLedger {
    fn default() -> Self {
        Self::new()
    }
}

impl LedgerConsumer for InMemoryLedger {
    fn consult_capability(
        &self,
        cap: &Capability,
        current_root: &SignedCheckpoint,
        now: u64,
        witness: Option<&WitnessRecord>,
    ) -> Result<Verdict, ConsultError> {
        // Step 1 — verify current_root is signed by the apex(es)
        // valid at its tree_size.
        let height = current_root.checkpoint.tree_size;
        let apex_verdict = self.apex.check_height(height);
        let apex_ok = match &apex_verdict {
            apex::ApexVerdict::NoApex => {
                return Ok(Verdict::Refuse(RefuseReason::ApexInvalid));
            }
            apex::ApexVerdict::Single { apex } => current_root
                .verify_signer(&apex.name, &apex.pubkey)
                .map_err(|e| ConsultError::InconsistentState(format!("verify_signer: {e:?}")))?,
            apex::ApexVerdict::Handover { old_apex, new_apex } => current_root
                .verify_apex_handover(
                    &old_apex.name,
                    &old_apex.pubkey,
                    &new_apex.name,
                    &new_apex.pubkey,
                )
                .map_err(|e| ConsultError::InconsistentState(format!("verify_handover: {e:?}")))?,
        };
        if !apex_ok {
            return Ok(Verdict::Refuse(RefuseReason::StaleApex));
        }

        // Step 2 — revocation check.
        let cap_hash = cap.hash();
        if self.revocations.contains(&cap_hash) {
            return Ok(Verdict::Refuse(RefuseReason::Revoked));
        }

        // Step 3 — expiry check.
        match cap.expiry_t {
            None => return Ok(Verdict::Allow),
            Some(t) if now < t => return Ok(Verdict::Allow),
            Some(_) => {} // past expiry; fall through to step 4
        }

        // Step 4 — witness extension path.
        let witness = match witness {
            Some(w) => w,
            None => return Ok(Verdict::Refuse(RefuseReason::Expired)),
        };
        let witness_pubkey = match cap.witness_pubkey.as_deref() {
            Some(pk) => pk,
            None => return Ok(Verdict::Refuse(RefuseReason::NotExtensible)),
        };
        // Witness must bind THIS capability.
        if witness.capability_hash != cap_hash {
            return Ok(Verdict::Refuse(RefuseReason::WitnessSignatureInvalid));
        }
        // Witness must extend (not retract) expiry.
        if let Some(prev_expiry) = cap.expiry_t {
            if witness.new_expiry_t <= prev_expiry {
                return Ok(Verdict::Refuse(RefuseReason::WitnessSignatureInvalid));
            }
        }
        // Witness must have been logged in the ledger.
        if !self.is_witness_logged(witness) {
            return Ok(Verdict::Refuse(RefuseReason::WitnessNotInLedger));
        }
        // Verify the SSH signature over (capability_hash || new_expiry_t.to_be_bytes()).
        let mut payload = Vec::with_capacity(40);
        payload.extend_from_slice(&witness.capability_hash);
        payload.extend_from_slice(&witness.new_expiry_t.to_be_bytes());
        let sig_ok = witness::verify_witness_signature(
            &witness.signature,
            &payload,
            witness_pubkey,
            &self.witness_identity,
        )
        .map_err(|e| ConsultError::WitnessVerifyFailed(format!("{e:?}")))?;
        if !sig_ok {
            return Ok(Verdict::Refuse(RefuseReason::WitnessSignatureInvalid));
        }
        Ok(Verdict::ExtendThenAllow {
            new_expiry_t: witness.new_expiry_t,
        })
    }

    fn apply_revocation(&mut self, event: revocation::RevocationEvent) -> Result<(), LedgerError> {
        self.revocations.apply_revocation(event);
        Ok(())
    }

    fn apply_apex_handover(
        &mut self,
        old_apex_name: &str,
        old_apex_pubkey: &[u8; 32],
        new_apex_name: &str,
        new_apex_pubkey: &[u8; 32],
        handover_checkpoint: &SignedCheckpoint,
    ) -> Result<(), LedgerError> {
        // Verify the handover checkpoint carries both signatures.
        let ok = handover_checkpoint
            .verify_apex_handover(
                old_apex_name,
                old_apex_pubkey,
                new_apex_name,
                new_apex_pubkey,
            )
            .map_err(|e| LedgerError::InvalidHandover(format!("verify_apex_handover: {e:?}")))?;
        if !ok {
            return Err(LedgerError::InvalidHandover(
                "handover checkpoint does not carry both apex signatures".to_string(),
            ));
        }
        // Apply to apex history.
        let height = handover_checkpoint.checkpoint.tree_size;
        self.apex
            .apply_handover(old_apex_pubkey, new_apex_name, *new_apex_pubkey, height)
            .map_err(|e| LedgerError::InvalidHandover(format!("apex.apply_handover: {e:?}")))?;
        // Cache + set as current checkpoint.
        self.cache.insert(handover_checkpoint.clone());
        self.current_checkpoint = Some(handover_checkpoint.clone());
        Ok(())
    }

    fn apply_witness_record(
        &mut self,
        record: WitnessRecord,
        proof: InclusionProof,
    ) -> Result<(), LedgerError> {
        // Production path per Master directive 2026-04-27: verify
        // the inclusion proof against the current root before
        // inserting. The unchecked shortcut for tests is the
        // inherent #[cfg(test)] method `apply_witness_record_unchecked`.
        let current = self
            .current_checkpoint
            .as_ref()
            .ok_or(LedgerError::NoCurrentCheckpoint)?;

        // Determine which apex signed the current checkpoint via
        // apex history.
        let height = current.checkpoint.tree_size;
        let apex_verdict = self.apex.check_height(height);
        let leaf_hash = Self::witness_record_leaf_hash(&record);

        match apex_verdict {
            apex::ApexVerdict::NoApex => {
                return Err(LedgerError::NoApexForCheckpoint);
            }
            apex::ApexVerdict::Single { apex } => current
                .verify_inclusion_proof(&proof, &leaf_hash, &apex.name, &apex.pubkey)
                .map_err(LedgerError::WitnessNotInRoot)?,
            apex::ApexVerdict::Handover {
                old_apex,
                new_apex: _,
            } => {
                // At a handover height, both apexes are valid; we
                // verify under the OLD apex (the verify_signer call
                // accepts the first matching signature, so any
                // valid signer works). For a strict policy that
                // requires both to be present, the consumer should
                // confirm via verify_apex_handover separately.
                current
                    .verify_inclusion_proof(&proof, &leaf_hash, &old_apex.name, &old_apex.pubkey)
                    .map_err(LedgerError::WitnessNotInRoot)?;
            }
        }

        self.witnessed.insert(leaf_hash);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use system_core::{
        Capability, CapabilityType, Checkpoint, LedgerAnchor, NoteSignature, Right,
        SignedCheckpoint, WitnessRecord,
    };

    fn keypair(seed: u8) -> (SigningKey, [u8; 32]) {
        let sk = SigningKey::from_bytes(&[seed; 32]);
        let pk = sk.verifying_key().to_bytes();
        (sk, pk)
    }

    fn fixture_anchor() -> LedgerAnchor {
        LedgerAnchor {
            origin: "foundry.test.cap-ledger".to_string(),
            tree_size: 1,
            root_hash: [0xAA; 32],
        }
    }

    fn fixture_capability(expiry_t: Option<u64>) -> Capability {
        Capability {
            cap_type: CapabilityType::Endpoint,
            rights: vec![Right::Invoke],
            expiry_t,
            witness_pubkey: None,
            ledger_anchor: fixture_anchor(),
        }
    }

    fn signed_checkpoint(
        tree_size: u64,
        root_byte: u8,
        signers: &[(&str, &SigningKey)],
    ) -> SignedCheckpoint {
        let cp = Checkpoint {
            origin: "foundry.test.cap-ledger".to_string(),
            tree_size,
            root_hash: [root_byte; 32],
            extensions: vec![],
        };
        let body = cp.body_bytes();
        let signatures = signers
            .iter()
            .map(|(name, sk)| {
                let pk = sk.verifying_key().to_bytes();
                let key_hash = NoteSignature::derive_key_hash(name, &pk);
                let sig = sk.sign(&body).to_bytes();
                NoteSignature {
                    signer_name: name.to_string(),
                    key_hash,
                    signature: sig,
                }
            })
            .collect();
        SignedCheckpoint {
            checkpoint: cp,
            signatures,
        }
    }

    fn ledger_with_genesis(apex_name: &str, sk: &SigningKey) -> InMemoryLedger {
        let mut ledger = InMemoryLedger::new();
        let pk = sk.verifying_key().to_bytes();
        ledger.apex.record_genesis(apex_name, pk, 0).unwrap();
        ledger
    }

    #[test]
    fn no_apex_refuses_with_apex_invalid() {
        let ledger = InMemoryLedger::new();
        let cap = fixture_capability(None);
        let (sk, _pk) = keypair(0x11);
        let root = signed_checkpoint(0, 0xAA, &[("apex", &sk)]);
        let v = ledger.consult_capability(&cap, &root, 1000, None).unwrap();
        assert_eq!(v, Verdict::Refuse(RefuseReason::ApexInvalid));
    }

    #[test]
    fn unsigned_root_refuses_with_stale_apex() {
        let (sk_apex, _pk_apex) = keypair(0x11);
        let (sk_other, _pk_other) = keypair(0x22);
        let ledger = ledger_with_genesis("apex", &sk_apex);
        let cap = fixture_capability(None);
        // Sign root under "apex" name but with the WRONG key — key_hash
        // won't match → verify_signer returns false → StaleApex.
        let root = signed_checkpoint(5, 0xAA, &[("apex", &sk_other)]);
        let v = ledger.consult_capability(&cap, &root, 1000, None).unwrap();
        assert_eq!(v, Verdict::Refuse(RefuseReason::StaleApex));
    }

    #[test]
    fn signed_root_with_no_expiry_allows() {
        let (sk_apex, _pk) = keypair(0x11);
        let ledger = ledger_with_genesis("apex", &sk_apex);
        let cap = fixture_capability(None);
        let root = signed_checkpoint(5, 0xAA, &[("apex", &sk_apex)]);
        let v = ledger.consult_capability(&cap, &root, 1000, None).unwrap();
        assert_eq!(v, Verdict::Allow);
    }

    #[test]
    fn revoked_capability_refuses() {
        let (sk_apex, _pk) = keypair(0x11);
        let mut ledger = ledger_with_genesis("apex", &sk_apex);
        let cap = fixture_capability(None);
        ledger
            .apply_revocation(revocation::RevocationEvent {
                capability_hash: cap.hash(),
                revoked_at: 999,
                signed_by: "apex".to_string(),
                ledger_height: 4,
            })
            .unwrap();
        let root = signed_checkpoint(5, 0xAA, &[("apex", &sk_apex)]);
        let v = ledger.consult_capability(&cap, &root, 1000, None).unwrap();
        assert_eq!(v, Verdict::Refuse(RefuseReason::Revoked));
    }

    #[test]
    fn unexpired_capability_with_expiry_allows() {
        let (sk_apex, _pk) = keypair(0x11);
        let ledger = ledger_with_genesis("apex", &sk_apex);
        let cap = fixture_capability(Some(2000));
        let root = signed_checkpoint(5, 0xAA, &[("apex", &sk_apex)]);
        let v = ledger.consult_capability(&cap, &root, 1000, None).unwrap();
        assert_eq!(v, Verdict::Allow);
    }

    #[test]
    fn expired_capability_no_witness_refuses() {
        let (sk_apex, _pk) = keypair(0x11);
        let ledger = ledger_with_genesis("apex", &sk_apex);
        let cap = fixture_capability(Some(500));
        let root = signed_checkpoint(5, 0xAA, &[("apex", &sk_apex)]);
        let v = ledger.consult_capability(&cap, &root, 1000, None).unwrap();
        assert_eq!(v, Verdict::Refuse(RefuseReason::Expired));
    }

    #[test]
    fn expired_capability_no_witness_pubkey_refuses_not_extensible() {
        let (sk_apex, _pk) = keypair(0x11);
        let ledger = ledger_with_genesis("apex", &sk_apex);
        let mut cap = fixture_capability(Some(500));
        cap.witness_pubkey = None; // explicit
        let witness = WitnessRecord {
            capability_hash: cap.hash(),
            new_expiry_t: 2000,
            signature: vec![0; 100], // doesn't matter; refused before sig check
        };
        let root = signed_checkpoint(5, 0xAA, &[("apex", &sk_apex)]);
        let v = ledger
            .consult_capability(&cap, &root, 1000, Some(&witness))
            .unwrap();
        assert_eq!(v, Verdict::Refuse(RefuseReason::NotExtensible));
    }

    #[test]
    fn witness_with_wrong_cap_hash_refuses() {
        let (sk_apex, _pk) = keypair(0x11);
        let ledger = ledger_with_genesis("apex", &sk_apex);
        let mut cap = fixture_capability(Some(500));
        cap.witness_pubkey = Some("ssh-ed25519 AAAA".to_string());
        let witness = WitnessRecord {
            capability_hash: [0xFF; 32], // doesn't match cap.hash()
            new_expiry_t: 2000,
            signature: vec![],
        };
        let root = signed_checkpoint(5, 0xAA, &[("apex", &sk_apex)]);
        let v = ledger
            .consult_capability(&cap, &root, 1000, Some(&witness))
            .unwrap();
        assert_eq!(v, Verdict::Refuse(RefuseReason::WitnessSignatureInvalid));
    }

    #[test]
    fn witness_not_logged_refuses() {
        let (sk_apex, _pk) = keypair(0x11);
        let ledger = ledger_with_genesis("apex", &sk_apex);
        let mut cap = fixture_capability(Some(500));
        cap.witness_pubkey = Some("ssh-ed25519 AAAA".to_string());
        let witness = WitnessRecord {
            capability_hash: cap.hash(),
            new_expiry_t: 2000,
            signature: vec![],
        };
        let root = signed_checkpoint(5, 0xAA, &[("apex", &sk_apex)]);
        // Witness has NOT been registered via apply_witness_record.
        let v = ledger
            .consult_capability(&cap, &root, 1000, Some(&witness))
            .unwrap();
        assert_eq!(v, Verdict::Refuse(RefuseReason::WitnessNotInLedger));
    }

    #[test]
    fn witness_extending_to_earlier_expiry_refuses() {
        let (sk_apex, _pk) = keypair(0x11);
        let mut ledger = ledger_with_genesis("apex", &sk_apex);
        let mut cap = fixture_capability(Some(1500));
        cap.witness_pubkey = Some("ssh-ed25519 AAAA".to_string());
        let witness = WitnessRecord {
            capability_hash: cap.hash(),
            new_expiry_t: 1000, // EARLIER than current expiry
            signature: vec![],
        };
        ledger.apply_witness_record_unchecked(witness.clone());
        let root = signed_checkpoint(5, 0xAA, &[("apex", &sk_apex)]);
        let v = ledger
            .consult_capability(&cap, &root, 2000, Some(&witness))
            .unwrap();
        assert_eq!(v, Verdict::Refuse(RefuseReason::WitnessSignatureInvalid));
    }

    #[test]
    fn apex_handover_application_succeeds() {
        let (sk_old, pk_old) = keypair(0x11);
        let (_sk_new, pk_new) = keypair(0x22);
        let mut ledger = ledger_with_genesis("apex-old", &sk_old);
        // Sign handover checkpoint at height 100 with both keys.
        let (sk_old_clone, _) = keypair(0x11);
        let (sk_new_again, _) = keypair(0x22);
        let handover = signed_checkpoint(
            100,
            0xCD,
            &[("apex-old", &sk_old_clone), ("apex-new", &sk_new_again)],
        );
        let r = ledger.apply_apex_handover("apex-old", &pk_old, "apex-new", &pk_new, &handover);
        assert!(r.is_ok());
        // Current apex is now apex-new.
        let cur = ledger.apex.current().unwrap();
        assert_eq!(cur.name, "apex-new");
    }

    #[test]
    fn apex_handover_with_one_missing_signature_refused() {
        let (sk_old, pk_old) = keypair(0x11);
        let (_sk_new, pk_new) = keypair(0x22);
        let mut ledger = ledger_with_genesis("apex-old", &sk_old);
        // Handover checkpoint signed ONLY by apex-old (missing apex-new).
        let (sk_old_clone, _) = keypair(0x11);
        let handover = signed_checkpoint(100, 0xCD, &[("apex-old", &sk_old_clone)]);
        let r = ledger.apply_apex_handover("apex-old", &pk_old, "apex-new", &pk_new, &handover);
        assert!(matches!(r, Err(LedgerError::InvalidHandover(_))));
    }

    // ---------- apply_witness_record with InclusionProof tests ----------

    use system_core::{rfc9162_internal_hash, rfc9162_leaf_hash};

    fn build_merkle_root(leaves: &[[u8; 32]]) -> [u8; 32] {
        let mut layer = leaves.to_vec();
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
        layer.into_iter().next().unwrap()
    }

    fn make_inclusion_proof(leaves: &[[u8; 32]], leaf_index: u64) -> InclusionProof {
        let mut path = Vec::new();
        let mut layer = leaves.to_vec();
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
            tree_size: leaves.len() as u64,
            sibling_hashes: path,
        }
    }

    fn witness_leaf_hash(record: &WitnessRecord) -> [u8; 32] {
        let mut buf = Vec::new();
        ciborium::into_writer(record, &mut buf).expect("serializable");
        rfc9162_leaf_hash(&buf)
    }

    #[test]
    fn apply_witness_record_with_no_current_checkpoint_errors() {
        let (sk, _pk) = keypair(0x11);
        let mut ledger = ledger_with_genesis("apex", &sk);
        let witness = WitnessRecord {
            capability_hash: [0xAA; 32],
            new_expiry_t: 2000,
            signature: vec![],
        };
        let proof = InclusionProof {
            leaf_index: 0,
            tree_size: 1,
            sibling_hashes: vec![],
        };
        let r = ledger.apply_witness_record(witness, proof);
        assert_eq!(r, Err(LedgerError::NoCurrentCheckpoint));
    }

    #[test]
    fn apply_witness_record_with_valid_proof_succeeds() {
        let (sk, _pk) = keypair(0x11);
        let mut ledger = ledger_with_genesis("apex", &sk);

        // Build a Merkle tree where leaf 0 is our witness record.
        let witness = WitnessRecord {
            capability_hash: [0xBB; 32],
            new_expiry_t: 5000,
            signature: vec![1, 2, 3],
        };
        let leaf0 = witness_leaf_hash(&witness);
        let other_leaves = [[0x10; 32], [0x20; 32], [0x30; 32]];
        let leaves = [leaf0, other_leaves[0], other_leaves[1], other_leaves[2]];
        let root = build_merkle_root(&leaves);
        let proof = make_inclusion_proof(&leaves, 0);

        // Set the current checkpoint to one signed at this root.
        let cp = signed_checkpoint(50, 0, &[("apex", &sk)]);
        // signed_checkpoint hardcodes root_byte; need a custom one
        // with the actual Merkle root.
        let custom_cp = SignedCheckpoint {
            checkpoint: Checkpoint {
                origin: "foundry.test.cap-ledger".to_string(),
                tree_size: 4,
                root_hash: root,
                extensions: vec![],
            },
            signatures: {
                let pk = sk.verifying_key().to_bytes();
                let body = Checkpoint {
                    origin: "foundry.test.cap-ledger".to_string(),
                    tree_size: 4,
                    root_hash: root,
                    extensions: vec![],
                }
                .body_bytes();
                let key_hash = NoteSignature::derive_key_hash("apex", &pk);
                let sig = sk.sign(&body).to_bytes();
                vec![NoteSignature {
                    signer_name: "apex".to_string(),
                    key_hash,
                    signature: sig,
                }]
            },
        };
        ledger.set_current_checkpoint(custom_cp);
        let _ = cp; // unused for this test

        let r = ledger.apply_witness_record(witness.clone(), proof);
        assert!(r.is_ok(), "valid proof should succeed; got {r:?}");

        // Subsequent is_witness_logged should reflect the insertion.
        // (Use the consult flow indirectly via the witnessed set.)
        assert!(ledger.is_witness_logged(&witness));
    }

    #[test]
    fn apply_witness_record_with_tampered_proof_fails() {
        let (sk, _pk) = keypair(0x11);
        let mut ledger = ledger_with_genesis("apex", &sk);

        let witness = WitnessRecord {
            capability_hash: [0xCC; 32],
            new_expiry_t: 5000,
            signature: vec![],
        };
        let leaf0 = witness_leaf_hash(&witness);
        let leaves = [leaf0, [0x10; 32], [0x20; 32], [0x30; 32]];
        let root = build_merkle_root(&leaves);
        let mut proof = make_inclusion_proof(&leaves, 0);
        proof.sibling_hashes[0] = [0xFF; 32]; // tamper

        let custom_cp = SignedCheckpoint {
            checkpoint: Checkpoint {
                origin: "foundry.test.cap-ledger".to_string(),
                tree_size: 4,
                root_hash: root,
                extensions: vec![],
            },
            signatures: {
                let pk = sk.verifying_key().to_bytes();
                let body = Checkpoint {
                    origin: "foundry.test.cap-ledger".to_string(),
                    tree_size: 4,
                    root_hash: root,
                    extensions: vec![],
                }
                .body_bytes();
                let key_hash = NoteSignature::derive_key_hash("apex", &pk);
                let sig = sk.sign(&body).to_bytes();
                vec![NoteSignature {
                    signer_name: "apex".to_string(),
                    key_hash,
                    signature: sig,
                }]
            },
        };
        ledger.set_current_checkpoint(custom_cp);

        let r = ledger.apply_witness_record(witness.clone(), proof);
        assert!(
            matches!(r, Err(LedgerError::WitnessNotInRoot(_))),
            "tampered proof should fail; got {r:?}"
        );
        // Witness must NOT have been recorded.
        assert!(!ledger.is_witness_logged(&witness));
    }

    #[test]
    fn apply_witness_record_unchecked_still_works_for_tests() {
        // Backward-compat: existing test path.
        let (sk, _pk) = keypair(0x11);
        let mut ledger = ledger_with_genesis("apex", &sk);
        let witness = WitnessRecord {
            capability_hash: [0xDD; 32],
            new_expiry_t: 5000,
            signature: vec![],
        };
        // No current checkpoint, no proof needed for unchecked.
        ledger.apply_witness_record_unchecked(witness.clone());
        assert!(ledger.is_witness_logged(&witness));
    }

    // ---------- Group 2D: ConsultError, LedgerError, handover-height ----------

    #[test]
    fn consult_with_bad_apex_pubkey_returns_inconsistent_state() {
        // Apex stored with a non-curve pubkey ([2, 0, ..] — quadratic non-residue
        // on Ed25519). verify_signer → VerifyError::BadPublicKey → InconsistentState.
        let mut ledger = InMemoryLedger::new();
        let mut bad_pk = [0u8; 32];
        bad_pk[0] = 2; // same non-curve point used in system-core's verify_error_bad_public_key
        ledger.apex.record_genesis("apex", bad_pk, 0).unwrap();
        let (sk_any, _) = keypair(0x77);
        let root = signed_checkpoint(5, 0xAA, &[("apex", &sk_any)]);
        let cap = fixture_capability(None);
        let result = ledger.consult_capability(&cap, &root, 1000, None);
        assert!(
            matches!(result, Err(ConsultError::InconsistentState(_))),
            "expected InconsistentState, got {result:?}"
        );
    }

    #[test]
    fn apply_witness_record_no_apex_returns_no_apex_for_checkpoint() {
        // No genesis → apex history empty → check_height returns NoApex.
        let (sk, _) = keypair(0x11);
        let mut ledger = InMemoryLedger::new(); // deliberately no genesis
        let cp = signed_checkpoint(4, 0xAA, &[("apex", &sk)]);
        ledger.set_current_checkpoint(cp);
        let witness = WitnessRecord {
            capability_hash: [0xAA; 32],
            new_expiry_t: 2000,
            signature: vec![],
        };
        let proof = InclusionProof {
            leaf_index: 0,
            tree_size: 1,
            sibling_hashes: vec![],
        };
        let r = ledger.apply_witness_record(witness, proof);
        assert_eq!(r, Err(LedgerError::NoApexForCheckpoint));
    }

    #[test]
    fn apply_witness_record_at_handover_height_succeeds() {
        // At handover height, apply_witness_record routes through ApexVerdict::Handover
        // and verifies the inclusion proof under old_apex's key.
        let (sk_old, pk_old) = keypair(0x11);
        let (sk_new, pk_new) = keypair(0x22);
        let mut ledger = ledger_with_genesis("apex-old", &sk_old);

        // Build a 2-leaf Merkle tree where leaf 0 is the witness record.
        let witness = WitnessRecord {
            capability_hash: [0xEE; 32],
            new_expiry_t: 9000,
            signature: vec![7, 8, 9],
        };
        let leaf0 = witness_leaf_hash(&witness);
        let leaves = [leaf0, [0x55u8; 32]];
        let root = build_merkle_root(&leaves);
        let proof = make_inclusion_proof(&leaves, 0);

        // Build handover checkpoint at height = tree size (2 leaves).
        // tree_size in checkpoint must match proof.tree_size (both = 2).
        // Signed by BOTH old and new apex (required for apply_apex_handover).
        let cp_body = Checkpoint {
            origin: "foundry.test.cap-ledger".to_string(),
            tree_size: 2, // must match leaves.len() so verify_inclusion_proof passes
            root_hash: root,
            extensions: vec![],
        };
        let body_bytes = cp_body.body_bytes();
        let make_sig = |name: &str, sk: &SigningKey| {
            let pk = sk.verifying_key().to_bytes();
            let key_hash = NoteSignature::derive_key_hash(name, &pk);
            NoteSignature {
                signer_name: name.to_string(),
                key_hash,
                signature: sk.sign(&body_bytes).to_bytes(),
            }
        };
        let handover_cp = SignedCheckpoint {
            checkpoint: cp_body,
            signatures: vec![make_sig("apex-old", &sk_old), make_sig("apex-new", &sk_new)],
        };

        // Apply handover; current_checkpoint is set to handover_cp (height 50).
        ledger
            .apply_apex_handover("apex-old", &pk_old, "apex-new", &pk_new, &handover_cp)
            .unwrap();

        // apply_witness_record at handover height should succeed via old_apex path.
        let r = ledger.apply_witness_record(witness.clone(), proof);
        assert!(
            r.is_ok(),
            "apply_witness_record at handover height should succeed; got {r:?}"
        );
        assert!(ledger.is_witness_logged(&witness));
    }

    #[test]
    fn full_handover_ceremony_end_to_end() {
        // Inbox brief Phase 1A item 4 — END-TO-END ceremony.
        // Synthesize deployment, append revocation entry by P-old,
        // append checkpoint with both P-old + P-new sigs, verify
        // kernel verifier accepts the handover, verify subsequent
        // checkpoints require only P-new (P-old at N+3 REFUSED).
        let (sk_old, pk_old) = keypair(0x11);
        let (sk_new, pk_new) = keypair(0x22);
        let mut ledger = ledger_with_genesis("apex-old", &sk_old);

        // Pre-handover: checkpoint at height 50 signed by P-old works
        // for capability consultation.
        let (sk_old_clone1, _) = keypair(0x11);
        let pre_root = signed_checkpoint(50, 0xAA, &[("apex-old", &sk_old_clone1)]);
        let cap = fixture_capability(None);
        assert_eq!(
            ledger
                .consult_capability(&cap, &pre_root, 100, None)
                .unwrap(),
            Verdict::Allow
        );

        // Append revocation entry by P-old (a separate revocation
        // for some unrelated capability — this just exercises the
        // revocation API as part of the ceremony).
        ledger
            .apply_revocation(revocation::RevocationEvent {
                capability_hash: [0xEE; 32],
                revoked_at: 100,
                signed_by: "apex-old".to_string(),
                ledger_height: 99,
            })
            .unwrap();

        // Apply the handover at height 100.
        let (sk_old_clone2, _) = keypair(0x11);
        let (sk_new_clone, _) = keypair(0x22);
        let handover = signed_checkpoint(
            100,
            0xCD,
            &[("apex-old", &sk_old_clone2), ("apex-new", &sk_new_clone)],
        );
        ledger
            .apply_apex_handover("apex-old", &pk_old, "apex-new", &pk_new, &handover)
            .unwrap();

        // Handover checkpoint (height 100) consults successfully.
        assert_eq!(
            ledger
                .consult_capability(&cap, &handover, 200, None)
                .unwrap(),
            Verdict::Allow
        );

        // Subsequent checkpoint (height 101) signed ONLY by P-new — accepted.
        let post_new = signed_checkpoint(101, 0xDD, &[("apex-new", &sk_new)]);
        assert_eq!(
            ledger
                .consult_capability(&cap, &post_new, 300, None)
                .unwrap(),
            Verdict::Allow
        );

        // Subsequent checkpoint (height 101) signed ONLY by P-old —
        // MUST BE REFUSED. This is the §4 N+3+ invariant.
        let post_old = signed_checkpoint(101, 0xDE, &[("apex-old", &sk_old)]);
        assert_eq!(
            ledger
                .consult_capability(&cap, &post_old, 300, None)
                .unwrap(),
            Verdict::Refuse(RefuseReason::StaleApex)
        );
    }
}
