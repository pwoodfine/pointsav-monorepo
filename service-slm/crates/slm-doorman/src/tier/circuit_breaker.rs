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

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::RwLock;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tracing::{error, info, warn};

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

/// On-disk representation of breaker state, written after every state
/// transition and read once at process startup. `Instant` cannot survive a
/// process restart (it is a monotonic-clock reading with no fixed epoch), so
/// `opened_at` is persisted as Unix seconds and converted back to a
/// (backdated) `Instant` on load — see `CircuitBreaker::load_persisted`.
#[derive(Debug, Serialize, Deserialize)]
struct PersistedState {
    state: String, // "closed" | "open" | "half_open"
    failure_count: u32,
    opened_at_unix: Option<u64>,
}

pub struct CircuitBreaker {
    inner: RwLock<Inner>,
    /// Where to persist state across restarts. `None` = in-memory only
    /// (used by unit tests and any caller that hasn't opted in).
    persist_path: Option<PathBuf>,
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        Self::new()
    }
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl CircuitBreaker {
    /// In-memory only — state resets to Closed on every process restart.
    /// Kept for tests and any call site that doesn't need durability.
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(Inner {
                state: CbState::Closed,
                failure_count: 0,
                opened_at: None,
            }),
            persist_path: None,
        }
    }

    /// Loads prior state from `path` if present (fail-open to a fresh Closed
    /// breaker on any read/parse error — a missing or corrupt state file must
    /// never itself block requests). Every subsequent state transition is
    /// written back to `path`, so a `systemctl restart` no longer wipes a
    /// genuinely-open breaker back to "healthy" while the real upstream is
    /// still down (the bug: closed-by-default `new()` was the only
    /// constructor ever called, so every restart reset the breaker
    /// regardless of Tier B's actual last-known state).
    pub fn new_with_persistence(path: PathBuf) -> Self {
        let inner = Self::load_persisted(&path).unwrap_or(Inner {
            state: CbState::Closed,
            failure_count: 0,
            opened_at: None,
        });
        Self {
            inner: RwLock::new(inner),
            persist_path: Some(path),
        }
    }

    fn load_persisted(path: &PathBuf) -> Option<Inner> {
        let bytes = std::fs::read(path).ok()?;
        let persisted: PersistedState = serde_json::from_slice(&bytes)
            .map_err(|e| {
                warn!(
                    target: "slm_doorman::tier::circuit_breaker",
                    error = %e,
                    "circuit breaker state file unparsable; starting fresh (fail-open, not fail-closed)"
                );
                e
            })
            .ok()?;
        match persisted.state.as_str() {
            "open" => {
                // Reconstruct the Instant so the existing cooldown-elapsed
                // check in allow_request() runs unmodified: back-date it by
                // however much wall-clock time has actually passed. If that
                // already exceeds COOLDOWN, allow_request()'s own logic
                // transitions it to HalfOpen on the very next call — no
                // separate branch needed here.
                let elapsed_secs = unix_now().saturating_sub(persisted.opened_at_unix.unwrap_or(0));
                let opened_at = Instant::now().checked_sub(Duration::from_secs(elapsed_secs));
                info!(
                    target: "slm_doorman::tier::circuit_breaker",
                    elapsed_secs,
                    failure_count = persisted.failure_count,
                    "restored Open circuit state from disk across restart"
                );
                Some(Inner {
                    state: CbState::Open,
                    failure_count: persisted.failure_count,
                    opened_at,
                })
            }
            // Closed and HalfOpen both restore as Closed: HalfOpen is a
            // single-probe transient state that resolves within one request,
            // so collapsing it to Closed on restart is a safe simplification
            // — the alternative (persisting a mid-probe state) buys nothing.
            _ => Some(Inner {
                state: CbState::Closed,
                failure_count: 0,
                opened_at: None,
            }),
        }
    }

    fn persist(&self, inner: &Inner) {
        let Some(path) = &self.persist_path else {
            return;
        };
        let persisted = PersistedState {
            state: match inner.state {
                CbState::Closed => "closed",
                CbState::Open => "open",
                CbState::HalfOpen => "half_open",
            }
            .to_string(),
            failure_count: inner.failure_count,
            // Must be the absolute time the breaker actually opened, not
            // "now" — persist() runs on every transition, including repeat
            // failures while already Open, so recomputing unix_now() here
            // would perpetually push the recorded open-time forward and the
            // cooldown would never appear to elapse across a restart.
            opened_at_unix: inner
                .opened_at
                .map(|t| unix_now().saturating_sub(t.elapsed().as_secs())),
        };
        match serde_json::to_vec(&persisted) {
            Ok(bytes) => {
                if let Err(e) = std::fs::write(path, bytes) {
                    error!(
                        target: "slm_doorman::tier::circuit_breaker",
                        error = %e,
                        path = %path.display(),
                        "failed to persist circuit breaker state (non-fatal; in-memory state still correct until next restart)"
                    );
                }
            }
            Err(e) => {
                error!(target: "slm_doorman::tier::circuit_breaker", error = %e, "failed to serialize circuit breaker state");
            }
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
                    self.persist(&inner);
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
        self.persist(&inner);
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
        self.persist(&inner);
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

    /// The actual bug this session fixed: a breaker that is genuinely Open
    /// must stay Open across a process restart, not silently reset to
    /// Closed just because `new()` was called again.
    #[test]
    fn open_state_survives_simulated_restart() {
        let dir = std::env::temp_dir().join(format!(
            "cb_test_{}",
            std::time::SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("circuit_breaker_state.json");

        let cb1 = CircuitBreaker::new_with_persistence(path.clone());
        for _ in 0..FAILURE_THRESHOLD {
            cb1.record_failure();
        }
        assert!(cb1.is_open());
        drop(cb1); // simulates process exit — no in-memory state carries over

        // "Restart": a fresh CircuitBreaker loading from the same path must
        // still be Open, not Closed. This is precisely what the pre-fix code
        // got wrong — CircuitBreaker::new() was the only constructor ever
        // called, so every restart produced a fresh Closed breaker regardless
        // of the real upstream's state.
        let cb2 = CircuitBreaker::new_with_persistence(path.clone());
        assert!(cb2.is_open(), "Open state must survive a restart");
        assert!(
            !cb2.allow_request(),
            "still within cooldown — must keep rejecting"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    /// If the persisted Open state's cooldown already elapsed while the
    /// process was down, restart should land in HalfOpen (one probe
    /// allowed), not stay artificially Open forever.
    #[test]
    fn open_state_past_cooldown_resumes_as_half_open_on_restart() {
        let dir = std::env::temp_dir().join(format!(
            "cb_test_{}",
            std::time::SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
                + 1
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("circuit_breaker_state.json");

        // Write a persisted state directly whose opened_at_unix is already
        // well past COOLDOWN, simulating "process was down long enough that
        // the real cooldown window has since elapsed".
        let stale = PersistedState {
            state: "open".to_string(),
            failure_count: FAILURE_THRESHOLD,
            opened_at_unix: Some(unix_now().saturating_sub(COOLDOWN.as_secs() + 60)),
        };
        std::fs::write(&path, serde_json::to_vec(&stale).unwrap()).unwrap();

        let cb = CircuitBreaker::new_with_persistence(path.clone());
        assert!(cb.is_open()); // still reports Open until allow_request() is called once
        assert!(
            cb.allow_request(),
            "cooldown already elapsed — must allow the probe"
        );
        assert!(
            !cb.is_open(),
            "allow_request() should have transitioned to HalfOpen"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    /// A missing or unreadable state file must fail open to a fresh Closed
    /// breaker, never block startup or panic.
    #[test]
    fn missing_state_file_fails_open_to_closed() {
        let path = std::env::temp_dir().join(format!(
            "cb_test_nonexistent_{}.json",
            std::time::SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let cb = CircuitBreaker::new_with_persistence(path);
        assert!(!cb.is_open());
        assert!(cb.allow_request());
    }
}
