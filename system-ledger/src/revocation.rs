//! Revoked-capability set. O(1) membership via `HashSet`; audit
//! detail in a sidecar `HashMap`.
//!
//! Per `~/Foundry/conventions/system-substrate-doctrine.md` §3.1:
//! "Before the kernel honors any capability invocation, it consults
//! the ledger for: current revocation status of the invoking
//! capability." This module IS the consultation surface.
//!
//! `apply_revocation` is idempotent — replaying an already-recorded
//! revocation is a no-op (returns `false`). Replay tolerance is
//! deliberate: ledger streams may re-deliver entries during
//! recovery / replication.

use system_core::Hash256;

#[cfg(not(feature = "std"))]
use alloc::{
    collections::{BTreeMap as HashMap, BTreeSet as HashSet},
    string::String,
};
#[cfg(feature = "std")]
use std::collections::{HashMap, HashSet};

/// One revocation entry — the audit detail behind a "this capability
/// is no longer honored" decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevocationEvent {
    pub capability_hash: Hash256,
    /// Unix seconds (UTC) when the revocation was recorded in the
    /// ledger.
    pub revoked_at: u64,
    /// Apex (or delegated witness) that signed the revocation.
    pub signed_by: String,
    /// Ledger height at which the revocation entry was anchored.
    pub ledger_height: u64,
}

/// Revoked-capability set. O(1) membership via `HashSet`; audit
/// detail in a sidecar `HashMap` keyed by `capability_hash`.
pub struct RevocationSet {
    revoked: HashSet<Hash256>,
    detail: HashMap<Hash256, RevocationEvent>,
}

impl RevocationSet {
    pub fn new() -> Self {
        Self {
            revoked: HashSet::new(),
            detail: HashMap::new(),
        }
    }

    /// Record a revocation. Returns `true` if this is a new
    /// revocation; `false` if the capability was already in the
    /// set (idempotent replay).
    pub fn apply_revocation(&mut self, event: RevocationEvent) -> bool {
        let cap_hash = event.capability_hash;
        let inserted = self.revoked.insert(cap_hash);
        if inserted {
            self.detail.insert(cap_hash, event);
        }
        inserted
    }

    /// O(1) check: is this capability revoked?
    pub fn contains(&self, capability_hash: &Hash256) -> bool {
        self.revoked.contains(capability_hash)
    }

    /// Audit accessor: retrieve the revocation event behind a
    /// capability_hash. Returns `None` if not revoked.
    pub fn detail(&self, capability_hash: &Hash256) -> Option<&RevocationEvent> {
        self.detail.get(capability_hash)
    }

    pub fn len(&self) -> usize {
        self.revoked.len()
    }

    pub fn is_empty(&self) -> bool {
        self.revoked.is_empty()
    }
}

impl Default for RevocationSet {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_event(hash_byte: u8) -> RevocationEvent {
        RevocationEvent {
            capability_hash: [hash_byte; 32],
            revoked_at: 1_730_000_000,
            signed_by: "apex-test".to_string(),
            ledger_height: 42,
        }
    }

    #[test]
    fn empty_set_contains_nothing() {
        let set = RevocationSet::new();
        assert!(set.is_empty());
        assert!(!set.contains(&[0; 32]));
        assert_eq!(set.detail(&[0; 32]), None);
    }

    #[test]
    fn apply_revocation_inserts() {
        let mut set = RevocationSet::new();
        let event = fixture_event(0xAA);
        let new = set.apply_revocation(event.clone());
        assert!(new);
        assert!(set.contains(&[0xAA; 32]));
        assert_eq!(set.detail(&[0xAA; 32]), Some(&event));
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn apply_revocation_is_idempotent() {
        let mut set = RevocationSet::new();
        let event = fixture_event(0xBB);
        let first = set.apply_revocation(event.clone());
        let second = set.apply_revocation(event.clone());
        assert!(first);
        assert!(!second); // replay returns false
        assert_eq!(set.len(), 1); // still just one entry
    }

    #[test]
    fn replay_does_not_overwrite_audit_detail() {
        let mut set = RevocationSet::new();
        let original = fixture_event(0xCC);
        set.apply_revocation(original.clone());
        // Try to overwrite with a different "revoked_at" — replay
        // is a no-op, so the original audit detail wins.
        let replay = RevocationEvent {
            capability_hash: [0xCC; 32],
            revoked_at: 2_000_000_000, // different
            signed_by: "apex-impostor".to_string(),
            ledger_height: 999,
        };
        set.apply_revocation(replay);
        assert_eq!(set.detail(&[0xCC; 32]), Some(&original));
    }

    #[test]
    fn distinct_capabilities_coexist() {
        let mut set = RevocationSet::new();
        set.apply_revocation(fixture_event(0x01));
        set.apply_revocation(fixture_event(0x02));
        set.apply_revocation(fixture_event(0x03));
        assert_eq!(set.len(), 3);
        assert!(set.contains(&[0x01; 32]));
        assert!(set.contains(&[0x02; 32]));
        assert!(set.contains(&[0x03; 32]));
        assert!(!set.contains(&[0x04; 32]));
    }
}
