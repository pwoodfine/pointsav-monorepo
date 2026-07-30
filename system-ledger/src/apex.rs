//! Apex history + post-handover invariant enforcement.
//!
//! Per `~/Foundry/conventions/system-substrate-doctrine.md` §4
//! (apex co-signing ownership transfer):
//!
//! ```text
//!   ledger height N    (previous apex P-old)
//!   ledger height N+1  P-old signs revocation entry
//!   ledger height N+2  both P-old + P-new sign the handover
//!                       checkpoint (multi-signature ceremony)
//!   ledger height N+3+ only P-new's signature is accepted
//! ```
//!
//! Modelling: at the handover height H, both apexes are
//! "simultaneously valid" — the handover checkpoint REQUIRES both
//! signatures. Above H, only the new apex is valid; below H, only
//! the old apex was valid. This module owns the height-to-apex(es)
//! mapping; the actual signature verification (against
//! [`system_core::SignedCheckpoint`]) happens in the
//! [`crate::LedgerConsumer`] impl in `lib.rs`.

#[cfg(not(feature = "std"))]
use alloc::{
    string::{String, ToString},
    vec::Vec,
};

/// One apex identity in the ledger's history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApexEntry {
    pub name: String,
    pub pubkey: [u8; 32],
    /// First ledger height at which this apex's signatures are
    /// accepted. For genesis: 0. For the new apex of a handover:
    /// the handover height (so both old + new are valid at the
    /// handover height itself).
    pub effective_from: u64,
    /// `Some(h)` means signatures from this apex are accepted only
    /// on checkpoints at heights `effective_from..=h`. `None` means
    /// this apex is the current apex (no successor yet).
    pub effective_until: Option<u64>,
}

/// What the verifier finds when asking "who's valid at height H?"
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApexVerdict {
    /// No apex defined for this height. The ledger has no genesis
    /// or `H` is below genesis.
    NoApex,
    /// Exactly one apex valid at this height. Standard verification:
    /// checkpoint must verify under this apex's signature.
    Single { apex: ApexEntry },
    /// Handover height: BOTH apexes valid simultaneously. Checkpoint
    /// MUST carry both signatures (per
    /// [`system_core::SignedCheckpoint::verify_apex_handover`]).
    Handover {
        old_apex: ApexEntry,
        new_apex: ApexEntry,
    },
}

/// Errors when applying apex state changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HandoverError {
    /// `record_genesis` called when the history already has entries.
    GenesisAlreadyRecorded,
    /// `apply_handover` called with an `old_apex_pubkey` that doesn't
    /// match the current apex's pubkey.
    OldApexMismatch,
    /// `apply_handover` called when there is no current apex (no
    /// genesis recorded).
    NoCurrentApex,
    /// `handover_height` precedes the current apex's `effective_from`.
    HandoverHeightBeforeCurrent,
    /// New apex pubkey equals the old apex pubkey — not a handover,
    /// just an alias rename. Refused as a malformed handover.
    NoOpHandover,
}

/// Append-only apex history. Genesis records the first apex; each
/// handover closes the prior apex's `effective_until` to the
/// handover height and appends the new apex with `effective_from =
/// handover_height` (overlap at the handover height itself).
pub struct ApexHistory {
    entries: Vec<ApexEntry>,
}

impl ApexHistory {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Record the first apex (genesis). Errors if any apex already
    /// recorded.
    pub fn record_genesis(
        &mut self,
        name: &str,
        pubkey: [u8; 32],
        from_height: u64,
    ) -> Result<(), HandoverError> {
        if !self.entries.is_empty() {
            return Err(HandoverError::GenesisAlreadyRecorded);
        }
        self.entries.push(ApexEntry {
            name: name.to_string(),
            pubkey,
            effective_from: from_height,
            effective_until: None,
        });
        Ok(())
    }

    /// Apply an apex handover. Verifies the old apex matches the
    /// current; closes its `effective_until` to `handover_height`;
    /// appends the new apex with `effective_from = handover_height`.
    /// Both apexes are valid at `handover_height` itself (the
    /// multi-signature ceremony); only the new apex is valid above.
    pub fn apply_handover(
        &mut self,
        old_apex_pubkey: &[u8; 32],
        new_apex_name: &str,
        new_apex_pubkey: [u8; 32],
        handover_height: u64,
    ) -> Result<(), HandoverError> {
        if old_apex_pubkey == &new_apex_pubkey {
            return Err(HandoverError::NoOpHandover);
        }
        let current_idx = self
            .entries
            .iter()
            .rposition(|e| e.effective_until.is_none())
            .ok_or(HandoverError::NoCurrentApex)?;
        if &self.entries[current_idx].pubkey != old_apex_pubkey {
            return Err(HandoverError::OldApexMismatch);
        }
        if handover_height < self.entries[current_idx].effective_from {
            return Err(HandoverError::HandoverHeightBeforeCurrent);
        }
        // Close current apex.
        self.entries[current_idx].effective_until = Some(handover_height);
        // Append new apex; effective_from = handover_height so both
        // are valid at the handover height itself.
        self.entries.push(ApexEntry {
            name: new_apex_name.to_string(),
            pubkey: new_apex_pubkey,
            effective_from: handover_height,
            effective_until: None,
        });
        Ok(())
    }

    /// The current apex — most recent entry with `effective_until =
    /// None`. `None` if no genesis has been recorded.
    pub fn current(&self) -> Option<&ApexEntry> {
        self.entries
            .iter()
            .rev()
            .find(|e| e.effective_until.is_none())
    }

    /// Who is valid at `height`? Returns [`ApexVerdict::Handover`]
    /// for the handover height itself, [`ApexVerdict::Single`] for
    /// non-handover heights with an apex, [`ApexVerdict::NoApex`]
    /// for heights before genesis.
    pub fn check_height(&self, height: u64) -> ApexVerdict {
        let mut valid: Vec<&ApexEntry> = self
            .entries
            .iter()
            .filter(|e| {
                e.effective_from <= height
                    && match e.effective_until {
                        Some(until) => height <= until,
                        None => true,
                    }
            })
            .collect();

        match valid.len() {
            0 => ApexVerdict::NoApex,
            1 => ApexVerdict::Single {
                apex: valid.remove(0).clone(),
            },
            2 => {
                // Sort by effective_from; the older one (smaller
                // effective_from) is the outgoing apex.
                valid.sort_by_key(|e| e.effective_from);
                let new_apex = valid.remove(1).clone();
                let old_apex = valid.remove(0).clone();
                ApexVerdict::Handover { old_apex, new_apex }
            }
            n => unreachable!(
                "more than 2 apexes valid at height {height} — invariant violation; \
                 apex history has {n} overlapping entries"
            ),
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for ApexHistory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pk(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    #[test]
    fn empty_history_has_no_current_apex() {
        let h = ApexHistory::new();
        assert!(h.is_empty());
        assert_eq!(h.current(), None);
        assert_eq!(h.check_height(0), ApexVerdict::NoApex);
        assert_eq!(h.check_height(100), ApexVerdict::NoApex);
    }

    #[test]
    fn record_genesis_succeeds_once() {
        let mut h = ApexHistory::new();
        let r1 = h.record_genesis("apex-old", pk(0x11), 0);
        assert!(r1.is_ok());
        // Second genesis call refused.
        let r2 = h.record_genesis("apex-other", pk(0x22), 0);
        assert_eq!(r2, Err(HandoverError::GenesisAlreadyRecorded));
        assert_eq!(h.len(), 1);
    }

    #[test]
    fn current_apex_after_genesis_is_genesis_apex() {
        let mut h = ApexHistory::new();
        h.record_genesis("apex-old", pk(0x11), 0).unwrap();
        let cur = h.current().unwrap();
        assert_eq!(cur.name, "apex-old");
        assert_eq!(cur.pubkey, pk(0x11));
        assert_eq!(cur.effective_from, 0);
        assert_eq!(cur.effective_until, None);
    }

    #[test]
    fn check_height_returns_single_after_genesis() {
        let mut h = ApexHistory::new();
        h.record_genesis("apex-old", pk(0x11), 0).unwrap();
        match h.check_height(0) {
            ApexVerdict::Single { apex } => assert_eq!(apex.name, "apex-old"),
            other => panic!("expected Single, got {other:?}"),
        }
        match h.check_height(1000) {
            ApexVerdict::Single { apex } => assert_eq!(apex.name, "apex-old"),
            other => panic!("expected Single, got {other:?}"),
        }
    }

    #[test]
    fn handover_without_genesis_refused() {
        let mut h = ApexHistory::new();
        let r = h.apply_handover(&pk(0x11), "apex-new", pk(0x22), 100);
        assert_eq!(r, Err(HandoverError::NoCurrentApex));
    }

    #[test]
    fn handover_with_wrong_old_pubkey_refused() {
        let mut h = ApexHistory::new();
        h.record_genesis("apex-old", pk(0x11), 0).unwrap();
        let r = h.apply_handover(&pk(0x99), "apex-new", pk(0x22), 100);
        assert_eq!(r, Err(HandoverError::OldApexMismatch));
    }

    #[test]
    fn handover_with_same_pubkey_refused() {
        let mut h = ApexHistory::new();
        h.record_genesis("apex-old", pk(0x11), 0).unwrap();
        let r = h.apply_handover(&pk(0x11), "apex-old-renamed", pk(0x11), 100);
        assert_eq!(r, Err(HandoverError::NoOpHandover));
    }

    #[test]
    fn handover_height_before_current_refused() {
        let mut h = ApexHistory::new();
        h.record_genesis("apex-old", pk(0x11), 50).unwrap();
        // Handover at height 49 — before genesis effective_from.
        let r = h.apply_handover(&pk(0x11), "apex-new", pk(0x22), 49);
        assert_eq!(r, Err(HandoverError::HandoverHeightBeforeCurrent));
    }

    #[test]
    fn full_handover_ceremony_per_inbox_brief_phase_1a_item_4() {
        // Per inbox brief: synthesize deployment, append revocation
        // entry signed by P-old, append checkpoint with both P-old +
        // P-new signatures, verify kernel verifier accepts the
        // handover and subsequent checkpoints require only P-new.
        //
        // This module covers the apex-history half. The signature-
        // verification half lives in lib.rs::LedgerConsumer (#20).
        let mut h = ApexHistory::new();

        // Genesis at height 0: P-old.
        h.record_genesis("apex-old", pk(0x11), 0).unwrap();

        // Heights 0..=99 — single P-old apex (revocation entry at
        // some intermediate height N+1=100 happens here in a
        // higher-level integration test).
        match h.check_height(50) {
            ApexVerdict::Single { apex } => assert_eq!(apex.name, "apex-old"),
            other => panic!("expected Single P-old at height 50, got {other:?}"),
        }

        // Handover at height 100 (the N+2 checkpoint with both sigs).
        h.apply_handover(&pk(0x11), "apex-new", pk(0x22), 100)
            .unwrap();

        // At handover height 100: BOTH apexes valid (handover
        // verdict).
        match h.check_height(100) {
            ApexVerdict::Handover { old_apex, new_apex } => {
                assert_eq!(old_apex.name, "apex-old");
                assert_eq!(new_apex.name, "apex-new");
                assert_eq!(old_apex.pubkey, pk(0x11));
                assert_eq!(new_apex.pubkey, pk(0x22));
            }
            other => panic!("expected Handover at height 100, got {other:?}"),
        }

        // At height 101 (N+3): only P-new accepted.
        match h.check_height(101) {
            ApexVerdict::Single { apex } => {
                assert_eq!(apex.name, "apex-new");
                assert_eq!(apex.pubkey, pk(0x22));
            }
            other => panic!("expected Single P-new at height 101, got {other:?}"),
        }

        // P-old at height 50 still resolves correctly (history
        // remains queryable for prior heights — audit property).
        match h.check_height(50) {
            ApexVerdict::Single { apex } => assert_eq!(apex.name, "apex-old"),
            other => panic!("expected Single P-old at height 50 post-handover, got {other:?}"),
        }

        // The current apex is P-new.
        let cur = h.current().unwrap();
        assert_eq!(cur.name, "apex-new");
    }

    #[test]
    fn second_handover_after_first() {
        // Sanity: chained handovers work. P-old → P-mid → P-new.
        let mut h = ApexHistory::new();
        h.record_genesis("apex-old", pk(0x11), 0).unwrap();
        h.apply_handover(&pk(0x11), "apex-mid", pk(0x22), 100)
            .unwrap();
        h.apply_handover(&pk(0x22), "apex-new", pk(0x33), 200)
            .unwrap();

        // At height 100: handover P-old↔P-mid.
        match h.check_height(100) {
            ApexVerdict::Handover { old_apex, new_apex } => {
                assert_eq!(old_apex.name, "apex-old");
                assert_eq!(new_apex.name, "apex-mid");
            }
            other => panic!("expected first-handover verdict, got {other:?}"),
        }

        // At height 150: P-mid alone.
        match h.check_height(150) {
            ApexVerdict::Single { apex } => assert_eq!(apex.name, "apex-mid"),
            other => panic!("expected Single P-mid at 150, got {other:?}"),
        }

        // At height 200: handover P-mid↔P-new.
        match h.check_height(200) {
            ApexVerdict::Handover { old_apex, new_apex } => {
                assert_eq!(old_apex.name, "apex-mid");
                assert_eq!(new_apex.name, "apex-new");
            }
            other => panic!("expected second-handover verdict, got {other:?}"),
        }

        // At height 201: P-new alone.
        match h.check_height(201) {
            ApexVerdict::Single { apex } => assert_eq!(apex.name, "apex-new"),
            other => panic!("expected Single P-new at 201, got {other:?}"),
        }

        // Audit: P-old still resolves at height 50.
        match h.check_height(50) {
            ApexVerdict::Single { apex } => assert_eq!(apex.name, "apex-old"),
            other => panic!("expected Single P-old at 50, got {other:?}"),
        }
    }
}
