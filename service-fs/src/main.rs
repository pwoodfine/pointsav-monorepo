// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `service-fs` — Ring 1 WORM Immutable Ledger.
//!
//! Per-tenant boundary-ingest service. Accepts append requests
//! from sibling Ring 1 services (`service-people`, `service-email`,
//! `service-input`) and serves read requests to Ring 2 callers
//! (`service-extraction`). One process per `moduleId` (per-tenant
//! by infrastructure, per Doctrine §IV.b).
//!
//! Hard rules:
//! - ADR-07 — zero AI in Ring 1; deterministic processing only.
//! - Append-only invariant — no path mutates a previously-persisted
//!   entry. The ledger module enforces this at its API surface.
//! - One process per moduleId — `FS_MODULE_ID` env is required and
//!   the daemon refuses to start without it. Cross-tenant
//!   reads/writes are out of scope; the caller's
//!   `X-Foundry-Module-ID` header MUST match `FS_MODULE_ID` or the
//!   request is rejected.
//!
//! Reference shape: `slm-doorman-server`
//! (project-slm cluster `78031c4`) — Tokio + axum, layered AppState
//! in an Arc, axum router with /healthz + /readyz + /v1/contract
//! plus business endpoints, anyhow at the top level, tracing via
//! EnvFilter.
//!
//! Environment configuration:
//!   FS_BIND_ADDR              default 127.0.0.1:9100
//!   FS_MODULE_ID              required; this instance's tenant moduleId
//!   FS_LEDGER_ROOT            required; absolute path to the per-tenant
//!                             WORM directory (created on first append
//!                             if absent)
//!   RUST_LOG                  default service_fs=info,axum=warn
//!
//! This is the B1-equivalent skeleton: routes mount, the ledger
//! exposes append + read_since, the in-memory storage is a placeholder
//! for the on-disk segment-file format that lands as the first
//! NEXT.md item after `cargo check` passes clean.

mod http;
mod ledger;
mod posix_tile;

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Context;
use tracing::info;

use crate::posix_tile::PosixTileLedger;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    let bind_addr: SocketAddr = std::env::var("FS_BIND_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:9100".to_string())
        .parse()
        .context("FS_BIND_ADDR must be a socket address")?;

    let module_id = std::env::var("FS_MODULE_ID")
        .context("FS_MODULE_ID is required (per-tenant boundary; one process per moduleId)")?;

    let ledger_root = std::env::var("FS_LEDGER_ROOT")
        .context("FS_LEDGER_ROOT is required (absolute path to WORM segment directory)")?;

    // Optional: path to a 32-byte raw Ed25519 seed file. When set,
    // every checkpoint is signed with the named key (signed-note body
    // per worm-ledger-design.md §5 step 3). Omit to run without
    // checkpoint signing — useful when key provisioning is deferred.
    let signing_key_path: Option<std::path::PathBuf> =
        std::env::var("FS_SIGNING_KEY").ok().map(Into::into);

    // Persistent POSIX hash-chain backend per
    // ~/Foundry/conventions/worm-ledger-design.md §5 step 2.
    // Loads existing entries on open + verifies the chain (returns
    // ChainTampered if any record's stored hash diverges from the
    // recomputed value). InMemoryLedger remains available behind
    // the same trait for tests + as a fallback.
    let ledger: Box<dyn ledger::LedgerBackend + Send + Sync> = Box::new(
        PosixTileLedger::open(&ledger_root, &module_id, signing_key_path.as_deref())
            .with_context(|| format!("failed to open WORM ledger at {ledger_root}"))?,
    );

    // ADR-07 audit sub-ledger per worm-ledger-design.md §5 step 4.
    // Placed at <ledger_root>/<moduleId>/audit-log/ — a sibling
    // directory inside the per-tenant tree, separate from the main
    // ledger's log.jsonl. The same PosixTileLedger / D4 discipline
    // applies; no signing key (audit log is integrity-protected by
    // the hash chain; a separate signing key for the audit log is
    // a follow-up if required by a specific compliance regime).
    let audit_ledger_root = format!("{ledger_root}/{module_id}");
    let audit_ledger: Box<dyn ledger::LedgerBackend + Send + Sync> = Box::new(
        PosixTileLedger::open(&audit_ledger_root, "audit-log", None::<&std::path::Path>)
            .with_context(|| {
                format!("failed to open audit ledger at {audit_ledger_root}/audit-log")
            })?,
    );

    let state = Arc::new(http::AppState {
        module_id: module_id.clone(),
        ledger,
        audit_ledger,
    });

    info!(
        version = env!("CARGO_PKG_VERSION"),
        %bind_addr,
        module_id = %module_id,
        ledger_root = %ledger_root,
        signing = signing_key_path.is_some(),
        "service-fs Ring 1 WORM ledger starting"
    );

    let app = http::router(state);
    let listener = tokio::net::TcpListener::bind(bind_addr)
        .await
        .with_context(|| format!("failed to bind {bind_addr}"))?;
    axum::serve(listener, app)
        .await
        .context("axum serve loop exited")?;
    Ok(())
}

fn init_tracing() {
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("service_fs=info,axum=warn"));
    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer())
        .init();
}
