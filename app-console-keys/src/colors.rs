//! os-console design-system palette (Phase C-1): "Graphite & Molten Cyan".
//!
//! The six original `tc_*` helpers keep their exact signatures, so every existing
//! caller is reskinned for free — only the truecolor RGB values are retuned. New
//! surface tokens (void/panel/accent-dim/text) are added for the cinematic shell.
//! Non-truecolor terminals continue to receive the named-color fallbacks.

use ratatui::style::Color;

pub fn tc_success(truecolor: bool) -> Color {
    if truecolor {
        Color::Rgb(0x5B, 0xD6, 0xA0)
    } else {
        Color::Green
    }
}

pub fn tc_error(truecolor: bool) -> Color {
    if truecolor {
        Color::Rgb(0xFF, 0x6B, 0x6B)
    } else {
        Color::Red
    }
}

pub fn tc_warn(truecolor: bool) -> Color {
    if truecolor {
        Color::Rgb(0xE8, 0xC4, 0x5A)
    } else {
        Color::Yellow
    }
}

pub fn tc_muted(truecolor: bool) -> Color {
    if truecolor {
        Color::Rgb(0x5A, 0x64, 0x70)
    } else {
        Color::DarkGray
    }
}

/// The single brand accent — molten cyan. Used sparingly (selection, active tab,
/// progress); the 10× look is restraint, one accent on screen at a time.
pub fn tc_accent(truecolor: bool) -> Color {
    if truecolor {
        Color::Rgb(0x3D, 0xD0, 0xE6)
    } else {
        Color::Cyan
    }
}

/// F12 "The Anchor" owns its own register — the commit/ingest gate is always
/// visually unmistakable.
pub fn tc_anchor(truecolor: bool) -> Color {
    if truecolor {
        Color::Rgb(0xB0, 0x5C, 0xFF)
    } else {
        Color::Magenta
    }
}

// --- New surface tokens (Phase C-1) ---

/// The deep matte base everything floats on.
pub fn tc_bg_void(truecolor: bool) -> Color {
    if truecolor {
        Color::Rgb(0x0B, 0x0D, 0x10)
    } else {
        Color::Black
    }
}

/// Raised surfaces / cards — one step up from the void.
pub fn tc_bg_panel(truecolor: bool) -> Color {
    if truecolor {
        Color::Rgb(0x14, 0x18, 0x1D)
    } else {
        Color::Black
    }
}

/// Trailing/ghost states of the accent (motion trails, streaming-token tail).
pub fn tc_accent_dim(truecolor: bool) -> Color {
    if truecolor {
        Color::Rgb(0x1C, 0x5C, 0x66)
    } else {
        Color::Cyan
    }
}

/// Primary body text — near-white, never pure white (pure white reads cheap).
pub fn tc_text(truecolor: bool) -> Color {
    if truecolor {
        Color::Rgb(0xE8, 0xEC, 0xF0)
    } else {
        Color::White
    }
}
