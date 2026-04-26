// SPDX-License-Identifier: Apache-2.0 OR MIT

//! WORM Ledger Layer 2 (per
//! `~/Foundry/conventions/worm-ledger-design.md`).
//!
//! L2 in the four-layer stack: the target-independent Rust trait
//! that the wire layer (L3, in `http.rs`) and the storage layer
//! (L1, per-backend in this file or sibling modules) compose
//! against. The trait is the durable contract that survives
//! changes above it (axum vs. MCP-over-IPC) and below it
//! (in-memory vs. POSIX hash-chain log vs. moonshot-database).
//!
//! Trait surface as ratified in worm-ledger-design.md §2 (this
//! file's current shape; full surface grows incrementally per
//! the §5 implementation roadmap):
//!
//! ```text
//!   open(path, module_id, [signing_key])  // per-impl inherent ctor
//!   append(payload_id, payload)           // L2 step 1, this trait
//!   read_since(cursor)                    // L2 step 1, this trait
//!   root()                                // diagnostic
//!   checkpoint()                          // L1 step 2, this trait
//!   verify_inclusion(entry, checkpoint)   // L1 step 2, this trait
//!   verify_consistency(c1, c2)            // L1 step 2, this trait
//! ```
//!
//! This file lands the L1-step-2 additions: `checkpoint`,
//! `verify_inclusion`, `verify_consistency` — implemented over a
//! linear SHA-256 hash chain (each entry's hash chains in the
//! prior entry's hash, providing structural tamper-evidence). The
//! linear-chain implementation is the v0.1.x baseline; a
//! Merkle-tree upgrade (logarithmic inclusion proofs) is a
//! follow-up refinement that keeps this trait surface unchanged.
//!
//! Step 3 (signed-note `checkpoint()`) wires Ed25519 signing into
//! the existing `Checkpoint::signature` field (today: always
//! `None`).

use std::sync::Mutex;

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use ed25519_dalek::SigningKey;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Domain separator for the very first entry's `prev_hash`. This
/// pins the chain origin and prevents cross-ledger collision
/// attacks (a chain rooted at this constant cannot be replayed in
/// another ledger context). The version suffix lets us migrate the
/// chain rule in the future without breaking historical
/// verification.
const CHAIN_ORIGIN: &[u8] = b"service-fs:linear-chain:v1";

#[derive(Debug)]
pub enum LedgerError {
    Io(std::io::Error),
    Serde(serde_json::Error),
    /// Entry with the given cursor was not found in the ledger
    /// (verify_inclusion).
    EntryNotFound(u64),
    /// On reload, the recomputed chain hash at the named cursor
    /// did not match the value stored on disk — tamper detected.
    ChainTampered {
        cursor: u64,
        expected: String,
        got: String,
    },
    /// verify_consistency: the two checkpoints are not in an
    /// append-only relationship (e.g., c2.tree_size < c1.tree_size,
    /// or recomputing forward from c1 doesn't reach c2.root_hash).
    InconsistentCheckpoints { reason: String },
    /// Signing key file is missing, wrong length, or not a valid
    /// Ed25519 compressed point (for verifying keys).
    InvalidKey(String),
    /// Ed25519 signing or signature encoding failed.
    SigningError(String),
}

impl std::fmt::Display for LedgerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LedgerError::Io(e) => write!(f, "ledger I/O error: {e}"),
            LedgerError::Serde(e) => write!(f, "ledger serde error: {e}"),
            LedgerError::EntryNotFound(c) => {
                write!(f, "entry not found at cursor {c}")
            }
            LedgerError::ChainTampered { cursor, expected, got } => {
                write!(
                    f,
                    "chain tampered at cursor {cursor}: expected {expected}, got {got}"
                )
            }
            LedgerError::InconsistentCheckpoints { reason } => {
                write!(f, "inconsistent checkpoints: {reason}")
            }
            LedgerError::InvalidKey(msg) => write!(f, "invalid key: {msg}"),
            LedgerError::SigningError(msg) => write!(f, "signing error: {msg}"),
        }
    }
}

impl std::error::Error for LedgerError {}

impl From<std::io::Error> for LedgerError {
    fn from(e: std::io::Error) -> Self {
        LedgerError::Io(e)
    }
}

impl From<serde_json::Error> for LedgerError {
    fn from(e: serde_json::Error) -> Self {
        LedgerError::Serde(e)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entry {
    pub cursor: u64,
    pub payload_id: String,
    pub payload: serde_json::Value,
    /// Hex-encoded SHA-256 of `prev_hash || cursor || payload_id ||
    /// payload_canonical_bytes`. Each entry's `prev_hash` is the
    /// previous entry's `this_hash`; the first entry's `prev_hash`
    /// is `SHA-256(CHAIN_ORIGIN)`. Computed at append time and
    /// stored alongside the entry; recomputed at reload time and
    /// checked against the stored value (tamper detection).
    pub this_hash: String,
}

/// A ledger checkpoint — a signed declaration of the chain's
/// state at a point in time.
///
/// Per worm-ledger-design.md §3 D2: the wire format is C2SP
/// signed-note. For v0.1.x the `signature` field is `None`;
/// step 3 of the implementation roadmap wires Ed25519 signing
/// (or whichever signature scheme `FS_SIGNING_KEY` resolves to)
/// to populate this field with a real signed-note signature.
///
/// The `algorithm` field is per worm-ledger-design.md §3 D3
/// algorithm-agility: SHA-256 today; a future migration to BLAKE3
/// or SHA-3 carries both algorithms during the transition.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Checkpoint {
    pub origin: String,
    pub tree_size: u64,
    /// Hex-encoded `this_hash` of the entry at `tree_size`. For
    /// `tree_size == 0` this is the chain origin hash.
    pub root_hash: String,
    pub algorithm: String,
    /// Unix seconds at the moment `checkpoint()` was called.
    pub timestamp: u64,
    /// `None` for v0.1.x; populated with a signed-note signature
    /// in step 3 of the implementation roadmap.
    pub signature: Option<Vec<u8>>,
}

/// Inclusion proof — evidence that a particular entry was in the
/// ledger at the time of a particular checkpoint. For the v0.1.x
/// linear-chain implementation this is the chain segment from the
/// entry to the checkpoint tip; the verifier recomputes the chain
/// to confirm the tip matches `checkpoint.root_hash`.
///
/// The Merkle-tree upgrade (follow-up commit) shrinks this to
/// O(log N) sibling hashes; the trait surface stays the same.
#[derive(Clone, Debug)]
pub struct InclusionProof {
    pub entry_cursor: u64,
    pub checkpoint_tree_size: u64,
    pub chain_segment: Vec<String>,
}

/// Consistency proof — evidence that checkpoint `c2` is an
/// append-only extension of checkpoint `c1` (no entries removed,
/// no entries modified). For the linear-chain implementation this
/// is the chain segment from `c1.tree_size + 1` to `c2.tree_size`.
#[derive(Clone, Debug)]
pub struct ConsistencyProof {
    pub from_size: u64,
    pub to_size: u64,
    pub chain_segment: Vec<String>,
}

/// L2 WORM Ledger contract per
/// `~/Foundry/conventions/worm-ledger-design.md` §2.
///
/// Object-safe: all methods take `&self` and return concrete
/// types so the daemon can hold a `Box<dyn LedgerBackend + Send +
/// Sync>` regardless of which storage backend (in-memory / POSIX
/// hash-chain / moonshot-database) is wired at startup.
///
/// Append-only invariant lives at the trait surface: there is no
/// public method that mutates or deletes a previously-persisted
/// entry. Implementations enforce the invariant additionally at
/// their storage layer (filesystem write-once for POSIX,
/// capability denial for moonshot-database).
pub trait LedgerBackend {
    /// Append a new payload. Returns the assigned monotonic cursor.
    /// The entry is now permanent — no API surface can remove or
    /// modify it.
    fn append(
        &self,
        payload_id: &str,
        payload: &serde_json::Value,
    ) -> Result<u64, LedgerError>;

    /// Read entries with cursor strictly greater than `since`.
    fn read_since(&self, since: u64) -> Result<Vec<Entry>, LedgerError>;

    /// Diagnostic — the on-disk root path (or backend identifier
    /// for non-filesystem backends). Surfaced via `/v1/contract`.
    fn root(&self) -> &str;

    /// Compute the current checkpoint over the full chain.
    /// Idempotent.
    fn checkpoint(&self) -> Result<Checkpoint, LedgerError>;

    /// Prove (and verify) that `entry_cursor` is in the ledger as
    /// of `checkpoint`. Returns the inclusion proof on success;
    /// `LedgerError::EntryNotFound` if the cursor is absent or
    /// beyond the checkpoint's tree size; `Io`/`Serde` for
    /// underlying errors.
    fn verify_inclusion(
        &self,
        entry_cursor: u64,
        checkpoint: &Checkpoint,
    ) -> Result<InclusionProof, LedgerError>;

    /// Prove (and verify) that `c2` is an append-only extension of
    /// `c1` (every entry recorded in `c1` is unchanged in `c2`,
    /// and `c2` has zero or more additional entries). Returns the
    /// consistency proof on success; `InconsistentCheckpoints`
    /// when `c1` and `c2` disagree on history.
    fn verify_consistency(
        &self,
        c1: &Checkpoint,
        c2: &Checkpoint,
    ) -> Result<ConsistencyProof, LedgerError>;
}

/// Compute the chain origin hash — `SHA-256(CHAIN_ORIGIN)`. This
/// is the `prev_hash` for the first entry in any new ledger.
/// Exposed `pub(crate)` so PosixTileLedger and tests can call it
/// without duplicating the constant.
pub(crate) fn chain_origin_hash() -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(CHAIN_ORIGIN);
    hasher.finalize().into()
}

/// Compute the next chain hash given the previous hash and the
/// new entry's coordinates. Encapsulates the chain-rule definition
/// so InMemoryLedger and PosixTileLedger compute identical hashes
/// from the same inputs.
pub(crate) fn compute_chain_hash(
    prev_hash: &[u8; 32],
    cursor: u64,
    payload_id: &str,
    payload: &serde_json::Value,
) -> Result<[u8; 32], LedgerError> {
    let payload_bytes = serde_json::to_vec(payload)?;
    let mut hasher = Sha256::new();
    hasher.update(prev_hash);
    hasher.update(cursor.to_be_bytes());
    hasher.update((payload_id.len() as u64).to_be_bytes());
    hasher.update(payload_id.as_bytes());
    hasher.update((payload_bytes.len() as u64).to_be_bytes());
    hasher.update(&payload_bytes);
    Ok(hasher.finalize().into())
}

/// Current Unix timestamp in seconds. Wallclock-derived; suitable
/// for checkpoint timestamps where second-granularity ordering
/// across replicas is acceptable. Production deployments running
/// across replicas will want to align clocks via NTP.
pub(crate) fn now_unix_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Hex-encode a 32-byte hash for the on-disk + on-wire form.
pub(crate) fn hex32(h: &[u8; 32]) -> String {
    hex::encode(h)
}

/// Decode a hex-encoded 32-byte hash. Returns the bytes on
/// success; an `InconsistentCheckpoints` error with a descriptive
/// reason on failure (used at proof-verification boundaries).
pub(crate) fn parse_hex32(s: &str) -> Result<[u8; 32], LedgerError> {
    let bytes = hex::decode(s).map_err(|e| LedgerError::InconsistentCheckpoints {
        reason: format!("hash hex decode failed: {e}"),
    })?;
    if bytes.len() != 32 {
        return Err(LedgerError::InconsistentCheckpoints {
            reason: format!("hash length {} != 32", bytes.len()),
        });
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Ok(out)
}

/// Build the signed-note body for a checkpoint per the C2SP signed-note
/// convention. Format: `"{origin}\n{tree_size}\n{base64(root_hash)}\n\n"`.
/// The double trailing newline separates the note body from the signature
/// section in the full signed-note wire format; we sign this exact body.
pub(crate) fn signed_note_body(
    origin: &str,
    tree_size: u64,
    root_hash_hex: &str,
) -> Result<String, LedgerError> {
    let hash_bytes = hex::decode(root_hash_hex).map_err(|e| {
        LedgerError::SigningError(format!("root_hash hex decode failed: {e}"))
    })?;
    let hash_b64 = B64.encode(&hash_bytes);
    Ok(format!("{origin}\n{tree_size}\n{hash_b64}\n\n"))
}

/// Sign `cp` in-place using `key`, populating `cp.signature` with the
/// 64-byte Ed25519 signature over the signed-note body.
pub(crate) fn sign_checkpoint_body(
    cp: &mut Checkpoint,
    key: &SigningKey,
) -> Result<(), LedgerError> {
    use ed25519_dalek::Signer as _;
    let body = signed_note_body(&cp.origin, cp.tree_size, &cp.root_hash)?;
    let sig: ed25519_dalek::Signature = key.sign(body.as_bytes());
    cp.signature = Some(sig.to_bytes().to_vec());
    Ok(())
}

/// Load a 32-byte Ed25519 signing key from a raw binary file.
/// The file must contain exactly 32 bytes (the Ed25519 seed / private key).
/// Tests can write `[1u8; 32]` to a tmp path and pass it here.
pub(crate) fn load_signing_key(path: &std::path::Path) -> Result<SigningKey, LedgerError> {
    let raw = std::fs::read(path).map_err(|e| {
        LedgerError::InvalidKey(format!(
            "could not read signing key at {}: {e}",
            path.display()
        ))
    })?;
    let arr: [u8; 32] = raw.try_into().map_err(|v: Vec<u8>| {
        LedgerError::InvalidKey(format!(
            "signing key file must be exactly 32 bytes (Ed25519 seed), got {}",
            v.len()
        ))
    })?;
    Ok(SigningKey::from_bytes(&arr))
}

/// Verify a signed checkpoint independently of the daemon. Returns `Ok(true)`
/// when the signature is valid, `Ok(false)` when it is absent or invalid,
/// and `Err(...)` when the key or signature bytes are malformed.
///
/// `verifying_key_bytes` must be 32 bytes — the Ed25519 compressed public
/// key (derivable from a `SigningKey` via `signing_key.verifying_key().to_bytes()`).
/// A Customer who keeps only the public key can call this after a Vendor
/// breakout, per Doctrine claim #28 (Customer always retains the option to
/// operate independently).
pub fn verify_checkpoint_signature(
    checkpoint: &Checkpoint,
    verifying_key_bytes: &[u8],
) -> Result<bool, LedgerError> {
    let sig_bytes = match &checkpoint.signature {
        None => return Ok(false),
        Some(v) => v,
    };
    let body = signed_note_body(&checkpoint.origin, checkpoint.tree_size, &checkpoint.root_hash)?;
    let vk_arr: [u8; 32] = verifying_key_bytes.try_into().map_err(|_| {
        LedgerError::InvalidKey(format!(
            "verifying key must be 32 bytes, got {}",
            verifying_key_bytes.len()
        ))
    })?;
    let sig_arr: [u8; 64] = sig_bytes.as_slice().try_into().map_err(|_| {
        LedgerError::SigningError(format!(
            "signature must be 64 bytes, got {}",
            sig_bytes.len()
        ))
    })?;
    let vk = ed25519_dalek::VerifyingKey::from_bytes(&vk_arr)
        .map_err(|e| LedgerError::InvalidKey(format!("invalid verifying key: {e}")))?;
    let sig = ed25519_dalek::Signature::from_bytes(&sig_arr);
    use ed25519_dalek::Verifier as _;
    Ok(vk.verify(body.as_bytes(), &sig).is_ok())
}

/// In-memory `LedgerBackend` implementation. Used today for unit
/// tests + as a fallback when `FS_LEDGER_ROOT` is unset; will be
/// retained indefinitely for integration tests that don't want to
/// write to disk.
///
/// Storage is `Vec<Entry>` behind a `Mutex` — daemon restart loses
/// state. Not suitable for production; use `PosixTileLedger` for
/// any real deployment.
pub struct InMemoryLedger {
    origin: String,
    signing_key: Option<SigningKey>,
    inner: Mutex<Inner>,
}

struct Inner {
    next_cursor: u64,
    entries: Vec<Entry>,
}

impl InMemoryLedger {
    /// Open the in-memory ledger labelled with `origin` (typically
    /// the moduleId). The path is accepted only for API parity
    /// with future `PosixTileLedger::open` so the daemon's main.rs
    /// flow does not need to know which backend is wired; the
    /// path is created if absent and stored only for the
    /// diagnostic `root()` accessor.
    pub fn open(
        path: impl Into<std::path::PathBuf>,
        origin: impl Into<String>,
    ) -> Result<Self, LedgerError> {
        let path: std::path::PathBuf = path.into();
        std::fs::create_dir_all(&path)?;
        Ok(Self {
            origin: origin.into(),
            signing_key: None,
            inner: Mutex::new(Inner {
                next_cursor: 1,
                entries: Vec::new(),
            }),
        })
    }

    /// Open with an Ed25519 signing key. Checkpoints produced by
    /// this ledger instance will carry a signed-note signature.
    /// Used in tests that need to exercise `verify_checkpoint_signature`.
    pub fn open_with_signing_key(
        path: impl Into<std::path::PathBuf>,
        origin: impl Into<String>,
        key: SigningKey,
    ) -> Result<Self, LedgerError> {
        let path: std::path::PathBuf = path.into();
        std::fs::create_dir_all(&path)?;
        Ok(Self {
            origin: origin.into(),
            signing_key: Some(key),
            inner: Mutex::new(Inner {
                next_cursor: 1,
                entries: Vec::new(),
            }),
        })
    }

    /// Compute the chain tip hash from the current in-memory
    /// state. Returns the chain-origin hash if the ledger is
    /// empty.
    fn tip_hash(inner: &Inner) -> Result<[u8; 32], LedgerError> {
        match inner.entries.last() {
            None => Ok(chain_origin_hash()),
            Some(e) => parse_hex32(&e.this_hash),
        }
    }
}

impl LedgerBackend for InMemoryLedger {
    fn append(
        &self,
        payload_id: &str,
        payload: &serde_json::Value,
    ) -> Result<u64, LedgerError> {
        let mut inner = self.inner.lock().expect("ledger mutex poisoned");
        let cursor = inner.next_cursor;
        let prev_hash = Self::tip_hash(&inner)?;
        let this_hash = compute_chain_hash(&prev_hash, cursor, payload_id, payload)?;
        inner.entries.push(Entry {
            cursor,
            payload_id: payload_id.to_string(),
            payload: payload.clone(),
            this_hash: hex32(&this_hash),
        });
        inner.next_cursor += 1;
        Ok(cursor)
    }

    fn read_since(&self, since: u64) -> Result<Vec<Entry>, LedgerError> {
        let inner = self.inner.lock().expect("ledger mutex poisoned");
        Ok(inner
            .entries
            .iter()
            .filter(|e| e.cursor > since)
            .cloned()
            .collect())
    }

    fn root(&self) -> &str {
        // The in-memory backend uses the origin as the diagnostic
        // identifier (no on-disk root path is meaningful).
        &self.origin
    }

    fn checkpoint(&self) -> Result<Checkpoint, LedgerError> {
        let inner = self.inner.lock().expect("ledger mutex poisoned");
        let tree_size = inner.entries.len() as u64;
        let root_hash = Self::tip_hash(&inner)?;
        let mut cp = Checkpoint {
            origin: self.origin.clone(),
            tree_size,
            root_hash: hex32(&root_hash),
            algorithm: "sha256".to_string(),
            timestamp: now_unix_seconds(),
            signature: None,
        };
        if let Some(key) = &self.signing_key {
            sign_checkpoint_body(&mut cp, key)?;
        }
        Ok(cp)
    }

    fn verify_inclusion(
        &self,
        entry_cursor: u64,
        checkpoint: &Checkpoint,
    ) -> Result<InclusionProof, LedgerError> {
        let inner = self.inner.lock().expect("ledger mutex poisoned");
        if entry_cursor == 0 || entry_cursor > checkpoint.tree_size {
            return Err(LedgerError::EntryNotFound(entry_cursor));
        }
        if (checkpoint.tree_size as usize) > inner.entries.len() {
            return Err(LedgerError::InconsistentCheckpoints {
                reason: format!(
                    "checkpoint tree_size {} exceeds in-memory entry count {}",
                    checkpoint.tree_size,
                    inner.entries.len()
                ),
            });
        }
        // Recompute the chain from entry_cursor to tree_size and
        // confirm the tip matches the checkpoint's root_hash.
        let chain_segment: Vec<String> = inner.entries
            [(entry_cursor as usize - 1)..(checkpoint.tree_size as usize)]
            .iter()
            .map(|e| e.this_hash.clone())
            .collect();
        if chain_segment.last() != Some(&checkpoint.root_hash) {
            return Err(LedgerError::InconsistentCheckpoints {
                reason: format!(
                    "checkpoint root_hash {} does not match in-memory tip {}",
                    checkpoint.root_hash,
                    chain_segment.last().cloned().unwrap_or_default()
                ),
            });
        }
        Ok(InclusionProof {
            entry_cursor,
            checkpoint_tree_size: checkpoint.tree_size,
            chain_segment,
        })
    }

    fn verify_consistency(
        &self,
        c1: &Checkpoint,
        c2: &Checkpoint,
    ) -> Result<ConsistencyProof, LedgerError> {
        if c2.tree_size < c1.tree_size {
            return Err(LedgerError::InconsistentCheckpoints {
                reason: format!(
                    "c2.tree_size {} < c1.tree_size {}",
                    c2.tree_size, c1.tree_size
                ),
            });
        }
        let inner = self.inner.lock().expect("ledger mutex poisoned");
        if (c2.tree_size as usize) > inner.entries.len() {
            return Err(LedgerError::InconsistentCheckpoints {
                reason: format!(
                    "c2.tree_size {} exceeds in-memory entry count {}",
                    c2.tree_size,
                    inner.entries.len()
                ),
            });
        }
        // Verify c1 still holds — entry at cursor c1.tree_size must
        // have hash c1.root_hash (or chain origin if c1 was empty).
        let observed_at_c1 = if c1.tree_size == 0 {
            hex32(&chain_origin_hash())
        } else {
            inner.entries[c1.tree_size as usize - 1].this_hash.clone()
        };
        if observed_at_c1 != c1.root_hash {
            return Err(LedgerError::InconsistentCheckpoints {
                reason: format!(
                    "c1.root_hash {} does not match in-memory hash {} at tree_size {}",
                    c1.root_hash, observed_at_c1, c1.tree_size
                ),
            });
        }
        // The consistency proof is the chain segment from c1+1 to c2.
        let chain_segment: Vec<String> = if c2.tree_size == c1.tree_size {
            Vec::new()
        } else {
            inner.entries[c1.tree_size as usize..(c2.tree_size as usize)]
                .iter()
                .map(|e| e.this_hash.clone())
                .collect()
        };
        let observed_at_c2 = chain_segment
            .last()
            .cloned()
            .unwrap_or_else(|| c1.root_hash.clone());
        if observed_at_c2 != c2.root_hash {
            return Err(LedgerError::InconsistentCheckpoints {
                reason: format!(
                    "c2.root_hash {} does not match recomputed tip {}",
                    c2.root_hash, observed_at_c2
                ),
            });
        }
        Ok(ConsistencyProof {
            from_size: c1.tree_size,
            to_size: c2.tree_size,
            chain_segment,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TMPCTR: AtomicU64 = AtomicU64::new(0);

    fn tmpdir() -> PathBuf {
        let n = TMPCTR.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "service-fs-test-{}-{}",
            std::process::id(),
            n
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Tests run against the trait surface, not the concrete
    /// `InMemoryLedger` type. This is deliberate — the same suite
    /// runs against the future `PosixTileLedger` per the
    /// convention's §5 step 2 roadmap. The trait is the contract;
    /// the backend is the implementation.
    fn make_ledger() -> Box<dyn LedgerBackend> {
        Box::new(InMemoryLedger::open(tmpdir(), "foundry").unwrap())
    }

    #[test]
    fn append_assigns_monotonic_cursors() {
        let l = make_ledger();
        let c1 = l.append("a", &serde_json::json!({"x": 1})).unwrap();
        let c2 = l.append("b", &serde_json::json!({"x": 2})).unwrap();
        assert!(c2 > c1, "cursor should advance");
    }

    #[test]
    fn read_since_filters_strictly_greater() {
        let l = make_ledger();
        let c1 = l.append("a", &serde_json::json!({"x": 1})).unwrap();
        l.append("b", &serde_json::json!({"x": 2})).unwrap();
        let after_first = l.read_since(c1).unwrap();
        assert_eq!(after_first.len(), 1, "only entries after c1");
        assert_eq!(after_first[0].payload_id, "b");
    }

    #[test]
    fn read_since_zero_returns_all() {
        let l = make_ledger();
        l.append("a", &serde_json::json!({"x": 1})).unwrap();
        l.append("b", &serde_json::json!({"x": 2})).unwrap();
        let all = l.read_since(0).unwrap();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn checkpoint_on_empty_returns_chain_origin() {
        let l = make_ledger();
        let cp = l.checkpoint().unwrap();
        assert_eq!(cp.tree_size, 0);
        assert_eq!(cp.root_hash, hex32(&chain_origin_hash()));
        assert_eq!(cp.algorithm, "sha256");
        assert!(cp.signature.is_none());
    }

    #[test]
    fn checkpoint_advances_with_appends() {
        let l = make_ledger();
        let cp0 = l.checkpoint().unwrap();
        l.append("a", &serde_json::json!({"x": 1})).unwrap();
        let cp1 = l.checkpoint().unwrap();
        assert_eq!(cp1.tree_size, 1);
        assert_ne!(cp1.root_hash, cp0.root_hash);
        l.append("b", &serde_json::json!({"x": 2})).unwrap();
        let cp2 = l.checkpoint().unwrap();
        assert_eq!(cp2.tree_size, 2);
        assert_ne!(cp2.root_hash, cp1.root_hash);
    }

    #[test]
    fn verify_inclusion_succeeds_for_present_entry() {
        let l = make_ledger();
        let c1 = l.append("a", &serde_json::json!({"x": 1})).unwrap();
        l.append("b", &serde_json::json!({"x": 2})).unwrap();
        let cp = l.checkpoint().unwrap();
        let proof = l.verify_inclusion(c1, &cp).unwrap();
        assert_eq!(proof.entry_cursor, c1);
        assert_eq!(proof.checkpoint_tree_size, cp.tree_size);
        assert!(!proof.chain_segment.is_empty());
    }

    #[test]
    fn verify_inclusion_fails_for_absent_entry() {
        let l = make_ledger();
        l.append("a", &serde_json::json!({"x": 1})).unwrap();
        let cp = l.checkpoint().unwrap();
        match l.verify_inclusion(99, &cp) {
            Err(LedgerError::EntryNotFound(99)) => {}
            other => panic!("expected EntryNotFound(99), got {other:?}"),
        }
    }

    #[test]
    fn verify_consistency_succeeds_for_extension() {
        let l = make_ledger();
        l.append("a", &serde_json::json!({"x": 1})).unwrap();
        let cp1 = l.checkpoint().unwrap();
        l.append("b", &serde_json::json!({"x": 2})).unwrap();
        l.append("c", &serde_json::json!({"x": 3})).unwrap();
        let cp2 = l.checkpoint().unwrap();
        let proof = l.verify_consistency(&cp1, &cp2).unwrap();
        assert_eq!(proof.from_size, 1);
        assert_eq!(proof.to_size, 3);
        assert_eq!(proof.chain_segment.len(), 2);
    }

    #[test]
    fn verify_consistency_fails_for_diverged_history() {
        let l = make_ledger();
        l.append("a", &serde_json::json!({"x": 1})).unwrap();
        let cp_real = l.checkpoint().unwrap();
        let cp_fake = Checkpoint {
            root_hash: hex32(&[42u8; 32]),
            ..cp_real.clone()
        };
        l.append("b", &serde_json::json!({"x": 2})).unwrap();
        let cp_now = l.checkpoint().unwrap();
        match l.verify_consistency(&cp_fake, &cp_now) {
            Err(LedgerError::InconsistentCheckpoints { .. }) => {}
            other => panic!("expected InconsistentCheckpoints, got {other:?}"),
        }
    }

    #[test]
    fn verify_consistency_rejects_smaller_c2() {
        let l = make_ledger();
        l.append("a", &serde_json::json!({"x": 1})).unwrap();
        l.append("b", &serde_json::json!({"x": 2})).unwrap();
        let cp_big = l.checkpoint().unwrap();
        let cp_small = Checkpoint {
            tree_size: 1,
            root_hash: hex32(&chain_origin_hash()),
            ..cp_big.clone()
        };
        match l.verify_consistency(&cp_big, &cp_small) {
            Err(LedgerError::InconsistentCheckpoints { .. }) => {}
            other => panic!("expected InconsistentCheckpoints, got {other:?}"),
        }
    }

    #[test]
    fn checkpoint_signed_when_key_provided() {
        let key = SigningKey::from_bytes(&[1u8; 32]);
        let vk_bytes = key.verifying_key().to_bytes();
        let l = InMemoryLedger::open_with_signing_key(tmpdir(), "foundry", key).unwrap();
        l.append("a", &serde_json::json!({"x": 1})).unwrap();
        let cp = l.checkpoint().unwrap();
        assert!(cp.signature.is_some(), "signed checkpoint must carry a signature");
        assert!(
            verify_checkpoint_signature(&cp, &vk_bytes).unwrap(),
            "signature must verify with the correct key"
        );
    }

    #[test]
    fn checkpoint_signature_fails_with_wrong_key() {
        let key = SigningKey::from_bytes(&[1u8; 32]);
        let l = InMemoryLedger::open_with_signing_key(tmpdir(), "foundry", key).unwrap();
        l.append("a", &serde_json::json!({"x": 1})).unwrap();
        let cp = l.checkpoint().unwrap();
        let wrong_vk = SigningKey::from_bytes(&[2u8; 32]).verifying_key().to_bytes();
        assert!(
            !verify_checkpoint_signature(&cp, &wrong_vk).unwrap(),
            "signature must NOT verify with the wrong key"
        );
    }

    #[test]
    fn checkpoint_signature_fails_on_tampered_fields() {
        let key = SigningKey::from_bytes(&[1u8; 32]);
        let vk_bytes = key.verifying_key().to_bytes();
        let l = InMemoryLedger::open_with_signing_key(tmpdir(), "foundry", key).unwrap();
        l.append("a", &serde_json::json!({"x": 1})).unwrap();
        let mut cp = l.checkpoint().unwrap();
        cp.tree_size += 1; // tamper with the checkpoint body
        assert!(
            !verify_checkpoint_signature(&cp, &vk_bytes).unwrap(),
            "signature must NOT verify after tampering with tree_size"
        );
    }

    #[test]
    fn chain_origin_hash_is_stable() {
        let h = chain_origin_hash();
        let h2 = chain_origin_hash();
        assert_eq!(h, h2);
        // Confirm the chosen domain separator hashes deterministically.
        assert_eq!(hex32(&h).len(), 64);
    }
}
