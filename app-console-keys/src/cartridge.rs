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
}
