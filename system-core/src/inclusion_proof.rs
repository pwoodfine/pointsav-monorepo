//! RFC 9162 v2 Merkle inclusion proofs (compatible with C2SP
//! tlog-tiles per `~/Foundry/conventions/worm-ledger-design.md`
//! §3 D1).
//!
//! Provides the cryptographic substrate for "is this record's hash
//! in the current Merkle root?" — the read-side verification half
//! of Mechanism A (Time-Bound Capabilities) and the bridge between
//! the WORM ledger primitive and the Capability Ledger Substrate.
//!
//! # Domain-separation tags
//!
//! Per RFC 9162 §2.1:
//! - Leaf hash: SHA-256(0x00 || leaf_data)
//! - Internal node hash: SHA-256(0x01 || left || right)
//!
//! [`rfc9162_leaf_hash`] provides the leaf-side helper; internal
//! hashing happens inside [`InclusionProof::verify`].
//!
//! # Verification algorithm
//!
//! Per RFC 9162 §2.1.3 verbatim. The kernel-facing API is
//! [`crate::SignedCheckpoint::verify_inclusion_proof`] (composed primitive
//! with signature verification); the raw `InclusionProof::verify`
//! is exposed as the building block but Master directive 2026-04-27
//! says treat the composition as load-bearing.

use sha2::{Digest, Sha256};

use crate::Hash256;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

/// SHA-256 of `(0x00 || leaf_data)` per RFC 9162 §2.1. Use this when
/// computing the leaf hash that gets committed to a transparency
/// log; the same value is what [`InclusionProof::verify`] expects
/// as `leaf_hash`.
pub fn rfc9162_leaf_hash(leaf_data: &[u8]) -> Hash256 {
    let mut hasher = Sha256::new();
    hasher.update([0x00]);
    hasher.update(leaf_data);
    hasher.finalize().into()
}

/// SHA-256 of `(0x01 || left || right)` per RFC 9162 §2.1. Internal
/// node hash. Exposed for completeness; not normally called outside
/// the verifier.
pub fn rfc9162_internal_hash(left: &Hash256, right: &Hash256) -> Hash256 {
    let mut hasher = Sha256::new();
    hasher.update([0x01]);
    hasher.update(left);
    hasher.update(right);
    hasher.finalize().into()
}

/// RFC 9162 v2 inclusion proof for a single leaf.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InclusionProof {
    /// 0-indexed position of the proven leaf in the tree.
    pub leaf_index: u64,
    /// Number of leaves in the tree at the time the proof was
    /// generated.
    pub tree_size: u64,
    /// Sequence of sibling hashes from leaf-side up to the root,
    /// in the order RFC 9162 §2.1.3 expects.
    pub sibling_hashes: Vec<Hash256>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InclusionVerifyError {
    /// `leaf_index >= tree_size`.
    LeafIndexOutOfBounds,
    /// Proof has more sibling hashes than the tree shape needs.
    PathTooLong,
    /// Proof has fewer sibling hashes than the tree shape needs.
    PathTooShort,
    /// Final reconstructed root does not match `expected_root`.
    RootMismatch,
}

impl InclusionProof {
    /// Verify this proof: starting from `leaf_hash` at
    /// `self.leaf_index` in a tree of size `self.tree_size`, walk up
    /// applying the sibling hashes; the final value must equal
    /// `expected_root`. Per RFC 9162 §2.1.3 verbatim.
    pub fn verify(
        &self,
        leaf_hash: &Hash256,
        expected_root: &Hash256,
    ) -> Result<(), InclusionVerifyError> {
        if self.leaf_index >= self.tree_size {
            return Err(InclusionVerifyError::LeafIndexOutOfBounds);
        }

        let mut fn_ = self.leaf_index;
        let mut sn = self.tree_size - 1;
        let mut r = *leaf_hash;

        for p in &self.sibling_hashes {
            if sn == 0 {
                return Err(InclusionVerifyError::PathTooLong);
            }
            if fn_ & 1 == 1 || fn_ == sn {
                r = rfc9162_internal_hash(p, &r);
                while fn_ & 1 == 0 {
                    fn_ >>= 1;
                    sn >>= 1;
                }
            } else {
                r = rfc9162_internal_hash(&r, p);
            }
            fn_ >>= 1;
            sn >>= 1;
        }

        if sn != 0 {
            return Err(InclusionVerifyError::PathTooShort);
        }
        if r != *expected_root {
            return Err(InclusionVerifyError::RootMismatch);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a Merkle root over `leaves` per RFC 9162 §2.1. Returns
    /// the root hash. Used by tests that need to construct fixtures.
    fn build_root(leaf_hashes: &[Hash256]) -> Hash256 {
        let mut layer = leaf_hashes.to_vec();
        while layer.len() > 1 {
            let mut next = Vec::with_capacity(layer.len().div_ceil(2));
            let mut i = 0;
            while i < layer.len() {
                if i + 1 < layer.len() {
                    next.push(rfc9162_internal_hash(&layer[i], &layer[i + 1]));
                } else {
                    // Odd-leaf right-edge promotion per RFC 9162 §2.1.
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

    /// Generate an inclusion proof for `leaf_index` in the tree
    /// formed over `leaf_hashes`. Used by tests as an oracle.
    fn make_proof_clean(leaf_hashes: &[Hash256], leaf_index: u64) -> InclusionProof {
        let mut path = Vec::new();
        let mut layer = leaf_hashes.to_vec();
        let mut idx = leaf_index as usize;
        while layer.len() > 1 {
            let sibling_idx = idx ^ 1;
            if sibling_idx < layer.len() {
                path.push(layer[sibling_idx]);
            }
            // Build next layer.
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

    fn fixture_leaves(n: u64) -> Vec<Hash256> {
        (0..n)
            .map(|i| rfc9162_leaf_hash(format!("leaf-{i}").as_bytes()))
            .collect()
    }

    #[test]
    fn rfc9162_leaf_hash_includes_zero_prefix() {
        // Sanity: leaf hash is NOT plain SHA-256.
        let plain = {
            let mut h = Sha256::new();
            h.update(b"data");
            let bytes: [u8; 32] = h.finalize().into();
            bytes
        };
        let prefixed = rfc9162_leaf_hash(b"data");
        assert_ne!(plain, prefixed);
    }

    #[test]
    fn rfc9162_internal_hash_includes_one_prefix() {
        let h0 = [0u8; 32];
        let plain = {
            let mut h = Sha256::new();
            h.update(h0);
            h.update(h0);
            let bytes: [u8; 32] = h.finalize().into();
            bytes
        };
        let prefixed = rfc9162_internal_hash(&h0, &h0);
        assert_ne!(plain, prefixed);
    }

    #[test]
    fn single_leaf_tree_proof_is_empty() {
        let leaves = fixture_leaves(1);
        let root = build_root(&leaves);
        // For a single-leaf tree, root = leaf and proof is empty.
        assert_eq!(root, leaves[0]);
        let proof = make_proof_clean(&leaves, 0);
        assert!(proof.sibling_hashes.is_empty());
        assert!(proof.verify(&leaves[0], &root).is_ok());
    }

    #[test]
    fn two_leaf_tree_proofs_verify() {
        let leaves = fixture_leaves(2);
        let root = build_root(&leaves);
        for i in 0..2u64 {
            let proof = make_proof_clean(&leaves, i);
            assert!(
                proof.verify(&leaves[i as usize], &root).is_ok(),
                "leaf {i} should verify in tree of size 2"
            );
        }
    }

    #[test]
    fn four_leaf_tree_proofs_verify() {
        let leaves = fixture_leaves(4);
        let root = build_root(&leaves);
        for i in 0..4u64 {
            let proof = make_proof_clean(&leaves, i);
            assert!(
                proof.verify(&leaves[i as usize], &root).is_ok(),
                "leaf {i} of 4 should verify"
            );
        }
    }

    #[test]
    fn eight_leaf_tree_proofs_verify() {
        let leaves = fixture_leaves(8);
        let root = build_root(&leaves);
        for i in 0..8u64 {
            let proof = make_proof_clean(&leaves, i);
            assert!(
                proof.verify(&leaves[i as usize], &root).is_ok(),
                "leaf {i} of 8 should verify"
            );
        }
    }

    #[test]
    fn odd_leaf_tree_proofs_verify() {
        // Tree of 5 leaves — exercises the right-edge odd-promotion
        // path in RFC 9162 §2.1.
        let leaves = fixture_leaves(5);
        let root = build_root(&leaves);
        for i in 0..5u64 {
            let proof = make_proof_clean(&leaves, i);
            assert!(
                proof.verify(&leaves[i as usize], &root).is_ok(),
                "leaf {i} of 5 should verify (odd tree)"
            );
        }
    }

    #[test]
    fn tampered_sibling_fails() {
        let leaves = fixture_leaves(4);
        let root = build_root(&leaves);
        let mut proof = make_proof_clean(&leaves, 1);
        // Corrupt the first sibling.
        proof.sibling_hashes[0] = [0xFF; 32];
        let r = proof.verify(&leaves[1], &root);
        assert_eq!(r, Err(InclusionVerifyError::RootMismatch));
    }

    #[test]
    fn wrong_leaf_hash_fails() {
        let leaves = fixture_leaves(4);
        let root = build_root(&leaves);
        let proof = make_proof_clean(&leaves, 1);
        let r = proof.verify(&[0xCC; 32], &root);
        assert_eq!(r, Err(InclusionVerifyError::RootMismatch));
    }

    #[test]
    fn wrong_root_fails() {
        let leaves = fixture_leaves(4);
        let proof = make_proof_clean(&leaves, 1);
        let r = proof.verify(&leaves[1], &[0xDD; 32]);
        assert_eq!(r, Err(InclusionVerifyError::RootMismatch));
    }

    #[test]
    fn leaf_index_out_of_bounds_fails() {
        let leaves = fixture_leaves(4);
        let root = build_root(&leaves);
        let proof = InclusionProof {
            leaf_index: 4, // == tree_size — invalid
            tree_size: 4,
            sibling_hashes: vec![],
        };
        let r = proof.verify(&leaves[0], &root);
        assert_eq!(r, Err(InclusionVerifyError::LeafIndexOutOfBounds));
    }

    #[test]
    fn path_too_long_fails() {
        let leaves = fixture_leaves(4);
        let root = build_root(&leaves);
        let mut proof = make_proof_clean(&leaves, 1);
        proof.sibling_hashes.push([0xAA; 32]); // extra
        let r = proof.verify(&leaves[1], &root);
        assert_eq!(r, Err(InclusionVerifyError::PathTooLong));
    }

    #[test]
    fn path_too_short_fails() {
        let leaves = fixture_leaves(4);
        let root = build_root(&leaves);
        let mut proof = make_proof_clean(&leaves, 1);
        proof.sibling_hashes.pop(); // remove last
        let r = proof.verify(&leaves[1], &root);
        assert_eq!(r, Err(InclusionVerifyError::PathTooShort));
    }

    #[test]
    fn proof_does_not_verify_for_other_leaf() {
        // Proof for leaf 1 must not verify if you swap leaf hash to
        // leaf 2's value, even though both leaves are in the tree.
        let leaves = fixture_leaves(4);
        let root = build_root(&leaves);
        let proof = make_proof_clean(&leaves, 1);
        let r = proof.verify(&leaves[2], &root);
        assert_eq!(r, Err(InclusionVerifyError::RootMismatch));
    }
}
