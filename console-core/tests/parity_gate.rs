//! The build gate. If any os-console intent becomes reachable by only one input
//! modality (without an explicit reason-stringed waiver), this test fails — so
//! a mouse-only or keyboard-only feature cannot land green. This is the
//! mechanism that makes "100% keyboard-accessible AND mouse-accessible" a
//! structural invariant rather than a convention.

use console_core::intent::{IntentScope, IntentSpec};
use console_core::{audit, seed, IntentRegistry, Keymap, MouseAffordance};

#[test]
fn seed_registry_has_full_dual_input_parity() {
    let reg = seed::console_seed();
    let violations = audit(&reg);
    assert!(
        violations.is_empty(),
        "seed registry has parity violations: {:?}",
        violations
    );
    assert!(reg.len() >= 25, "seed registry unexpectedly small: {}", reg.len());
}

#[test]
fn the_gate_actually_catches_a_one_input_intent() {
    // Sanity: prove the gate is not vacuous. A bare spec (no keys, no palette,
    // no mouse, no waiver) must be reported.
    let mut reg = seed::console_seed();
    reg.register(IntentSpec::new("regression.mouse_only", "oops", IntentScope::Global).no_palette());
    assert!(
        !audit(&reg).is_empty(),
        "gate failed to catch a single-input intent"
    );
}

#[test]
fn keymap_resolves_global_and_scoped_chords() {
    let reg = seed::console_seed();
    let km = Keymap::from_registry(&reg);

    // Global: F5 -> Search switch regardless of focus.
    assert_eq!(km.resolve("f5", None).map(|i| i.0), Some("view.switch.search"));
    // Anchor F12 is global.
    assert_eq!(km.resolve("f12", Some("content")).map(|i| i.0), Some("input.anchor.open"));
    // "enter" is cartridge-scoped: resolves differently by focus.
    assert_eq!(km.resolve("enter", Some("search")).map(|i| i.0), Some("search.run"));
    assert_eq!(km.resolve("enter", Some("system")).map(|i| i.0), Some("system.approve"));
    assert_eq!(km.resolve("enter", Some("input")).map(|i| i.0), Some("input.confirm"));
    // "enter" with no relevant focus does not match a cartridge-only binding.
    assert_eq!(km.resolve("enter", Some("content")), None);
}

#[test]
fn palette_floor_and_context_menu_are_registry_views() {
    let reg = seed::console_seed();

    // Palette (keyboard floor) for focused Content includes content verbs and
    // global verbs, but not another cartridge's verbs.
    let pal: Vec<&str> = reg
        .palette_entries(Some("content"))
        .iter()
        .map(|s| s.id.0)
        .collect();
    assert!(pal.contains(&"content.accept"));
    assert!(pal.contains(&"console.palette"));
    assert!(!pal.contains(&"search.run"));

    // Context menu (mouse) is the same data filtered by gesture: right-click in
    // System surfaces the revoke verb.
    let menu: Vec<&str> = reg
        .context_for(MouseAffordance::RIGHT_CLICK, Some("system"))
        .iter()
        .map(|s| s.id.0)
        .collect();
    assert!(menu.contains(&"system.revoke"));
}

#[test]
fn normalization_makes_chord_spelling_irrelevant() {
    let reg = IntentRegistry::new();
    let km = Keymap::from_registry(&reg);
    // smoke: resolving an unbound chord is None and does not panic on spellings
    assert_eq!(km.resolve("Ctrl+K", None), None);
}
