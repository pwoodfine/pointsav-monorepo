//! The merged intent registry. The chassis builds one of these at startup from
//! every cartridge's declared intents plus its own global/pane verbs. Context
//! menus, the command palette, the help screen, and the keymap are all derived
//! *from* this registry, so they cannot drift from the real action set.

use crate::intent::{IntentId, IntentScope, IntentSpec, MouseAffordance};

#[derive(Debug, Default, Clone)]
pub struct IntentRegistry {
    specs: Vec<IntentSpec>,
}

impl IntentRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a spec. If an intent with the same id already exists, it is
    /// replaced (allows user/policy overrides to win over defaults).
    pub fn register(&mut self, spec: IntentSpec) -> &mut Self {
        if let Some(existing) = self.specs.iter_mut().find(|s| s.id == spec.id) {
            *existing = spec;
        } else {
            self.specs.push(spec);
        }
        self
    }

    pub fn extend<I: IntoIterator<Item = IntentSpec>>(&mut self, it: I) -> &mut Self {
        for s in it {
            self.register(s);
        }
        self
    }

    pub fn get(&self, id: IntentId) -> Option<&IntentSpec> {
        self.specs.iter().find(|s| s.id == id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &IntentSpec> {
        self.specs.iter()
    }

    pub fn len(&self) -> usize {
        self.specs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.specs.is_empty()
    }

    /// Command-palette entries (the keyboard floor): every palette-visible
    /// intent in scope for the focused cartridge.
    pub fn palette_entries(&self, focused: Option<&str>) -> Vec<&IntentSpec> {
        self.specs
            .iter()
            .filter(|s| s.palette_visible && in_scope(s.scope, focused))
            .collect()
    }

    /// Context-menu entries (the mouse path): intents whose declared affordance
    /// includes `gesture` and whose scope matches the focused cartridge. This is
    /// exactly the same data the palette draws from — the menu is a *view* of
    /// the registry, so it can never list an action the keyboard cannot reach.
    pub fn context_for(&self, gesture: MouseAffordance, focused: Option<&str>) -> Vec<&IntentSpec> {
        self.specs
            .iter()
            .filter(|s| s.mouse.contains(gesture) && in_scope(s.scope, focused))
            .collect()
    }
}

fn in_scope(scope: IntentScope, focused: Option<&str>) -> bool {
    match scope {
        IntentScope::Global | IntentScope::Pane => true,
        IntentScope::Cartridge(c) => focused == Some(c),
    }
}
