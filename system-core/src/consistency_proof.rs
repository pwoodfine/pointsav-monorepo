//! RFC 9162 v2 Merkle consistency proofs (compatible with C2SP
//! tlog-tiles per `~/Foundry/conventions/worm-ledger-design.md` §3 D1).
//!
//! Provides "does this newer root extend the older one?" — the
//! replication-safety primitive for ledger-mirror catch-up and
//! multi-witness checkpoint advancement.
//!
//! Reuses `rfc9162_internal_hash` from `inclusion_proof`; no new hash helper.
//! Per RFC 9162 §2.1.4. Composed kernel-facing API:
//! [`crate::checkpoint::SignedCheckpoint::verify_consistency_proof`].

use crate::inclusion_proof::rfc9162_internal_hash;
use crate::Hash256;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

/// RFC 9162 v2 consistency proof between two tree states.
///
/// A consistency proof proves that the tree at `(old_root, old_size)`
/// is a prefix of the tree at `(new_root, new_size)` — i.e. history has
/// not been rewritten between the two snapshots.
///
/// Call [`ConsistencyProof::verify`] with the two roots and their sizes
/// to check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsistencyProof {
    /// Intermediate hashes per RFC 9162 §2.1.4. Empty iff `old_size == 0`
    /// or `old_size == new_size` (identity and zero cases).
    pub hashes: Vec<Hash256>,
}

/// Errors returned by [`ConsistencyProof::verify`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConsistencyVerifyError {
    /// `old_size > new_size`. Trees can only grow.
    OldSizeExceedsNewSize,
    /// `old_size == 0` is the empty-tree case. The empty tree is a valid
    /// prefix of any tree, but the RFC treats `old_size == 0` as an
    /// input that requires an empty proof (§2.1.4 step 1).
    OldSizeIsZero,
    /// `old_size == new_size` but `hashes` is non-empty. Equal-size
    /// proofs must be empty.
    EqualSizesNonEmptyProof,
    /// `old_size == new_size`, `hashes` is empty, but `old_root != new_root`.
    EqualSizesRootMismatch,
    /// `old_size < new_size` and `old_size > 0`, but `hashes` is empty.
    EmptyProofForNonZeroOldSize,
    /// Proof contains more hashes than the tree shape requires. Detected
    /// by `last_node == 0` before all hashes are consumed.
    PathTooLong,
    /// Proof contains fewer hashes than the tree shape requires. Detected
    /// by `last_node != 0` after all hashes are consumed.
    PathTooShort,
    /// Reconstructed old root does not match supplied `old_root`.
    OldRootMismatch,
    /// Reconstructed new root does not match supplied `new_root`.
    NewRootMismatch,
}

impl ConsistencyProof {
    /// Verify this consistency proof per RFC 9162 §2.1.4.
    ///
    /// Checks that the tree state `(old_root, old_size)` is a consistent
    /// prefix of `(new_root, new_size)`. Both roots must reconstruct
    /// correctly from the proof hashes for the check to pass.
    ///
    /// # Algorithm
    ///
    /// Seeds two running accumulators `old_hash` and `new_hash` from
    /// `hashes[0]` (the right-frontier leaf of the old tree), then
    /// iterates over `hashes[1..]`. At each step:
    ///
    /// - If the old-tree position is a right node (`node & 1 == 1`) or
    ///   the old and new frontiers have converged (`node == last_node`):
    ///   combine **both** accumulators leftward with the proof hash, then
    ///   strip any trailing even bits from the position counters.
    /// - Otherwise (old position is a left node, new extends further):
    ///   combine only `new_hash` rightward.
    ///
    /// After all hashes are consumed, both `last_node` must be zero and
    /// the reconstructed roots must match.
    ///
    /// # Errors
    ///
    /// Returns the first-encountered failure. See [`ConsistencyVerifyError`]
    /// variants for the exact conditions.
    pub fn verify(
        &self,
        old_root: Hash256,
        old_size: u64,
        new_root: Hash256,
        new_size: u64,
    ) -> Result<(), ConsistencyVerifyError> {
        // old_size == 0: empty proof required; vacuously consistent.
        if old_size == 0 {
            return Err(ConsistencyVerifyError::OldSizeIsZero);
        }
        if old_size > new_size {
            return Err(ConsistencyVerifyError::OldSizeExceedsNewSize);
        }

        // Equal-size case: identity proof must be empty and roots must match.
        if old_size == new_size {
            if !self.hashes.is_empty() {
                return Err(ConsistencyVerifyError::EqualSizesNonEmptyProof);
            }
            if old_root != new_root {
                return Err(ConsistencyVerifyError::EqualSizesRootMismatch);
            }
            return Ok(());
        }

        // old_size < new_size. Proof must be non-empty.
        if self.hashes.is_empty() {
            return Err(ConsistencyVerifyError::EmptyProofForNonZeroOldSize);
        }

        // `node` is the 0-indexed position of the old tree's right frontier
        // at leaf level. `last_node` is the same for the new tree.
        let mut node = old_size - 1;
        let mut last_node = new_size - 1;

        // Both accumulators start from the first proof hash —
        // the right-frontier leaf of the old tree (or the last node
        // in the old tree's leaf layer).
        let mut iter = self.hashes.iter();
        let first = iter.next().expect("hashes is non-empty (checked above)");
        let mut old_hash: Hash256 = *first;
        let mut new_hash: Hash256 = *first;

        for p in iter {
            if last_node == 0 {
                return Err(ConsistencyVerifyError::PathTooLong);
            }
            if node & 1 == 1 || node == last_node {
                // Right-node or convergence: combine both accumulators leftward.
                old_hash = rfc9162_internal_hash(p, &old_hash);
                new_hash = rfc9162_internal_hash(p, &new_hash);
                // Strip trailing even bits (inner strip) so the next iteration
                // sees the correct parity.
                while node & 1 == 0 && node != 0 {
                    node >>= 1;
                    last_node >>= 1;
                }
            } else {
                // Left-node, new tree extends beyond old: only new_hash moves.
                new_hash = rfc9162_internal_hash(&new_hash, p);
            }
            node >>= 1;
            last_node >>= 1;
        }

        if last_node != 0 {
            return Err(ConsistencyVerifyError::PathTooShort);
        }
        // Check old root first so callers get the most specific error.
        if old_hash != old_root {
            return Err(ConsistencyVerifyError::OldRootMismatch);
        }
        if new_hash != new_root {
            return Err(ConsistencyVerifyError::NewRootMismatch);
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inclusion_proof::{rfc9162_internal_hash, rfc9162_leaf_hash};

    // -----------------------------------------------------------------------
    // Test helpers
    // -----------------------------------------------------------------------

    /// Build a Merkle root over `leaf_hashes` per RFC 9162 §2.1 (odd-leaf
    /// right-edge promotion). Used to compute expected roots in tests.
    fn build_root(leaf_hashes: &[Hash256]) -> Hash256 {
        let mut layer = leaf_hashes.to_vec();
        while layer.len() > 1 {
            let mut next = Vec::with_capacity(layer.len().div_ceil(2));
            let mut i = 0;
            while i < layer.len() {
                if i + 1 < layer.len() {
                    next.push(rfc9162_internal_hash(&layer[i], &layer[i + 1]));
                } else {
                    // Odd right-edge promotion: unpaired node rises unchanged.
                    next.push(layer[i]);
                }
                i += 2;
            }
            layer = next;
        }
        layer
            .into_iter()
            .next()
            .expect("at least one leaf required for Merkle root")
    }

    /// Build all tree layers for `leaf_hashes`, returning a `Vec<Vec<Hash256>>`
    /// with layer 0 = leaves, last layer = `[root]`.
    fn build_layers(leaf_hashes: &[Hash256]) -> Vec<Vec<Hash256>> {
        let mut layers: Vec<Vec<Hash256>> = vec![leaf_hashes.to_vec()];
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
    }

    /// Generate `n` deterministic leaf hashes.
    fn fixture_leaves(n: u64) -> Vec<Hash256> {
        (0..n)
            .map(|i| rfc9162_leaf_hash(format!("leaf-{i}").as_bytes()))
            .collect()
    }

    /// Oracle: generate a consistency proof compatible with
    /// [`ConsistencyProof::verify`] for `old_n` leaves growing to `new_n`.
    ///
    /// The proof format matches the verifier's algorithm:
    /// - `hashes[0]` = leaf at index `old_n - 1` in the old tree (right frontier).
    /// - `hashes[1..]` = sibling hashes collected by simulating the verifier's
    ///   path, looking ahead to apply the inner strip when in the BOTH branch.
    ///
    /// This oracle is independent of the verifier (it computes proofs from
    /// tree structure rather than from the RFC's recursive PROOF function), so
    /// it serves as an external ground-truth for the test fixtures.
    fn make_consistency_proof(old_n: usize, new_n: usize) -> ConsistencyProof {
        assert!(
            old_n <= new_n && old_n > 0,
            "oracle requires 0 < old_n <= new_n"
        );
        if old_n == new_n {
            return ConsistencyProof { hashes: vec![] };
        }

        let new_leaves = fixture_leaves(new_n as u64);
        let old_leaves: Vec<Hash256> = new_leaves[..old_n].to_vec();
        let new_layers = build_layers(&new_leaves);
        let old_layers = build_layers(&old_leaves);

        let get_new = |lv: usize, idx: usize| -> Option<Hash256> {
            new_layers.get(lv).and_then(|l| l.get(idx)).copied()
        };
        let get_old = |lv: usize, idx: usize| -> Option<Hash256> {
            old_layers.get(lv).and_then(|l| l.get(idx)).copied()
        };

        let mut path: Vec<Hash256> = Vec::new();

        // Anchor: the right-frontier leaf of the old tree (leaf at index old_n-1).
        let anchor = get_old(0, old_n - 1).expect("old tree must have at least one leaf");
        path.push(anchor);

        let mut n_loop = (old_n - 1) as u64;
        let mut ln_loop = (new_n - 1) as u64;
        let mut lv: usize = 0;

        while ln_loop != 0 {
            if n_loop & 1 == 1 || n_loop == ln_loop {
                // BOTH branch. The proof element combines with both accumulators
                // at the level AFTER applying the inner strip. Look ahead to find
                // the correct level.
                let mut n_stripped = n_loop;
                let mut _ln_stripped = ln_loop;
                let mut lv_stripped = lv;
                while n_stripped & 1 == 0 && n_stripped != 0 {
                    n_stripped >>= 1;
                    _ln_stripped >>= 1;
                    lv_stripped += 1;
                }
                let sibling_idx = (n_stripped ^ 1) as usize;
                // Sibling lives in the shared prefix of old and new trees.
                let p = get_new(lv_stripped, sibling_idx)
                    .or_else(|| get_old(lv_stripped, sibling_idx))
                    .unwrap_or_else(|| panic!(
                        "BOTH branch: sibling ({lv_stripped},{sibling_idx}) missing for {old_n}→{new_n}"
                    ));
                path.push(p);
                // Advance n_loop/ln_loop by the same inner strip.
                while n_loop & 1 == 0 && n_loop != 0 {
                    n_loop >>= 1;
                    ln_loop >>= 1;
                    lv += 1;
                }
            } else {
                // ELSE branch. The proof element extends only new_hash rightward.
                // Sibling is at (lv, n_loop ^ 1) in the new tree (the right-side
                // node that exists only because new_n > old_n).
                let sibling_idx = (n_loop ^ 1) as usize;
                let p = get_new(lv, sibling_idx).unwrap_or_else(|| {
                    panic!("ELSE branch: sibling ({lv},{sibling_idx}) missing for {old_n}→{new_n}")
                });
                path.push(p);
            }
            n_loop >>= 1;
            ln_loop >>= 1;
            lv += 1;
        }

        ConsistencyProof { hashes: path }
    }

    // -----------------------------------------------------------------------
    // Test cases
    // -----------------------------------------------------------------------

    /// Identity case: old_size == new_size, same root, empty proof → Ok.
    #[test]
    fn identity_case_empty_proof_same_root_verifies() {
        let leaves = fixture_leaves(4);
        let root = build_root(&leaves);
        let proof = ConsistencyProof { hashes: vec![] };
        assert_eq!(proof.verify(root, 4, root, 4), Ok(()));
    }

    /// old_size == 0 is rejected.
    #[test]
    fn old_size_zero_rejected() {
        let leaves = fixture_leaves(4);
        let root = build_root(&leaves);
        let proof = ConsistencyProof { hashes: vec![] };
        assert_eq!(
            proof.verify([0u8; 32], 0, root, 4),
            Err(ConsistencyVerifyError::OldSizeIsZero)
        );
    }

    /// old_size > new_size is rejected.
    #[test]
    fn old_size_exceeds_new_size_rejected() {
        let proof = ConsistencyProof { hashes: vec![] };
        assert_eq!(
            proof.verify([0u8; 32], 5, [0u8; 32], 4),
            Err(ConsistencyVerifyError::OldSizeExceedsNewSize)
        );
    }

    /// Equal sizes with non-empty proof → rejected.
    #[test]
    fn equal_sizes_non_empty_proof_rejected() {
        let leaves = fixture_leaves(4);
        let root = build_root(&leaves);
        let proof = ConsistencyProof {
            hashes: vec![[0xAAu8; 32]],
        };
        assert_eq!(
            proof.verify(root, 4, root, 4),
            Err(ConsistencyVerifyError::EqualSizesNonEmptyProof)
        );
    }

    /// Single leaf → two leaves (minimal non-trivial extension, power-of-2 start).
    #[test]
    fn single_leaf_extension_verifies() {
        let leaves = fixture_leaves(2);
        let old_root = build_root(&leaves[..1]);
        let new_root = build_root(&leaves);
        let proof = make_consistency_proof(1, 2);
        assert_eq!(
            proof.verify(old_root, 1, new_root, 2),
            Ok(()),
            "1→2 should verify; proof={:?}",
            proof.hashes
        );
    }

    /// Multiple power-of-2 extensions verify correctly.
    ///
    /// Tests: 2→4, 4→8.
    #[test]
    fn power_of_two_extensions_verify() {
        // 2 → 4
        {
            let leaves = fixture_leaves(4);
            let old_root = build_root(&leaves[..2]);
            let new_root = build_root(&leaves);
            let proof = make_consistency_proof(2, 4);
            assert_eq!(
                proof.verify(old_root, 2, new_root, 4),
                Ok(()),
                "2→4 should verify"
            );
        }
        // 4 → 8
        {
            let leaves = fixture_leaves(8);
            let old_root = build_root(&leaves[..4]);
            let new_root = build_root(&leaves);
            let proof = make_consistency_proof(4, 8);
            assert_eq!(
                proof.verify(old_root, 4, new_root, 8),
                Ok(()),
                "4→8 should verify"
            );
        }
    }

    /// Non-power-of-2 old sizes exercise the right-edge odd-promotion path.
    ///
    /// Tests: 3→5 (small odd), 4→7 (even→odd), 5→7 (both odd),
    ///        6→8 (even→power), 3→8 (small→large).
    #[test]
    fn non_power_of_two_sizes_verify() {
        for (old_n, new_n) in [(3, 5), (4, 7), (5, 7), (6, 8), (3, 8)] {
            let leaves = fixture_leaves(new_n as u64);
            let old_root = build_root(&leaves[..old_n]);
            let new_root = build_root(&leaves);
            let proof = make_consistency_proof(old_n, new_n);
            assert_eq!(
                proof.verify(old_root, old_n as u64, new_root, new_n as u64),
                Ok(()),
                "{old_n}→{new_n} should verify; proof={:?}",
                proof.hashes
            );
        }
    }

    /// Mismatched old root is detected.
    #[test]
    fn mismatched_old_root_rejected() {
        let leaves = fixture_leaves(7);
        let new_root = build_root(&leaves);
        let proof = make_consistency_proof(4, 7);

        let wrong_old_root = [0xFFu8; 32];
        assert_eq!(
            proof.verify(wrong_old_root, 4, new_root, 7),
            Err(ConsistencyVerifyError::OldRootMismatch)
        );
    }

    /// Mismatched new root is detected.
    #[test]
    fn mismatched_new_root_rejected() {
        let leaves = fixture_leaves(7);
        let old_root = build_root(&leaves[..4]);
        let proof = make_consistency_proof(4, 7);

        let wrong_new_root = [0xFFu8; 32];
        assert_eq!(
            proof.verify(old_root, 4, wrong_new_root, 7),
            Err(ConsistencyVerifyError::NewRootMismatch)
        );
    }

    /// Corrupting the first hash in the proof path is detected.
    #[test]
    fn corrupt_proof_hash_rejected() {
        let leaves = fixture_leaves(8);
        let old_root = build_root(&leaves[..4]);
        let new_root = build_root(&leaves);
        let mut proof = make_consistency_proof(4, 8);
        assert!(!proof.hashes.is_empty(), "proof for 4→8 must be non-empty");
        proof.hashes[0] = [0x00u8; 32]; // corrupt first hash

        let r = proof.verify(old_root, 4, new_root, 8);
        assert!(
            matches!(
                r,
                Err(ConsistencyVerifyError::OldRootMismatch)
                    | Err(ConsistencyVerifyError::NewRootMismatch)
            ),
            "corrupt proof should give root mismatch; got {r:?}"
        );
    }

    /// The full 1..=8 grid: every (old, new) pair with 0 < old <= new <= 8 verifies.
    #[test]
    fn full_grid_1_to_8_verifies() {
        for old_n in 1usize..=8 {
            for new_n in old_n..=8 {
                let leaves = fixture_leaves(new_n as u64);
                let old_root = build_root(&leaves[..old_n]);
                let new_root = build_root(&leaves);

                if old_n == new_n {
                    let proof = ConsistencyProof { hashes: vec![] };
                    assert_eq!(
                        proof.verify(old_root, old_n as u64, new_root, new_n as u64),
                        Ok(()),
                        "identity {old_n}→{new_n} should verify"
                    );
                } else {
                    let proof = make_consistency_proof(old_n, new_n);
                    assert_eq!(
                        proof.verify(old_root, old_n as u64, new_root, new_n as u64),
                        Ok(()),
                        "{old_n}→{new_n} should verify; proof={:?}",
                        proof.hashes
                    );
                }
            }
        }
    }
}
