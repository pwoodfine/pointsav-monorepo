//! The structural dual-input guarantee.
//!
//! [`audit`] returns every intent that is not reachable by *both* the keyboard
//! and the mouse (unless explicitly, reason-stringed, waived). An empty result
//! is the build gate's pass condition. Wiring this into CI (see the crate's
//! `tests/`) is what makes "100% keyboard-accessible AND mouse-accessible" an
//! invariant the compiler/CI hold, not a convention humans must remember.

use crate::intent::{IntentId, Waiver};
use crate::registry::IntentRegistry;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParityFault {
    /// Not in the palette and has no key binding, and not `MouseOnly`-waived.
    NotKeyboardReachable,
    /// No pointer affordance, and not `KeyboardOnly`-waived.
    NotMouseReachable,
}

#[derive(Debug, Clone, Copy)]
pub struct ParityViolation {
    pub id: IntentId,
    pub fault: ParityFault,
}

/// Audit the registry for dual-input parity. Returns one [`ParityViolation`]
/// per failing modality per intent. Empty == the gate passes.
pub fn audit(reg: &IntentRegistry) -> Vec<ParityViolation> {
    let mut out = Vec::new();
    for s in reg.iter() {
        let kb_ok = s.keyboard_reachable() || matches!(s.waiver, Some(Waiver::MouseOnly(_)));
        if !kb_ok {
            out.push(ParityViolation {
                id: s.id,
                fault: ParityFault::NotKeyboardReachable,
            });
        }
        let mouse_ok = s.mouse_reachable() || matches!(s.waiver, Some(Waiver::KeyboardOnly(_)));
        if !mouse_ok {
            out.push(ParityViolation {
                id: s.id,
                fault: ParityFault::NotMouseReachable,
            });
        }
    }
    out
}

/// Number of intents carrying a parity waiver — a tracked metric. A growing
/// count is a review signal, not an error.
pub fn waiver_count(reg: &IntentRegistry) -> usize {
    reg.iter().filter(|s| s.waiver.is_some()).count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intent::{IntentScope, IntentSpec, MouseAffordance, Waiver};
    use crate::registry::IntentRegistry;

    #[test]
    fn clean_dual_input_spec_passes() {
        let mut reg = IntentRegistry::new();
        reg.register(
            IntentSpec::new("content.accept", "Accept suggestion", IntentScope::Cartridge("content"))
                .key("a")
                .mouse(MouseAffordance::CLICK),
        );
        assert!(audit(&reg).is_empty());
    }

    #[test]
    fn bare_spec_fails_both_legs() {
        let mut reg = IntentRegistry::new();
        // no keys, no palette, no mouse, no waiver -> fails both
        reg.register(
            IntentSpec::new("bad.intent", "Bad", IntentScope::Global).no_palette(),
        );
        let v = audit(&reg);
        assert_eq!(v.len(), 2);
        assert!(v.iter().any(|x| x.fault == ParityFault::NotKeyboardReachable));
        assert!(v.iter().any(|x| x.fault == ParityFault::NotMouseReachable));
    }

    #[test]
    fn palette_is_the_keyboard_floor() {
        let mut reg = IntentRegistry::new();
        // palette-visible, no chord, has mouse -> keyboard reachable via palette
        reg.register(
            IntentSpec::new("x.y", "X Y", IntentScope::Global).mouse(MouseAffordance::CLICK),
        );
        assert!(audit(&reg).is_empty());
    }

    #[test]
    fn keyboard_only_waiver_excuses_missing_mouse() {
        let mut reg = IntentRegistry::new();
        reg.register(
            IntentSpec::new("console.repeat_last", "Repeat last action", IntentScope::Global)
                .key(".")
                .waive(Waiver::KeyboardOnly("accelerator; underlying actions stay mouse-reachable")),
        );
        assert!(audit(&reg).is_empty());
        assert_eq!(waiver_count(&reg), 1);
    }

    #[test]
    fn mouse_only_waiver_excuses_missing_keyboard() {
        let mut reg = IntentRegistry::new();
        reg.register(
            IntentSpec::new("pane.resize.drag", "Drag split divider", IntentScope::Pane)
                .no_palette()
                .mouse(MouseAffordance::DRAG)
                .waive(Waiver::MouseOnly("continuous drag; keyboard uses pane.resize")),
        );
        assert!(audit(&reg).is_empty());
    }
}
