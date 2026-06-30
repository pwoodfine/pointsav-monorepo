use std::sync::mpsc;
use std::thread;

use sha2::{Digest, Sha256};

use app_console_keys::{
    Cartridge, CartridgeAction, FKey, IntentArgs, IntentId, IntentScope, IntentSpec,
    MouseAffordance,
};
use crossterm::event::{Event, KeyCode, KeyModifiers};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::{
    audit::{self, IngestRecord},
    ingest::{self, IngestResult},
};

const SPINNER: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

// ── Single-line path input ───────────────────────────────────────────────────

struct PathInput {
    text: String,
    cursor: usize,
}

impl PathInput {
    fn new() -> Self {
        Self {
            text: String::new(),
            cursor: 0,
        }
    }

    fn handle_key(&mut self, key: &crossterm::event::KeyEvent) -> Option<PathInputAction> {
        match key.code {
            KeyCode::Esc => return Some(PathInputAction::Cancel),
            KeyCode::Enter => {
                let t = self.text.trim().to_string();
                if !t.is_empty() {
                    return Some(PathInputAction::Submit(t));
                }
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.text.insert(self.cursor, c);
                self.cursor += c.len_utf8();
            }
            KeyCode::Backspace if self.cursor > 0 => {
                let len = self.text[..self.cursor]
                    .chars()
                    .last()
                    .map(|c| c.len_utf8())
                    .unwrap_or(1);
                self.cursor -= len;
                self.text.remove(self.cursor);
            }
            KeyCode::Delete if self.cursor < self.text.len() => {
                self.text.remove(self.cursor);
            }
            KeyCode::Left if self.cursor > 0 => {
                let len = self.text[..self.cursor]
                    .chars()
                    .last()
                    .map(|c| c.len_utf8())
                    .unwrap_or(1);
                self.cursor -= len;
            }
            KeyCode::Right if self.cursor < self.text.len() => {
                let len = self.text[self.cursor..]
                    .chars()
                    .next()
                    .map(|c| c.len_utf8())
                    .unwrap_or(1);
                self.cursor += len;
            }
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = self.text.len(),
            _ => {}
        }
        None
    }

    fn render_into(&self, frame: &mut Frame, area: Rect) {
        let before = &self.text[..self.cursor];
        let cursor_char = self.text[self.cursor..].chars().next().unwrap_or(' ');
        let after = if self.cursor < self.text.len() {
            &self.text[self.cursor + cursor_char.len_utf8()..]
        } else {
            ""
        };
        let line = Line::from(vec![
            Span::raw(before.to_string()),
            Span::styled(
                cursor_char.to_string(),
                Style::default().fg(Color::Black).bg(Color::White),
            ),
            Span::raw(after.to_string()),
        ]);
        frame.render_widget(
            Paragraph::new(line).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::White)),
            ),
            area,
        );
    }
}

enum PathInputAction {
    Submit(String),
    Cancel,
}

// ── State machine ─────────────────────────────────────────────────────────────

enum InputState {
    Entry,
    Confirm {
        path: String,
    },
    Submitting {
        path: String,
        spinner: usize,
        rx: mpsc::Receiver<anyhow::Result<IngestResult>>,
    },
    Done {
        path: String,
        result: IngestResult,
    },
    AuditLog {
        records: Vec<IngestRecord>,
        scroll: u16,
    },
    Error {
        message: String,
    },
}

// ── InputCartridge ────────────────────────────────────────────────────────────

pub struct InputCartridge {
    username: String,
    tenant: String,
    ingest_endpoint: String,
    state: InputState,
    path_input: PathInput,
    truecolor: bool,
    ledger_root: [u8; 32],
    ledger_height: u64,
}

impl InputCartridge {
    pub fn new() -> Self {
        Self::new_for("operator", "local", "http://127.0.0.1:9106")
    }

    pub fn new_for(
        username: impl Into<String>,
        tenant: impl Into<String>,
        ingest_endpoint: impl Into<String>,
    ) -> Self {
        Self {
            username: username.into(),
            tenant: tenant.into(),
            ingest_endpoint: ingest_endpoint.into(),
            state: InputState::Entry,
            path_input: PathInput::new(),
            truecolor: false,
            ledger_root: [0u8; 32],
            ledger_height: 0,
        }
    }

    fn reset(&mut self) {
        self.state = InputState::Entry;
        self.path_input = PathInput::new();
    }

    fn render_modal(frame: &mut Frame, area: Rect) -> Rect {
        let vchunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Fill(1),
                Constraint::Length(12),
                Constraint::Fill(1),
            ])
            .split(area);
        let hchunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Fill(1),
                Constraint::Percentage(70),
                Constraint::Fill(1),
            ])
            .split(vchunks[1]);
        frame.render_widget(Clear, hchunks[1]);
        hchunks[1]
    }

    fn render_entry(&self, frame: &mut Frame, area: Rect) {
        let modal = Self::render_modal(frame, area);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            )
            .title(" F12: Input Machine — The Anchor (SYS-ADR-10) ");
        let inner = block.inner(modal);
        frame.render_widget(block, modal);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(3),
                Constraint::Length(1),
                Constraint::Fill(1),
            ])
            .split(inner);

        frame.render_widget(
            Paragraph::new("  File path to submit for ingest:")
                .style(Style::default().fg(Color::White)),
            chunks[1],
        );

        self.path_input.render_into(frame, chunks[2]);

        frame.render_widget(
            Paragraph::new("  [Enter: confirm  Esc: cancel  Ctrl-A: audit log]")
                .style(Style::default().fg(Color::DarkGray)),
            chunks[3],
        );
    }

    fn render_confirm(frame: &mut Frame, area: Rect, path: &str) {
        let modal = Self::render_modal(frame, area);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
            .title(" F12: Input Machine — Confirm Ingest ");
        let inner = block.inner(modal);
        frame.render_widget(block, modal);

        let lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                "  Submit this file for ingest?",
                Style::default().fg(Color::White),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled("  Path: ", Style::default().fg(Color::DarkGray)),
                Span::styled(path.to_string(), Style::default().fg(Color::Cyan)),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "  [Y: submit  N / Esc: cancel]",
                Style::default().fg(Color::DarkGray),
            )),
        ];
        frame.render_widget(Paragraph::new(lines), inner);
    }

    fn render_submitting(frame: &mut Frame, area: Rect, spinner: usize) {
        let modal = Self::render_modal(frame, area);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Magenta))
            .title(" F12: Input Machine — Submitting... ");
        let inner = block.inner(modal);
        frame.render_widget(block, modal);

        let mid = Rect {
            y: inner.y + inner.height / 2,
            height: 2,
            ..inner
        };
        frame.render_widget(
            Paragraph::new(format!(
                "  {} Submitting to service-fs — please wait…",
                SPINNER[spinner % SPINNER.len()]
            ))
            .style(Style::default().fg(Color::Yellow)),
            mid,
        );
    }

    fn render_done(
        frame: &mut Frame,
        area: Rect,
        path: &str,
        result: &IngestResult,
        ledger_height: u64,
        ledger_root: &[u8; 32],
    ) {
        let modal = Self::render_modal(frame, area);

        let (title, color) = if result.warning.is_some() {
            (" F12: Input Machine — Submitted (warning) ", Color::Yellow)
        } else {
            (" F12: Input Machine — Submitted ", Color::Green)
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(color).add_modifier(Modifier::BOLD))
            .title(title);
        let inner = block.inner(modal);
        frame.render_widget(block, modal);

        let root_hex: String = ledger_root[..8]
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect();

        let mut lines = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("  ✓ ", Style::default().fg(Color::Green)),
                Span::styled(path.to_string(), Style::default().fg(Color::White)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  Payload ID: ", Style::default().fg(Color::DarkGray)),
                Span::styled(result.payload_id.clone(), Style::default().fg(Color::Cyan)),
            ]),
            Line::from(vec![
                Span::styled("  ⬡ Ledger:  ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("#{} root:{}", ledger_height, root_hex),
                    Style::default().fg(Color::Magenta),
                ),
            ]),
        ];
        if let Some(ledger) = &result.ledger_root {
            lines.push(Line::from(vec![
                Span::styled("    Service: ", Style::default().fg(Color::DarkGray)),
                Span::styled(ledger.clone(), Style::default().fg(Color::Cyan)),
            ]));
        }
        if let Some(warn) = &result.warning {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                format!("  ⚠ {}", warn),
                Style::default().fg(Color::Yellow),
            )));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  [any key: return to previous pane]",
            Style::default().fg(Color::DarkGray),
        )));

        frame.render_widget(Paragraph::new(lines), inner);
    }

    fn render_audit(frame: &mut Frame, area: Rect, records: &[IngestRecord], scroll: u16) {
        // Full-pane (not modal) — audit log can be long.
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title(" F12: Input Machine — Audit Log    [j/k: scroll  Esc: back] ");
        let inner = block.inner(area);
        frame.render_widget(block, area);

        if records.is_empty() {
            frame.render_widget(
                Paragraph::new("  No ingest events recorded yet.")
                    .style(Style::default().fg(Color::DarkGray)),
                inner,
            );
            return;
        }

        let lines: Vec<Line> = records
            .iter()
            .map(|r| {
                let status_color = match r.status.as_str() {
                    "ok" => Color::Green,
                    "warned" => Color::Yellow,
                    "error" => Color::Red,
                    _ => Color::DarkGray,
                };
                Line::from(vec![
                    Span::styled(
                        format!("{:<20} ", truncate(&r.created_at, 19)),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(
                        format!("{:<8} ", truncate(&r.status, 7)),
                        Style::default().fg(status_color),
                    ),
                    Span::raw(format!(
                        "{:<18} {}",
                        truncate(&format!("{}@{}", r.username, r.tenant), 18),
                        truncate(&r.path, 60),
                    )),
                ])
            })
            .collect();

        let total = lines.len() as u16;
        let visible = inner.height;
        let offset = scroll.min(total.saturating_sub(visible));
        frame.render_widget(Paragraph::new(lines).scroll((offset, 0)), inner);
    }

    fn render_error(frame: &mut Frame, area: Rect, message: &str) {
        let modal = Self::render_modal(frame, area);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
            .title(" F12: Input Machine — Error ");
        let inner = block.inner(modal);
        frame.render_widget(block, modal);

        let lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                format!("  Error: {}", message),
                Style::default().fg(Color::Red),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  [any key: return to previous pane]",
                Style::default().fg(Color::DarkGray),
            )),
        ];
        frame.render_widget(Paragraph::new(lines), inner);
    }
}

impl Default for InputCartridge {
    fn default() -> Self {
        Self::new()
    }
}

impl Cartridge for InputCartridge {
    fn fkey(&self) -> FKey {
        FKey::F12
    }

    fn title(&self) -> &str {
        "Input"
    }

    fn set_graphics_caps(
        &mut self,
        _kitty: bool,
        _sixel: bool,
        _font_size: (u16, u16),
        truecolor: bool,
    ) {
        self.truecolor = truecolor;
    }

    fn render(&mut self, frame: &mut Frame, area: Rect) {
        // Poll HTTP result
        let new_state: Option<InputState> =
            if let InputState::Submitting { rx, path, .. } = &mut self.state {
                match rx.try_recv() {
                    Ok(Ok(result)) => {
                        let ts = chrono::Utc::now().to_rfc3339();
                        let _ = audit::append(&IngestRecord {
                            created_at: ts,
                            username: self.username.clone(),
                            tenant: self.tenant.clone(),
                            path: path.clone(),
                            ledger_id: result.ledger_root.clone(),
                            status: if result.warning.is_some() {
                                "warned".into()
                            } else {
                                "ok".into()
                            },
                        });
                        Some(InputState::Done {
                            path: path.clone(),
                            result,
                        })
                    }
                    Ok(Err(e)) => {
                        let ts = chrono::Utc::now().to_rfc3339();
                        let _ = audit::append(&IngestRecord {
                            created_at: ts,
                            username: self.username.clone(),
                            tenant: self.tenant.clone(),
                            path: path.clone(),
                            ledger_id: None,
                            status: "error".into(),
                        });
                        Some(InputState::Error {
                            message: e.to_string(),
                        })
                    }
                    Err(mpsc::TryRecvError::Disconnected) => Some(InputState::Error {
                        message: "HTTP thread disconnected".into(),
                    }),
                    Err(mpsc::TryRecvError::Empty) => None,
                }
            } else {
                None
            };
        if let Some(ns) = new_state {
            if let InputState::Done { result, .. } = &ns {
                let mut h = Sha256::new();
                h.update(self.ledger_root);
                h.update(result.payload_id.as_bytes());
                self.ledger_root = h.finalize().into();
                self.ledger_height += 1;
            }
            self.state = ns;
        }

        if let InputState::Submitting { spinner, .. } = &mut self.state {
            *spinner = spinner.wrapping_add(1);
        }

        enum Cmd<'a> {
            Entry,
            Confirm(&'a str),
            Submitting(usize),
            Done(&'a str, &'a IngestResult, u64, [u8; 32]),
            Audit(&'a [IngestRecord], u16),
            Error(&'a str),
        }

        let ledger_h = self.ledger_height;
        let ledger_r = self.ledger_root;
        let cmd = match &self.state {
            InputState::Entry => Cmd::Entry,
            InputState::Confirm { path } => Cmd::Confirm(path.as_str()),
            InputState::Submitting { spinner, .. } => Cmd::Submitting(*spinner),
            InputState::Done { path, result } => Cmd::Done(path.as_str(), result, ledger_h, ledger_r),
            InputState::AuditLog { records, scroll } => Cmd::Audit(records.as_slice(), *scroll),
            InputState::Error { message } => Cmd::Error(message.as_str()),
        };

        match cmd {
            Cmd::Entry => self.render_entry(frame, area),
            Cmd::Confirm(p) => Self::render_confirm(frame, area, p),
            Cmd::Submitting(sp) => Self::render_submitting(frame, area, sp),
            Cmd::Done(p, r, lh, lr) => Self::render_done(frame, area, p, r, lh, &lr),
            Cmd::Audit(recs, sc) => Self::render_audit(frame, area, recs, sc),
            Cmd::Error(m) => Self::render_error(frame, area, m),
        }
    }

    fn handle_event(&mut self, event: &Event) -> CartridgeAction {
        let Event::Key(key) = event else {
            return CartridgeAction::None;
        };

        // F12 pressed again while in Entry → cancel and go back
        if key.code == KeyCode::F(12) {
            self.reset();
            return CartridgeAction::GoBack;
        }

        match &self.state {
            InputState::Entry => {
                // Ctrl-A → open the local ingest audit log (Ctrl-modified keys bypass PathInput).
                if key.code == KeyCode::Char('a') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    let records = audit::query_recent(200).unwrap_or_default();
                    self.state = InputState::AuditLog { records, scroll: 0 };
                    return CartridgeAction::Consumed;
                }
                match self.path_input.handle_key(key) {
                    Some(PathInputAction::Submit(path)) => {
                        self.state = InputState::Confirm { path };
                    }
                    Some(PathInputAction::Cancel) => {
                        self.reset();
                        return CartridgeAction::GoBack;
                    }
                    None => {}
                }
                CartridgeAction::Consumed
            }

            InputState::AuditLog { .. } => {
                match key.code {
                    KeyCode::Esc | KeyCode::Char('q') => {
                        self.reset();
                    }
                    KeyCode::Char('j') | KeyCode::Down => {
                        if let InputState::AuditLog { scroll, .. } = &mut self.state {
                            *scroll = scroll.saturating_add(1);
                        }
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        if let InputState::AuditLog { scroll, .. } = &mut self.state {
                            *scroll = scroll.saturating_sub(1);
                        }
                    }
                    _ => {}
                }
                CartridgeAction::Consumed
            }

            InputState::Confirm { .. } => {
                match key.code {
                    KeyCode::Char('y') | KeyCode::Char('Y') => {
                        let path = if let InputState::Confirm { path } = &self.state {
                            path.clone()
                        } else {
                            unreachable!()
                        };
                        let username = self.username.clone();
                        let tenant = self.tenant.clone();
                        let endpoint = self.ingest_endpoint.clone();
                        let path_clone = path.clone();
                        let (tx, rx) = mpsc::channel();
                        thread::spawn(move || {
                            let _ =
                                tx.send(ingest::submit(&path_clone, &username, &tenant, &endpoint));
                        });
                        self.state = InputState::Submitting {
                            path,
                            spinner: 0,
                            rx,
                        };
                    }
                    KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                        self.reset();
                        return CartridgeAction::GoBack;
                    }
                    _ => {}
                }
                CartridgeAction::Consumed
            }

            InputState::Submitting { .. } => CartridgeAction::Consumed,

            InputState::Done { .. } | InputState::Error { .. } => {
                self.reset();
                CartridgeAction::GoBack
            }
        }
    }

    fn intent_scope(&self) -> Option<&'static str> {
        Some("input")
    }

    fn intents(&self) -> Vec<IntentSpec> {
        vec![
            // SYS-ADR-10: ingest-class verbs (submit, confirm) are intentionally absent.
            // The confirm gate (y/n modal) must be a direct keyboard action — no palette
            // or mouse path may bypass it. Only the audit viewer is palette-reachable.
            IntentSpec::new(
                "input.audit",
                "View ingest audit log",
                IntentScope::Cartridge("input"),
            )
            .key("ctrl-a")
            .mouse(MouseAffordance::CLICK),
        ]
    }

    fn dispatch(&mut self, id: IntentId, _args: &IntentArgs) -> CartridgeAction {
        match id.0 {
            "input.audit" => {
                let records = audit::query_recent(200).unwrap_or_default();
                self.state = InputState::AuditLog { records, scroll: 0 };
                CartridgeAction::Consumed
            }
            _ => CartridgeAction::None,
        }
    }
}
