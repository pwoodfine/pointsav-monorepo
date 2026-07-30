// SPDX-License-Identifier: Apache-2.0 OR MIT

//! In-flight request scheduler — priority slot reservation for the Doorman.
//!
//! The Doorman's llama.cpp backend runs with `--parallel 2` (two concurrent
//! inference slots). This module controls which in-flight HTTP requests get
//! those slots based on three priority classes, mapping to the
//! `X-Foundry-Priority` header values:
//!
//! | Class | Header value | Caller               | Admission rule |
//! |-------|-------------|----------------------|----------------|
//! | P0    | `p0`        | Interactive chat, editorial | Admit if any slot free |
//! | P1    | `p1`        | Tier A extraction worker    | Admit if any slot free |
//! | P2    | `p2`        | Training/nightly batch      | Admit only if ALL slots free |
//!
//! P2 runs only when the backend is otherwise idle. This prevents training
//! requests from competing with extraction during a drain burst. The 300-second
//! escape valve (caller-side `Retry-After` countdown) is the responsibility
//! of the HTTP layer — see `http.rs`.
//!
//! ## Starvation properties
//!
//! - P0/P1 are never starved by P2 (P2 is rejected when any slot is in use).
//! - P2 can be starved by continuous P0/P1 load. This is acceptable for
//!   training: GPU training runs on a separate yoyo node and is not affected.
//!   CPU training requests (unlikely at our scale) can retry with `Retry-After`.
//! - P0 cannot starve P1 at current slot counts (both use the same pool with
//!   equal admission rules). If P0-only starvation of P1 becomes a concern,
//!   add a P0 burst counter (see plan §4 WARN-1).

use std::sync::Arc;

use tokio::sync::{OwnedSemaphorePermit, Semaphore, TryAcquireError};

/// The three in-flight priority classes for Doorman request scheduling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InfightPriority {
    /// Interactive chat or editorial — highest priority.
    P0,
    /// Tier A extraction background worker — normal priority.
    P1,
    /// Training / nightly batch — lowest priority; only when idle.
    P2,
}

impl InfightPriority {
    /// Parse from the `X-Foundry-Priority` header value.
    /// Defaults to `P1` (background extraction) when the header is absent or
    /// unrecognized, since most callers are extraction workers. Interactive
    /// callers must set `p0` explicitly.
    pub fn from_header(value: Option<&str>) -> Self {
        match value.map(|v| v.trim().to_ascii_lowercase()).as_deref() {
            Some("p0") => InfightPriority::P0,
            Some("p2") => InfightPriority::P2,
            _ => InfightPriority::P1,
        }
    }
}

/// A scheduler permit. Holds one inference slot for the duration of a request.
/// The slot is returned when this guard is dropped.
pub struct SchedulerPermit {
    _inner: OwnedSemaphorePermit,
}

/// Three-priority in-flight request scheduler backed by a shared `Semaphore`.
///
/// Create once at server startup and clone the `Arc` into handler state.
#[derive(Debug)]
pub struct RequestScheduler {
    slots: Arc<Semaphore>,
    capacity: usize,
}

impl RequestScheduler {
    /// Create a new scheduler with `capacity` concurrent inference slots.
    /// Set `capacity` to match llama.cpp's `--parallel` value (typically 2).
    pub fn new(capacity: usize) -> Self {
        Self {
            slots: Arc::new(Semaphore::new(capacity.max(1))),
            capacity: capacity.max(1),
        }
    }

    /// Try to admit a request without blocking. Returns a `SchedulerPermit`
    /// on success, or `None` when the priority rules deny admission.
    ///
    /// Callers that receive `None` should return HTTP 429 with a
    /// `Retry-After: 30` header. P2 callers should use a longer backoff
    /// (e.g. 300 s) to avoid a busy-loop against a sustained extraction load.
    pub fn try_admit(&self, priority: InfightPriority) -> Option<SchedulerPermit> {
        match priority {
            InfightPriority::P0 | InfightPriority::P1 => {
                // Admit when any slot is free.
                match self.slots.clone().try_acquire_owned() {
                    Ok(permit) => Some(SchedulerPermit { _inner: permit }),
                    Err(TryAcquireError::NoPermits) | Err(TryAcquireError::Closed) => None,
                }
            }
            InfightPriority::P2 => {
                // Admit only when ALL slots are free (backend is fully idle).
                // available_permits() is a snapshot — a concurrent P0/P1 may
                // grab a slot between the check and the acquire. In that case
                // try_acquire_owned() correctly fails; we return None.
                if self.slots.available_permits() < self.capacity {
                    return None;
                }
                match self.slots.clone().try_acquire_owned() {
                    Ok(permit) => Some(SchedulerPermit { _inner: permit }),
                    Err(TryAcquireError::NoPermits) | Err(TryAcquireError::Closed) => None,
                }
            }
        }
    }

    /// Available permit count — exposed for the health / metrics endpoint.
    pub fn available_permits(&self) -> usize {
        self.slots.available_permits()
    }

    /// Total capacity.
    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn p0_and_p1_share_slots() {
        let s = RequestScheduler::new(2);
        let p0 = s.try_admit(InfightPriority::P0).expect("first slot");
        let p1 = s.try_admit(InfightPriority::P1).expect("second slot");
        // Both slots occupied — P0 should be rejected
        assert!(s.try_admit(InfightPriority::P0).is_none());
        // Release one
        drop(p0);
        assert!(s.try_admit(InfightPriority::P0).is_some());
        drop(p1);
    }

    #[tokio::test]
    async fn p2_only_when_idle() {
        let s = RequestScheduler::new(2);
        // P2 admitted when both slots free
        let _permit = s.try_admit(InfightPriority::P2).expect("idle → P2 admitted");
        // P2 rejected while one slot is occupied (not fully idle)
        assert!(s.try_admit(InfightPriority::P2).is_none());
    }

    #[tokio::test]
    async fn p2_rejected_when_p1_inflight() {
        let s = RequestScheduler::new(2);
        let _p1a = s.try_admit(InfightPriority::P1).expect("slot 1");
        // One slot occupied by P1 — P2 requires ALL slots free, so rejected
        assert!(s.try_admit(InfightPriority::P2).is_none());
    }

    #[tokio::test]
    async fn p0_not_starved_by_p2() {
        let s = RequestScheduler::new(2);
        let _p2 = s.try_admit(InfightPriority::P2).expect("idle");
        // P0 can still get the remaining slot
        assert!(s.try_admit(InfightPriority::P0).is_some());
    }
}
