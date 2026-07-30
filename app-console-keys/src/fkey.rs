use crossterm::event::KeyCode;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FKey {
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
}

impl FKey {
    pub fn label(self) -> &'static str {
        match self {
            FKey::F1 => "F1: Help",
            FKey::F2 => "F2: People",
            FKey::F3 => "F3: Email",
            FKey::F4 => "F4: Content",
            FKey::F5 => "F5: Minutes",
            FKey::F6 => "F6: Bookkeeper",
            FKey::F7 => "F7: BIM",
            FKey::F8 => "F8: GIS",
            FKey::F9 => "F9: SLM",
            FKey::F10 => "F10: Mesh",
            FKey::F11 => "F11: System",
            FKey::F12 => "F12: Input",
        }
    }

    pub fn short(self) -> &'static str {
        match self {
            FKey::F1 => "F1",
            FKey::F2 => "F2",
            FKey::F3 => "F3",
            FKey::F4 => "F4",
            FKey::F5 => "F5",
            FKey::F6 => "F6",
            FKey::F7 => "F7",
            FKey::F8 => "F8",
            FKey::F9 => "F9",
            FKey::F10 => "F10",
            FKey::F11 => "F11",
            FKey::F12 => "F12",
        }
    }

    pub fn from_keycode(code: KeyCode) -> Option<Self> {
        match code {
            KeyCode::F(1) => Some(FKey::F1),
            KeyCode::F(2) => Some(FKey::F2),
            KeyCode::F(3) => Some(FKey::F3),
            KeyCode::F(4) => Some(FKey::F4),
            KeyCode::F(5) => Some(FKey::F5),
            KeyCode::F(6) => Some(FKey::F6),
            KeyCode::F(7) => Some(FKey::F7),
            KeyCode::F(8) => Some(FKey::F8),
            KeyCode::F(9) => Some(FKey::F9),
            KeyCode::F(10) => Some(FKey::F10),
            KeyCode::F(11) => Some(FKey::F11),
            KeyCode::F(12) => Some(FKey::F12),
            _ => None,
        }
    }

    pub fn all() -> [FKey; 12] {
        [
            FKey::F1,
            FKey::F2,
            FKey::F3,
            FKey::F4,
            FKey::F5,
            FKey::F6,
            FKey::F7,
            FKey::F8,
            FKey::F9,
            FKey::F10,
            FKey::F11,
            FKey::F12,
        ]
    }
}
