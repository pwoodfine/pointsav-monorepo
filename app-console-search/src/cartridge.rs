use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use app_console_keys::{Cartridge, CartridgeAction, FKey};
use crossterm::event::{Event, KeyCode, KeyModifiers};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
    Frame,
};
use serde::Deserialize;

const SPINNER: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

#[derive(Debug, Clone, Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub slug: String,
    #[serde(default)]
    pub excerpt: String,
    #[serde(default)]
    pub redacted: bool,
    #[serde(default)]
    pub refuse_reason: Option<String>,
}

#[derive(Deserialize)]
struct SearchResponse {
    results: Vec<SearchResult>,
}

fn fetch_search(endpoint: &str, query: &str) -> anyhow::Result<Vec<SearchResult>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;
    let url = format!("{}/v1/search", endpoint.trim_end_matches('/'));
    let resp = client
        .get(&url)
        .query(&[("q", query), ("limit", "20")])
        .send()?;
    if resp.status().is_success() {
        Ok(resp.json::<SearchResponse>()?.results)
    } else {
        anyhow::bail!("HTTP {}", resp.status())
    }
}

enum SearchState {
    Idle { query: String },
    Searching {
        query: String,
        rx: mpsc::Receiver<anyhow::Result<Vec<SearchResult>>>,
        spinner: usize,
    },
    Results {
        query: String,
        results: Vec<SearchResult>,
        selected: usize,
        scroll: u16,
    },
    Error {
        query: String,
        message: String,
    },
}

pub struct SearchCartridge {
    content_endpoint: String,
    state: SearchState,
    query_cursor: usize,
}

impl SearchCartridge {
    pub fn new() -> Self {
        Self::new_for("http://127.0.0.1:9081")
    }

    pub fn new_for(content_endpoint: impl Into<String>) -> Self {
        Self {
            content_endpoint: content_endpoint.into(),
            state: SearchState::Idle {
                query: String::new(),
            },
            query_cursor: 0,
        }
    }

    fn submit_search(&mut self) {
        let query = match &self.state {
            SearchState::Idle { query } | SearchState::Results { query, .. } | SearchState::Error { query, .. } => {
                query.clone()
            }
            SearchState::Searching { query, .. } => query.clone(),
        };
        if query.trim().is_empty() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        let ep = self.content_endpoint.clone();
        let q = query.clone();
        thread::spawn(move || {
            let _ = tx.send(fetch_search(&ep, &q));
        });
        self.state = SearchState::Searching {
            query,
            rx,
            spinner: 0,
        };
    }

    fn current_query(&self) -> &str {
        match &self.state {
            SearchState::Idle { query } => query.as_str(),
            SearchState::Searching { query, .. } => query.as_str(),
            SearchState::Results { query, .. } => query.as_str(),
            SearchState::Error { query, .. } => query.as_str(),
        }
    }

    fn render_idle(frame: &mut Frame, area: Rect, query: &str, cursor: usize) {
        let outer = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title(" F5: Search — [Enter: search  Esc: clear  q: quit] ");
        let inner = outer.inner(area);
        frame.render_widget(outer, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Fill(1)])
            .split(inner);

        let before = &query[..cursor.min(query.len())];
        let cursor_char = query[cursor.min(query.len())..].chars().next().unwrap_or(' ');
        let after = if cursor < query.len() {
            &query[cursor + cursor_char.len_utf8()..]
        } else {
            ""
        };
        let query_line = Line::from(vec![
            Span::styled("  ⌕ ", Style::default().fg(Color::Cyan)),
            Span::raw(before.to_string()),
            Span::styled(cursor_char.to_string(), Style::default().fg(Color::Black).bg(Color::Cyan)),
            Span::raw(after.to_string()),
        ]);
        frame.render_widget(
            Paragraph::new(query_line).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan))
                    .title(" Query "),
            ),
            chunks[0],
        );

        frame.render_widget(
            Paragraph::new("  Type to search — press Enter to run.")
                .style(Style::default().fg(Color::DarkGray)),
            chunks[1],
        );
    }

    fn render_searching(frame: &mut Frame, area: Rect, query: &str, spinner: usize) {
        let outer = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow))
            .title(" F5: Search — Searching... ");
        let inner = outer.inner(area);
        frame.render_widget(outer, area);

        let mid = Rect { y: inner.y + inner.height / 2, height: 2, ..inner };
        frame.render_widget(
            Paragraph::new(format!(
                "  {} Searching for \"{}\"…",
                SPINNER[spinner % SPINNER.len()],
                query
            ))
            .style(Style::default().fg(Color::Yellow)),
            mid,
        );
    }

    fn render_results(
        frame: &mut Frame,
        area: Rect,
        query: &str,
        results: &[SearchResult],
        selected: usize,
        scroll: u16,
    ) {
        let outer = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Green))
            .title(format!(
                " F5: Search — {} result(s) for \"{}\"    [j/k: move  S: send to Proofreader  Esc: clear] ",
                results.len(),
                query
            ));
        let inner = outer.inner(area);
        frame.render_widget(outer, area);

        if results.is_empty() {
            frame.render_widget(
                Paragraph::new(format!("  No results for \"{}\".", query))
                    .style(Style::default().fg(Color::DarkGray)),
                inner,
            );
            return;
        }

        let items: Vec<ListItem> = results
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let is_sel = i == selected;
                let (icon, title_color) = if r.redacted {
                    ("▤ ", Color::DarkGray)
                } else {
                    ("› ", Color::White)
                };
                let style = if is_sel {
                    Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(title_color)
                };
                let mut spans = vec![
                    Span::styled(format!("  {}{}", icon, r.title), style),
                ];
                if r.redacted {
                    let reason = r.refuse_reason.as_deref().unwrap_or("capability required");
                    spans.push(Span::styled(
                        format!(" [REDACTED: {}]", reason),
                        Style::default().fg(Color::Red),
                    ));
                } else if !r.excerpt.is_empty() {
                    spans.push(Span::styled(
                        format!("  {}", &r.excerpt[..r.excerpt.len().min(60)]),
                        Style::default().fg(Color::DarkGray),
                    ));
                }
                ListItem::new(Line::from(spans))
            })
            .collect();

        let total = items.len() as u16;
        let visible = inner.height;
        let offset = scroll.min(total.saturating_sub(visible));

        let list = List::new(items).highlight_style(Style::default());
        frame.render_widget(list, inner);

        if total > visible {
            let mut sb = ScrollbarState::new(total as usize).position(offset as usize);
            frame.render_stateful_widget(
                Scrollbar::new(ScrollbarOrientation::VerticalRight),
                inner,
                &mut sb,
            );
        }
    }

    fn render_error(frame: &mut Frame, area: Rect, query: &str, message: &str) {
        let outer = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Red))
            .title(" F5: Search — Error ");
        let inner = outer.inner(area);
        frame.render_widget(outer, area);

        let lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                format!("  Query: {}", query),
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(""),
            Line::from(Span::styled(
                format!("  ✗ {}", message),
                Style::default().fg(Color::Red),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  [Esc: clear query]",
                Style::default().fg(Color::DarkGray),
            )),
        ];
        frame.render_widget(Paragraph::new(lines), inner);
    }
}

impl Default for SearchCartridge {
    fn default() -> Self {
        Self::new()
    }
}

impl Cartridge for SearchCartridge {
    fn fkey(&self) -> FKey {
        FKey::F5
    }

    fn title(&self) -> &str {
        "Search"
    }

    fn render(&mut self, frame: &mut Frame, area: Rect) {
        // Poll for search results
        let new_state: Option<SearchState> = if let SearchState::Searching { rx, query, .. } = &mut self.state {
            match rx.try_recv() {
                Ok(Ok(results)) => Some(SearchState::Results {
                    query: query.clone(),
                    results,
                    selected: 0,
                    scroll: 0,
                }),
                Ok(Err(e)) => Some(SearchState::Error {
                    query: query.clone(),
                    message: e.to_string(),
                }),
                Err(mpsc::TryRecvError::Disconnected) => Some(SearchState::Error {
                    query: query.clone(),
                    message: "Search thread disconnected".into(),
                }),
                Err(mpsc::TryRecvError::Empty) => None,
            }
        } else {
            None
        };
        if let Some(ns) = new_state {
            self.state = ns;
        }

        if let SearchState::Searching { spinner, .. } = &mut self.state {
            *spinner = spinner.wrapping_add(1);
        }

        enum Cmd<'a> {
            Idle(&'a str, usize),
            Searching(&'a str, usize),
            Results(&'a str, &'a [SearchResult], usize, u16),
            Error(&'a str, &'a str),
        }

        let cursor = self.query_cursor;
        let cmd = match &self.state {
            SearchState::Idle { query } => Cmd::Idle(query.as_str(), cursor),
            SearchState::Searching { query, spinner, .. } => Cmd::Searching(query.as_str(), *spinner),
            SearchState::Results { query, results, selected, scroll } => {
                Cmd::Results(query.as_str(), results.as_slice(), *selected, *scroll)
            }
            SearchState::Error { query, message } => Cmd::Error(query.as_str(), message.as_str()),
        };

        match cmd {
            Cmd::Idle(q, cur) => Self::render_idle(frame, area, q, cur),
            Cmd::Searching(q, sp) => Self::render_searching(frame, area, q, sp),
            Cmd::Results(q, results, sel, sc) => Self::render_results(frame, area, q, results, sel, sc),
            Cmd::Error(q, msg) => Self::render_error(frame, area, q, msg),
        }
    }

    fn handle_event(&mut self, event: &Event) -> CartridgeAction {
        let Event::Key(key) = event else {
            return CartridgeAction::None;
        };

        match &self.state {
            SearchState::Idle { .. } | SearchState::Error { .. } => {
                match key.code {
                    KeyCode::Enter => {
                        self.submit_search();
                        return CartridgeAction::Consumed;
                    }
                    KeyCode::Esc => {
                        self.state = SearchState::Idle { query: String::new() };
                        self.query_cursor = 0;
                        return CartridgeAction::Consumed;
                    }
                    KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                        let q = self.current_query().to_string();
                        let cursor = self.query_cursor;
                        let mut new_q = q;
                        new_q.insert(cursor, c);
                        let new_cursor = cursor + c.len_utf8();
                        self.query_cursor = new_cursor;
                        match &mut self.state {
                            SearchState::Idle { query } => *query = new_q,
                            SearchState::Error { .. } => {
                                self.state = SearchState::Idle { query: new_q };
                            }
                            _ => {}
                        }
                        return CartridgeAction::Consumed;
                    }
                    KeyCode::Backspace if self.query_cursor > 0 => {
                        let q = self.current_query().to_string();
                        let cursor = self.query_cursor;
                        let ch_len = q[..cursor].chars().last().map(|c| c.len_utf8()).unwrap_or(1);
                        let new_cursor = cursor - ch_len;
                        let mut new_q = q;
                        new_q.remove(new_cursor);
                        self.query_cursor = new_cursor;
                        if let SearchState::Idle { query } = &mut self.state {
                            *query = new_q;
                        }
                        return CartridgeAction::Consumed;
                    }
                    KeyCode::Left if self.query_cursor > 0 => {
                        let q = self.current_query().to_string();
                        let ch_len = q[..self.query_cursor].chars().last().map(|c| c.len_utf8()).unwrap_or(1);
                        self.query_cursor -= ch_len;
                        return CartridgeAction::Consumed;
                    }
                    KeyCode::Right if self.query_cursor < self.current_query().len() => {
                        let q = self.current_query().to_string();
                        let ch_len = q[self.query_cursor..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
                        self.query_cursor += ch_len;
                        return CartridgeAction::Consumed;
                    }
                    _ => {}
                }
            }
            SearchState::Results { .. } => {
                match key.code {
                    KeyCode::Esc => {
                        let q = self.current_query().to_string();
                        self.state = SearchState::Idle { query: q };
                        self.query_cursor = self.current_query().len();
                        return CartridgeAction::Consumed;
                    }
                    KeyCode::Char('j') | KeyCode::Down => {
                        if let SearchState::Results { results, selected, .. } = &mut self.state {
                            *selected = (*selected + 1).min(results.len().saturating_sub(1));
                        }
                        return CartridgeAction::Consumed;
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        if let SearchState::Results { selected, .. } = &mut self.state {
                            *selected = selected.saturating_sub(1);
                        }
                        return CartridgeAction::Consumed;
                    }
                    // Send selected result to Proofreader (F4).
                    KeyCode::Char('s') | KeyCode::Char('S') => {
                        if let SearchState::Results { results, selected, .. } = &self.state {
                            if let Some(r) = results.get(*selected).filter(|r| !r.redacted) {
                                return CartridgeAction::SendToContent(r.title.clone());
                            }
                        }
                        return CartridgeAction::Consumed;
                    }
                    _ => {}
                }
            }
            SearchState::Searching { .. } => {
                if key.code == KeyCode::Esc {
                    let q = self.current_query().to_string();
                    self.state = SearchState::Idle { query: q };
                    return CartridgeAction::Consumed;
                }
            }
        }

        CartridgeAction::None
    }
}
