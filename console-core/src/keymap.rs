//! Resolves a canonical keyboard [`Chord`] to an [`IntentId`], honoring scope
//! precedence (pane → focused cartridge → global). Built from the registry's
//! `default_keys` plus optional user/policy overrides.

use std::collections::HashMap;

use crate::intent::{Chord, IntentId, IntentScope};
use crate::registry::IntentRegistry;

#[derive(Debug, Default, Clone)]
pub struct Keymap {
    /// canonical chord string -> candidate (scope, intent) bindings
    bindings: HashMap<String, Vec<(IntentScope, IntentId)>>,
}

impl Keymap {
    /// Build a keymap from every intent's declared default keys.
    pub fn from_registry(reg: &IntentRegistry) -> Self {
        let mut km = Keymap::default();
        for spec in reg.iter() {
            for chord in &spec.default_keys {
                km.bindings
                    .entry(chord.0.clone())
                    .or_default()
                    .push((spec.scope, spec.id));
            }
        }
        km
    }

    /// Add or override a binding at runtime (user/policy keymap).
    pub fn bind(&mut self, chord: &str, scope: IntentScope, id: IntentId) {
        self.bindings
            .entry(Chord::new(chord).0)
            .or_default()
            .push((scope, id));
    }

    /// Resolve a chord for the currently focused cartridge. Precedence:
    /// `Pane` > `Cartridge(focused)` > `Global`. A cartridge-scoped binding for a
    /// cartridge that is not focused does not match.
    pub fn resolve(&self, chord: &str, focused: Option<&str>) -> Option<IntentId> {
        let key = Chord::new(chord).0;
        let cands = self.bindings.get(&key)?;
        let mut best: Option<(u8, IntentId)> = None;
        for (sc, id) in cands {
            let rank = match sc {
                IntentScope::Pane => 0u8,
                IntentScope::Cartridge(c) => {
                    if focused == Some(*c) {
                        1
                    } else {
                        continue;
                    }
                }
                IntentScope::Global => 2,
            };
            if best.is_none_or(|(r, _)| rank < r) {
                best = Some((rank, *id));
            }
        }
        best.map(|(_, id)| id)
    }
}
