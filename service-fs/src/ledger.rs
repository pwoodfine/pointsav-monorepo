// SPDX-License-Identifier: Apache-2.0 OR MIT

//! WORM (Write-Once-Read-Many) Immutable Ledger primitive.
//!
//! Append-only invariant is enforced at the API surface — there is
//! no public method that mutates or deletes a previously-persisted
//! entry. Cursors are monotonically increasing `u64`; reads filter
//! by `cursor > since`.
//!
//! This module is the storage abstraction. The current
//! implementation uses an in-memory `Vec<Entry>` behind a `Mutex`
//! as a placeholder so the daemon can run end-to-end against unit
//! tests and curl probes without committing to a disk format. The
//! first NEXT.md item after `cargo check` passes clean is to swap
//! the storage backend for hash-addressed segment files in
//! immutable directories rooted at `FS_LEDGER_ROOT`. The API
//! surface (`open`, `append`, `read_since`, `root`) is the contract
//! that survives that swap.

use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Debug)]
pub enum LedgerError {
    Io(std::io::Error),
}

impl std::fmt::Display for LedgerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LedgerError::Io(e) => write!(f, "ledger I/O error: {e}"),
        }
    }
}

impl std::error::Error for LedgerError {}

impl From<std::io::Error> for LedgerError {
    fn from(e: std::io::Error) -> Self {
        LedgerError::Io(e)
    }
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub cursor: u64,
    pub payload_id: String,
    pub payload: serde_json::Value,
}

pub struct WormLedger {
    root: PathBuf,
    inner: Mutex<Inner>,
}

struct Inner {
    next_cursor: u64,
    entries: Vec<Entry>,
}

impl WormLedger {
    /// Open the ledger at `root`. Creates the directory if it does
    /// not exist; does not load any prior entries (placeholder
    /// implementation — disk-backed reload lands with the
    /// segment-file storage swap).
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, LedgerError> {
        let root: PathBuf = root.into();
        std::fs::create_dir_all(&root)?;
        Ok(Self {
            root,
            inner: Mutex::new(Inner {
                next_cursor: 1,
                entries: Vec::new(),
            }),
        })
    }

    pub fn root(&self) -> &str {
        // Only used for diagnostic surfaces (/v1/contract); fine to
        // lossy-convert non-UTF8 paths since the operator-supplied
        // FS_LEDGER_ROOT will be UTF-8 in practice.
        std::str::from_utf8(self.root.as_os_str().as_encoded_bytes()).unwrap_or("<non-utf8>")
    }

    /// Append a new entry. Returns the assigned cursor. The entry
    /// is now permanent — no API surface can remove or modify it.
    pub fn append(
        &self,
        payload_id: &str,
        payload: &serde_json::Value,
    ) -> Result<u64, LedgerError> {
        let mut inner = self.inner.lock().expect("ledger mutex poisoned");
        let cursor = inner.next_cursor;
        inner.entries.push(Entry {
            cursor,
            payload_id: payload_id.to_string(),
            payload: payload.clone(),
        });
        inner.next_cursor += 1;
        Ok(cursor)
    }

    /// Read entries with cursor strictly greater than `since`.
    pub fn read_since(&self, since: u64) -> Result<Vec<Entry>, LedgerError> {
        let inner = self.inner.lock().expect("ledger mutex poisoned");
        Ok(inner
            .entries
            .iter()
            .filter(|e| e.cursor > since)
            .cloned()
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "service-fs-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn append_assigns_monotonic_cursors() {
        let l = WormLedger::open(tmpdir()).unwrap();
        let c1 = l.append("a", &serde_json::json!({"x": 1})).unwrap();
        let c2 = l.append("b", &serde_json::json!({"x": 2})).unwrap();
        assert!(c2 > c1, "cursor should advance");
    }

    #[test]
    fn read_since_filters_strictly_greater() {
        let l = WormLedger::open(tmpdir()).unwrap();
        let c1 = l.append("a", &serde_json::json!({"x": 1})).unwrap();
        l.append("b", &serde_json::json!({"x": 2})).unwrap();
        let after_first = l.read_since(c1).unwrap();
        assert_eq!(after_first.len(), 1, "only entries after c1");
        assert_eq!(after_first[0].payload_id, "b");
    }

    #[test]
    fn read_since_zero_returns_all() {
        let l = WormLedger::open(tmpdir()).unwrap();
        l.append("a", &serde_json::json!({"x": 1})).unwrap();
        l.append("b", &serde_json::json!({"x": 2})).unwrap();
        let all = l.read_since(0).unwrap();
        assert_eq!(all.len(), 2);
    }
}
