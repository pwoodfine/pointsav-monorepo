use crossterm::event::{Event, KeyCode};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use app_console_keys::cartridge::{Cartridge, CartridgeAction};
use app_console_keys::fkey::FKey;

use crate::health::{
    fetch_cost, fetch_queue, fetch_readyz, fetch_tier_a, fetch_yoyo, CostStatus, DoormanHealth,
    QueueStatus, TierAStatus, YoyoStatus,
};

const POLL_INTERVAL: Duration = Duration::from_secs(10);

struct AllStatus {
    health: DoormanHealth,
    queue: Option<QueueStatus>,
    cost: Option<CostStatus>,
    tier_a: Option<TierAStatus>,
    yoyo: Option<YoyoStatus>,
}

enum BgMsg {
    Status(Box<AllStatus>),
    Err(String),
}

pub struct SlmCartridge {
    endpoint: String,
    status: Option<AllStatus>,
    last_updated: Option<Instant>,
    error: Option<String>,
    plain: bool,
    truecolor: bool,
    refresh_tx: mpsc::SyncSender<()>,
    bg_rx: mpsc::Receiver<BgMsg>,
    show_help: bool,
}

impl SlmCartridge {
    pub fn new(endpoint: &str, plain: bool) -> Self {
        let (refresh_tx, refresh_rx) = mpsc::sync_channel::<()>(4);
        let (bg_tx, bg_rx) = mpsc::channel::<BgMsg>();

        let ep = endpoint.to_string();
        std::thread::spawn(move || {
            poll_loop(ep, bg_tx, refresh_rx);
        });

        Self {
            endpoint: endpoint.to_string(),
            status: None,
            last_updated: None,
            error: None,
            plain,
            truecolor: false,
            refresh_tx,
            bg_rx,
            show_help: false,
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

    fn trigger_refresh(&mut self) {
        let _ = self.refresh_tx.try_send(());
        self.error = None;
    }

    fn render_dashboard(&self, frame: &mut Frame, area: Rect) {
        let outer = Block::default()
            .title(" F9 — SLM Infrastructure ")
            .borders(Borders::ALL);
        let inner = outer.inner(area);
        frame.render_widget(outer, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(6), // Gateway (4 inner lines)
                Constraint::Length(5), // YoYo Fleet (3 inner lines)
                Constraint::Length(4), // DataGraph (2 inner lines)
                Constraint::Length(4), // Queue (2 inner lines)
                Constraint::Length(3), // Cost Today (1 inner line)
                Constraint::Min(1),    // Controls hint
            ])
            .split(inner);

        // Gateway section
        let gw_block = Block::default().title(" Gateway ").borders(Borders::ALL);
        let gw_inner = gw_block.inner(chunks[0]);
        frame.render_widget(gw_block, chunks[0]);
        frame.render_widget(Paragraph::new(self.build_gateway_lines()), gw_inner);

        // YoYo Fleet section
        let yoyo_block = Block::default().title(" YoYo Fleet ").borders(Borders::ALL);
        let yoyo_inner = yoyo_block.inner(chunks[1]);
        frame.render_widget(yoyo_block, chunks[1]);
        frame.render_widget(Paragraph::new(self.build_yoyo_lines()), yoyo_inner);

        // DataGraph section
        let graph_block = Block::default().title(" DataGraph ").borders(Borders::ALL);
        let graph_inner = graph_block.inner(chunks[2]);
        frame.render_widget(graph_block, chunks[2]);
        frame.render_widget(Paragraph::new(self.build_graph_lines()), graph_inner);

        // Queue section
        let queue_block = Block::default().title(" Queue ").borders(Borders::ALL);
        let queue_inner = queue_block.inner(chunks[3]);
        frame.render_widget(queue_block, chunks[3]);
        frame.render_widget(Paragraph::new(self.build_queue_lines()), queue_inner);

        // Cost Today section
        let cost_block = Block::default().title(" Cost Today ").borders(Borders::ALL);
        let cost_inner = cost_block.inner(chunks[4]);
        frame.render_widget(cost_block, chunks[4]);
        frame.render_widget(Paragraph::new(self.build_cost_lines()), cost_inner);

        // Controls hint
        let updated = self
            .last_updated
            .map(|t| {
                let secs = t.elapsed().as_secs();
                if secs < 60 {
                    format!("{}s ago", secs)
                } else {
                    format!("{}m ago", secs / 60)
                }
            })
            .unwrap_or_else(|| "never".into());
        let hint = Paragraph::new(format!(" R=refresh  ?=help  [updated {}]", updated)).style(
            Style::default().fg(if self.plain {
                Color::Reset
            } else {
                self.muted_color()
            }),
        );
        frame.render_widget(hint, chunks[5]);
    }

    fn build_gateway_lines(&self) -> Vec<Line<'static>> {
        let Some(s) = &self.status else {
            if let Some(e) = &self.error {
                return vec![
                    Line::from(vec![
                        Span::styled(
                            if self.plain { "[ERROR] " } else { "✗  " },
                            Style::default().fg(self.error_color()),
                        ),
                        Span::raw(e.clone()),
                    ]),
                    Line::raw("   Doorman unreachable"),
                ];
            }
            return vec![Line::raw("   Loading...")];
        };
        let h = &s.health;

        let status_span = Span::styled(
            if self.plain {
                "[RUNNING]"
            } else {
                "● running"
            },
            Style::default()
                .fg(self.success_color())
                .add_modifier(Modifier::BOLD),
        );

        let tok_label = match &s.tier_a {
            Some(t) if t.reachable => match t.tok_per_s {
                Some(v) => format!("{:.2} tok/s", v),
                None => "? tok/s".into(),
            },
            _ => "? tok/s".into(),
        };

        let tier_a_icon = if h.tier_a {
            if self.plain {
                "[A]"
            } else {
                "✓"
            }
        } else {
            if self.plain {
                "[ ]"
            } else {
                "○"
            }
        };
        let tier_a_style = if h.tier_a {
            Style::default().fg(self.success_color())
        } else {
            Style::default().fg(self.muted_color())
        };

        let circuit = if h.tier_b_circuit_state.is_empty() {
            "—".to_string()
        } else {
            h.tier_b_circuit_state.clone()
        };

        let node_class = h.node_class.as_deref().unwrap_or("—");

        vec![
            Line::from(vec![
                Span::raw("  Status:    "),
                status_span,
                Span::raw(format!("  {}", self.endpoint)),
            ]),
            Line::from(vec![
                Span::raw("  Tier A:    "),
                Span::styled(tier_a_icon.to_string(), tier_a_style),
                Span::raw(format!("  {}   Tier B circuit: {}", tok_label, circuit)),
            ]),
            Line::raw(format!("  Node:       {}", node_class)),
            Line::raw(format!(
                "  Reason:     {}",
                h.tier_a_reason.as_deref().unwrap_or("—")
            )),
        ]
    }

    fn build_yoyo_lines(&self) -> Vec<Line<'static>> {
        let Some(s) = &self.status else {
            return vec![Line::raw("  —")];
        };

        match &s.yoyo {
            None => vec![Line::raw("  (unavailable)")],
            Some(y) if y.nodes.is_empty() => vec![
                Line::raw("  No Yo-Yo nodes configured"),
                Line::raw("  (Tier B offline — batch + express nodes pending)"),
            ],
            Some(y) => {
                let mut lines: Vec<Line<'static>> = Vec::new();
                for (name, info) in &y.nodes {
                    let state = info
                        .get("state")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    let state_style = match state {
                        "Available" => Style::default().fg(self.success_color()),
                        "Stopped" | "FailedStart" | "Zombie" => {
                            Style::default().fg(self.muted_color())
                        }
                        _ => Style::default(),
                    };
                    lines.push(Line::from(vec![
                        Span::raw(format!("  {:12}  ", name)),
                        Span::styled(state.to_string(), state_style),
                    ]));
                }
                lines
            }
        }
    }

    fn build_graph_lines(&self) -> Vec<Line<'static>> {
        let Some(s) = &self.status else {
            return vec![Line::raw("  Entities:  —")];
        };
        let h = &s.health;
        let count = h
            .entity_count
            .map(|n| {
                if n >= 1_000 {
                    format!("{},{:03}", n / 1_000, n % 1_000)
                } else {
                    n.to_string()
                }
            })
            .unwrap_or_else(|| "—".into());
        let circuit_style = if h.tier_b_circuit_state == "open" {
            Style::default().fg(self.warn_color())
        } else {
            Style::default()
        };
        vec![
            Line::raw(format!("  Entities:  {}", count)),
            Line::from(vec![
                Span::raw("  Circuit:   "),
                Span::styled(
                    if h.tier_b_circuit_state.is_empty() {
                        "—".to_string()
                    } else {
                        h.tier_b_circuit_state.clone()
                    },
                    circuit_style,
                ),
            ]),
        ]
    }

    fn build_queue_lines(&self) -> Vec<Line<'static>> {
        let Some(s) = &self.status else {
            return vec![Line::raw("  —")];
        };

        let q = match &s.queue {
            Some(q) => q.clone(),
            None => {
                // Fall back to fields from /readyz
                let h = &s.health;
                QueueStatus {
                    pending: h.queue_pending.unwrap_or(0),
                    done: h.queue_done.unwrap_or(0),
                    poison: h.queue_poison.unwrap_or(0),
                    ..Default::default()
                }
            }
        };

        let poison_style = if q.poison > 0 {
            Style::default().fg(self.error_color())
        } else {
            Style::default()
        };

        vec![
            Line::from(vec![Span::raw(format!(
                "  pending: {:4}  in-flight: {:2}  paused: {:3}",
                q.pending, q.in_flight, q.paused
            ))]),
            Line::from(vec![
                Span::raw(format!(
                    "  done: {:6}  quarantine: {:3}  poison: ",
                    q.done, q.quarantine
                )),
                Span::styled(q.poison.to_string(), poison_style),
            ]),
        ]
    }

    fn build_cost_lines(&self) -> Vec<Line<'static>> {
        let Some(s) = &self.status else {
            return vec![Line::raw("  —")];
        };

        match &s.cost {
            None => vec![Line::raw("  (unavailable)")],
            Some(c) if !c.ledger_available => vec![Line::raw("  Ledger unavailable")],
            Some(c) => {
                vec![Line::raw(format!(
                    "  ${:.4}  yoyo: ${:.4}  vm: ${:.4}  reqs: {}",
                    c.daily_usd, c.yoyo_usd, c.vm_hours_usd, c.request_count
                ))]
            }
        }
    }

    fn render_help(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .title(" F9 SLM — Help ")
            .borders(Borders::ALL);
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let text = format!(
            "Endpoint: {}\n\nKeybindings:\n  R       Refresh all status\n  ?       Toggle this help\n  Esc     Return to previous cartridge\n\nData sources:\n  /readyz           gateway status, entity_count, circuit\n  /v1/status/queue  full queue breakdown\n  /v1/status/cost   today's cost rollup\n  /v1/status/tier-a llama-server tok/s\n  /v1/status/yoyo   Yo-Yo fleet node states",
            self.endpoint
        );
        frame.render_widget(Paragraph::new(text), inner);
    }
}

impl Cartridge for SlmCartridge {
    fn fkey(&self) -> FKey {
        FKey::F9
    }

    fn title(&self) -> &str {
        "SLM"
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
        while let Ok(msg) = self.bg_rx.try_recv() {
            match msg {
                BgMsg::Status(s) => {
                    self.status = Some(*s);
                    self.last_updated = Some(Instant::now());
                    self.error = None;
                }
                BgMsg::Err(e) => {
                    self.error = Some(e);
                    self.last_updated = Some(Instant::now());
                }
            }
        }
    }

    fn render(&mut self, frame: &mut Frame, area: Rect) {
        if self.show_help {
            self.render_help(frame, area);
        } else {
            self.render_dashboard(frame, area);
        }
    }

    fn handle_event(&mut self, event: &Event) -> CartridgeAction {
        if let Event::Key(key) = event {
            match key.code {
                KeyCode::Char('r') | KeyCode::Char('R') => {
                    self.trigger_refresh();
                    CartridgeAction::Consumed
                }
                KeyCode::Char('?') => {
                    self.show_help = !self.show_help;
                    CartridgeAction::Consumed
                }
                KeyCode::Esc if self.show_help => {
                    self.show_help = false;
                    CartridgeAction::Consumed
                }
                _ => CartridgeAction::None,
            }
        } else {
            CartridgeAction::None
        }
    }
}

fn poll_loop(endpoint: String, tx: mpsc::Sender<BgMsg>, refresh_rx: mpsc::Receiver<()>) {
    let _ = do_fetch(&endpoint, &tx);

    while let Ok(()) | Err(mpsc::RecvTimeoutError::Timeout) = refresh_rx.recv_timeout(POLL_INTERVAL)
    {
        if do_fetch(&endpoint, &tx).is_err() {
            break;
        }
    }
}

fn do_fetch(endpoint: &str, tx: &mpsc::Sender<BgMsg>) -> Result<(), ()> {
    let msg = match fetch_readyz(endpoint) {
        Err(e) => BgMsg::Err(e.to_string()),
        Ok(health) => {
            let queue = fetch_queue(endpoint).ok();
            let cost = fetch_cost(endpoint).ok();
            let tier_a = fetch_tier_a(endpoint).ok();
            let yoyo = fetch_yoyo(endpoint).ok();
            BgMsg::Status(Box::new(AllStatus {
                health,
                queue,
                cost,
                tier_a,
                yoyo,
            }))
        }
    };
    tx.send(msg).map_err(|_| ())
}
