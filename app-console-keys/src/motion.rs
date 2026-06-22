//! The motion engine (Phase C-1).
//!
//! Pure, deterministic easing sampled by the chassis's existing ~16ms redraw
//! loop — so 60fps animation costs no new event loop. Animations are sampled by
//! an explicit `elapsed_ms` (the caller tracks the start `Instant` and passes the
//! elapsed time), which keeps this module free of wall-clock state and fully
//! unit-testable.
//!
//! Motion-as-meaning vocabulary (see `BRIEF-os-console-rebuild-2030` Layer 3):
//! Settle (arriving panels), Verdict-pop (confident landing, slight overshoot),
//! Sweep (reveals), Anchor-charge (the F12 hold-to-commit), Pulse (ambient
//! breathing). Anxious fast spinners are deliberately not part of the vocabulary.

use std::f32::consts::TAU;

/// Easing curves over normalized time `t ∈ [0,1]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ease {
    Linear,
    /// Decelerating — the default for things arriving deliberately.
    OutCubic,
    /// Ease in and out — for ambient loops.
    InOutCubic,
    /// Overshoot then settle — confident "pop".
    OutBack,
    /// Sharp decelerating reveal.
    OutExpo,
}

impl Ease {
    /// Apply the curve to a normalized `t` (clamped to `[0,1]`). May return
    /// values slightly outside `[0,1]` for `OutBack` (intentional overshoot).
    pub fn apply(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Ease::Linear => t,
            Ease::OutCubic => 1.0 - (1.0 - t).powi(3),
            Ease::InOutCubic => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
                }
            }
            Ease::OutBack => {
                let c1 = 1.701_58_f32;
                let c3 = c1 + 1.0;
                1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
            }
            Ease::OutExpo => {
                if t >= 1.0 {
                    1.0
                } else {
                    1.0 - 2f32.powf(-10.0 * t)
                }
            }
        }
    }
}

/// A one-shot interpolation from `from` to `to` over `dur_ms`, eased.
#[derive(Debug, Clone, Copy)]
pub struct Anim {
    pub from: f32,
    pub to: f32,
    pub dur_ms: u64,
    pub ease: Ease,
}

impl Anim {
    pub fn new(from: f32, to: f32, dur_ms: u64, ease: Ease) -> Self {
        Self {
            from,
            to,
            dur_ms,
            ease,
        }
    }

    /// A panel/card arriving (slide + fade), 220ms decelerating.
    pub fn settle(from: f32, to: f32) -> Self {
        Self::new(from, to, 220, Ease::OutCubic)
    }

    /// A verdict card landing, 180ms with a small confident overshoot.
    pub fn verdict_pop() -> Self {
        Self::new(0.0, 1.0, 180, Ease::OutBack)
    }

    /// A reveal sweep (boot wordmark, search results), 400ms.
    pub fn sweep() -> Self {
        Self::new(0.0, 1.0, 400, Ease::OutExpo)
    }

    /// The F12 hold-to-commit charge — 700ms of deliberate weight.
    pub fn anchor_charge() -> Self {
        Self::new(0.0, 1.0, 700, Ease::OutCubic)
    }

    /// The eased value at `elapsed_ms`. Returns `to` once complete (or for a
    /// zero-duration animation).
    pub fn value(&self, elapsed_ms: u64) -> f32 {
        if self.dur_ms == 0 {
            return self.to;
        }
        let t = (elapsed_ms as f32 / self.dur_ms as f32).clamp(0.0, 1.0);
        self.from + (self.to - self.from) * self.ease.apply(t)
    }

    pub fn done(&self, elapsed_ms: u64) -> bool {
        elapsed_ms >= self.dur_ms
    }
}

/// Ambient breathing in `[0,1]` with the given period — slow reads as patience.
/// Use for waiting/standby states (never a fast spinner).
pub fn pulse(elapsed_ms: u64, period_ms: u64) -> f32 {
    if period_ms == 0 {
        return 0.0;
    }
    let phase = (elapsed_ms % period_ms) as f32 / period_ms as f32;
    0.5 - 0.5 * (phase * TAU).cos()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ease_endpoints() {
        for e in [
            Ease::Linear,
            Ease::OutCubic,
            Ease::InOutCubic,
            Ease::OutBack,
            Ease::OutExpo,
        ] {
            assert!((e.apply(0.0) - 0.0).abs() < 1e-4, "{e:?} at 0");
            assert!((e.apply(1.0) - 1.0).abs() < 1e-4, "{e:?} at 1");
        }
    }

    #[test]
    fn anim_value_endpoints_and_clamp() {
        let a = Anim::settle(0.0, 10.0);
        assert!((a.value(0) - 0.0).abs() < 1e-4);
        assert!((a.value(220) - 10.0).abs() < 1e-4);
        assert!((a.value(99999) - 10.0).abs() < 1e-4); // clamps past duration
        assert!(a.done(220));
        assert!(!a.done(100));
        // monotonic-ish midpoint stays within range for OutCubic
        let mid = a.value(110);
        assert!(mid > 0.0 && mid < 10.0);
    }

    #[test]
    fn zero_duration_is_immediate() {
        let a = Anim::new(3.0, 7.0, 0, Ease::Linear);
        assert!((a.value(0) - 7.0).abs() < 1e-4);
        assert!(a.done(0));
    }

    #[test]
    fn pulse_stays_in_unit_range() {
        for ms in [0u64, 100, 350, 700, 900, 1400, 2000] {
            let v = pulse(ms, 1400);
            assert!((0.0..=1.0).contains(&v), "pulse {ms} = {v}");
        }
        assert!(pulse(0, 1400) < 1e-4); // starts at trough
    }
}
