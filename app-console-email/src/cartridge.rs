use crossterm::event::{Event, KeyCode, KeyModifiers};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};
use serde::{Deserialize, Serialize};
use std::sync::mpsc;
use std::time::Duration;

use app_console_keys::cartridge::{Cartridge, CartridgeAction};
use app_console_keys::fkey::FKey;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct EmailSummary {
    pub id: String,
    pub from: String,
    pub subject: String,
    pub date: String,
    #[serde(default)]
    pub read: bool,
}

#[derive(Serialize)]
struct SendBody {
    to: String,
    subject: String,
    body: String,
}

enum EmailState {
    InboxList,
    ReadMessage,
    Compose,
    Error(String),
}

#[derive(Clone, Copy, PartialEq)]
enum ComposeField {
    To,
    Subject,
    Body,
}

enum BgMsg {
    Inbox(Vec<EmailSummary>),
    Body(String),
    SendOk,
    Err(String),
}

pub struct EmailCartridge {
    endpoint: String,
    state: EmailState,
    messages: Vec<EmailSummary>,
    list_state: ListState,
    open_body: String,
    compose_to: String,
    compose_subject: String,
    compose_body: String,
    compose_focus: ComposeField,
    status: String,
    plain: bool,
    truecolor: bool,
    refresh_tx: mpsc::SyncSender<()>,
    bg_tx: mpsc::Sender<BgMsg>,
    bg_rx: mpsc::Receiver<BgMsg>,
}

impl EmailCartridge {
    pub fn new_for(endpoint: &str, plain: bool) -> Self {
        let (refresh_tx, refresh_rx) = mpsc::sync_channel::<()>(4);
        let (bg_tx, bg_rx) = mpsc::channel::<BgMsg>();

        let ep = endpoint.to_string();
        let thread_tx = bg_tx.clone();
        std::thread::spawn(move || {
            fetch_inbox(&ep, &thread_tx);
            for _ in refresh_rx.iter() {
                fetch_inbox(&ep, &thread_tx);
            }
        });

        let mut list_state = ListState::default();
        list_state.select(Some(0));

        Self {
            endpoint: endpoint.to_string(),
            state: EmailState::InboxList,
            messages: vec![],
            list_state,
            open_body: String::new(),
            compose_to: String::new(),
            compose_subject: String::new(),
            compose_body: String::new(),
            compose_focus: ComposeField::To,
            status: "Loading inbox...".into(),
            plain,
            truecolor: false,
            refresh_tx,
            bg_tx,
            bg_rx,
        }
    }

    fn muted_color(&self) -> Color {
        app_console_keys::colors::tc_muted(self.truecolor)
    }
    fn warn_color(&self) -> Color {
        app_console_keys::colors::tc_warn(self.truecolor)
    }
    fn error_color(&self) -> Color {
        app_console_keys::colors::tc_error(self.truecolor)
    }
    fn accent_color(&self) -> Color {
        app_console_keys::colors::tc_accent(self.truecolor)
    }

    fn move_up(&mut self) {
        if self.messages.is_empty() {
            return;
        }
        let i = self.list_state.selected().unwrap_or(0);
        let next = if i == 0 {
            self.messages.len() - 1
        } else {
            i - 1
        };
        self.list_state.select(Some(next));
    }

    fn move_down(&mut self) {
        if self.messages.is_empty() {
            return;
        }
        let i = self.list_state.selected().unwrap_or(0);
        self.list_state.select(Some((i + 1) % self.messages.len()));
    }

    fn open_selected(&mut self) {
        if let Some(idx) = self.list_state.selected() {
            if let Some(msg) = self.messages.get(idx) {
                let ep = self.endpoint.clone();
                let id = msg.id.clone();
                let tx = self.bg_tx.clone();
                std::thread::spawn(move || {
                    fetch_body(&ep, &id, &tx);
                });
                self.open_body = "Loading...".into();
                self.state = EmailState::ReadMessage;
            }
        }
    }

    fn start_reply(&mut self) {
        if let Some(idx) = self.list_state.selected() {
            if let Some(msg) = self.messages.get(idx) {
                self.compose_to = msg.from.clone();
                self.compose_subject = format!("Re: {}", msg.subject);
                self.compose_body.clear();
                self.compose_focus = ComposeField::Body;
                self.state = EmailState::Compose;
            }
        }
    }

    fn start_compose(&mut self) {
        self.compose_to.clear();
        self.compose_subject.clear();
        self.compose_body.clear();
        self.compose_focus = ComposeField::To;
        self.state = EmailState::Compose;
    }

    fn do_send(&mut self) {
        let to = self.compose_to.trim().to_string();
        if to.is_empty() {
            self.state = EmailState::Error("To field is required.".into());
            return;
        }
        let ep = self.endpoint.clone();
        let send_body = SendBody {
            to,
            subject: self.compose_subject.clone(),
            body: self.compose_body.clone(),
        };
        let tx = self.bg_tx.clone();
        std::thread::spawn(move || {
            let client = match reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
            {
                Ok(c) => c,
                Err(e) => {
                    let _ = tx.send(BgMsg::Err(e.to_string()));
                    return;
                }
            };
            let url = format!("{}/v1/send", ep);
            match client.post(&url).json(&send_body).send() {
                Ok(resp) if resp.status().is_success() => {
                    let _ = tx.send(BgMsg::SendOk);
                }
                Ok(resp) => {
                    let _ = tx.send(BgMsg::Err(format!("HTTP {}", resp.status())));
                }
                Err(e) => {
                    let _ = tx.send(BgMsg::Err(e.to_string()));
                }
            }
        });
        self.status = "Sending...".into();
    }

    fn clear_compose(&mut self) {
        self.compose_to.clear();
        self.compose_subject.clear();
        self.compose_body.clear();
        self.compose_focus = ComposeField::To;
    }

    fn render_inbox(&mut self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .title(" Email — Inbox ")
            .borders(Borders::ALL);
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(inner);

        let items: Vec<ListItem> = self
            .messages
            .iter()
            .map(|m| {
                let marker = if !m.read {
                    if self.plain {
                        "[*] "
                    } else {
                        " ● "
                    }
                } else {
                    if self.plain {
                        "    "
                    } else {
                        "   "
                    }
                };
                let line = Line::from(vec![
                    Span::styled(
                        marker,
                        if !m.read && !self.plain {
                            Style::default().fg(self.accent_color())
                        } else {
                            Style::default()
                        },
                    ),
                    Span::raw(format!(
                        "{:<26} {:<34} {}",
                        truncate(&m.from, 26),
                        truncate(&m.subject, 34),
                        truncate(&m.date, 16),
                    )),
                ]);
                ListItem::new(line)
            })
            .collect();

        if items.is_empty() {
            let para = Paragraph::new(self.status.as_str());
            frame.render_widget(para, chunks[0]);
        } else {
            let list = List::new(items)
                .highlight_style(if self.plain {
                    Style::default().add_modifier(Modifier::REVERSED)
                } else {
                    Style::default()
                        .bg(self.muted_color())
                        .add_modifier(Modifier::BOLD)
                })
                .highlight_symbol("> ");
            frame.render_stateful_widget(list, chunks[0], &mut self.list_state);
        }

        let hint = Paragraph::new(" j/k=navigate  Enter=open  N=compose  R=refresh  Esc=—").style(
            Style::default().fg(if self.plain {
                Color::Reset
            } else {
                self.muted_color()
            }),
        );
        frame.render_widget(hint, chunks[1]);
    }

    fn render_read(&mut self, frame: &mut Frame, area: Rect) {
        let title = self
            .list_state
            .selected()
            .and_then(|i| self.messages.get(i))
            .map(|m| format!(" {} ", truncate(&m.subject, 48)))
            .unwrap_or_else(|| " Message ".into());

        let block = Block::default().title(title).borders(Borders::ALL);
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(inner);

        let body = Paragraph::new(self.open_body.as_str()).wrap(Wrap { trim: false });
        frame.render_widget(body, chunks[0]);

        let hint =
            Paragraph::new(" Esc/q=back  R=reply").style(Style::default().fg(if self.plain {
                Color::Reset
            } else {
                self.muted_color()
            }));
        frame.render_widget(hint, chunks[1]);
    }

    fn render_compose(&mut self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .title(" New Message — Ctrl-S to send, Esc to discard ")
            .borders(Borders::ALL);
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Min(4),
                Constraint::Length(1),
            ])
            .split(inner);

        self.render_field(
            frame,
            chunks[0],
            " To ",
            &self.compose_to.clone(),
            self.compose_focus == ComposeField::To,
        );
        self.render_field(
            frame,
            chunks[1],
            " Subject ",
            &self.compose_subject.clone(),
            self.compose_focus == ComposeField::Subject,
        );

        let body_style = if self.compose_focus == ComposeField::Body && !self.plain {
            Style::default().fg(self.warn_color())
        } else {
            Style::default()
        };
        let body_block = Block::default()
            .title(" Body ")
            .borders(Borders::ALL)
            .border_style(body_style);
        let body_para = Paragraph::new(self.compose_body.as_str())
            .block(body_block)
            .wrap(Wrap { trim: false });
        frame.render_widget(body_para, chunks[2]);

        let hint = Paragraph::new(format!(
            " Tab/Shift-Tab=field  Ctrl-S=send  Esc=discard  [{}]",
            self.status
        ))
        .style(Style::default().fg(if self.plain {
            Color::Reset
        } else {
            self.muted_color()
        }));
        frame.render_widget(hint, chunks[3]);
    }

    fn render_field(&self, frame: &mut Frame, area: Rect, label: &str, value: &str, active: bool) {
        let style = if active && !self.plain {
            Style::default().fg(self.warn_color())
        } else {
            Style::default()
        };
        let block = Block::default()
            .title(label)
            .borders(Borders::ALL)
            .border_style(style);
        let para = Paragraph::new(value).block(block);
        frame.render_widget(para, area);
    }

    fn render_error(&mut self, frame: &mut Frame, area: Rect, msg: String) {
        let title = if self.plain {
            " [ERROR] "
        } else {
            " ✗ Error "
        };
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(if self.plain {
                Style::default()
            } else {
                Style::default().fg(self.error_color())
            });
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let para = Paragraph::new(format!("{}\n\nPress Esc to return.", msg))
            .style(if self.plain {
                Style::default()
            } else {
                Style::default().fg(self.error_color())
            })
            .wrap(Wrap { trim: false });
        frame.render_widget(para, inner);
    }

    fn handle_inbox_key(&mut self, code: KeyCode, mods: KeyModifiers) -> CartridgeAction {
        match (code, mods) {
            (KeyCode::Char('j'), _) | (KeyCode::Down, _) => {
                self.move_down();
                CartridgeAction::Consumed
            }
            (KeyCode::Char('k'), _) | (KeyCode::Up, _) => {
                self.move_up();
                CartridgeAction::Consumed
            }
            (KeyCode::Enter, _) => {
                self.open_selected();
                CartridgeAction::Consumed
            }
            (KeyCode::Char('n'), _) | (KeyCode::Char('N'), _) => {
                self.start_compose();
                CartridgeAction::Consumed
            }
            (KeyCode::Char('r'), _) | (KeyCode::Char('R'), _) => {
                self.status = "Refreshing...".into();
                let _ = self.refresh_tx.try_send(());
                CartridgeAction::Consumed
            }
            _ => CartridgeAction::None,
        }
    }

    fn handle_read_key(&mut self, code: KeyCode, _mods: KeyModifiers) -> CartridgeAction {
        match code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.state = EmailState::InboxList;
                CartridgeAction::Consumed
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                self.start_reply();
                CartridgeAction::Consumed
            }
            _ => CartridgeAction::None,
        }
    }

    fn handle_compose_key(&mut self, code: KeyCode, mods: KeyModifiers) -> CartridgeAction {
        match (code, mods) {
            (KeyCode::Esc, _) => {
                self.clear_compose();
                self.state = EmailState::InboxList;
                CartridgeAction::Consumed
            }
            (KeyCode::Tab, _) => {
                self.compose_focus = match self.compose_focus {
                    ComposeField::To => ComposeField::Subject,
                    ComposeField::Subject => ComposeField::Body,
                    ComposeField::Body => ComposeField::To,
                };
                CartridgeAction::Consumed
            }
            (KeyCode::BackTab, _) => {
                self.compose_focus = match self.compose_focus {
                    ComposeField::To => ComposeField::Body,
                    ComposeField::Subject => ComposeField::To,
                    ComposeField::Body => ComposeField::Subject,
                };
                CartridgeAction::Consumed
            }
            (KeyCode::Char('s'), m) if m.contains(KeyModifiers::CONTROL) => {
                self.do_send();
                CartridgeAction::Consumed
            }
            (KeyCode::Enter, _) => {
                if self.compose_focus == ComposeField::Body {
                    self.compose_body.push('\n');
                }
                CartridgeAction::Consumed
            }
            (KeyCode::Backspace, _) => {
                let field = match self.compose_focus {
                    ComposeField::To => &mut self.compose_to,
                    ComposeField::Subject => &mut self.compose_subject,
                    ComposeField::Body => &mut self.compose_body,
                };
                field.pop();
                CartridgeAction::Consumed
            }
            (KeyCode::Char(c), _) => {
                let field = match self.compose_focus {
                    ComposeField::To => &mut self.compose_to,
                    ComposeField::Subject => &mut self.compose_subject,
                    ComposeField::Body => &mut self.compose_body,
                };
                field.push(c);
                CartridgeAction::Consumed
            }
            _ => CartridgeAction::None,
        }
    }
}

impl Cartridge for EmailCartridge {
    fn fkey(&self) -> FKey {
        FKey::F3
    }

    fn title(&self) -> &str {
        "Email"
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
                BgMsg::Inbox(msgs) => {
                    let count = msgs.len();
                    self.messages = msgs;
                    if self.list_state.selected().is_none() && count > 0 {
                        self.list_state.select(Some(0));
                    }
                    self.status = format!("{} message{}", count, if count == 1 { "" } else { "s" });
                }
                BgMsg::Body(body) => {
                    self.open_body = body;
                }
                BgMsg::SendOk => {
                    self.clear_compose();
                    self.state = EmailState::InboxList;
                    self.status = if self.plain {
                        "[OK] Sent."
                    } else {
                        "✓ Sent."
                    }
                    .into();
                    let _ = self.refresh_tx.try_send(());
                }
                BgMsg::Err(e) => {
                    self.state = EmailState::Error(e);
                }
            }
        }
    }

    fn render(&mut self, frame: &mut Frame, area: Rect) {
        // Extract error message before any &mut self calls to avoid borrow conflict.
        let error_msg: Option<String> = if let EmailState::Error(ref msg) = self.state {
            Some(msg.clone())
        } else {
            None
        };
        if matches!(self.state, EmailState::InboxList) {
            self.render_inbox(frame, area);
        } else if matches!(self.state, EmailState::ReadMessage) {
            self.render_read(frame, area);
        } else if matches!(self.state, EmailState::Compose) {
            self.render_compose(frame, area);
        } else if let Some(msg) = error_msg {
            self.render_error(frame, area, msg);
        }
    }

    fn handle_event(&mut self, event: &Event) -> CartridgeAction {
        let Event::Key(key) = event else {
            return CartridgeAction::None;
        };
        // Use if-matches! chain so each check releases the borrow before the &mut self call.
        if matches!(self.state, EmailState::InboxList) {
            self.handle_inbox_key(key.code, key.modifiers)
        } else if matches!(self.state, EmailState::ReadMessage) {
            self.handle_read_key(key.code, key.modifiers)
        } else if matches!(self.state, EmailState::Compose) {
            self.handle_compose_key(key.code, key.modifiers)
        } else if matches!(self.state, EmailState::Error(_)) {
            if key.code == KeyCode::Esc {
                self.state = EmailState::InboxList;
                CartridgeAction::Consumed
            } else {
                CartridgeAction::None
            }
        } else {
            CartridgeAction::None
        }
    }
}

fn fetch_inbox(endpoint: &str, tx: &mpsc::Sender<BgMsg>) {
    let client = match reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            let _ = tx.send(BgMsg::Err(format!("client build: {e}")));
            return;
        }
    };
    let url = format!("{}/v1/inbox", endpoint);
    match client.get(&url).send() {
        Ok(resp) if resp.status().is_success() => match resp.json::<Vec<EmailSummary>>() {
            Ok(msgs) => {
                let _ = tx.send(BgMsg::Inbox(msgs));
            }
            Err(e) => {
                let _ = tx.send(BgMsg::Err(format!("parse: {e}")));
            }
        },
        Ok(resp) => {
            let _ = tx.send(BgMsg::Err(format!("HTTP {}", resp.status())));
        }
        Err(e) => {
            let _ = tx.send(BgMsg::Err(format!("service-email unavailable: {e}")));
        }
    }
}

fn fetch_body(endpoint: &str, id: &str, tx: &mpsc::Sender<BgMsg>) {
    let client = match reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            let _ = tx.send(BgMsg::Err(format!("client build: {e}")));
            return;
        }
    };
    let url = format!("{}/v1/message/{}", endpoint, id);
    match client.get(&url).send() {
        Ok(resp) if resp.status().is_success() => match resp.text() {
            Ok(body) => {
                let _ = tx.send(BgMsg::Body(body));
            }
            Err(e) => {
                let _ = tx.send(BgMsg::Err(format!("read body: {e}")));
            }
        },
        Ok(resp) => {
            let _ = tx.send(BgMsg::Err(format!("HTTP {}", resp.status())));
        }
        Err(_) => {
            let _ = tx.send(BgMsg::Body(
                "(message body unavailable — service-email not running)".into(),
            ));
        }
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}
