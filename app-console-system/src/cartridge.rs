use std::{
    sync::mpsc,
    time::{Duration, Instant},
};

use app_console_keys::{
    glyphs::{ANCHOR, DENY, MARKER, OK, SEAL, WAIT},
    Cartridge, CartridgeAction, FKey, IntentArgs, IntentId,
};
use crossterm::event::{Event, KeyCode};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};
use system_ledger::{revocation::RevocationEvent, InMemoryLedger, LedgerConsumer};

use crate::{
    ledger_vis::{
        build_demo_caps, cap_type_label, compute_verdict, expiry_label, now_secs, rights_label,
        CapEntry, CapVerdict, LedgerLogEntry,
    },
    peers::{self, PeerRecord},
    pending::{self, PendingRequest},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Graph,
    Topology,
    Ledger,
    Peers,
}

pub struct SystemCartridge {
    // Tab state
    tab: Tab,

    // GRAPH tab — real system-ledger state machine
    ledger: InMemoryLedger,
    caps: Vec<CapEntry>,
    graph_selected: usize,
    revoke_flash: u8,
    ledger_log: Vec<LedgerLogEntry>,
    /// Cascade animation: indices of caps whose basis was just revoked.
    cascade_queue: Vec<usize>,
    /// Ticks remaining until the next cascade entry is revealed (~10 ticks = 160ms).
    cascade_timer: u8,

    // TOPOLOGY tab — existing pending-approval flow
    base_url: String,
    content_endpoint: String,
    pending: Vec<PendingRequest>,
    topo_selected: usize,
    show_fingerprint: bool,
    poll_rx: mpsc::Receiver<Vec<PendingRequest>>,
    last_manual_refresh: Instant,

    // Shared feedback line
    feedback: Option<String>,
    truecolor: bool,
    // Peers (paired nodes) — service-content GET /v1/pairs.
    peers: Vec<PeerRecord>,
    peers_selected: usize,
    peers_poll_rx: mpsc::Receiver<Vec<PeerRecord>>,
    peers_feedback: Option<String>,
}

impl SystemCartridge {
    pub fn new(base_url: impl Into<String>, content_endpoint: impl Into<String>) -> Self {
        let base_url = base_url.into();
        let content_endpoint = content_endpoint.into();
        let poll_rx = Self::spawn_poller(base_url.clone());
        let peers_poll_rx = Self::spawn_peers_poller(content_endpoint.clone());

        // Boot the InMemoryLedger with a synthetic genesis apex.
        let mut ledger = InMemoryLedger::new();
        let _ = ledger.apex.record_genesis("console-apex", [0x42u8; 32], 0);

        let caps = build_demo_caps();

        // Pre-revoke "notify-d" (index 3) to show a REVOKED verdict on load.
        let notify_hash = caps[3].cap.hash();
        let _ = ledger.apply_revocation(RevocationEvent {
            capability_hash: notify_hash,
            revoked_at: now_secs().saturating_sub(60),
            signed_by: "console-apex".to_string(),
            ledger_height: 1,
        });

        let ledger_log = vec![LedgerLogEntry {
            height: 1,
            cap_label: "notify-d".to_string(),
            action: "REVOKE",
        }];

        Self {
            tab: Tab::Graph,
            ledger,
            caps,
            graph_selected: 0,
            revoke_flash: 0,
            ledger_log,
            cascade_queue: Vec::new(),
            cascade_timer: 0,

            base_url,
            content_endpoint,
            pending: Vec::new(),
            topo_selected: 0,
            show_fingerprint: false,
            poll_rx,
            last_manual_refresh: Instant::now(),

            feedback: None,
            truecolor: false,
            peers: Vec::new(),
            peers_selected: 0,
            peers_poll_rx,
            peers_feedback: None,
        }
    }

    pub fn pending_count(&self) -> u16 {
        self.pending.len() as u16
    }

    // ── colour helpers ──────────────────────────────────────────────────────

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

    // ── background poll ────────────────────────────────────────────────────

    fn spawn_poller(base_url: String) -> mpsc::Receiver<Vec<PendingRequest>> {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || loop {
            let list = pending::fetch_pending(&base_url).unwrap_or_default();
            if tx.send(list).is_err() {
                break;
            }
            std::thread::sleep(Duration::from_secs(5));
        });
        rx
    }

    fn drain_poll(&mut self) {
        let mut latest: Option<Vec<PendingRequest>> = None;
        while let Ok(list) = self.poll_rx.try_recv() {
            latest = Some(list);
        }
        if let Some(list) = latest {
            self.pending = list;
            if self.topo_selected >= self.pending.len() {
                self.topo_selected = self.pending.len().saturating_sub(1);
            }
        }
    }

    fn spawn_peers_poller(content_endpoint: String) -> mpsc::Receiver<Vec<PeerRecord>> {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || loop {
            match peers::fetch_peers(&content_endpoint) {
                Ok(list) => {
                    if tx.send(list).is_err() {
                        break;
                    }
                }
                Err(_) => {
                    // Silently swallow poll errors (network blip); keep stale data displayed.
                    if tx.send(Vec::new()).is_err() {
                        break;
                    }
                }
            }
            // Peers change far less often than pending approvals — poll at a slower cadence.
            std::thread::sleep(Duration::from_secs(15));
        });
        rx
    }

    fn drain_peers_poll(&mut self) {
        let mut latest: Option<Vec<PeerRecord>> = None;
        while let Ok(list) = self.peers_poll_rx.try_recv() {
            latest = Some(list);
        }
        if let Some(list) = latest {
            if !list.is_empty() || self.peers.is_empty() {
                self.peers = list;
            }
            if self.peers_selected >= self.peers.len() {
                self.peers_selected = self.peers.len().saturating_sub(1);
            }
        }
    }

    fn refresh_peers_now(&mut self) {
        match peers::fetch_peers(&self.content_endpoint) {
            Ok(list) => {
                self.peers = list;
                if self.peers_selected >= self.peers.len() {
                    self.peers_selected = self.peers.len().saturating_sub(1);
                }
                self.peers_feedback = None;
            }
            Err(e) => {
                self.peers_feedback = Some(format!("Error: {e}"));
            }
        }
    }

    fn refresh_now(&mut self) {
        if let Ok(list) = pending::fetch_pending(&self.base_url) {
            self.pending = list;
            if self.topo_selected >= self.pending.len() {
                self.topo_selected = self.pending.len().saturating_sub(1);
            }
        }
        self.last_manual_refresh = Instant::now();
    }

    // ── topology actions ───────────────────────────────────────────────────

    fn do_approve(&mut self) {
        if let Some(req) = self.pending.get(self.topo_selected) {
            let code = req.code.clone();
            let user = format!("{}@{}", req.username, req.tenant);
            match pending::approve(&self.base_url, &code) {
                Ok(()) => {
                    self.feedback = Some(format!("Approved  {user}"));
                    self.show_fingerprint = false;
                    self.refresh_now();
                }
                Err(e) => self.feedback = Some(format!("Error: {e}")),
            }
        }
    }

    fn do_deny(&mut self) {
        if let Some(req) = self.pending.get(self.topo_selected) {
            let code = req.code.clone();
            let user = format!("{}@{}", req.username, req.tenant);
            match pending::deny(&self.base_url, &code) {
                Ok(()) => {
                    self.feedback = Some(format!("Denied  {user}"));
                    self.show_fingerprint = false;
                    self.refresh_now();
                }
                Err(e) => self.feedback = Some(format!("Error: {e}")),
            }
        }
    }

    // ── graph / ledger actions ─────────────────────────────────────────────

    /// Return indices of caps that lose their basis when cap `idx` is revoked.
    /// Hardcoded for the demo set: endpoint-a(0) is the root of memory-b(1),
    /// irq-c(2), notify-d(3); cnode-e(4) grants endpoint-a(0).
    fn cascade_for(idx: usize) -> Vec<usize> {
        match idx {
            0 => vec![1, 2, 3], // endpoint-a revoked → memory-b, irq-c, notify-d cascade
            4 => vec![0],       // cnode-e revoked → endpoint-a loses grant basis
            _ => vec![],
        }
    }

    fn do_revoke_selected(&mut self) {
        let n = self.caps.len();
        if n == 0 {
            return;
        }
        let idx = self.graph_selected.min(n - 1);
        let verdict = compute_verdict(&self.caps[idx].cap, &self.ledger);
        if verdict == CapVerdict::Revoked {
            self.feedback = Some("Already revoked.".to_string());
            return;
        }

        let cap_hash = self.caps[idx].cap.hash();
        let label = self.caps[idx].label.to_string();
        let height = self.ledger_log.len() as u64 + 2;

        match self.ledger.apply_revocation(RevocationEvent {
            capability_hash: cap_hash,
            revoked_at: now_secs(),
            signed_by: "console-apex".to_string(),
            ledger_height: height,
        }) {
            Ok(()) => {
                self.ledger_log.push(LedgerLogEntry {
                    height,
                    cap_label: label.clone(),
                    action: "REVOKE",
                });
                self.revoke_flash = 20; // ~320ms at 16ms tick
                // Compute cascade: caps that lose their basis when this one is revoked.
                let cascade = Self::cascade_for(idx)
                    .into_iter()
                    .filter(|&ci| compute_verdict(&self.caps[ci].cap, &self.ledger) != CapVerdict::Revoked)
                    .collect::<Vec<_>>();
                if cascade.is_empty() {
                    self.feedback = Some(format!("{DENY} Revoked: {label}"));
                } else {
                    self.feedback = Some(format!("{DENY} Revoked: {label} — cascade pending ({} dependent)", cascade.len()));
                    self.cascade_queue = cascade;
                    self.cascade_timer = 12;
                }
            }
            Err(e) => {
                self.feedback = Some(format!("Revoke failed: {e:?}"));
            }
        }
    }

    fn cycle_tab_next(&mut self) {
        self.tab = match self.tab {
            Tab::Graph => Tab::Topology,
            Tab::Topology => Tab::Ledger,
            Tab::Ledger => Tab::Peers,
            Tab::Peers => Tab::Graph,
        };
        self.feedback = None;
    }

    fn cycle_tab_prev(&mut self) {
        self.tab = match self.tab {
            Tab::Graph => Tab::Peers,
            Tab::Topology => Tab::Graph,
            Tab::Ledger => Tab::Topology,
            Tab::Peers => Tab::Ledger,
        };
        self.feedback = None;
    }

    // ── tab strip ──────────────────────────────────────────────────────────

    fn render_tab_strip(&self, frame: &mut Frame, area: Rect) {
        let tabs: &[(&str, Tab)] = &[
            ("GRAPH", Tab::Graph),
            ("TOPOLOGY", Tab::Topology),
            ("LEDGER", Tab::Ledger),
            ("PEERS", Tab::Peers),
        ];
        let mut spans = vec![Span::raw("  ")];
        for (name, t) in tabs {
            if *t == self.tab {
                spans.push(Span::styled(
                    format!("[{name}]"),
                    Style::default()
                        .fg(self.accent_color())
                        .add_modifier(Modifier::BOLD),
                ));
            } else {
                spans.push(Span::styled(
                    format!(" {name} "),
                    Style::default().fg(self.muted_color()),
                ));
            }
            spans.push(Span::raw("  "));
        }
        frame.render_widget(Paragraph::new(Line::from(spans)), area);
    }

    // ── GRAPH tab ──────────────────────────────────────────────────────────

    fn render_graph(&self, frame: &mut Frame, area: Rect) {
        let n_caps = self.caps.len();
        let n_revoked = self
            .caps
            .iter()
            .filter(|e| compute_verdict(&e.cap, &self.ledger) == CapVerdict::Revoked)
            .count();

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // heading
                Constraint::Length(1), // column headers
                Constraint::Fill(1),   // cap list
                Constraint::Length(1), // hint
            ])
            .split(area);

        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(ANCHOR, Style::default().fg(self.accent_color())),
                Span::styled(
                    format!(" Capabilities  ({n_caps} objects, {n_revoked} revoked)"),
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
            ])),
            chunks[0],
        );

        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "  TYPE      RIGHTS                EXPIRY     VERDICT",
                Style::default().fg(self.muted_color()),
            ))),
            chunks[1],
        );

        let flash_active = self.revoke_flash > 0;
        let cascade_pending: std::collections::HashSet<usize> = self.cascade_queue.iter().copied().collect();
        let items: Vec<ListItem> = self
            .caps
            .iter()
            .enumerate()
            .map(|(i, entry)| {
                let verdict = compute_verdict(&entry.cap, &self.ledger);
                let is_sel = i == self.graph_selected;
                let is_cascade = cascade_pending.contains(&i);
                let sel_mark = if is_sel { MARKER } else if is_cascade { "⟲" } else { " " };
                let ct = cap_type_label(&entry.cap.cap_type);
                let rights = format!("{:<20}", rights_label(&entry.cap.rights));
                let expiry = expiry_label(entry.cap.expiry_t);

                let (v_glyph, v_label, v_color) = match verdict {
                    CapVerdict::Allow => (OK, "Allow  ", self.success_color()),
                    CapVerdict::Expired => (WAIT, "Expired", self.warn_color()),
                    CapVerdict::Revoked => {
                        // Newly-revoked selected row pops bright red during flash.
                        let c = if flash_active && is_sel {
                            Color::Red
                        } else {
                            self.error_color()
                        };
                        (DENY, "Revoked", c)
                    }
                };

                let line = Line::from(vec![
                    Span::styled(
                        format!("{sel_mark} "),
                        Style::default().fg(self.accent_color()),
                    ),
                    Span::styled(format!("{ct}  "), Style::default().fg(Color::White)),
                    Span::styled(rights, Style::default().fg(self.muted_color())),
                    Span::styled(expiry, Style::default().fg(self.muted_color())),
                    Span::styled(
                        format!("{v_glyph} {v_label}"),
                        Style::default().fg(v_color),
                    ),
                ]);

                if is_sel {
                    ListItem::new(line).style(Style::default().bg(Color::DarkGray))
                } else if is_cascade {
                    ListItem::new(line).style(Style::default().bg(Color::Rgb(60, 40, 0)))
                } else {
                    ListItem::new(line)
                }
            })
            .collect();
        frame.render_widget(List::new(items), chunks[2]);

        // Hint / feedback
        let hint_line = Self::graph_hint_line(
            self.feedback.as_deref(),
            self.graph_selected,
            &self.caps,
            &self.ledger,
            self.muted_color(),
            self.error_color(),
            self.success_color(),
            self.warn_color(),
        );
        frame.render_widget(Paragraph::new(hint_line), chunks[3]);
    }

    #[allow(clippy::too_many_arguments)]
    fn graph_hint_line<'a>(
        feedback: Option<&'a str>,
        graph_selected: usize,
        caps: &[CapEntry],
        ledger: &InMemoryLedger,
        muted: Color,
        error: Color,
        success: Color,
        warn: Color,
    ) -> Line<'a> {
        if let Some(msg) = feedback {
            let color = if msg.starts_with("Error") || msg.starts_with("Revoke failed") {
                warn
            } else if msg.starts_with(DENY) || msg.contains("Revoked") || msg.contains("revoked") {
                error
            } else if msg.starts_with("Approved") {
                success
            } else {
                muted
            };
            return Line::from(Span::styled(msg, Style::default().fg(color)));
        }
        let hint = if graph_selected < caps.len() {
            if compute_verdict(&caps[graph_selected].cap, ledger) == CapVerdict::Revoked {
                "Already revoked  [↑↓] select  [Tab] switch tab"
            } else {
                "[R] revoke  [↑↓] select  [Tab] switch tab"
            }
        } else {
            "[Tab] switch tab"
        };
        Line::from(Span::styled(hint, Style::default().fg(muted)))
    }

    // ── TOPOLOGY tab ───────────────────────────────────────────────────────

    fn render_topology(&self, frame: &mut Frame, area: Rect) {
        let fp_height = if self.show_fingerprint && !self.pending.is_empty() {
            3u16
        } else {
            0
        };

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Fill(1),
                Constraint::Length(fp_height),
                Constraint::Length(1),
            ])
            .split(area);

        frame.render_widget(
            Paragraph::new(Line::from(vec![
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
            ])),
            chunks[0],
        );

        if self.pending.is_empty() {
            frame.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    "  No pending connection requests.",
                    Style::default().fg(self.muted_color()),
                ))),
                chunks[1],
            );
        } else {
            let items: Vec<ListItem> = self
                .pending
                .iter()
                .enumerate()
                .map(|(i, req)| {
                    let marker = if i == self.topo_selected { ">" } else { " " };
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
                            format!(
                                "  {:<28}",
                                format!("{}@{}", req.username, req.tenant)
                            ),
                            Style::default().fg(Color::White),
                        ),
                        Span::styled(ts.to_string(), Style::default().fg(self.muted_color())),
                    ]);
                    if i == self.topo_selected {
                        ListItem::new(line).style(Style::default().bg(Color::DarkGray))
                    } else {
                        ListItem::new(line)
                    }
                })
                .collect();
            frame.render_widget(List::new(items), chunks[1]);
        }

        if fp_height > 0 {
            let fp_text = self
                .pending
                .get(self.topo_selected)
                .and_then(|r| r.fingerprint.as_deref())
                .unwrap_or("(not yet returned by server)");
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled("  Fingerprint: ", Style::default().fg(self.muted_color())),
                    Span::styled(fp_text, Style::default().fg(self.accent_color())),
                ]))
                .block(
                    Block::default()
                        .borders(Borders::TOP)
                        .border_style(Style::default().fg(self.muted_color())),
                ),
                chunks[2],
            );
        }

        // Hint line
        let hint_line = if let Some(msg) = &self.feedback {
            let color = if msg.starts_with("Error") {
                self.warn_color()
            } else if msg.starts_with("Approved") {
                self.success_color()
            } else {
                self.error_color()
            };
            Line::from(Span::styled(msg.clone(), Style::default().fg(color)))
        } else if !self.pending.is_empty() {
            let fp_hint = if self.show_fingerprint { "[?] hide fp  " } else { "[?] show fp  " };
            Line::from(vec![
                Span::styled("[Enter] approve  ", Style::default().fg(self.muted_color())),
                Span::styled("[D] deny  ", Style::default().fg(self.muted_color())),
                Span::styled(fp_hint, Style::default().fg(self.muted_color())),
                Span::styled("[↑↓] select  ", Style::default().fg(self.muted_color())),
                Span::styled("[Tab] switch tab", Style::default().fg(self.muted_color())),
            ])
        } else {
            Line::from(Span::styled(
                "Refreshes every 5 seconds.  [Tab] switch tab",
                Style::default().fg(self.muted_color()),
            ))
        };
        frame.render_widget(Paragraph::new(hint_line), chunks[3]);
    }

    // ── LEDGER tab ─────────────────────────────────────────────────────────

    fn render_ledger(&self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // heading
                Constraint::Length(1), // column headers
                Constraint::Fill(1),   // log entries
                Constraint::Length(1), // apex footer
            ])
            .split(area);

        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(SEAL, Style::default().fg(self.accent_color())),
                Span::styled(
                    format!(" WORM Revocation Log  ({} entries)", self.ledger_log.len()),
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
            ])),
            chunks[0],
        );

        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "  #     CAPABILITY           ACTION",
                Style::default().fg(self.muted_color()),
            ))),
            chunks[1],
        );

        let items: Vec<ListItem> = self
            .ledger_log
            .iter()
            .map(|e| {
                ListItem::new(Line::from(vec![
                    Span::styled(
                        format!("  #{:04}  ", e.height),
                        Style::default().fg(self.muted_color()),
                    ),
                    Span::styled(
                        format!("{:<22}", e.cap_label),
                        Style::default().fg(self.accent_color()),
                    ),
                    Span::styled(
                        e.action.to_string(),
                        Style::default()
                            .fg(self.error_color())
                            .add_modifier(Modifier::BOLD),
                    ),
                ]))
            })
            .collect();
        frame.render_widget(List::new(items), chunks[2]);

        let apex_name = self
            .ledger
            .apex
            .current()
            .map(|a| a.name.as_str())
            .unwrap_or("none");
        let n_rev = self.ledger_log.len();
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("  Apex: ", Style::default().fg(self.muted_color())),
                Span::styled(apex_name, Style::default().fg(self.accent_color())),
                Span::styled(
                    format!("  ·  revoked: {n_rev}  ·  "),
                    Style::default().fg(self.muted_color()),
                ),
                Span::styled("[Tab] switch tab", Style::default().fg(self.muted_color())),
            ])),
            chunks[3],
        );
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
        self.drain_peers_poll();
        if self.revoke_flash > 0 {
            self.revoke_flash -= 1;
        }
        // Advance cascade animation: one entry revealed per cascade_timer expiry.
        if !self.cascade_queue.is_empty() {
            if self.cascade_timer > 0 {
                self.cascade_timer -= 1;
            } else if let Some(cascade_idx) = self.cascade_queue.first().copied() {
                self.cascade_queue.remove(0);
                let cap_hash = self.caps[cascade_idx].cap.hash();
                let label = self.caps[cascade_idx].label.to_string();
                if compute_verdict(&self.caps[cascade_idx].cap, &self.ledger) != CapVerdict::Revoked {
                    let height = self.ledger_log.len() as u64 + 2;
                    let _ = self.ledger.apply_revocation(RevocationEvent {
                        capability_hash: cap_hash,
                        revoked_at: now_secs(),
                        signed_by: "console-apex".to_string(),
                        ledger_height: height,
                    });
                    self.ledger_log.push(LedgerLogEntry {
                        height,
                        cap_label: label.clone(),
                        action: "REVOKE ⟲",
                    });
                    self.revoke_flash = 10;
                    if self.cascade_queue.is_empty() {
                        self.feedback = Some(format!("{DENY} Cascade complete — {label} struck"));
                    }
                }
                self.cascade_timer = 12;
            }
        }
    }

    fn pending_badge(&self) -> u16 {
        self.pending_count()
    }

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
                self.do_revoke_selected();
                CartridgeAction::Consumed
            }
            _ => CartridgeAction::None,
        }
    }

    fn render(&mut self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(self.muted_color()))
            .title(" F11: System — Capability Console ");
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // tab strip
                Constraint::Length(1), // separator
                Constraint::Fill(1),   // active tab content
            ])
            .split(inner);

        self.render_tab_strip(frame, chunks[0]);

        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "─".repeat(chunks[1].width as usize),
                Style::default().fg(self.muted_color()),
            ))),
            chunks[1],
        );

        let content = chunks[2];
        match self.tab {
            Tab::Graph => self.render_graph(frame, content),
            Tab::Topology => self.render_topology(frame, content),
            Tab::Ledger => self.render_ledger(frame, content),
            Tab::Peers => self.render_peers(frame, content),
        }
    }

    fn cap_verdicts(&self) -> Vec<(String, String)> {
        build_demo_caps()
            .iter()
            .map(|e| {
                let verdict_str = match compute_verdict(&e.cap, &self.ledger) {
                    CapVerdict::Allow => "✓ ALLOW".into(),
                    CapVerdict::Revoked => "✗ REVOKED".into(),
                    CapVerdict::Expired => "⟳ EXPIRED".into(),
                };
                (e.label.to_string(), verdict_str)
            })
            .collect()
    }

    fn handle_event(&mut self, event: &Event) -> CartridgeAction {
        if let Event::Key(key) = event {
            // Tab switching — active in every tab
            match key.code {
                KeyCode::Tab => {
                    self.cycle_tab_next();
                    return CartridgeAction::Consumed;
                }
                KeyCode::BackTab => {
                    self.cycle_tab_prev();
                    return CartridgeAction::Consumed;
                }
                KeyCode::Char('1') => {
                    self.tab = Tab::Graph;
                    self.feedback = None;
                    return CartridgeAction::Consumed;
                }
                KeyCode::Char('2') => {
                    self.tab = Tab::Topology;
                    self.feedback = None;
                    return CartridgeAction::Consumed;
                }
                KeyCode::Char('3') => {
                    self.tab = Tab::Ledger;
                    self.feedback = None;
                    return CartridgeAction::Consumed;
                }
                KeyCode::Char('4') => {
                    self.tab = Tab::Peers;
                    self.feedback = None;
                    return CartridgeAction::Consumed;
                }
                _ => {}
            }

            // Tab-specific bindings
            match self.tab {
                Tab::Graph => match key.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        self.graph_selected = self.graph_selected.saturating_sub(1);
                        self.feedback = None;
                        return CartridgeAction::Consumed;
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        let n = self.caps.len().saturating_sub(1);
                        self.graph_selected = (self.graph_selected + 1).min(n);
                        self.feedback = None;
                        return CartridgeAction::Consumed;
                    }
                    KeyCode::Char('r') | KeyCode::Char('R') => {
                        self.do_revoke_selected();
                        return CartridgeAction::Consumed;
                    }
                    _ => {}
                },

                Tab::Topology => match key.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        if !self.pending.is_empty() {
                            self.topo_selected = self.topo_selected.saturating_sub(1);
                        }
                        return CartridgeAction::Consumed;
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if !self.pending.is_empty() {
                            self.topo_selected = (self.topo_selected + 1)
                                .min(self.pending.len().saturating_sub(1));
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
                    KeyCode::Char('r') => {
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
                },

                Tab::Ledger => {
                    // Read-only; only tab-switching keys apply.
                }

                Tab::Peers => match key.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        if !self.peers.is_empty() {
                            self.peers_selected = self.peers_selected.saturating_sub(1);
                        }
                        return CartridgeAction::Consumed;
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if !self.peers.is_empty() {
                            self.peers_selected =
                                (self.peers_selected + 1).min(self.peers.len().saturating_sub(1));
                        }
                        return CartridgeAction::Consumed;
                    }
                    KeyCode::Char('r') | KeyCode::Char('R') => {
                        self.refresh_peers_now();
                        return CartridgeAction::Consumed;
                    }
                    _ => {}
                },
            }
        }
        CartridgeAction::None
    }
}

impl SystemCartridge {
    fn render_peers(&mut self, frame: &mut Frame, inner: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2), // heading
                Constraint::Fill(1),   // list
                Constraint::Length(1), // feedback / hint
            ])
            .split(inner);

        let heading = Paragraph::new(Line::from(vec![
            Span::styled(
                "Paired Nodes",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  ({})", self.peers.len()),
                Style::default().fg(self.muted_color()),
            ),
        ]));
        frame.render_widget(heading, chunks[0]);

        if self.peers.is_empty() {
            let msg = Paragraph::new(Line::from(Span::styled(
                "  No paired nodes.",
                Style::default().fg(self.muted_color()),
            )));
            frame.render_widget(msg, chunks[1]);
        } else {
            let items: Vec<ListItem> = self
                .peers
                .iter()
                .enumerate()
                .map(|(i, peer)| {
                    let marker = if i == self.peers_selected { ">" } else { " " };
                    let ts = peer.paired_on.get(..19).unwrap_or(&peer.paired_on);
                    let scope = if peer.archive_scope.is_empty() {
                        "(all)".to_string()
                    } else {
                        peer.archive_scope.join(",")
                    };
                    let line = Line::from(vec![
                        Span::styled(
                            format!(" {marker} "),
                            Style::default().fg(self.accent_color()),
                        ),
                        Span::styled(
                            format!("{:<16}", peer.node_label),
                            Style::default()
                                .fg(self.accent_color())
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            format!("{:<13}", peer.role),
                            Style::default().fg(Color::White),
                        ),
                        Span::styled(
                            format!("{:<22}", peer.peer_type),
                            Style::default().fg(self.muted_color()),
                        ),
                        Span::styled(
                            format!("{:<28}", scope),
                            Style::default().fg(self.muted_color()),
                        ),
                        Span::styled(ts.to_string(), Style::default().fg(self.muted_color())),
                    ]);
                    if i == self.peers_selected {
                        ListItem::new(line).style(Style::default().bg(self.muted_color()))
                    } else {
                        ListItem::new(line)
                    }
                })
                .collect();
            frame.render_widget(List::new(items), chunks[1]);
        }

        let hint = if let Some(msg) = &self.peers_feedback {
            Line::from(Span::styled(
                msg.clone(),
                Style::default().fg(self.warn_color()),
            ))
        } else {
            Line::from(vec![
                Span::styled("[Tab] cycle  ", Style::default().fg(self.muted_color())),
                Span::styled("[R] refresh  ", Style::default().fg(self.muted_color())),
                Span::styled("[↑↓] select", Style::default().fg(self.muted_color())),
            ])
        };
        frame.render_widget(Paragraph::new(hint), chunks[2]);
    }
}
