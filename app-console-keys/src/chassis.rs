use std::{
    collections::BTreeMap,
    io::{self, Write},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

/// Set by a SIGTERM/SIGINT handler to request clean shutdown on the next chassis tick.
static SHUTDOWN_REQUESTED: AtomicBool = AtomicBool::new(false);

/// Signal the chassis to exit cleanly on the next event-loop tick.
/// Call this from a SIGTERM or SIGINT handler registered before `run_local()`.
pub fn request_shutdown() {
    SHUTDOWN_REQUESTED.store(true, Ordering::SeqCst);
}

use anyhow::Result;
use console_core::{IntentArgs, IntentId, IntentRegistry, IntentScope, Keymap};
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame, Terminal,
};
use ratatui_image::{
    picker::{Picker, ProtocolType},
    protocol::StatefulProtocol,
    StatefulImage,
};

use crate::{
    cartridge::{Cartridge, CartridgeAction},
    fkey::FKey,
    pairing::{self, PairingEvent, PairingState},
    widgets::status_bar::MbaStatus,
};
use std::collections::BTreeSet;

pub enum ChassisAction {
    None,
    Quit,
}

/// Runtime terminal capability snapshot — populated once at startup via probe + env var.
/// Cartridges should read this to choose render paths rather than hard-coding platform assumptions.
#[derive(Debug, Clone, Copy)]
pub struct TerminalCaps {
    pub kitty: bool,
    pub sixel: bool,
    pub truecolor: bool,
}

impl TerminalCaps {
    fn detect(picker: &Option<Picker>) -> Self {
        let truecolor = std::env::var("COLORTERM")
            .map(|v| v == "truecolor" || v == "24bit")
            .unwrap_or(false);
        match picker {
            None => Self {
                kitty: false,
                sixel: false,
                truecolor,
            },
            Some(p) => Self {
                kitty: matches!(p.protocol_type(), ProtocolType::Kitty),
                sixel: matches!(p.protocol_type(), ProtocolType::Sixel),
                truecolor,
            },
        }
    }
}

/// Command-palette overlay state (Phase I-1). The palette is the universal
/// keyboard floor: every enabled, in-scope intent is reachable here whether or
/// not it has a chord. Entries are a *view* of the intent registry.
struct Palette {
    query: String,
    entries: Vec<(IntentId, String)>,
    selected: usize,
}

impl Palette {
    fn filtered(&self) -> Vec<&(IntentId, String)> {
        let q = self.query.to_ascii_lowercase();
        self.entries
            .iter()
            .filter(|(_, t)| q.is_empty() || t.to_ascii_lowercase().contains(&q))
            .collect()
    }

    fn move_sel(&mut self, delta: i32) {
        let n = self.filtered().len() as i32;
        if n == 0 {
            self.selected = 0;
            return;
        }
        self.selected = (self.selected as i32 + delta).rem_euclid(n) as usize;
    }

    fn selected_id(&self) -> Option<IntentId> {
        self.filtered().get(self.selected).map(|(id, _)| *id)
    }
}

pub struct AppConsoleKeys {
    cartridges: BTreeMap<FKey, Box<dyn Cartridge>>,
    active: FKey,
    previous: FKey,
    started: Instant,
    mba_status: MbaStatus,
    username: String,
    tenant: String,
    pairing_state: PairingState,
    pair_rx: Option<mpsc::Receiver<PairingEvent>>,
    pair_base_url: String,
    // Receives `true` when the reconnect watchdog successfully re-establishes the MBA link.
    mba_reconnect_rx: Option<mpsc::Receiver<bool>>,
    // Kitty/Sixel graphics protocol (local PTY path only; None over russh).
    picker: Option<Picker>,
    // Cached QR image protocol state for the AwaitingApproval screen.
    qr_state: Option<StatefulProtocol>,
    // Terminal capability snapshot — populated in run_local() after enable_raw_mode + probe.
    caps: TerminalCaps,
    // --- Intent system (Phase I-1) ---
    registry: IntentRegistry,
    keymap: Keymap,
    palette: Option<Palette>,
}

impl AppConsoleKeys {
    pub fn new(username: impl Into<String>, tenant: impl Into<String>) -> Self {
        Self {
            cartridges: BTreeMap::new(),
            active: FKey::F4,
            previous: FKey::F4,
            started: Instant::now(),
            mba_status: MbaStatus::Inactive("not configured".into()),
            username: username.into(),
            tenant: tenant.into(),
            pairing_state: PairingState::default(),
            pair_rx: None,
            pair_base_url: String::new(),
            mba_reconnect_rx: None,
            picker: None,
            qr_state: None,
            caps: TerminalCaps {
                kitty: false,
                sixel: false,
                truecolor: false,
            },
            registry: IntentRegistry::new(),
            keymap: Keymap::default(),
            palette: None,
        }
    }

    /// Build the live intent registry (seed vocabulary + each cartridge's
    /// `intents()`) and the keymap. Called once after all cartridges are
    /// registered, at the start of the run loop.
    fn build_intents(&mut self) {
        let mut reg = console_core::seed::console_seed();
        for c in self.cartridges.values() {
            reg.extend(c.intents());
        }
        self.keymap = Keymap::from_registry(&reg);
        self.registry = reg;
    }

    /// Scope id of the focused cartridge for keymap/palette filtering.
    fn focused_scope(&self) -> Option<&'static str> {
        self.cartridges.get(&self.active).and_then(|c| c.intent_scope())
    }

    fn is_global_or_pane(&self, id: IntentId) -> bool {
        matches!(
            self.registry.get(id).map(|s| s.scope),
            Some(IntentScope::Global) | Some(IntentScope::Pane)
        )
    }

    fn switch_to(&mut self, fkey: FKey) {
        if self.active != fkey {
            self.previous = self.active;
        }
        self.active = fkey;
    }

    fn open_palette(&mut self) {
        let focused = self.focused_scope();
        let entries: Vec<(IntentId, String)> = self
            .registry
            .palette_entries(focused)
            .iter()
            .map(|s| (s.id, s.title.to_string()))
            .collect();
        self.palette = Some(Palette {
            query: String::new(),
            entries,
            selected: 0,
        });
    }

    /// Route a resolved intent: chassis-global verbs are handled here; anything
    /// else is dispatched to the active cartridge's `dispatch()`.
    fn dispatch_intent(&mut self, id: IntentId) -> ChassisAction {
        match id.0 {
            "console.quit" => return ChassisAction::Quit,
            "console.palette" => {
                self.open_palette();
                return ChassisAction::None;
            }
            "console.help" => {
                self.switch_to(FKey::F1);
                return ChassisAction::None;
            }
            "view.switch.search" => {
                self.switch_to(FKey::F5);
                return ChassisAction::None;
            }
            "view.switch.content" => {
                self.switch_to(FKey::F4);
                return ChassisAction::None;
            }
            "view.switch.system" => {
                self.switch_to(FKey::F11);
                return ChassisAction::None;
            }
            "input.anchor.open" => {
                self.switch_to(FKey::F12);
                return ChassisAction::None;
            }
            _ => {}
        }
        // Cartridge-scoped intent: hand to the active cartridge.
        let args = IntentArgs::default();
        if let Some(c) = self.cartridges.get_mut(&self.active) {
            match c.dispatch(id, &args) {
                CartridgeAction::Quit => return ChassisAction::Quit,
                CartridgeAction::GoBack => self.active = self.previous,
                CartridgeAction::Consumed | CartridgeAction::None => {}
            }
        }
        ChassisAction::None
    }

    fn handle_palette_event(&mut self, event: &Event) -> ChassisAction {
        let Event::Key(key) = event else {
            return ChassisAction::None;
        };
        enum Outcome {
            Stay,
            Close,
            Run(IntentId),
        }
        let outcome = {
            let Some(p) = self.palette.as_mut() else {
                return ChassisAction::None;
            };
            match key.code {
                KeyCode::Esc => Outcome::Close,
                KeyCode::Up => {
                    p.move_sel(-1);
                    Outcome::Stay
                }
                KeyCode::Down => {
                    p.move_sel(1);
                    Outcome::Stay
                }
                KeyCode::Enter => match p.selected_id() {
                    Some(id) => Outcome::Run(id),
                    None => Outcome::Close,
                },
                KeyCode::Backspace => {
                    p.query.pop();
                    p.selected = 0;
                    Outcome::Stay
                }
                KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                    p.query.push(ch);
                    p.selected = 0;
                    Outcome::Stay
                }
                _ => Outcome::Stay,
            }
        };
        match outcome {
            Outcome::Stay => ChassisAction::None,
            Outcome::Close => {
                self.palette = None;
                ChassisAction::None
            }
            Outcome::Run(id) => {
                self.palette = None;
                self.dispatch_intent(id)
            }
        }
    }

    pub fn set_mba_active(&mut self) {
        self.mba_status = MbaStatus::Active;
    }

    pub fn set_pairing_unpaired(&mut self, fingerprint: String) {
        self.pairing_state = PairingState::Unpaired { fingerprint };
    }

    pub fn set_pairing_awaiting(&mut self, code: String, request_id: String, fingerprint: String) {
        // Pre-generate the pixel QR state for Kitty/Sixel rendering if picker is available.
        let qr_content = format!("PAIR:{}", code.replace('-', ""));
        self.qr_state = self
            .picker
            .as_mut()
            .and_then(|p| crate::qr::qr_image(&qr_content).map(|img| p.new_resize_protocol(img)));
        self.pairing_state = PairingState::AwaitingApproval {
            code,
            request_id,
            fingerprint,
        };
    }

    pub fn set_pairing_error(&mut self, msg: String) {
        self.pairing_state = PairingState::Error(msg);
    }

    pub fn set_pair_rx(&mut self, rx: mpsc::Receiver<PairingEvent>) {
        self.pair_rx = Some(rx);
    }

    pub fn set_pair_base_url(&mut self, url: String) {
        self.pair_base_url = url;
    }

    pub fn set_mba_reconnect_rx(&mut self, rx: mpsc::Receiver<bool>) {
        self.mba_reconnect_rx = Some(rx);
    }

    pub fn caps(&self) -> TerminalCaps {
        self.caps
    }

    pub fn register(&mut self, cartridge: Box<dyn Cartridge>) {
        let fkey = cartridge.fkey();
        if self.cartridges.is_empty() {
            self.active = fkey;
        }
        self.cartridges.insert(fkey, cartridge);
    }

    fn installed(&self) -> BTreeSet<FKey> {
        self.cartridges.keys().copied().collect()
    }

    fn apply_pairing_event(&mut self, ev: PairingEvent) {
        match ev {
            PairingEvent::Approved => {
                self.mba_status = MbaStatus::Active;
                self.pairing_state = PairingState::Approved;
            }
            PairingEvent::Denied => {
                self.pairing_state = PairingState::Denied;
            }
            PairingEvent::Expired => {
                self.pairing_state = PairingState::Expired;
            }
            PairingEvent::Error(e) => {
                if matches!(self.pairing_state, PairingState::AwaitingApproval { .. }) {
                    self.pairing_state = PairingState::Error(e);
                }
            }
            PairingEvent::InitSuccess {
                code,
                request_id,
                fingerprint,
            } => {
                self.pairing_state = PairingState::AwaitingApproval {
                    code: code.clone(),
                    request_id: request_id.clone(),
                    fingerprint,
                };
                let new_rx = pairing::spawn_status_poll(self.pair_base_url.clone(), request_id);
                self.pair_rx = Some(new_rx);
            }
        }
    }

    pub fn render(&mut self, frame: &mut Frame) {
        let area = frame.area();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Fill(1),
                Constraint::Length(1),
            ])
            .split(area);

        let installed = self.installed();
        crate::widgets::fkey_strip::render(frame, chunks[0], self.active, &installed);

        let mba_inactive = matches!(self.mba_status, MbaStatus::Inactive(_));
        if mba_inactive {
            Self::render_pairing_screen(frame, chunks[1], &self.pairing_state, &mut self.qr_state);
        } else if let Some(c) = self.cartridges.get_mut(&self.active) {
            c.render(frame, chunks[1]);
        } else {
            frame.render_widget(
                Paragraph::new(format!("\n  {} — not installed", self.active.label())),
                chunks[1],
            );
        }

        let elapsed = self.started.elapsed().as_secs();
        let pending_pairs = self
            .cartridges
            .get(&FKey::F11)
            .map(|c| c.pending_badge())
            .unwrap_or(0);
        crate::widgets::status_bar::render(
            frame,
            chunks[2],
            &self.username,
            &self.tenant,
            &self.mba_status,
            self.active,
            elapsed,
            pending_pairs,
        );

        // Command palette overlay (Phase I-1) — drawn on top of everything.
        if let Some(p) = self.palette.as_ref() {
            render_palette(frame, area, p);
        }
    }

    pub fn handle_event(&mut self, event: &Event) -> ChassisAction {
        // The command palette captures all input while open.
        if self.palette.is_some() {
            return self.handle_palette_event(event);
        }

        // Ctrl-K (resolved through the keymap) opens the palette.
        if let Event::Key(key) = event {
            if let Some(chord) = key_to_chord(key) {
                if self.keymap.resolve(&chord, self.focused_scope()) == Some(IntentId("console.palette"))
                {
                    self.open_palette();
                    return ChassisAction::None;
                }
            }
            // F12 anchor pre-empt (SYS-ADR-10) — unchanged, immovable.
            if key.code == KeyCode::F(12) {
                if self.active != FKey::F12 {
                    self.previous = self.active;
                }
                self.active = FKey::F12;
                return ChassisAction::None;
            }
        }

        if let Some(c) = self.cartridges.get_mut(&self.active) {
            match c.handle_event(event) {
                CartridgeAction::Consumed => return ChassisAction::None,
                CartridgeAction::Quit => return ChassisAction::Quit,
                CartridgeAction::GoBack => {
                    self.active = self.previous;
                    return ChassisAction::None;
                }
                CartridgeAction::None => {}
            }
        }

        if let Event::Key(key) = event {
            // Global/pane intents the focused cartridge did not consume resolve
            // through the keymap (cartridge-scoped chords are left to the
            // cartridge's own handler above, so no behavior is overridden).
            if let Some(chord) = key_to_chord(key) {
                if let Some(id) = self.keymap.resolve(&chord, self.focused_scope()) {
                    if self.is_global_or_pane(id) {
                        return self.dispatch_intent(id);
                    }
                }
            }
            if key.code == KeyCode::Char('q')
                || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))
            {
                return ChassisAction::Quit;
            }
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                let fkey = match key.code {
                    KeyCode::Char('3') => Some(FKey::F3),
                    KeyCode::Char('4') => Some(FKey::F4),
                    KeyCode::Char('6') => Some(FKey::F6),
                    KeyCode::Char('9') => Some(FKey::F9),
                    KeyCode::Char('1') => Some(FKey::F11),
                    KeyCode::Char('2') => Some(FKey::F12),
                    _ => None,
                };
                if let Some(fkey) = fkey {
                    if self.active != fkey {
                        self.previous = self.active;
                    }
                    self.active = fkey;
                    return ChassisAction::None;
                }
            }
            if let Some(fkey) = FKey::from_keycode(key.code) {
                if self.active != fkey {
                    self.previous = self.active;
                }
                self.active = fkey;
                return ChassisAction::None;
            }
        }

        ChassisAction::None
    }

    fn render_pairing_screen(
        frame: &mut Frame,
        area: Rect,
        state: &PairingState,
        qr_state: &mut Option<StatefulProtocol>,
    ) {
        match state {
            PairingState::Unpaired { .. } => {
                let block = Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Blue))
                    .title(" Connecting to your workspace ");
                let inner = block.inner(area);
                frame.render_widget(block, area);
                let lines = vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        "  Connecting to your workspace…",
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        "  Sending connection request to your administrator.",
                        Style::default().fg(Color::Gray),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        "  [Q / Ctrl-C: quit]",
                        Style::default().fg(Color::DarkGray),
                    )),
                ];
                frame.render_widget(Paragraph::new(lines), inner);
            }

            PairingState::AwaitingApproval { code, .. } => {
                let block = Block::default()
                    .borders(Borders::ALL)
                    .border_style(
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    )
                    .title(" Connect this computer to your workspace ");
                let inner = block.inner(area);
                frame.render_widget(block, area);

                // QR content — same as what was encoded at set_pairing_awaiting time.
                let qr_content = format!("PAIR:{}", code.replace('-', ""));

                // Right column: code pill + instructions (always rendered).
                let right_content = vec![
                    Line::from(""),
                    Line::from(""),
                    Line::from(Span::styled(
                        " Share this code with your administrator:",
                        Style::default().fg(Color::White),
                    )),
                    Line::from(""),
                    Line::from(""),
                    Line::from(vec![
                        Span::raw("  "),
                        Span::styled(
                            format!("  {}  ", code),
                            Style::default()
                                .fg(Color::Cyan)
                                .bg(Color::DarkGray)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ]),
                    Line::from(""),
                    Line::from(""),
                    Line::from(Span::styled(
                        " Your administrator approves it.",
                        Style::default().fg(Color::Gray),
                    )),
                    Line::from(Span::styled(
                        " This screen updates automatically.",
                        Style::default().fg(Color::Gray),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        " ◌  Waiting for approval…",
                        Style::default().fg(Color::Blue),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        " [Q / Ctrl-C: quit and come back later]",
                        Style::default().fg(Color::DarkGray),
                    )),
                ];

                if let Some(proto) = qr_state.as_mut() {
                    // Kitty/Sixel path: pixel-perfect QR on left, code pill on right.
                    // Minimum width for the pixel QR column: 20 cells.
                    let qr_w = inner.width.saturating_sub(36).max(20).min(inner.width / 2);
                    if inner.width >= qr_w + 32 {
                        let cols = Layout::default()
                            .direction(Direction::Horizontal)
                            .constraints([Constraint::Length(qr_w), Constraint::Fill(1)])
                            .split(inner);

                        frame.render_stateful_widget(StatefulImage::new(), cols[0], proto);
                        frame.render_widget(Paragraph::new(right_content), cols[1]);
                        return;
                    }
                }

                // Unicode Dense1x2 fallback (Phase 2 path) or narrow terminal.
                let qr_text = crate::qr::qr_unicode(&qr_content);
                let qr_col_w: u16 = qr_text
                    .lines()
                    .find(|l| !l.is_empty())
                    .map(|l| l.chars().count() as u16)
                    .unwrap_or(30);

                if !qr_text.is_empty() && inner.width >= qr_col_w + 32 {
                    let cols = Layout::default()
                        .direction(Direction::Horizontal)
                        .constraints([Constraint::Length(qr_col_w), Constraint::Fill(1)])
                        .split(inner);

                    let qr_lines: Vec<Line> =
                        qr_text.lines().map(|l| Line::from(l.to_string())).collect();
                    frame.render_widget(Paragraph::new(qr_lines), cols[0]);
                    frame.render_widget(Paragraph::new(right_content), cols[1]);
                } else {
                    // Narrow layout: code pill only.
                    let lines = vec![
                        Line::from(""),
                        Line::from(Span::styled(
                            "  Share this code with your administrator:",
                            Style::default().fg(Color::White),
                        )),
                        Line::from(""),
                        Line::from(""),
                        Line::from(vec![
                            Span::raw("          "),
                            Span::styled(
                                format!("   {}   ", code),
                                Style::default()
                                    .fg(Color::Cyan)
                                    .bg(Color::DarkGray)
                                    .add_modifier(Modifier::BOLD),
                            ),
                        ]),
                        Line::from(""),
                        Line::from(""),
                        Line::from(Span::styled(
                            "  Your administrator approves it — this screen updates automatically.",
                            Style::default().fg(Color::Gray),
                        )),
                        Line::from(""),
                        Line::from(Span::styled(
                            "  ◌  Waiting for approval…",
                            Style::default().fg(Color::Blue),
                        )),
                        Line::from(""),
                        Line::from(Span::styled(
                            "  [Q / Ctrl-C: quit and come back later]",
                            Style::default().fg(Color::DarkGray),
                        )),
                    ];
                    frame.render_widget(Paragraph::new(lines), inner);
                }
            }

            PairingState::Approved => {} // mba_status is Active — normal chassis renders

            PairingState::Denied => {
                let block = Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Yellow))
                    .title(" Connection not approved ");
                let inner = block.inner(area);
                frame.render_widget(block, area);
                let lines = vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        "  Your administrator didn't approve this computer.",
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        "  This is usually a quick mix-up. Talk to your administrator,",
                        Style::default().fg(Color::Gray),
                    )),
                    Line::from(Span::styled(
                        "  then restart os-console to try again.",
                        Style::default().fg(Color::Gray),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        "  [Q / Ctrl-C: quit]",
                        Style::default().fg(Color::DarkGray),
                    )),
                ];
                frame.render_widget(Paragraph::new(lines), inner);
            }

            PairingState::Expired => {
                let block = Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Yellow))
                    .title(" Connection code expired ");
                let inner = block.inner(area);
                frame.render_widget(block, area);
                let lines = vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        "  The code timed out — nothing was lost.",
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        "  Restart os-console to get a fresh code.",
                        Style::default().fg(Color::Gray),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        "  [Q / Ctrl-C: quit]",
                        Style::default().fg(Color::DarkGray),
                    )),
                ];
                frame.render_widget(Paragraph::new(lines), inner);
            }

            PairingState::Error(msg) => {
                let block = Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Yellow))
                    .title(" Can't reach your workspace ");
                let inner = block.inner(area);
                frame.render_widget(block, area);
                let lines = vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        "  We couldn't connect right now.",
                        Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        "  This is almost always a network hiccup, not a problem with your computer.",
                        Style::default().fg(Color::Gray),
                    )),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("  Detail:  ", Style::default().fg(Color::DarkGray)),
                        Span::styled(msg.as_str(), Style::default().fg(Color::DarkGray)),
                    ]),
                    Line::from(""),
                    Line::from(Span::styled(
                        "  [Q / Ctrl-C: quit]",
                        Style::default().fg(Color::DarkGray),
                    )),
                ];
                frame.render_widget(Paragraph::new(lines), inner);
            }
        }
    }

    fn drain_pair_events(&mut self) {
        let events: Vec<PairingEvent> = self
            .pair_rx
            .as_ref()
            .map(|rx| std::iter::from_fn(|| rx.try_recv().ok()).collect())
            .unwrap_or_default();
        for ev in events {
            self.apply_pairing_event(ev);
        }
        // Check for MBA reconnect from watchdog thread.
        let reconnected = self
            .mba_reconnect_rx
            .as_ref()
            .and_then(|rx| rx.try_recv().ok())
            .unwrap_or(false);
        if reconnected {
            self.mba_status = MbaStatus::Active;
        }
        for c in self.cartridges.values_mut() {
            c.tick();
        }
    }

    /// Run driven by raw SSH bytes.
    pub fn run_with_bytes<W: Write + Send>(
        mut self,
        mut terminal: ratatui::Terminal<CrosstermBackend<W>>,
        rx: mpsc::Receiver<u8>,
    ) {
        self.build_intents();
        let mut parser = crate::input_bytes::ByteParser::new();
        loop {
            self.drain_pair_events();
            if terminal.draw(|f| self.render(f)).is_err() {
                break;
            }
            match rx.recv_timeout(Duration::from_millis(16)) {
                Ok(byte) => {
                    if let Some(ev) = parser.push(byte) {
                        if let ChassisAction::Quit = self.handle_event(&ev) {
                            break;
                        }
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
    }

    /// Run in local crossterm PTY mode (default).
    pub fn run_local(mut self) -> Result<()> {
        enable_raw_mode()?;
        // Must run after enable_raw_mode — Picker sends XTGETTCAP in raw mode.
        self.picker = Picker::from_query_stdio().ok();
        self.caps = TerminalCaps::detect(&self.picker);
        // Notify cartridges of probed capabilities so they can configure pixel rendering.
        let (kitty, sixel, truecolor) = (self.caps.kitty, self.caps.sixel, self.caps.truecolor);
        let font_size = self
            .picker
            .as_ref()
            .map(|p| p.font_size())
            .unwrap_or((10, 20));
        for c in self.cartridges.values_mut() {
            c.set_graphics_caps(kitty, sixel, font_size, truecolor);
        }
        self.build_intents();
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, cursor::Hide)?;

        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let run_result = (|| -> Result<()> {
            loop {
                if SHUTDOWN_REQUESTED.load(Ordering::SeqCst) {
                    break;
                }
                self.drain_pair_events();
                terminal.draw(|f| self.render(f))?;
                if let Some(c) = self.cartridges.get(&self.active) {
                    c.flush_hyperlinks();
                }
                if event::poll(Duration::from_millis(16))? {
                    let ev = event::read()?;
                    if let ChassisAction::Quit = self.handle_event(&ev) {
                        break;
                    }
                }
            }
            Ok(())
        })();

        let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen, cursor::Show);
        let _ = disable_raw_mode();

        run_result
    }
}

/// Translate a crossterm key event into a canonical console-core chord string.
fn key_to_chord(key: &KeyEvent) -> Option<String> {
    let base = match key.code {
        KeyCode::Char(c) => c.to_ascii_lowercase().to_string(),
        KeyCode::Enter => "enter".to_string(),
        KeyCode::Esc => "esc".to_string(),
        KeyCode::Tab | KeyCode::BackTab => "tab".to_string(),
        KeyCode::Backspace => "backspace".to_string(),
        KeyCode::Left => "left".to_string(),
        KeyCode::Right => "right".to_string(),
        KeyCode::Up => "up".to_string(),
        KeyCode::Down => "down".to_string(),
        KeyCode::F(n) => format!("f{n}"),
        _ => return None,
    };
    let mut s = String::new();
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        s.push_str("ctrl-");
    }
    if key.modifiers.contains(KeyModifiers::ALT) {
        s.push_str("alt-");
    }
    if key.modifiers.contains(KeyModifiers::SHIFT) {
        s.push_str("shift-");
    }
    s.push_str(&base);
    Some(console_core::intent::normalize_chord(&s))
}

/// Render the command-palette overlay: a centered box with the query line and
/// the filtered, scope-aware entries (selected highlighted). The entries are a
/// view of the intent registry — the palette cannot list an action the keyboard
/// cannot reach.
fn render_palette(frame: &mut Frame, area: Rect, p: &Palette) {
    let want_w = (area.width / 5).saturating_mul(3);
    let min_w = 40u16.min(area.width);
    let max_w = area.width.saturating_sub(2).max(min_w);
    let w = want_w.clamp(min_w, max_w);
    let want_h = (area.height / 3).saturating_mul(2);
    let min_h = 8u16.min(area.height);
    let max_h = area.height.saturating_sub(2).max(min_h);
    let h = want_h.clamp(min_h, max_h);
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + area.height.saturating_sub(h) / 2;
    let rect = Rect {
        x,
        y,
        width: w,
        height: h,
    };

    frame.render_widget(Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .title(" Command palette  (type to filter · ↑↓ · Enter · Esc) ");
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(vec![
        Span::styled("  › ", Style::default().fg(Color::Cyan)),
        Span::styled(
            if p.query.is_empty() {
                "type to filter…".to_string()
            } else {
                p.query.clone()
            },
            Style::default().fg(if p.query.is_empty() {
                Color::DarkGray
            } else {
                Color::White
            }),
        ),
    ]));
    lines.push(Line::from(""));

    let filtered = p.filtered();
    let max_rows = inner.height.saturating_sub(2) as usize;
    for (i, (id, title)) in filtered.iter().enumerate().take(max_rows) {
        let selected = i == p.selected;
        let style = if selected {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };
        let marker = if selected { " ▌ " } else { "   " };
        lines.push(Line::from(vec![
            Span::styled(marker, style),
            Span::styled(format!("{:<32}", title), style),
            Span::styled(format!(" {}", id.0), Style::default().fg(Color::DarkGray)),
        ]));
    }
    frame.render_widget(Paragraph::new(lines), inner);
}
