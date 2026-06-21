use std::{
    sync::mpsc,
    time::{Duration, Instant},
};

use app_console_keys::{Cartridge, CartridgeAction, FKey, IntentArgs, IntentId};
use crossterm::event::{Event, KeyCode};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

use crate::pending::{self, PendingRequest};

pub struct SystemCartridge {
    base_url: String,
    pending: Vec<PendingRequest>,
    selected: usize,
    // Feedback line shown after approve/deny action.
    feedback: Option<String>,
    // Background poll channel: receives fresh pending lists every ~5s.
    poll_rx: mpsc::Receiver<Vec<PendingRequest>>,
    last_manual_refresh: Instant,
    // Whether the fingerprint detail block is visible for the selected request.
    show_fingerprint: bool,
    truecolor: bool,
}

impl SystemCartridge {
    pub fn new(base_url: impl Into<String>) -> Self {
        let base_url = base_url.into();
        let poll_rx = Self::spawn_poller(base_url.clone());
        Self {
            base_url,
            pending: Vec::new(),
            selected: 0,
            feedback: None,
            poll_rx,
            last_manual_refresh: Instant::now(),
            show_fingerprint: false,
            truecolor: false,
        }
    }

    fn muted_color(&self) -> Color {
        app_console_keys::colors::tc_muted(self.truecolor)
    }
    fn success_color(&self) -> Color {
        app_console_keys::colors::tc_success(self.truecolor)
    }
    fn error_color(&self) -> Color {
        app_console_keys::colors::tc_error(self.truecolor)
    }
    fn warn_color(&self) -> Color {
        app_console_keys::colors::tc_warn(self.truecolor)
    }
    fn accent_color(&self) -> Color {
        app_console_keys::colors::tc_accent(self.truecolor)
    }

    pub fn pending_count(&self) -> u16 {
        self.pending.len() as u16
    }

    fn spawn_poller(base_url: String) -> mpsc::Receiver<Vec<PendingRequest>> {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || loop {
            match pending::fetch_pending(&base_url) {
                Ok(list) => {
                    if tx.send(list).is_err() {
                        break;
                    }
                }
                Err(_) => {
                    // Silently swallow poll errors (network blip); send empty to clear stale data.
                    if tx.send(vec![]).is_err() {
                        break;
                    }
                }
            }
            std::thread::sleep(Duration::from_secs(5));
        });
        rx
    }

    fn drain_poll(&mut self) {
        // Drain all queued poll results; keep the most recent.
        let mut latest: Option<Vec<PendingRequest>> = None;
        while let Ok(list) = self.poll_rx.try_recv() {
            latest = Some(list);
        }
        if let Some(list) = latest {
            self.pending = list;
            if self.selected >= self.pending.len() {
                self.selected = self.pending.len().saturating_sub(1);
            }
        }
    }

    fn do_approve(&mut self) {
        if let Some(req) = self.pending.get(self.selected) {
            let code = req.code.clone();
            let user = format!("{}@{}", req.username, req.tenant);
            match pending::approve(&self.base_url, &code) {
                Ok(()) => {
                    self.feedback = Some(format!("Approved  {user}"));
                    self.show_fingerprint = false;
                    self.refresh_now();
                }
                Err(e) => {
                    self.feedback = Some(format!("Error: {e}"));
                }
            }
        }
    }

    fn do_deny(&mut self) {
        if let Some(req) = self.pending.get(self.selected) {
            let code = req.code.clone();
            let user = format!("{}@{}", req.username, req.tenant);
            match pending::deny(&self.base_url, &code) {
                Ok(()) => {
                    self.feedback = Some(format!("Denied  {user}"));
                    self.show_fingerprint = false;
                    self.refresh_now();
                }
                Err(e) => {
                    self.feedback = Some(format!("Error: {e}"));
                }
            }
        }
    }

    fn refresh_now(&mut self) {
        if let Ok(list) = pending::fetch_pending(&self.base_url) {
            self.pending = list;
            if self.selected >= self.pending.len() {
                self.selected = self.pending.len().saturating_sub(1);
            }
        }
        self.last_manual_refresh = Instant::now();
    }
}

impl Cartridge for SystemCartridge {
    fn fkey(&self) -> FKey {
        FKey::F11
    }

    fn title(&self) -> &str {
        "F11: System"
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

    fn tick(&mut self) {
        self.drain_poll();
    }

    fn pending_badge(&self) -> u16 {
        self.pending_count()
    }

    // --- Intent system (Phase I-1): System is the migration template. The seed
    // vocabulary declares system.* (already dual-input), so this cartridge only
    // needs to advertise its scope and act on the intents. Keyboard via
    // handle_event still works; the command palette drives the same actions
    // through dispatch — both paths converge on the same methods. ---

    fn intent_scope(&self) -> Option<&'static str> {
        Some("system")
    }

    fn dispatch(&mut self, id: IntentId, _args: &IntentArgs) -> CartridgeAction {
        match id.0 {
            "system.approve" => {
                self.do_approve();
                CartridgeAction::Consumed
            }
            "system.deny" => {
                self.do_deny();
                CartridgeAction::Consumed
            }
            "system.show_fingerprint" => {
                if !self.pending.is_empty() {
                    self.show_fingerprint = !self.show_fingerprint;
                }
                CartridgeAction::Consumed
            }
            "system.revoke" => {
                // Revocation arrives with cert-based identity (rebuild Phase D-A/C).
                self.feedback = Some("Revoke: planned (cert-based identity)".to_string());
                CartridgeAction::Consumed
            }
            _ => CartridgeAction::None,
        }
    }

    fn render(&mut self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(self.muted_color()))
            .title(" F11: System — Operator Panel ");
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let fp_height = if self.show_fingerprint && !self.pending.is_empty() {
            3u16
        } else {
            0u16
        };
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2),         // heading
                Constraint::Fill(1),           // list
                Constraint::Length(fp_height), // fingerprint detail (0 when hidden)
                Constraint::Length(1),         // feedback / hint
            ])
            .split(inner);

        // Heading
        let heading = Paragraph::new(Line::from(vec![
            Span::styled(
                "Pending Connection Requests",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  ({})", self.pending.len()),
                Style::default().fg(self.muted_color()),
            ),
        ]));
        frame.render_widget(heading, chunks[0]);

        // Pending list
        if self.pending.is_empty() {
            let msg = Paragraph::new(Line::from(Span::styled(
                "  No pending connection requests.",
                Style::default().fg(self.muted_color()),
            )));
            frame.render_widget(msg, chunks[1]);
        } else {
            let items: Vec<ListItem> = self
                .pending
                .iter()
                .enumerate()
                .map(|(i, req)| {
                    let marker = if i == self.selected { ">" } else { " " };
                    let ts = req.created_at.get(..19).unwrap_or(&req.created_at);
                    let line = Line::from(vec![
                        Span::styled(
                            format!(" {marker} "),
                            Style::default().fg(self.accent_color()),
                        ),
                        Span::styled(
                            format!("{:<14}", req.code),
                            Style::default()
                                .fg(self.accent_color())
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            format!("  {:<28}", format!("{}@{}", req.username, req.tenant)),
                            Style::default().fg(Color::White),
                        ),
                        Span::styled(ts.to_string(), Style::default().fg(self.muted_color())),
                    ]);
                    if i == self.selected {
                        ListItem::new(line).style(Style::default().bg(self.muted_color()))
                    } else {
                        ListItem::new(line)
                    }
                })
                .collect();
            frame.render_widget(List::new(items), chunks[1]);
        }

        // Fingerprint detail block (chunks[2], zero-height when hidden)
        if fp_height > 0 {
            let fp_text = self
                .pending
                .get(self.selected)
                .and_then(|r| r.fingerprint.as_deref())
                .unwrap_or("(not yet returned by server)");
            let fp_widget = Paragraph::new(Line::from(vec![
                Span::styled("  Fingerprint: ", Style::default().fg(self.muted_color())),
                Span::styled(fp_text, Style::default().fg(self.accent_color())),
            ]))
            .block(
                Block::default()
                    .borders(Borders::TOP)
                    .border_style(Style::default().fg(self.muted_color())),
            );
            frame.render_widget(fp_widget, chunks[2]);
        }

        // Feedback / hint line (chunks[3])
        let fp_hint = if self.show_fingerprint {
            "[?] hide fp  "
        } else {
            "[?] show fp  "
        };
        let hint = if let Some(msg) = &self.feedback {
            let (color, prefix) = if msg.starts_with("Error") {
                (self.warn_color(), "")
            } else if msg.starts_with("Approved") {
                (self.success_color(), "")
            } else {
                (self.error_color(), "")
            };
            Line::from(Span::styled(
                format!("{prefix}{msg}"),
                Style::default().fg(color),
            ))
        } else if !self.pending.is_empty() {
            Line::from(vec![
                Span::styled("[Enter] approve  ", Style::default().fg(self.muted_color())),
                Span::styled("[D] deny  ", Style::default().fg(self.muted_color())),
                Span::styled(fp_hint, Style::default().fg(self.muted_color())),
                Span::styled("[↑↓] select", Style::default().fg(self.muted_color())),
            ])
        } else {
            Line::from(Span::styled(
                "Refreshes every 5 seconds.",
                Style::default().fg(self.muted_color()),
            ))
        };
        frame.render_widget(Paragraph::new(hint), chunks[3]);
    }

    fn handle_event(&mut self, event: &Event) -> CartridgeAction {
        if let Event::Key(key) = event {
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    if !self.pending.is_empty() {
                        self.selected = self.selected.saturating_sub(1);
                    }
                    return CartridgeAction::Consumed;
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if !self.pending.is_empty() {
                        self.selected =
                            (self.selected + 1).min(self.pending.len().saturating_sub(1));
                    }
                    return CartridgeAction::Consumed;
                }
                KeyCode::Enter => {
                    self.do_approve();
                    return CartridgeAction::Consumed;
                }
                KeyCode::Char('d') | KeyCode::Char('D') => {
                    self.do_deny();
                    return CartridgeAction::Consumed;
                }
                KeyCode::Char('r') | KeyCode::Char('R') => {
                    self.feedback = None;
                    self.refresh_now();
                    return CartridgeAction::Consumed;
                }
                KeyCode::Char('?') => {
                    if !self.pending.is_empty() {
                        self.show_fingerprint = !self.show_fingerprint;
                    }
                    return CartridgeAction::Consumed;
                }
                _ => {}
            }
        }
        CartridgeAction::None
    }
}
