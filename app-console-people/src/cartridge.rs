use crossterm::event::{Event, KeyCode};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};
use std::sync::mpsc;

use app_console_keys::{
    cartridge::{Cartridge, CartridgeAction},
    colors::{tc_accent, tc_muted},
    fkey::FKey,
    IntentArgs, IntentId, IntentScope, IntentSpec, MouseAffordance,
};

#[derive(Debug, Clone)]
struct Person {
    id: String,
    name: String,
    email: String,
    role: Option<String>,
}

enum BgMsg {
    People(Vec<Person>),
    Err(String),
}

enum PeopleView {
    List,
    Detail(usize),
}

pub struct PeopleCartridge {
    truecolor: bool,
    people: Vec<Person>,
    list_state: ListState,
    view: PeopleView,
    status: String,
    bg_rx: mpsc::Receiver<BgMsg>,
    refresh_tx: mpsc::SyncSender<()>,
}

impl PeopleCartridge {
    pub fn new_for(endpoint: &str) -> Self {
        let (refresh_tx, refresh_rx) = mpsc::sync_channel::<()>(4);
        let (bg_tx, bg_rx) = mpsc::channel::<BgMsg>();

        let ep = endpoint.to_string();
        std::thread::spawn(move || fetch_loop(ep, bg_tx, refresh_rx));

        Self {
            truecolor: false,
            people: Vec::new(),
            list_state: ListState::default(),
            view: PeopleView::List,
            status: "Press R to load contacts".into(),
            bg_rx,
            refresh_tx,
        }
    }

    fn trigger_refresh(&self) {
        let _ = self.refresh_tx.try_send(());
    }

    fn drain_bg(&mut self) {
        while let Ok(msg) = self.bg_rx.try_recv() {
            match msg {
                BgMsg::People(people) => {
                    let n = people.len();
                    self.people = people;
                    self.status = format!("{} contacts", n);
                    if !self.people.is_empty() && self.list_state.selected().is_none() {
                        self.list_state.select(Some(0));
                    }
                }
                BgMsg::Err(e) => {
                    self.status = format!("Error: {}", e);
                }
            }
        }
    }

    fn render_list(&mut self, frame: &mut Frame, area: Rect) {
        let muted = tc_muted(self.truecolor);
        let accent = tc_accent(self.truecolor);
        let status_text = self.status.clone();

        let outer = Block::default()
            .title(" F2 — People ")
            .borders(Borders::ALL);
        let inner = outer.inner(area);
        frame.render_widget(outer, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Fill(1), Constraint::Length(1)])
            .split(inner);

        let items: Vec<ListItem> = self
            .people
            .iter()
            .map(|p| {
                let role_tag = p
                    .role
                    .as_deref()
                    .map(|r| format!(" [{}]", r))
                    .unwrap_or_default();
                ListItem::new(Line::from(vec![
                    Span::styled(p.name.clone(), Style::default().fg(Color::White)),
                    Span::styled(format!("  {}", p.email), Style::default().fg(muted)),
                    Span::styled(role_tag, Style::default().fg(accent)),
                ]))
            })
            .collect();

        let list = List::new(items)
            .highlight_style(
                Style::default()
                    .bg(accent)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("▶ ");
        frame.render_stateful_widget(list, chunks[0], &mut self.list_state);

        let hint = Line::from(vec![
            Span::styled(" j/k ", Style::default().fg(accent)),
            Span::raw("nav  "),
            Span::styled(" Enter ", Style::default().fg(accent)),
            Span::raw("detail  "),
            Span::styled(" N ", Style::default().fg(accent)),
            Span::raw("new  "),
            Span::styled(" R ", Style::default().fg(accent)),
            Span::raw("refresh    "),
            Span::styled(status_text, Style::default().fg(muted)),
        ]);
        frame.render_widget(Paragraph::new(hint), chunks[1]);
    }

    fn render_detail_inner(
        frame: &mut Frame,
        area: Rect,
        person: Option<&Person>,
        truecolor: bool,
    ) {
        let muted = tc_muted(truecolor);
        let accent = tc_accent(truecolor);

        let Some(person) = person else {
            frame.render_widget(Paragraph::new("  Contact not found"), area);
            return;
        };

        let outer = Block::default()
            .title(format!(" {} ", person.name))
            .borders(Borders::ALL);
        let inner = outer.inner(area);
        frame.render_widget(outer, area);

        let lines = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("  Name:   ", Style::default().fg(muted)),
                Span::styled(
                    person.name.clone(),
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled("  Email:  ", Style::default().fg(muted)),
                Span::styled(person.email.clone(), Style::default().fg(accent)),
            ]),
            Line::from(vec![
                Span::styled("  Role:   ", Style::default().fg(muted)),
                Span::styled(
                    person.role.as_deref().unwrap_or("—").to_string(),
                    Style::default().fg(Color::White),
                ),
            ]),
            Line::from(vec![
                Span::styled("  ID:     ", Style::default().fg(muted)),
                Span::styled(person.id.clone(), Style::default().fg(muted)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled(" E ", Style::default().fg(accent)),
                Span::raw("edit  "),
                Span::styled(" D ", Style::default().fg(accent)),
                Span::raw("delete  "),
                Span::styled(" Esc ", Style::default().fg(accent)),
                Span::raw("back"),
            ]),
        ];
        frame.render_widget(Paragraph::new(lines), inner);
    }
}

fn fetch_loop(endpoint: String, tx: mpsc::Sender<BgMsg>, rx: mpsc::Receiver<()>) {
    loop {
        if rx.recv().is_err() {
            break;
        }
        let url = format!("{}/v1/people", endpoint);
        match reqwest::blocking::get(&url) {
            Ok(resp) if resp.status().is_success() => match resp.json::<serde_json::Value>() {
                Ok(json) => {
                    let people = json
                        .as_array()
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| {
                                    Some(Person {
                                        id: v["id"].as_str()?.to_string(),
                                        name: v["name"].as_str().unwrap_or("Unknown").to_string(),
                                        email: v["email"].as_str().unwrap_or("").to_string(),
                                        role: v["role"].as_str().map(|s| s.to_string()),
                                    })
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    let _ = tx.send(BgMsg::People(people));
                }
                Err(e) => {
                    let _ = tx.send(BgMsg::Err(e.to_string()));
                }
            },
            Ok(resp) => {
                let _ = tx.send(BgMsg::Err(format!("HTTP {}", resp.status())));
            }
            Err(e) => {
                let _ = tx.send(BgMsg::Err(e.to_string()));
            }
        }
    }
}

impl Cartridge for PeopleCartridge {
    fn fkey(&self) -> FKey {
        FKey::F2
    }

    fn title(&self) -> &str {
        "People"
    }

    fn tick(&mut self) {
        self.drain_bg();
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
        self.drain_bg();
        let detail_idx = match self.view {
            PeopleView::List => None,
            PeopleView::Detail(i) => Some(i),
        };
        match detail_idx {
            None => self.render_list(frame, area),
            Some(idx) => {
                let person = self.people.get(idx).cloned();
                let truecolor = self.truecolor;
                Self::render_detail_inner(frame, area, person.as_ref(), truecolor);
            }
        }
    }

    fn handle_event(&mut self, event: &Event) -> CartridgeAction {
        let Event::Key(key) = event else {
            return CartridgeAction::None;
        };

        let in_list = matches!(self.view, PeopleView::List);
        let detail_idx = if let PeopleView::Detail(i) = self.view {
            Some(i)
        } else {
            None
        };

        if in_list {
            match key.code {
                KeyCode::Char('r') | KeyCode::Char('R') => {
                    self.status = "Loading\u{2026}".into();
                    self.trigger_refresh();
                }
                KeyCode::Char('j') | KeyCode::Down => {
                    let next = self
                        .list_state
                        .selected()
                        .map(|i| (i + 1).min(self.people.len().saturating_sub(1)))
                        .unwrap_or(0);
                    self.list_state.select(Some(next));
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    let prev = self
                        .list_state
                        .selected()
                        .map(|i| i.saturating_sub(1))
                        .unwrap_or(0);
                    self.list_state.select(Some(prev));
                }
                KeyCode::Enter => {
                    if let Some(idx) = self.list_state.selected() {
                        if idx < self.people.len() {
                            self.view = PeopleView::Detail(idx);
                        }
                    }
                }
                KeyCode::Char('n') | KeyCode::Char('N') => {
                    self.status = "New contact \u{2014} not yet implemented".into();
                }
                _ => return CartridgeAction::None,
            }
        } else if let Some(_idx) = detail_idx {
            match key.code {
                KeyCode::Esc | KeyCode::Char('q') => {
                    self.view = PeopleView::List;
                }
                KeyCode::Char('e') | KeyCode::Char('E') => {
                    self.status = "Edit \u{2014} not yet implemented".into();
                    self.view = PeopleView::List;
                }
                KeyCode::Char('d') | KeyCode::Char('D') => {
                    self.status = "Delete \u{2014} not yet implemented".into();
                    self.view = PeopleView::List;
                }
                _ => return CartridgeAction::None,
            }
        }

        CartridgeAction::Consumed
    }

    fn intent_scope(&self) -> Option<&'static str> {
        Some("people")
    }

    fn intents(&self) -> Vec<IntentSpec> {
        vec![
            IntentSpec::new(
                "people.refresh",
                "Refresh contact list",
                IntentScope::Cartridge("people"),
            )
            .key("r")
            .mouse(MouseAffordance::CLICK),
            IntentSpec::new(
                "people.view_detail",
                "Open contact detail",
                IntentScope::Cartridge("people"),
            )
            .key("enter")
            .mouse(MouseAffordance::CLICK),
        ]
    }

    fn dispatch(&mut self, id: IntentId, _args: &IntentArgs) -> CartridgeAction {
        match id.0 {
            "people.refresh" => {
                self.status = "Loading\u{2026}".into();
                self.trigger_refresh();
                CartridgeAction::Consumed
            }
            "people.view_detail" => {
                if let PeopleView::List = self.view {
                    if let Some(idx) = self.list_state.selected() {
                        if idx < self.people.len() {
                            self.view = PeopleView::Detail(idx);
                        }
                    }
                }
                CartridgeAction::Consumed
            }
            _ => CartridgeAction::None,
        }
    }
}
