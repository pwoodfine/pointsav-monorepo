//! The intent vocabulary: identifiers, scope, the canonical keyboard [`Chord`]
//! form, mouse affordances, parity waivers, and the [`IntentSpec`] a cartridge
//! declares for each verb it exposes.

use std::fmt;

/// Stable, namespaced identifier for an action, e.g. `IntentId("email.archive")`.
/// The string id is the parity anchor and the palette/help/keymap key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IntentId(pub &'static str);

impl fmt::Display for IntentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

/// Where an intent is in scope. `Cartridge` scope uses a stable string id so
/// this crate stays free of any UI/keymap/`FKey` dependency.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntentScope {
    /// Reachable from anywhere (switch cartridge, open palette, quit, …).
    Global,
    /// Reachable only when the named cartridge is focused.
    Cartridge(&'static str),
    /// Reachable within the focused pane (layout verbs).
    Pane,
}

/// Which mouse gestures may raise an intent. Bit flags; [`MouseAffordance::NONE`]
/// means the intent has no pointer affordance (it must then be keyboard-only
/// waived to pass the parity gate).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MouseAffordance(u8);

impl MouseAffordance {
    pub const NONE: Self = Self(0);
    pub const CLICK: Self = Self(1 << 0);
    pub const RIGHT_CLICK: Self = Self(1 << 1);
    pub const DRAG: Self = Self(1 << 2);
    pub const SCROLL: Self = Self(1 << 3);
    pub const HOVER: Self = Self(1 << 4);

    /// True if `self` includes every gesture in `other`.
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0 && other.0 != 0
    }
    pub const fn is_none(self) -> bool {
        self.0 == 0
    }
}

impl std::ops::BitOr for MouseAffordance {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

/// A keyboard binding in canonical text form, e.g. `"ctrl-k"`, `"enter"`,
/// `"f5"`, `"shift-tab"`. Front-ends translate raw key events into this form
/// before resolving against the [`crate::keymap::Keymap`], so this crate needs
/// no terminal dependency.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Chord(pub String);

impl Chord {
    pub fn new(s: impl AsRef<str>) -> Self {
        Self(normalize_chord(s.as_ref()))
    }
}

/// Normalize a chord string to canonical form: lower-case, modifiers ordered
/// `ctrl` < `alt` < `shift`, hyphen-joined, with the key last. Accepts `-` or
/// `+` separators and common modifier spellings.
pub fn normalize_chord(s: &str) -> String {
    let raw = s.trim().to_ascii_lowercase();
    let mut parts: Vec<&str> = raw
        .split(['-', '+'])
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .collect();
    if parts.is_empty() {
        return String::new();
    }
    let key = parts.pop().unwrap();
    let (mut ctrl, mut alt, mut shift) = (false, false, false);
    for p in &parts {
        match *p {
            "ctrl" | "control" => ctrl = true,
            "alt" | "meta" | "option" | "super" | "cmd" | "command" => alt = true,
            "shift" => shift = true,
            _ => {}
        }
    }
    let mut out = String::new();
    if ctrl {
        out.push_str("ctrl-");
    }
    if alt {
        out.push_str("alt-");
    }
    if shift {
        out.push_str("shift-");
    }
    out.push_str(key);
    out
}

/// Arguments accompanying a dispatched intent. Deliberately small for I-1;
/// richer payloads (transfers, pane-local coordinates) arrive with the data bus
/// in I-4.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IntentArgs {
    pub string: Option<String>,
    pub index: Option<usize>,
}

/// An explicit, reason-carrying exemption from one half of dual-input parity.
/// Every waiver is counted ([`crate::parity::waiver_count`]) and reviewable —
/// the number of waivers is a tracked metric, not a silent escape hatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Waiver {
    /// No sensible pointer gesture (e.g. a keyboard-only accelerator whose
    /// underlying actions remain mouse-reachable).
    KeyboardOnly(&'static str),
    /// No sensible keyboard equal (rare; e.g. dragging a split divider freely,
    /// where the discrete keyboard equivalent is a separate intent).
    MouseOnly(&'static str),
}

/// The declaration a cartridge makes for one verb. Built with the chained
/// setters; defaults are palette-visible, no keys, no mouse affordance, no
/// waiver (a bare spec therefore *fails* the parity gate until it is made
/// dual-input or explicitly waived — which is the point).
#[derive(Debug, Clone)]
pub struct IntentSpec {
    pub id: IntentId,
    pub title: &'static str,
    pub scope: IntentScope,
    pub default_keys: Vec<Chord>,
    pub mouse: MouseAffordance,
    /// Whether the command palette lists this intent. The palette is the
    /// universal keyboard floor: a palette-visible intent is keyboard-reachable
    /// even with zero `default_keys`.
    pub palette_visible: bool,
    /// Explicit parity exemption, if any.
    pub waiver: Option<Waiver>,
}

impl IntentSpec {
    pub fn new(id: &'static str, title: &'static str, scope: IntentScope) -> Self {
        Self {
            id: IntentId(id),
            title,
            scope,
            default_keys: Vec::new(),
            mouse: MouseAffordance::NONE,
            palette_visible: true,
            waiver: None,
        }
    }

    /// Add a default keyboard binding (canonical-normalized).
    pub fn key(mut self, chord: &str) -> Self {
        self.default_keys.push(Chord::new(chord));
        self
    }

    /// Set the pointer affordance(s) that may raise this intent.
    pub fn mouse(mut self, m: MouseAffordance) -> Self {
        self.mouse = m;
        self
    }

    /// Hide from the command palette (rare — removes the keyboard floor, so the
    /// intent then needs an explicit key binding or a `MouseOnly` waiver).
    pub fn no_palette(mut self) -> Self {
        self.palette_visible = false;
        self
    }

    /// Record an explicit parity waiver with a reason string.
    pub fn waive(mut self, w: Waiver) -> Self {
        self.waiver = Some(w);
        self
    }

    /// Reachable by keyboard if it is in the palette (the floor) or has a chord.
    pub fn keyboard_reachable(&self) -> bool {
        self.palette_visible || !self.default_keys.is_empty()
    }

    /// Reachable by mouse if it declares any pointer affordance.
    pub fn mouse_reachable(&self) -> bool {
        !self.mouse.is_none()
    }
}
