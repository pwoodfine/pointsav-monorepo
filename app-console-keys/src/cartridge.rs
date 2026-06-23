use console_core::{IntentArgs, IntentId, IntentSpec};
use crossterm::event::Event;
use ratatui::{layout::Rect, Frame};

use crate::fkey::FKey;

pub enum CartridgeAction {
    None,
    Consumed,
    Quit,
    GoBack,
}

pub trait Cartridge: Send {
    fn fkey(&self) -> FKey;
    fn title(&self) -> &str;
    fn is_installed(&self) -> bool {
        true
    }
    /// Called every frame before render; drain background channels into local state.
    fn tick(&mut self) {}
    /// Non-zero if this cartridge has a badge count (e.g. pending items).
    fn pending_badge(&self) -> u16 {
        0
    }
    /// Called once by the chassis after terminal graphics capabilities are probed
    /// (local PTY mode only; not called over russh). `font_size` is the terminal's
    /// cell size in `(width, height)` pixels — needed to size pixel images correctly.
    /// `truecolor` is true when `COLORTERM=truecolor|24bit` — cartridges may use
    /// `Color::Rgb` instead of named/indexed colors when this is set.
    /// Cartridges that render pixel graphics override this to store the caps.
    fn set_graphics_caps(
        &mut self,
        _kitty: bool,
        _sixel: bool,
        _font_size: (u16, u16),
        _truecolor: bool,
    ) {
    }
    fn render(&mut self, frame: &mut Frame, area: Rect);
    fn handle_event(&mut self, event: &Event) -> CartridgeAction;
    /// Called by the chassis after each terminal.draw() to emit OSC 8 hyperlinks
    /// for any rendered links. Default no-op; override in cartridges that render links.
    fn flush_hyperlinks(&self) {}

    /// Live capability verdicts for the `?` capability overlay (Phase K). Default: empty.
    /// Returns `(label, verdict)` pairs where verdict is "✓ ALLOW", "✗ REVOKED", or "⟳ EXPIRED".
    /// SystemCartridge overrides this with real ledger verdicts.
    fn cap_verdicts(&self) -> Vec<(String, String)> {
        Vec::new()
    }

    // --- Intent system (Phase I-1; additive, all defaulted) ---

    /// Stable scope id for the dual-input intent system (e.g. `"system"`). A
    /// cartridge that returns `Some(..)` participates in the command palette and
    /// intent dispatch under that scope. `None` (default) = not yet migrated; the
    /// cartridge still works via legacy `handle_event`.
    fn intent_scope(&self) -> Option<&'static str> {
        None
    }

    /// Intents this cartridge contributes to the registry. Default: none (the
    /// seed vocabulary in `console-core` still covers the cartridge). Anything
    /// returned here must be dual-input or the parity gate fails the build.
    fn intents(&self) -> Vec<IntentSpec> {
        Vec::new()
    }

    /// Act on a resolved intent (raised by either a keyboard chord via the
    /// command palette/keymap or, from I-2, a mouse gesture). Default: not
    /// handled, so the chassis leaves behavior to legacy `handle_event`.
    fn dispatch(&mut self, _id: IntentId, _args: &IntentArgs) -> CartridgeAction {
        CartridgeAction::None
    }
}
