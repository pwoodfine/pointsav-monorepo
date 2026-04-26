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

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Context;
use tracing::info;

use crate::ledger::WormLedger;

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

    let ledger = WormLedger::open(&ledger_root)
        .with_context(|| format!("failed to open WORM ledger at {ledger_root}"))?;

    let state = Arc::new(http::AppState {
        module_id: module_id.clone(),
        ledger,
    });

    info!(
        version = env!("CARGO_PKG_VERSION"),
        %bind_addr,
        module_id = %module_id,
        ledger_root = %ledger_root,
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
