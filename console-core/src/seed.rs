//! The seed intent vocabulary for os-console: global verbs, pane/layout verbs,
//! and the core verbs of the four demo-slice cartridges (Search = F5, Content =
//! F4 Proofreader, System = F11 Admin, Input = F12 Anchor). The chassis builds
//! its live registry on top of this; cartridges contribute their own
//! `intents()` as they are migrated. Every entry here is dual-input by
//! construction, so [`crate::parity::audit`] of this registry is empty.

use crate::intent::{IntentScope, IntentSpec, MouseAffordance, Waiver};
use crate::registry::IntentRegistry;

const CLICK: MouseAffordance = MouseAffordance::CLICK;
const RCLICK: MouseAffordance = MouseAffordance::RIGHT_CLICK;
const DRAG: MouseAffordance = MouseAffordance::DRAG;
const HOVER: MouseAffordance = MouseAffordance::HOVER;

/// Cartridge scope ids (stable strings; map 1:1 to the live cartridges).
pub const SEARCH: IntentScope = IntentScope::Cartridge("search");
pub const CONTENT: IntentScope = IntentScope::Cartridge("content");
pub const SYSTEM: IntentScope = IntentScope::Cartridge("system");
pub const INPUT: IntentScope = IntentScope::Cartridge("input");

/// Build the seed registry. This is the canonical starting set; it passes the
/// parity gate and is the foundation the chassis extends at startup.
pub fn console_seed() -> IntentRegistry {
    use IntentScope::{Global, Pane};
    let mut r = IntentRegistry::new();

    // --- Global ---
    r.register(IntentSpec::new("console.palette", "Open command palette", Global).key("ctrl-k").mouse(CLICK));
    r.register(IntentSpec::new("console.help", "Help", Global).key("f1").mouse(CLICK));
    r.register(IntentSpec::new("console.quit", "Quit os-console", Global).key("ctrl-q").mouse(RCLICK));
    r.register(IntentSpec::new("console.capability_mode", "Toggle capability mode", Global).key("ctrl-g").mouse(CLICK));
    r.register(
        IntentSpec::new("console.repeat_last", "Repeat last action", Global)
            .key(".")
            .waive(Waiver::KeyboardOnly("accelerator; the underlying actions remain mouse-reachable")),
    );

    // --- Cartridge switching (Global) ---
    r.register(IntentSpec::new("view.switch.search", "Go to Search (F5)", Global).key("f5").mouse(CLICK));
    r.register(IntentSpec::new("view.switch.content", "Go to Content (F4)", Global).key("f4").mouse(CLICK));
    r.register(IntentSpec::new("view.switch.system", "Go to System (F11)", Global).key("f11").mouse(CLICK));

    // --- Pane / layout ---
    r.register(IntentSpec::new("pane.focus.left", "Focus pane left", Pane).key("ctrl-h").mouse(CLICK));
    r.register(IntentSpec::new("pane.focus.right", "Focus pane right", Pane).key("ctrl-l").mouse(CLICK));
    r.register(IntentSpec::new("pane.split.v", "Split pane vertically", Pane).key("ctrl-\\").mouse(RCLICK));
    r.register(IntentSpec::new("pane.close", "Close pane", Pane).key("ctrl-w").mouse(RCLICK));
    r.register(IntentSpec::new("pane.resize", "Resize pane", Pane).key("ctrl-=").mouse(RCLICK));
    r.register(
        IntentSpec::new("pane.resize.drag", "Drag split divider", Pane)
            .no_palette()
            .mouse(DRAG)
            .waive(Waiver::MouseOnly("continuous drag; the keyboard equivalent is pane.resize")),
    );

    // --- Search (F5) ---
    r.register(IntentSpec::new("search.run", "Run search", SEARCH).key("enter").mouse(CLICK));
    r.register(IntentSpec::new("search.open", "Open result", SEARCH).key("o").mouse(CLICK));
    r.register(IntentSpec::new("search.send_to_proofreader", "Send result to Proofreader", SEARCH).key("s").mouse(DRAG | RCLICK));
    r.register(IntentSpec::new("search.stage_to_desk", "Stage result to Desk", SEARCH).key("d").mouse(DRAG | RCLICK));

    // --- Content / Proofreader (F4) ---
    r.register(IntentSpec::new("content.submit", "Submit for proofread", CONTENT).key("ctrl-s").mouse(CLICK));
    r.register(IntentSpec::new("content.accept", "Accept suggestion", CONTENT).key("a").mouse(CLICK));
    r.register(IntentSpec::new("content.reject", "Reject suggestion", CONTENT).key("r").mouse(CLICK));
    r.register(IntentSpec::new("content.accept_all", "Accept all", CONTENT).key("shift-a").mouse(CLICK));
    r.register(IntentSpec::new("content.draft_new", "New draft", CONTENT).key("ctrl-n").mouse(CLICK));

    // --- System / Admin (F11) ---
    r.register(IntentSpec::new("system.approve", "Approve pending device", SYSTEM).key("enter").mouse(CLICK));
    r.register(IntentSpec::new("system.deny", "Deny pending device", SYSTEM).key("d").mouse(CLICK));
    r.register(IntentSpec::new("system.revoke", "Revoke device", SYSTEM).key("x").mouse(RCLICK));
    r.register(IntentSpec::new("system.show_fingerprint", "Show fingerprint", SYSTEM).key("?").mouse(CLICK | HOVER));

    // --- Input Machine / The Anchor (F12) ---
    // The Anchor is a global pre-empt; ingest verbs live only under this cartridge.
    r.register(IntentSpec::new("input.anchor.open", "Open the Input Machine (F12)", Global).key("f12").mouse(CLICK));
    r.register(IntentSpec::new("input.confirm", "Confirm ingest", INPUT).key("enter").mouse(CLICK));
    r.register(IntentSpec::new("input.cancel", "Cancel ingest", INPUT).key("esc").mouse(CLICK));
    r.register(IntentSpec::new("input.audit", "View audit ledger", INPUT).key("ctrl-a").mouse(CLICK));

    r
}
