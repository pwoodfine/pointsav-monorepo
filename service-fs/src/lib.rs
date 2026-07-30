// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `service-fs` library surface — exposed for integration tests.
//!
//! The daemon binary at `src/main.rs` and integration tests in
//! `tests/` both build against this crate. The public surface is
//! the minimum needed to construct the axum router with a real
//! ledger backend for end-to-end testing without a TCP listener.

pub mod http;
pub mod ledger;
pub mod mcp;
pub mod posix_tile;

pub use http::{router, AppState};

/// Open a `PosixTileLedger` and box it as a `LedgerBackend` trait object.
/// Convenience wrapper for integration-test setup where the caller
/// needs a `Box<dyn LedgerBackend + Send + Sync>` without importing
/// `posix_tile` directly.
pub fn posix_tile_open(
    root: impl Into<std::path::PathBuf>,
    origin: impl Into<String>,
    signing_key_path: Option<impl AsRef<std::path::Path>>,
) -> Result<Box<dyn ledger::LedgerBackend + Send + Sync>, ledger::LedgerError> {
    Ok(Box::new(posix_tile::PosixTileLedger::open(
        root,
        origin,
        signing_key_path,
    )?))
}
