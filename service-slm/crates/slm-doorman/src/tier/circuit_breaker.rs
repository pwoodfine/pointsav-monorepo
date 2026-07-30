// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Three-state circuit breaker for Tier B (Yo-Yo) requests.
//!
//! States:
//!   Closed   — normal operation; failure counter increments on each failure
//!   Open     — 5+ consecutive failures; all requests rejected for 5-min cooldown
//!   HalfOpen — cooldown elapsed; one probe request allowed; closes on success,
//!              reopens on failure
//!
//! The breaker is shared via `Arc<CircuitBreaker>` between `YoYoTierClient::complete()`
//! (which drives state transitions from request outcomes) and the health probe
//! background task (which can observe but does not drive state directly).

use std::sync::RwLock;
use std::time::{Duration, Instant};
use tracing::{info, warn};

const FAILURE_THRESHOLD: u32 = 5;
const COOLDOWN: Duration = Duration::from_secs(300); // 5 minutes

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CbState {
    Closed,
    Open,
    HalfOpen,
}

struct Inner {
    state: CbState,
    failure_count: u32,
    opened_at: Option<Instant>,
}

pub struct CircuitBreaker {
    inner: RwLock<Inner>,
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        Self::new()
    }
}

impl CircuitBreaker {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(Inner {
                state: CbState::Closed,
                failure_count: 0,
                opened_at: None,
            }),
        }
    }

    /// Check whether a request should be allowed.
    ///
    /// - Closed / HalfOpen: allowed (returns true)
    /// - Open + still cooling: rejected (returns false)
    /// - Open + cooled down: transitions to HalfOpen, allows one probe (returns true)
    pub fn allow_request(&self) -> bool {
        let mut inner = self.inner.write().unwrap_or_else(|p| p.into_inner());
        match inner.state {
            CbState::Closed | CbState::HalfOpen => true,
            CbState::Open => {
                let cooled = inner
                    .opened_at
                    .map(|t| t.elapsed() >= COOLDOWN)
                    .unwrap_or(true);
                if cooled {
                    inner.state = CbState::HalfOpen;
                    info!(
                        target: "slm_doorman::tier::circuit_breaker",
                        "circuit half-open; allowing probe request"
                    );
                    true
                } else {
                    false
                }
            }
        }
    }

    /// Record a successful request. Resets failure count and closes the circuit.
    pub fn record_success(&self) {
        let mut inner = self.inner.write().unwrap_or_else(|p| p.into_inner());
        if inner.state != CbState::Closed {
            info!(
                target: "slm_doorman::tier::circuit_breaker",
                prev_state = ?inner.state,
                "circuit closed after successful request"
            );
        }
        inner.state = CbState::Closed;
        inner.failure_count = 0;
        inner.opened_at = None;
    }

    /// Record a failed request. Opens the circuit after `FAILURE_THRESHOLD`
    /// consecutive failures, or immediately if the circuit is HalfOpen.
    pub fn record_failure(&self) {
        let mut inner = self.inner.write().unwrap_or_else(|p| p.into_inner());
        inner.failure_count += 1;
        let should_open =
            inner.failure_count >= FAILURE_THRESHOLD || inner.state == CbState::HalfOpen;
        if should_open && inner.state != CbState::Open {
            warn!(
                target: "slm_doorman::tier::circuit_breaker",
                failure_count = inner.failure_count,
                "circuit opened after consecutive failures; cooling down for {} s",
                COOLDOWN.as_secs()
            );
            inner.state = CbState::Open;
            inner.opened_at = Some(Instant::now());
        }
    }

    /// Returns true when the circuit is currently Open (not HalfOpen).
    pub fn is_open(&self) -> bool {
        let inner = self.inner.read().unwrap_or_else(|p| p.into_inner());
        inner.state == CbState::Open
    }

    /// Returns the current state as a static label: "closed", "open", or "half_open".
    pub fn state_label(&self) -> &'static str {
        let inner = self.inner.read().unwrap_or_else(|p| p.into_inner());
        match inner.state {
            CbState::Closed => "closed",
            CbState::Open => "open",
            CbState::HalfOpen => "half_open",
        }
    }

    /// Returns the number of seconds since the circuit opened, or `None` if it is closed.
    pub fn opened_for_secs(&self) -> Option<u64> {
        let inner = self.inner.read().unwrap_or_else(|p| p.into_inner());
        inner.opened_at.map(|t| t.elapsed().as_secs())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_closed_and_allows_requests() {
        let cb = CircuitBreaker::new();
        assert!(cb.allow_request());
        assert!(!cb.is_open());
    }

    #[test]
    fn opens_after_failure_threshold() {
        let cb = CircuitBreaker::new();
        for _ in 0..FAILURE_THRESHOLD {
            assert!(cb.allow_request());
            cb.record_failure();
        }
        // Now open — should reject
        assert!(cb.is_open());
        assert!(!cb.allow_request());
    }

    #[test]
    fn closes_after_success() {
        let cb = CircuitBreaker::new();
        for _ in 0..FAILURE_THRESHOLD {
            cb.record_failure();
        }
        assert!(cb.is_open());
        cb.record_success();
        assert!(!cb.is_open());
        assert!(cb.allow_request());
    }

    #[test]
    fn half_open_probe_failure_reopens() {
        let cb = CircuitBreaker::new();
        for _ in 0..FAILURE_THRESHOLD {
            cb.record_failure();
        }
        assert!(cb.is_open());

        // Force HalfOpen by overriding opened_at to be in the past
        {
            let mut inner = cb.inner.write().unwrap();
            inner.opened_at = Some(Instant::now() - COOLDOWN - Duration::from_secs(1));
        }

        // allow_request() should transition to HalfOpen and return true
        assert!(cb.allow_request());
        assert!(!cb.is_open());

        // Failure from HalfOpen should immediately reopen
        cb.record_failure();
        assert!(cb.is_open());
    }
}
