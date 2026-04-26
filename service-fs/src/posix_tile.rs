// SPDX-License-Identifier: Apache-2.0 OR MIT

//! POSIX-backed `LedgerBackend` implementation — persistent
//! linear-hash-chain WORM ledger per
//! `~/Foundry/conventions/worm-ledger-design.md` §5 step 2.
//!
//! v0.1.x baseline:
//! - Single newline-delimited JSON log file at
//!   `<root>/<moduleId>/log.jsonl`.
//! - Each record: `{cursor, payload_id, payload, this_hash}`. The
//!   `this_hash` is `SHA-256(prev_hash || cursor || payload_id ||
//!   payload_canonical_bytes)` (chain rule shared with
//!   `InMemoryLedger` via `crate::ledger::compute_chain_hash`).
//! - On `append`: append the new record under a Mutex; rewrite the
//!   log file via the D4 atomic-write discipline (write `.tmp` →
//!   fsync → rename → chmod 0o444 on the renamed final path).
//! - On `open`: load all records, recompute each record's
//!   `this_hash` from prior state, return `ChainTampered` if any
//!   stored hash diverges from the recomputed value.
//!
//! Performance trade-off (acknowledged): rewriting the whole log
//! per `append` is O(N). Acceptable for v0.1.x — segment-batched
//! tile files (256 entries per sealed segment + a current open
//! segment) are the natural performance upgrade and a follow-up
//! commit. The `LedgerBackend` trait surface and the on-disk
//! record schema both survive that upgrade.
//!
//! D4 (per the convention) v0.1.x baseline implemented here:
//! 1. Write candidate bytes to `<log_path>.tmp`
//! 2. fsync the temp file
//! 3. Atomic rename to `<log_path>` (POSIX guarantees atomicity)
//! 4. chmod 0o444 on the final log file
//!
//! `chattr +i` (filesystem-immutable flag) requires
//! `CAP_LINUX_IMMUTABLE` and is deferred to systemd-unit time per
//! D4. ext4/xfs `journal_data` mode is mount-time, not per-file;
//! that's a deployment-tier concern documented in the systemd
//! unit Master is authoring at `infrastructure/local-fs/`.

use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use ed25519_dalek::SigningKey;

use crate::ledger::{
    chain_origin_hash, compute_chain_hash, hex32, load_signing_key, now_unix_seconds, parse_hex32,
    sign_checkpoint_body, Checkpoint, ConsistencyProof, Entry, InclusionProof, LedgerBackend,
    LedgerError,
};

/// On-disk record shape — what each line of `log.jsonl` is.
/// Matches `ledger::Entry` field-for-field; kept as a separate
/// struct so the on-disk schema can evolve independently of the
/// in-memory representation if needed.
#[derive(serde::Deserialize, serde::Serialize)]
struct OnDiskRecord {
    cursor: u64,
    payload_id: String,
    payload: serde_json::Value,
    this_hash: String,
}

impl From<OnDiskRecord> for Entry {
    fn from(r: OnDiskRecord) -> Self {
        Entry {
            cursor: r.cursor,
            payload_id: r.payload_id,
            payload: r.payload,
            this_hash: r.this_hash,
        }
    }
}

pub struct PosixTileLedger {
    origin: String,
    root: PathBuf,
    log_path: PathBuf,
    signing_key: Option<SigningKey>,
    inner: Mutex<Inner>,
}

struct Inner {
    next_cursor: u64,
    entries: Vec<Entry>,
}

impl PosixTileLedger {
    /// Open the ledger at `<root>/<origin>/log.jsonl`. Creates the
    /// per-tenant directory if absent. Loads existing entries and
    /// verifies the chain integrity — returns
    /// `LedgerError::ChainTampered` if any record's stored hash
    /// disagrees with the recomputed value.
    ///
    /// `signing_key_path` — optional path to a 32-byte raw Ed25519
    /// seed file. When present, `checkpoint()` populates
    /// `Checkpoint::signature` with an Ed25519 signed-note signature.
    /// Pass `None::<&std::path::Path>` to open without signing (tests,
    /// deployments where the key is not yet provisioned).
    pub fn open(
        root: impl Into<PathBuf>,
        origin: impl Into<String>,
        signing_key_path: Option<impl AsRef<std::path::Path>>,
    ) -> Result<Self, LedgerError> {
        let root: PathBuf = root.into();
        let origin: String = origin.into();
        let tenant_dir = root.join(&origin);
        std::fs::create_dir_all(&tenant_dir)?;
        let log_path = tenant_dir.join("log.jsonl");

        let entries = if log_path.exists() {
            load_and_verify_log(&log_path)?
        } else {
            Vec::new()
        };

        let next_cursor = entries.last().map(|e| e.cursor + 1).unwrap_or(1);

        let signing_key = signing_key_path
            .map(|p| load_signing_key(p.as_ref()))
            .transpose()?;

        Ok(Self {
            origin,
            root: tenant_dir,
            log_path,
            signing_key,
            inner: Mutex::new(Inner {
                next_cursor,
                entries,
            }),
        })
    }

    fn tip_hash(inner: &Inner) -> Result<[u8; 32], LedgerError> {
        match inner.entries.last() {
            None => Ok(chain_origin_hash()),
            Some(e) => parse_hex32(&e.this_hash),
        }
    }

    /// Rewrite the log file with the current `inner.entries` using
    /// the D4 atomic-write discipline. Caller holds the inner
    /// mutex.
    fn flush_log(&self, inner: &Inner) -> Result<(), LedgerError> {
        let tmp_path = with_tmp_suffix(&self.log_path);
        // Open the temp file. Mode 0o644 here so the write
        // succeeds; the final read-only mode is set on the renamed
        // final file below.
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o644)
            .open(&tmp_path)?;
        for entry in &inner.entries {
            let record = OnDiskRecord {
                cursor: entry.cursor,
                payload_id: entry.payload_id.clone(),
                payload: entry.payload.clone(),
                this_hash: entry.this_hash.clone(),
            };
            let line = serde_json::to_vec(&record)?;
            file.write_all(&line)?;
            file.write_all(b"\n")?;
        }
        file.sync_all()?;
        drop(file);

        // Atomic rename — POSIX guarantees this is atomic on the
        // same filesystem.
        std::fs::rename(&tmp_path, &self.log_path)?;

        // chmod 0o444 on the final log path — readable by all,
        // writable by none. Per D4. The next append re-opens via
        // OpenOptions::truncate which still works on a 0o444 file
        // when the daemon process owns it.
        let mut perms = std::fs::metadata(&self.log_path)?.permissions();
        perms.set_mode(0o444);
        std::fs::set_permissions(&self.log_path, perms)?;
        Ok(())
    }
}

fn with_tmp_suffix(p: &Path) -> PathBuf {
    let mut new_name = p.file_name().expect("log path has filename").to_owned();
    new_name.push(".tmp");
    p.with_file_name(new_name)
}

/// Load `log.jsonl`, parse every line, recompute the chain
/// progressively, and verify each record's stored `this_hash`
/// against the recomputed value. Returns the parsed entries on
/// success; `LedgerError::ChainTampered` on the first mismatch.
fn load_and_verify_log(log_path: &Path) -> Result<Vec<Entry>, LedgerError> {
    let file = File::open(log_path)?;
    let reader = BufReader::new(file);
    let mut entries = Vec::new();
    let mut prev_hash = chain_origin_hash();
    for (line_no, line_result) in reader.lines().enumerate() {
        let line = line_result?;
        if line.trim().is_empty() {
            continue;
        }
        let record: OnDiskRecord = serde_json::from_str(&line)?;
        let recomputed = compute_chain_hash(
            &prev_hash,
            record.cursor,
            &record.payload_id,
            &record.payload,
        )?;
        let expected_hex = hex32(&recomputed);
        if record.this_hash != expected_hex {
            return Err(LedgerError::ChainTampered {
                cursor: record.cursor,
                expected: expected_hex,
                got: record.this_hash,
            });
        }
        // Sanity: cursors are monotonic 1-based with no gaps.
        let expected_cursor = (line_no as u64) + 1;
        if record.cursor != expected_cursor {
            return Err(LedgerError::ChainTampered {
                cursor: record.cursor,
                expected: format!("monotonic 1-based cursor {expected_cursor}"),
                got: format!("cursor {}", record.cursor),
            });
        }
        prev_hash = recomputed;
        entries.push(record.into());
    }
    Ok(entries)
}

impl LedgerBackend for PosixTileLedger {
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
        // Persist before bumping next_cursor so a flush failure
        // leaves the in-memory + on-disk state consistent.
        if let Err(e) = self.flush_log(&inner) {
            // Roll back the in-memory append on flush failure.
            inner.entries.pop();
            return Err(e);
        }
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
        std::str::from_utf8(self.root.as_os_str().as_encoded_bytes()).unwrap_or("<non-utf8>")
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
                    "checkpoint tree_size {} exceeds on-disk entry count {}",
                    checkpoint.tree_size,
                    inner.entries.len()
                ),
            });
        }
        let chain_segment: Vec<String> = inner.entries
            [(entry_cursor as usize - 1)..(checkpoint.tree_size as usize)]
            .iter()
            .map(|e| e.this_hash.clone())
            .collect();
        if chain_segment.last() != Some(&checkpoint.root_hash) {
            return Err(LedgerError::InconsistentCheckpoints {
                reason: format!(
                    "checkpoint root_hash {} does not match on-disk tip {}",
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
                    "c2.tree_size {} exceeds on-disk entry count {}",
                    c2.tree_size,
                    inner.entries.len()
                ),
            });
        }
        let observed_at_c1 = if c1.tree_size == 0 {
            hex32(&chain_origin_hash())
        } else {
            inner.entries[c1.tree_size as usize - 1].this_hash.clone()
        };
        if observed_at_c1 != c1.root_hash {
            return Err(LedgerError::InconsistentCheckpoints {
                reason: format!(
                    "c1.root_hash {} does not match on-disk hash {} at tree_size {}",
                    c1.root_hash, observed_at_c1, c1.tree_size
                ),
            });
        }
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
    use std::sync::atomic::{AtomicU64, Ordering};

    static TMPCTR: AtomicU64 = AtomicU64::new(0);

    fn tmpdir() -> PathBuf {
        let n = TMPCTR.fetch_add(1, Ordering::SeqCst);
        let d = std::env::temp_dir().join(format!(
            "service-fs-posix-test-{}-{}",
            std::process::id(),
            n
        ));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn append_persists_across_restart() {
        let root = tmpdir();
        {
            let l = PosixTileLedger::open(&root, "foundry", None::<&std::path::Path>).unwrap();
            l.append("a", &serde_json::json!({"x": 1})).unwrap();
            l.append("b", &serde_json::json!({"x": 2})).unwrap();
        }
        // Re-open — entries should be intact.
        let l2 = PosixTileLedger::open(&root, "foundry", None::<&std::path::Path>).unwrap();
        let all = l2.read_since(0).unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].payload_id, "a");
        assert_eq!(all[1].payload_id, "b");
    }

    #[test]
    fn checkpoint_after_restart_matches_pre_restart() {
        let root = tmpdir();
        let cp_before = {
            let l = PosixTileLedger::open(&root, "foundry", None::<&std::path::Path>).unwrap();
            l.append("a", &serde_json::json!({"x": 1})).unwrap();
            l.append("b", &serde_json::json!({"x": 2})).unwrap();
            l.checkpoint().unwrap()
        };
        let l2 = PosixTileLedger::open(&root, "foundry", None::<&std::path::Path>).unwrap();
        let cp_after = l2.checkpoint().unwrap();
        // Timestamps will differ; tree_size + root_hash must not.
        assert_eq!(cp_before.tree_size, cp_after.tree_size);
        assert_eq!(cp_before.root_hash, cp_after.root_hash);
        assert_eq!(cp_before.algorithm, cp_after.algorithm);
    }

    #[test]
    fn next_append_after_restart_continues_chain() {
        let root = tmpdir();
        let cp1 = {
            let l = PosixTileLedger::open(&root, "foundry", None::<&std::path::Path>).unwrap();
            l.append("a", &serde_json::json!({"x": 1})).unwrap();
            l.checkpoint().unwrap()
        };
        let l2 = PosixTileLedger::open(&root, "foundry", None::<&std::path::Path>).unwrap();
        l2.append("b", &serde_json::json!({"x": 2})).unwrap();
        let cp2 = l2.checkpoint().unwrap();
        // cp2 must extend cp1 — verify_consistency confirms it.
        let proof = l2.verify_consistency(&cp1, &cp2).unwrap();
        assert_eq!(proof.from_size, 1);
        assert_eq!(proof.to_size, 2);
    }

    #[test]
    fn tamper_detection_on_reload() {
        let root = tmpdir();
        let log_path = {
            let l = PosixTileLedger::open(&root, "foundry", None::<&std::path::Path>).unwrap();
            l.append("a", &serde_json::json!({"x": 1})).unwrap();
            l.append("b", &serde_json::json!({"x": 2})).unwrap();
            // We need the log path to manipulate it after the
            // ledger has dropped its file lock (Mutex). The
            // ledger's `root` accessor returns the tenant-dir.
            std::path::PathBuf::from(l.root()).join("log.jsonl")
        };
        // Tamper with the on-disk log: edit entry "a"'s payload
        // without recomputing this_hash. Reload should detect
        // mismatch.
        let bytes = std::fs::read_to_string(&log_path).unwrap();
        let tampered = bytes.replace(r#""x":1"#, r#""x":99"#);
        // Need write permission since the file is 0o444 from the
        // last append.
        let mut perms = std::fs::metadata(&log_path).unwrap().permissions();
        perms.set_mode(0o644);
        std::fs::set_permissions(&log_path, perms).unwrap();
        std::fs::write(&log_path, tampered).unwrap();
        // Reopen — should detect tamper.
        match PosixTileLedger::open(&root, "foundry", None::<&std::path::Path>) {
            Err(LedgerError::ChainTampered { cursor: 1, .. }) => {}
            Err(other) => panic!("expected ChainTampered at cursor 1, got error {other:?}"),
            Ok(_) => panic!("expected ChainTampered at cursor 1, got Ok(ledger)"),
        }
    }

    #[test]
    fn log_file_is_read_only_after_append() {
        let root = tmpdir();
        let l = PosixTileLedger::open(&root, "foundry", None::<&std::path::Path>).unwrap();
        l.append("a", &serde_json::json!({"x": 1})).unwrap();
        let log_path = std::path::PathBuf::from(l.root()).join("log.jsonl");
        let perms = std::fs::metadata(&log_path).unwrap().permissions();
        // Mode 0o444 — readable by all, writable by none.
        assert_eq!(perms.mode() & 0o777, 0o444);
    }

    #[test]
    fn empty_ledger_checkpoint_equals_chain_origin() {
        let root = tmpdir();
        let l = PosixTileLedger::open(&root, "foundry", None::<&std::path::Path>).unwrap();
        let cp = l.checkpoint().unwrap();
        assert_eq!(cp.tree_size, 0);
        assert_eq!(cp.root_hash, hex32(&chain_origin_hash()));
    }

    #[test]
    fn verify_inclusion_works_after_restart() {
        let root = tmpdir();
        let c1 = {
            let l = PosixTileLedger::open(&root, "foundry", None::<&std::path::Path>).unwrap();
            let c = l.append("a", &serde_json::json!({"x": 1})).unwrap();
            l.append("b", &serde_json::json!({"x": 2})).unwrap();
            c
        };
        let l2 = PosixTileLedger::open(&root, "foundry", None::<&std::path::Path>).unwrap();
        let cp = l2.checkpoint().unwrap();
        let proof = l2.verify_inclusion(c1, &cp).unwrap();
        assert_eq!(proof.entry_cursor, c1);
        assert_eq!(proof.checkpoint_tree_size, 2);
    }

    #[test]
    fn checkpoint_signed_by_posix_ledger_verifies_independently() {
        // Write a deterministic test key to a temp file.
        let key_dir = tmpdir();
        let key_path = key_dir.join("test.key");
        std::fs::write(&key_path, &[3u8; 32]).unwrap();

        let signing_key = ed25519_dalek::SigningKey::from_bytes(&[3u8; 32]);
        let vk_bytes = signing_key.verifying_key().to_bytes();

        let root = tmpdir();
        let l = PosixTileLedger::open(&root, "foundry", Some(&key_path)).unwrap();
        l.append("a", &serde_json::json!({"x": 1})).unwrap();
        let cp = l.checkpoint().unwrap();

        assert!(cp.signature.is_some(), "signed checkpoint must carry a signature");
        assert!(
            crate::ledger::verify_checkpoint_signature(&cp, &vk_bytes).unwrap(),
            "signature must verify with the correct public key (independent of daemon)"
        );
    }
}
