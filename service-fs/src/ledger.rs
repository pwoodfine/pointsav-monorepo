// SPDX-License-Identifier: Apache-2.0 OR MIT

//! WORM Ledger Layer 2 (per
//! `~/Foundry/conventions/worm-ledger-design.md`).
//!
//! L2 in the four-layer stack: the target-independent Rust trait
//! that the wire layer (L3, in `http.rs`) and the storage layer (L1,
//! per-backend) compose against. The trait is the durable contract
//! that survives changes above it (axum vs. MCP-over-IPC) and below
//! it (in-memory vs. POSIX tiles vs. moonshot-database).
//!
//! The trait surface as ratified in worm-ledger-design.md §2:
//!
//! ```text
//!   open(path, module_id, signing_key) -> Self
//!   append(payload_id, payload_bytes) -> Cursor
//!   read_since(cursor) -> Iterator<Entry>
//!   checkpoint() -> SignedNote
//!   verify_inclusion(entry, checkpoint) -> Proof
//!   verify_consistency(c1, c2) -> Proof
//! ```
//!
//! This commit lands the trait + the in-memory backend behind it
//! per the convention's §5 implementation roadmap step 1 ("L2 trait
//! extraction"). Steps 2–5 fill in the rest:
//! - Step 2 (L1 POSIX tile backend) adds `verify_inclusion` /
//!   `verify_consistency` in real terms; in-memory backend's
//!   verify_* implementations are trivial.
//! - Step 3 (checkpoint signing) lands the `signing_key` open
//!   parameter and a real `checkpoint()` returning a signed-note.
//! - Steps 4–5 (audit sub-ledger + MCP layer) compose against this
//!   trait without changing it.
//!
//! Today's three runtime methods (`append`, `read_since`, `root`)
//! are in the trait. `open` stays as a per-impl inherent
//! constructor — `InMemoryLedger::open(...)` returns the concrete
//! type, then the daemon wraps it in `Box<dyn LedgerBackend + Send
//! + Sync>` for use through the wire layer. This keeps the trait
//! object-safe.
//!
//! `checkpoint` / `verify_inclusion` / `verify_consistency` are
//! NOT in the trait yet — they land in steps 2–3 with the POSIX
//! backend and the signed-note signing wiring. The convention is
//! the END-state contract; this trait grows incrementally per the
//! roadmap.

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

/// L2 WORM Ledger contract per
/// `~/Foundry/conventions/worm-ledger-design.md` §2.
///
/// Object-safe: all methods take `&self` and return concrete types,
/// so the daemon can hold a `Box<dyn LedgerBackend + Send + Sync>`
/// regardless of which storage backend (in-memory / POSIX tile /
/// moonshot-database) is wired at startup.
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
}

/// In-memory `LedgerBackend` implementation. Used today for the
/// service-fs Tokio skeleton + unit tests; will be retained for
/// integration tests once the POSIX tile backend lands per
/// worm-ledger-design.md §5 step 2.
///
/// Storage is `Vec<Entry>` behind a `Mutex` — daemon restart loses
/// state. Not suitable for production; use `PosixTileLedger` (next
/// commit) for any real deployment.
pub struct InMemoryLedger {
    root: PathBuf,
    inner: Mutex<Inner>,
}

struct Inner {
    next_cursor: u64,
    entries: Vec<Entry>,
}

impl InMemoryLedger {
    /// Open the in-memory ledger at `root`. Creates the directory
    /// if it does not exist (kept for API parity with future
    /// `PosixTileLedger::open` so the daemon's main.rs flow does
    /// not need to know which backend is wired). Does not load any
    /// prior entries — in-memory by definition.
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
}

impl LedgerBackend for InMemoryLedger {
    fn append(
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
        // Only used for diagnostic surfaces (/v1/contract); fine to
        // lossy-convert non-UTF8 paths since the operator-supplied
        // FS_LEDGER_ROOT will be UTF-8 in practice.
        std::str::from_utf8(self.root.as_os_str().as_encoded_bytes()).unwrap_or("<non-utf8>")
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

    /// Tests run against the trait surface, not the concrete
    /// `InMemoryLedger` type. This is deliberate — the same suite
    /// runs against the future `PosixTileLedger` per the convention's
    /// §5 step 2 roadmap. The trait is the contract; the backend
    /// is the implementation.
    fn make_ledger() -> Box<dyn LedgerBackend> {
        Box::new(InMemoryLedger::open(tmpdir()).unwrap())
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
}
