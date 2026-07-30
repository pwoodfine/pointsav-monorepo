//! Recent-N checkpoint cache. Lookup by `(origin, tree_size)` and
//! `(origin, root_hash)`. LRU eviction. Single-writer; not
//! thread-safe — matches the kernel-side single-threaded substrate
//! model.
//!
//! Per `~/Foundry/conventions/system-substrate-doctrine.md` §3.1:
//! "The kernel maintains a current-checkpoint cache and refuses
//! invocations whose capability is not present in the cache. Cache
//! misses fall through to a userland verifier."
//!
//! This module owns the cache-side. The userland-fallback verifier
//! is the consumer's responsibility; this cache is a fast positive
//! oracle, not the source of truth.

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
use system_core::{Hash256, SignedCheckpoint};

/// Bounded checkpoint cache with LRU eviction. Order: most-recent
/// at the back; oldest at the front. Eviction pops the front when
/// at `capacity`.
pub struct CheckpointCache {
    capacity: usize,
    /// Insertion-order log; most-recent at the back.
    entries: Vec<SignedCheckpoint>,
}

impl CheckpointCache {
    /// Create a cache holding up to `capacity` entries. `capacity`
    /// of 0 is allowed (always-miss); useful for tests.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            capacity,
            entries: Vec::with_capacity(capacity),
        }
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Insert a checkpoint. If `capacity` is full, evicts the
    /// oldest entry first. Duplicates (same `origin` + `tree_size`)
    /// are NOT deduplicated — the consumer is expected to dedupe
    /// upstream; the cache is a positive-only oracle.
    pub fn insert(&mut self, checkpoint: SignedCheckpoint) {
        if self.capacity == 0 {
            return;
        }
        if self.entries.len() == self.capacity {
            self.entries.remove(0);
        }
        self.entries.push(checkpoint);
    }

    /// Lookup by `(origin, tree_size)`. Returns the most-recent
    /// matching entry. O(N) over the cache; N is small (default 64).
    pub fn lookup_by_tree_size(&self, origin: &str, tree_size: u64) -> Option<&SignedCheckpoint> {
        self.entries
            .iter()
            .rev()
            .find(|c| c.checkpoint.origin == origin && c.checkpoint.tree_size == tree_size)
    }

    /// Lookup by `(origin, root_hash)`. Returns the most-recent
    /// matching entry. Useful when the consumer has the Merkle root
    /// but not the height. O(N) over the cache.
    pub fn lookup_by_root_hash(
        &self,
        origin: &str,
        root_hash: &Hash256,
    ) -> Option<&SignedCheckpoint> {
        self.entries
            .iter()
            .rev()
            .find(|c| c.checkpoint.origin == origin && &c.checkpoint.root_hash == root_hash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use system_core::{Checkpoint, NoteSignature, SignedCheckpoint};

    fn fixture_checkpoint(origin: &str, tree_size: u64, root_byte: u8) -> SignedCheckpoint {
        SignedCheckpoint {
            checkpoint: Checkpoint {
                origin: origin.to_string(),
                tree_size,
                root_hash: [root_byte; 32],
                extensions: vec![],
            },
            signatures: vec![NoteSignature {
                signer_name: "test".to_string(),
                key_hash: [0; 4],
                signature: [0; 64],
            }],
        }
    }

    #[test]
    fn empty_cache_returns_none() {
        let cache = CheckpointCache::with_capacity(8);
        assert!(cache.is_empty());
        assert_eq!(cache.lookup_by_tree_size("foundry.test", 1), None);
        assert_eq!(cache.lookup_by_root_hash("foundry.test", &[0; 32]), None);
    }

    #[test]
    fn insert_and_lookup_by_tree_size() {
        let mut cache = CheckpointCache::with_capacity(8);
        let cp = fixture_checkpoint("foundry.test", 42, 0xAB);
        cache.insert(cp.clone());
        assert_eq!(cache.lookup_by_tree_size("foundry.test", 42), Some(&cp));
        assert_eq!(cache.lookup_by_tree_size("foundry.test", 99), None);
        assert_eq!(cache.lookup_by_tree_size("foundry.other", 42), None);
    }

    #[test]
    fn insert_and_lookup_by_root_hash() {
        let mut cache = CheckpointCache::with_capacity(8);
        let cp = fixture_checkpoint("foundry.test", 42, 0xCD);
        cache.insert(cp.clone());
        assert_eq!(
            cache.lookup_by_root_hash("foundry.test", &[0xCD; 32]),
            Some(&cp)
        );
        assert_eq!(cache.lookup_by_root_hash("foundry.test", &[0x00; 32]), None);
    }

    #[test]
    fn lru_evicts_oldest_when_full() {
        let mut cache = CheckpointCache::with_capacity(2);
        let cp1 = fixture_checkpoint("foundry.test", 1, 0x01);
        let cp2 = fixture_checkpoint("foundry.test", 2, 0x02);
        let cp3 = fixture_checkpoint("foundry.test", 3, 0x03);
        cache.insert(cp1);
        cache.insert(cp2.clone());
        assert_eq!(cache.len(), 2);
        cache.insert(cp3.clone());
        // Capacity 2: cp1 evicted; cp2, cp3 remain.
        assert_eq!(cache.len(), 2);
        assert_eq!(cache.lookup_by_tree_size("foundry.test", 1), None);
        assert_eq!(cache.lookup_by_tree_size("foundry.test", 2), Some(&cp2));
        assert_eq!(cache.lookup_by_tree_size("foundry.test", 3), Some(&cp3));
    }

    #[test]
    fn zero_capacity_always_misses() {
        let mut cache = CheckpointCache::with_capacity(0);
        let cp = fixture_checkpoint("foundry.test", 1, 0xFF);
        cache.insert(cp);
        assert!(cache.is_empty());
        assert_eq!(cache.lookup_by_tree_size("foundry.test", 1), None);
    }

    #[test]
    fn lookup_returns_most_recent_on_duplicate_tree_size() {
        let mut cache = CheckpointCache::with_capacity(8);
        let cp_old = fixture_checkpoint("foundry.test", 1, 0x01);
        let cp_new = fixture_checkpoint("foundry.test", 1, 0x02);
        cache.insert(cp_old);
        cache.insert(cp_new.clone());
        // The cache is positive-only; consumer dedupes upstream;
        // here we just confirm lookup returns the most-recent insert.
        assert_eq!(cache.lookup_by_tree_size("foundry.test", 1), Some(&cp_new));
    }

    #[test]
    fn capacity_accessor_reflects_construction() {
        let cache = CheckpointCache::with_capacity(64);
        assert_eq!(cache.capacity(), 64);
        assert_eq!(cache.len(), 0);
    }
}
