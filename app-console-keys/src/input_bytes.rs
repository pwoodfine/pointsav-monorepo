use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

/// Stateful parser: feed raw bytes one at a time; returns a crossterm Event
/// when a complete key sequence has been recognised.
pub struct ByteParser {
    buf: Vec<u8>,
}

impl Default for ByteParser {
    fn default() -> Self {
        Self::new()
    }
}

impl ByteParser {
    pub fn new() -> Self {
        Self { buf: Vec::new() }
    }

    pub fn push(&mut self, byte: u8) -> Option<Event> {
        self.buf.push(byte);
        self.try_parse()
    }

    fn try_parse(&mut self) -> Option<Event> {
        if self.buf.is_empty() {
            return None;
        }

        if self.buf[0] != 0x1b {
            // Single non-ESC byte
            let b = self.buf[0];
            self.buf.clear();
            return match b {
                0x03 => Some(key_mod(KeyCode::Char('c'), KeyModifiers::CONTROL)),
                0x0d => Some(key(KeyCode::Enter)),
                0x7f => Some(key(KeyCode::Backspace)),
                b if b.is_ascii_graphic() || b == b' ' => Some(key(KeyCode::Char(b as char))),
                _ => None,
            };
        }

        // ESC byte — need more context
        if self.buf.len() == 1 {
            return None;
        }

        // ESC O x — VT100 application keypad (F1-F4 variant)
        if self.buf[1] == b'O' {
            if self.buf.len() < 3 {
                return None;
            }
            let code = match self.buf[2] {
                b'P' => KeyCode::F(1),
                b'Q' => KeyCode::F(2),
                b'R' => KeyCode::F(3),
                b'S' => KeyCode::F(4),
                _ => {
                    self.buf.clear();
                    return None;
                }
            };
            self.buf.clear();
            return Some(key(code));
        }

        // ESC [ ... — CSI sequences
        if self.buf[1] == b'[' {
            let last = *self.buf.last().unwrap();
            let complete = last == b'~' || (last.is_ascii_uppercase() && self.buf.len() >= 3);
            if !complete {
                if self.buf.len() > 12 {
                    self.buf.clear();
                }
                return None;
            }

            let seq = self.buf.clone();
            self.buf.clear();

            if last == b'~' && seq.len() >= 4 {
                if let Ok(s) = std::str::from_utf8(&seq[2..seq.len() - 1]) {
                    if let Ok(n) = s.parse::<u8>() {
                        let code = match n {
                            11 => KeyCode::F(1),
                            12 => KeyCode::F(2),
                            13 => KeyCode::F(3),
                            14 => KeyCode::F(4),
                            15 => KeyCode::F(5),
                            17 => KeyCode::F(6),
                            18 => KeyCode::F(7),
                            19 => KeyCode::F(8),
                            20 => KeyCode::F(9),
                            21 => KeyCode::F(10),
                            23 => KeyCode::F(11),
                            24 => KeyCode::F(12),
                            _ => return None,
                        };
                        return Some(key(code));
                    }
                }
            }
            return None;
        }

        // Unrecognised or overlong sequence — discard
        if self.buf.len() > 8 {
            self.buf.clear();
        }
        None
    }
}

fn key(code: KeyCode) -> Event {
    Event::Key(KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    })
}

fn key_mod(code: KeyCode, modifiers: KeyModifiers) -> Event {
    Event::Key(KeyEvent {
        code,
        modifiers,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    })
}
