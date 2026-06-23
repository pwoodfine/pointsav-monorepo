use std::sync::mpsc;
use std::thread;
use std::time::Instant;

use app_console_keys::motion::{self, Anim};
use app_console_keys::session::SessionState;
use app_console_keys::{Cartridge, CartridgeAction, FKey};
use crossterm::event::{Event, KeyCode, KeyModifiers};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
    },
    Frame,
};
use tui_textarea::TextArea;

use image::DynamicImage;
use ratatui_image::{
    picker::{Picker, ProtocolType},
    protocol::StatefulProtocol,
    StatefulImage,
};

use crate::draft::{self, DraftEvent};
use crate::draft_save::DraftSave;
use crate::drafts_out;
use crate::pdf::{self, PdfPageData};
use crate::proofreader::{self, ProofreadResponse, DEFAULT_PROTOCOL_IDX, PROTOCOLS};
use crate::search::{self, SearchResult};

const PATIENCE_RING: &[&str] = &["◌", "◎", "⊙", "●", "⊙", "◎"];

/// A hyperlink position recorded during render — consumed by flush_hyperlinks().
struct HyperlinkTarget {
    col: u16,
    row: u16,
    text: String,
    url: String,
}
const PLACEHOLDER: &str =
    "Paste or type text — Ctrl-S: submit · Tab: protocol · /new: draft · /search: search · /pdf: view PDF";

// ── State machine ─────────────────────────────────────────────────────────────

struct DraftTab {
    label: String,
    lines: Vec<String>,
    protocol_idx: usize,
}

/// Serialised snapshot of a non-active tab.
struct TabSnapshot {
    label: String,
    state: ContentState,
    textarea: TextArea<'static>,
    pending_hyperlinks: Vec<HyperlinkTarget>,
    restored_hint: bool,
}

impl TabSnapshot {
    fn fresh_input() -> Self {
        let mut ta = TextArea::default();
        ta.set_placeholder_text(PLACEHOLDER);
        Self {
            label: "Content".into(),
            state: ContentState::Input {
                protocol_idx: DEFAULT_PROTOCOL_IDX,
            },
            textarea: ta,
            pending_hyperlinks: Vec::new(),
            restored_hint: false,
        }
    }
}

enum ContentState {
    Input {
        protocol_idx: usize,
    },
    PickProtocol {
        saved_text: Vec<String>,
        selected: usize,
    },
    Submitting {
        original: String,
        #[allow(dead_code)]
        protocol_idx: usize,
        rx: mpsc::Receiver<anyhow::Result<ProofreadResponse>>,
        wait_since: Instant,
    },
    Results {
        response: ProofreadResponse,
        original: String,
        scroll: u16,
        born_at: Instant,
        /// Wall-clock time the result landed (HH:MM UTC) — shown in egress-witness strip.
        witness_at: String,
    },
    DraftingNew {
        title: String,
        protocol_idx: usize,
        rx: mpsc::Receiver<DraftEvent>,
        buffer: String,
        done: bool,
        error: Option<String>,
        scroll: u16,
    },
    SearchResults {
        query: String,
        results: Vec<SearchResult>,
        search_rx: Option<mpsc::Receiver<anyhow::Result<Vec<SearchResult>>>>,
        selected: usize,
        scroll: u16,
    },
    PdfView {
        path: String,
        page: u32,
        total_pages: u32,
        /// Background render channel — Some while loading, None once result received.
        render_rx: Option<mpsc::Receiver<anyhow::Result<PdfPageData>>>,
        /// Current page image. None while first load is in flight.
        current_image: Option<DynamicImage>,
        /// ratatui-image protocol state. Reset to None when current_image changes.
        proto_state: Option<StatefulProtocol>,
        error: Option<String>,
    },
    Error {
        message: String,
    },
    MultiDraft {
        tabs: Vec<DraftTab>,
        active: usize,
    },
}

// ── ContentCartridge ──────────────────────────────────────────────────────────

pub struct ContentCartridge {
    username: String,
    tenant: String,
    proof_endpoint: String,
    slm_endpoint: String,
    drafts_outbound_path: String,
    content_endpoint: String,
    state: ContentState,
    textarea: TextArea<'static>,
    offline: bool,
    health_rx: mpsc::Receiver<bool>,
    // Graphics capabilities — set by chassis after terminal probe.
    pdf_kitty: bool,
    pdf_sixel: bool,
    pdf_font_size: (u16, u16),
    truecolor: bool,
    // Hyperlinks produced by the most recent render() call — consumed by flush_hyperlinks().
    pending_hyperlinks: Vec<HyperlinkTarget>,
    // Draft persistence — save/restore across sessions.
    draft_save: DraftSave,
    restored_hint: bool,
    tabs: Vec<TabSnapshot>,
    active_tab_idx: usize,
    // Last search query — restored from session.toml at startup.
    last_search: String,
    // mTLS Phase A: PEM cert bytes for the service-ingress server (None = system CA pool).
    tls_cert_pem: Option<Vec<u8>>,
}

fn format_hhmm_utc() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{:02}:{:02}", (secs % 86400) / 3600, (secs % 3600) / 60)
}

impl ContentCartridge {
    pub fn new() -> Self {
        Self::new_for(
            "operator",
            "local",
            "http://127.0.0.1:9092",
            "http://localhost:9080",
            format!(
                "{}/.local/share/os-console/drafts-outbound",
                std::env::var("HOME").unwrap_or_else(|_| ".".into())
            ),
            "http://127.0.0.1:9081",
            None,
            None,
            None,
            None,
        )
    }

    pub fn new_for(
        username: impl Into<String>,
        tenant: impl Into<String>,
        proof_endpoint: impl Into<String>,
        slm_endpoint: impl Into<String>,
        drafts_outbound_path: impl Into<String>,
        content_endpoint: impl Into<String>,
        initial_query: Option<String>,
        initial_selected: Option<usize>,
        initial_scroll: Option<u16>,
        tls_cert_pem: Option<Vec<u8>>,
    ) -> Self {
        let slm = slm_endpoint.into();
        let content_ep: String = content_endpoint.into();

        // Background health poller — polls Doorman /readyz every 30s
        let (health_tx, health_rx) = mpsc::channel::<bool>();
        let slm_clone = slm.clone();
        thread::spawn(move || loop {
            let available = reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(3))
                .build()
                .ok()
                .and_then(|c| c.get(format!("{}/readyz", slm_clone)).send().ok())
                .and_then(|r| {
                    if r.status().is_success() {
                        r.json::<serde_json::Value>().ok()
                    } else {
                        None
                    }
                })
                .and_then(|v| v["ai_available"].as_bool())
                .unwrap_or(false);
            let _ = health_tx.send(available);
            thread::sleep(std::time::Duration::from_secs(30));
        });

        let saved_session = SessionState::load();
        let draft_save = DraftSave::open();
        let saved_draft = draft_save.load();
        let restored_hint = saved_draft.is_some();
        let protocol_idx = saved_draft
            .as_ref()
            .and_then(|s| {
                PROTOCOLS
                    .iter()
                    .position(|(slug, _)| *slug == s.protocol.as_str())
            })
            .unwrap_or(DEFAULT_PROTOCOL_IDX);
        let mut ta: TextArea<'static> = if let Some(ref saved) = saved_draft {
            TextArea::from(saved.content.lines().map(String::from).collect::<Vec<_>>())
        } else {
            TextArea::default()
        };
        ta.set_placeholder_text(PLACEHOLDER);
        let initial_state = if !saved_session.content_query.is_empty() {
            let ep = content_ep.clone();
            let query_str = saved_session.content_query.clone();
            let (tx, rx) = mpsc::channel();
            thread::spawn(move || {
                let _ = tx.send(search::fetch_search(&ep, &query_str));
            });
            ContentState::SearchResults {
                query: saved_session.content_query.clone(),
                results: vec![],
                search_rx: Some(rx),
                selected: 0,
                scroll: 0,
            }
        } else {
            ContentState::Input { protocol_idx }
        };
        Self {
            username: username.into(),
            tenant: tenant.into(),
            proof_endpoint: proof_endpoint.into(),
            slm_endpoint: slm,
            drafts_outbound_path: drafts_outbound_path.into(),
            content_endpoint: content_ep,
            state: initial_state,
            textarea: ta,
            offline: false,
            health_rx,
            pdf_kitty: false,
            pdf_sixel: false,
            pdf_font_size: (10, 20),
            truecolor: false,
            pending_hyperlinks: Vec::new(),
            draft_save,
            restored_hint,
            tabs: vec![TabSnapshot::fresh_input()],
            active_tab_idx: 0,
            last_search: saved_session.content_query.unwrap_or_default(),
            tls_cert_pem,
        }
    }

    /// Accent color — teal-family; richer RGB on truecolor terminals.
    fn accent_color(&self) -> Color {
        if self.truecolor {
            Color::Rgb(32, 178, 170)
        } else {
            Color::Cyan
        }
    }

    /// Selection background — deep teal on truecolor, Cyan otherwise.
    fn selection_bg(&self) -> Color {
        if self.truecolor {
            Color::Rgb(0, 95, 135)
        } else {
            Color::Cyan
        }
    }

    /// Persist current SearchResults state to the session file.
    /// Saves a cleared record when not in SearchResults (so the next launch starts fresh).
    fn save_session(&self) {
        use app_console_keys::SessionState;
        let state = match &self.state {
            ContentState::SearchResults { query, .. } => SessionState {
                content_query: query.clone(),
            },
            _ => SessionState::default(),
        };
        state.save();
    }

    fn reset_textarea(&mut self, protocol_idx: usize) {
        let mut ta = TextArea::default();
        ta.set_placeholder_text(PLACEHOLDER);
        self.textarea = ta;
        self.state = ContentState::Input { protocol_idx };
    }

    // ── Render helpers ────────────────────────────────────────────────────────

    fn render_input(&mut self, frame: &mut Frame, area: Rect, protocol_idx: usize) {
        let (slug, display) = PROTOCOLS[protocol_idx];
        let border_color = if self.offline {
            Color::DarkGray
        } else {
            self.accent_color()
        };
        let outer = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(" F4: Content — Proofreader ");
        let inner = outer.inner(area);
        frame.render_widget(outer, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Fill(1), Constraint::Length(1)])
            .split(inner);

        frame.render_widget(&self.textarea, chunks[0]);

        let offline_note = if self.offline {
            "  [⚠ AI OFFLINE — /new disabled]"
        } else {
            ""
        };
        let restored_note = if self.restored_hint {
            "  [restored]"
        } else {
            ""
        };
        let search_note = if !self.last_search.is_empty() {
            format!("  [last: {}]", &self.last_search)
        } else {
            String::new()
        };
        let hint = Paragraph::new(format!(
            " Protocol: {}  —  {}    [Tab: change  Ctrl-S: submit  /new: draft  /search: search  q/Ctrl-C: quit]{}{}{}",
            slug, display, offline_note, restored_note, search_note
        ))
        .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(hint, chunks[1]);
        self.restored_hint = false; // Clear after one frame
    }

    fn render_picker(frame: &mut Frame, area: Rect, selected: usize, sel_bg: Color) {
        let outer = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow))
            .title(" F4: Content — Pick Protocol    [↑↓: navigate  Enter: select  Esc: back] ");
        let inner = outer.inner(area);
        frame.render_widget(outer, area);

        let items: Vec<ListItem> = PROTOCOLS
            .iter()
            .enumerate()
            .map(|(i, (slug, name))| {
                let style = if i == selected {
                    Style::default()
                        .fg(Color::Black)
                        .bg(sel_bg)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                ListItem::new(format!("  {}  —  {}", slug, name)).style(style)
            })
            .collect();

        frame.render_widget(List::new(items), inner);
    }

    fn render_submitting(frame: &mut Frame, area: Rect, elapsed_ms: u64, truecolor: bool) {
        let t = motion::pulse(elapsed_ms, 2800);
        let ring = PATIENCE_RING[((t * PATIENCE_RING.len() as f32) as usize).min(PATIENCE_RING.len() - 1)];
        let border_color = if truecolor {
            let v = 80 + (t * 175.0) as u8;
            Color::Rgb(0, v, v)
        } else {
            Color::Cyan
        };
        let outer = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(" F4: Content — Proofreading... ");
        let inner = outer.inner(area);
        frame.render_widget(outer, area);

        let mid = Rect {
            y: inner.y + inner.height / 2,
            height: 2,
            ..inner
        };
        frame.render_widget(
            Paragraph::new(format!(
                "  {} Sending to service-proofreader — please wait (up to 300s)…",
                ring,
            ))
            .style(Style::default().fg(Color::Yellow)),
            mid,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn render_results(
        frame: &mut Frame,
        area: Rect,
        response: &ProofreadResponse,
        original: &str,
        scroll: u16,
        born_ms: u64,
        truecolor: bool,
        witness_at: &str,
    ) {
        use similar::{ChangeTag, TextDiff};

        let degraded_str = if response.degraded.is_empty() {
            String::new()
        } else {
            format!("  [DEGRADED: {}]", response.degraded.join(", "))
        };
        let title = format!(
            " F4: Content — Results{}    [A: accept  R: reject  ↑↓: scroll  Esc: back] ",
            degraded_str
        );

        let pop_t = Anim::verdict_pop().value(born_ms);
        let border_style = if truecolor {
            let base = 80u8;
            let bright = 255u8;
            let v = base + (pop_t * (bright - base) as f32) as u8;
            Style::default().fg(Color::Rgb(0, v, 60))
        } else if born_ms < 200 {
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Green)
        };
        let outer = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(title.as_str());
        let inner = outer.inner(area);
        frame.render_widget(outer, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Fill(1), Constraint::Length(1)])
            .split(inner);

        // Info line
        let tier = response.tier_used.as_deref().unwrap_or("?");
        let tmpl = response
            .template_display_name
            .as_deref()
            .unwrap_or(&response.protocol);
        frame.render_widget(
            Paragraph::new(format!(" tier: {}  │  {}", tier, tmpl))
                .style(Style::default().fg(Color::DarkGray)),
            chunks[0],
        );

        // Diff pane
        let diff = TextDiff::from_lines(original, &response.improved_text);
        let mut lines: Vec<Line> = Vec::new();

        for change in diff.iter_all_changes() {
            let (prefix, style) = match change.tag() {
                ChangeTag::Delete => ("- ", Style::default().fg(Color::Red)),
                ChangeTag::Insert => ("+ ", Style::default().fg(Color::LightGreen)),
                ChangeTag::Equal => ("  ", Style::default().fg(Color::DarkGray)),
            };
            let text = change.value().trim_end_matches('\n');
            lines.push(Line::from(Span::styled(
                format!("{}{}", prefix, text),
                style,
            )));
        }

        if lines.is_empty() || original == response.improved_text {
            lines = vec![Line::from(Span::styled(
                "  (No changes — service degraded or text already clean)",
                Style::default().fg(Color::DarkGray),
            ))];
        }

        let total = lines.len() as u16;
        let visible = chunks[1].height;
        let offset = scroll.min(total.saturating_sub(visible));

        frame.render_widget(Paragraph::new(lines.clone()).scroll((offset, 0)), chunks[1]);

        if total > visible {
            let mut sb_state = ScrollbarState::new(total as usize).position(offset as usize);
            frame.render_stateful_widget(
                Scrollbar::new(ScrollbarOrientation::VerticalRight),
                chunks[1],
                &mut sb_state,
            );
        }

        // Egress-witness strip — shows which Doorman tier reviewed this draft.
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("  ⬡ Witnessed by Doorman · Local · ", Style::default().fg(Color::DarkGray)),
                Span::styled(witness_at, Style::default().fg(Color::Cyan)),
            ])),
            chunks[2],
        );
    }

    fn render_error(frame: &mut Frame, area: Rect, message: &str) {
        let outer = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Red))
            .title(" F4: Content — Error    [any key: back] ");
        let inner = outer.inner(area);
        frame.render_widget(outer, area);

        let mid = Rect {
            y: inner.y + inner.height / 2,
            height: 3,
            ..inner
        };
        frame.render_widget(
            Paragraph::new(format!("  Error: {}", message)).style(Style::default().fg(Color::Red)),
            mid,
        );
    }

    fn render_drafting(
        frame: &mut Frame,
        area: Rect,
        title: &str,
        buffer: &str,
        done: bool,
        error: Option<&str>,
        scroll: u16,
    ) {
        let (border_color, status) = if let Some(err) = error {
            (Color::Red, format!(" Error: {} ", err))
        } else if done {
            (Color::Green, " [Enter: accept  Esc: discard] ".to_string())
        } else {
            (Color::Yellow, " [drafting…  Esc: cancel] ".to_string())
        };

        let title_str = format!(" F4: Content — Draft: \"{}\" {} ", title, status);
        let outer = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(title_str.as_str());
        let inner = outer.inner(area);
        frame.render_widget(outer, area);

        let lines: Vec<Line> = buffer
            .lines()
            .map(|l| {
                Line::from(Span::styled(
                    l.to_string(),
                    Style::default().fg(Color::White),
                ))
            })
            .collect();

        let total = lines.len() as u16;
        let visible = inner.height;
        // Auto-scroll to bottom while streaming; user can override once done
        let offset = if done {
            scroll.min(total.saturating_sub(visible))
        } else {
            total.saturating_sub(visible)
        };

        frame.render_widget(Paragraph::new(lines.clone()).scroll((offset, 0)), inner);

        if total > visible {
            let mut sb = ScrollbarState::new(total as usize).position(offset as usize);
            frame.render_stateful_widget(
                Scrollbar::new(ScrollbarOrientation::VerticalRight),
                inner,
                &mut sb,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn render_search_results(
        frame: &mut Frame,
        area: Rect,
        query: &str,
        results: &[SearchResult],
        selected: usize,
        scroll: u16,
        loading: bool,
        sel_bg: Color,
        content_endpoint: &str,
    ) -> Vec<HyperlinkTarget> {
        let title = format!(
            " F4: Content — Search: \"{}\"    [j/k: navigate  Esc: back] ",
            query
        );
        let outer = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title(title.as_str());
        let inner = outer.inner(area);
        frame.render_widget(outer, area);

        if loading {
            frame.render_widget(
                Paragraph::new("  Searching…").style(Style::default().fg(Color::Yellow)),
                inner,
            );
            return Vec::new();
        }

        if results.is_empty() {
            frame.render_widget(
                Paragraph::new("  No results.").style(Style::default().fg(Color::DarkGray)),
                inner,
            );
            return Vec::new();
        }

        let lines: Vec<Line> = results
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let style = if i == selected {
                    Style::default().fg(Color::Black).bg(sel_bg)
                } else {
                    Style::default()
                };
                let text = if r.excerpt.is_empty() {
                    format!("  {}  [{}]", r.title, r.slug)
                } else {
                    format!("  {} [{}]  {}", r.title, r.slug, r.excerpt)
                };
                Line::from(Span::styled(text, style))
            })
            .collect();

        let total = lines.len() as u16;
        let visible = inner.height;
        let offset = scroll.min(total.saturating_sub(visible));

        frame.render_widget(Paragraph::new(lines).scroll((offset, 0)), inner);

        if total > visible {
            let mut sb = ScrollbarState::new(total as usize).position(offset as usize);
            frame.render_stateful_widget(
                Scrollbar::new(ScrollbarOrientation::VerticalRight),
                inner,
                &mut sb,
            );
        }

        // Build hyperlink targets for each visible result title.
        results
            .iter()
            .enumerate()
            .filter(|(i, _)| {
                let row = *i as u16;
                row >= offset && row < offset + visible
            })
            .map(|(i, r)| HyperlinkTarget {
                col: inner.x + 2,
                row: inner.y + (i as u16 - offset),
                text: r.title.clone(),
                url: format!("{}/wiki/{}", content_endpoint, r.slug),
            })
            .collect()
    }

    fn render_pdf_view(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        path: &str,
        page: u32,
        total: u32,
    ) {
        let title = format!(
            " F4: Content — PDF: {}   page {}/{}    [j/k PgUp/PgDn: navigate  Esc: back] ",
            std::path::Path::new(path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(path),
            page + 1,
            total,
        );
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title(title.as_str());
        let inner = block.inner(area);
        frame.render_widget(block, area);

        // Show error if any
        let error_opt = if let ContentState::PdfView { error, .. } = &self.state {
            error.clone()
        } else {
            None
        };
        if let Some(e) = error_opt {
            frame.render_widget(
                Paragraph::new(format!("  Error: {}", e)).style(Style::default().fg(Color::Red)),
                inner,
            );
            return;
        }

        // Loading indicator while first page renders
        let loading = if let ContentState::PdfView {
            render_rx,
            current_image,
            ..
        } = &self.state
        {
            render_rx.is_some() && current_image.is_none()
        } else {
            false
        };
        if loading {
            frame.render_widget(
                Paragraph::new("  Rendering page…").style(Style::default().fg(Color::Yellow)),
                inner,
            );
            return;
        }

        // No graphics support — show fallback text
        if !self.pdf_kitty && !self.pdf_sixel {
            frame.render_widget(
                Paragraph::new(
                    "  PDF rendered (graphics protocol not available on this terminal).\n\
                     \n  Requires Kitty, iTerm2, Ghostty, or WezTerm for pixel rendering.\n\
                     \n  Navigation: j/k or PgUp/PgDn to change pages  ·  Esc to exit",
                )
                .style(Style::default().fg(Color::DarkGray)),
                inner,
            );
            return;
        }

        // Build proto_state from current_image if not yet created, then render it.
        let protocol = if self.pdf_kitty {
            ProtocolType::Kitty
        } else {
            ProtocolType::Sixel
        };
        let font_size = self.pdf_font_size;
        if let ContentState::PdfView {
            current_image,
            proto_state,
            ..
        } = &mut self.state
        {
            if proto_state.is_none() {
                if let Some(img) = current_image {
                    // from_fontsize is deprecated upstream in favour of from_query_stdio,
                    // but we cannot re-query the terminal from inside the render loop.
                    // The chassis already probed the real font size and protocol and handed
                    // them to us via set_graphics_caps — reconstruct the picker from those.
                    #[allow(deprecated)]
                    let mut picker = Picker::from_fontsize(font_size);
                    picker.set_protocol_type(protocol);
                    *proto_state = Some(picker.new_resize_protocol(img.clone()));
                }
            }
            if let Some(state) = proto_state {
                frame.render_stateful_widget(StatefulImage::new(), inner, state);
            }
        }
    }

    // ── Event handlers ────────────────────────────────────────────────────────

    fn on_input_key(&mut self, event: &Event, protocol_idx: usize) -> CartridgeAction {
        let Event::Key(key) = event else {
            return CartridgeAction::None;
        };

        // Let F-keys and Ctrl-C fall through to chassis
        if matches!(key.code, KeyCode::F(_)) {
            return CartridgeAction::None;
        }
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return CartridgeAction::None;
        }

        // Tab → protocol picker
        if key.code == KeyCode::Tab {
            let saved: Vec<String> = self
                .textarea
                .lines()
                .iter()
                .map(|s| s.to_string())
                .collect();
            self.state = ContentState::PickProtocol {
                saved_text: saved,
                selected: protocol_idx,
            };
            return CartridgeAction::Consumed;
        }

        // Ctrl-S → proofread OR /new <title> → draft mode
        if key.code == KeyCode::Char('s') && key.modifiers.contains(KeyModifiers::CONTROL) {
            let text = self.textarea.lines().join("\n");
            if text.trim().is_empty() {
                return CartridgeAction::Consumed;
            }

            // Intercept slash commands before proofread
            let trimmed = text.trim();

            // /new <title> → draft mode (blocked when offline)
            if let Some(rest) = trimmed.strip_prefix("/new") {
                if self.offline {
                    self.state = ContentState::Error {
                        message: "AI unavailable — Doorman is offline. Check the F9 SLM panel."
                            .into(),
                    };
                    return CartridgeAction::Consumed;
                }
                let title = rest.trim().to_string();
                let title = if title.is_empty() {
                    "Untitled".to_string()
                } else {
                    title
                };
                let protocol = PROTOCOLS[protocol_idx].0.to_string();
                let tenant = self.tenant.clone();
                let slm = self.slm_endpoint.clone();
                let prompt = title.clone();
                let (tx, rx) = mpsc::channel();
                thread::spawn(move || {
                    draft::stream_draft(&prompt, &protocol, &tenant, &slm, tx);
                });
                self.state = ContentState::DraftingNew {
                    title,
                    protocol_idx,
                    rx,
                    buffer: String::new(),
                    done: false,
                    error: None,
                    scroll: 0,
                };
                return CartridgeAction::Consumed;
            }

            // /search <query> → Tantivy search results
            if let Some(rest) = trimmed.strip_prefix("/search") {
                let query = rest.trim().to_string();
                if !query.is_empty() {
                    let ep = self.content_endpoint.clone();
                    let q = query.clone();
                    let (tx, rx) = mpsc::channel();
                    thread::spawn(move || {
                        let _ = tx.send(search::fetch_search(&ep, &q));
                    });
                    self.state = ContentState::SearchResults {
                        query,
                        results: vec![],
                        search_rx: Some(rx),
                        selected: 0,
                        scroll: 0,
                    };
                    self.save_session();
                }
                return CartridgeAction::Consumed;
            }

            // /pdf <path> → PDF viewer
            if let Some(rest) = trimmed.strip_prefix("/pdf") {
                let path = rest.trim().to_string();
                if path.is_empty() {
                    self.state = ContentState::Error {
                        message: "Usage: /pdf <path/to/file.pdf>".into(),
                    };
                    return CartridgeAction::Consumed;
                }
                let path_clone = path.clone();
                let (tx, rx) = mpsc::channel();
                thread::spawn(move || {
                    let _ = tx.send(pdf::render_page(&path_clone, 0));
                });
                self.state = ContentState::PdfView {
                    path,
                    page: 0,
                    total_pages: 1,
                    render_rx: Some(rx),
                    current_image: None,
                    proto_state: None,
                    error: None,
                };
                return CartridgeAction::Consumed;
            }

            let protocol = PROTOCOLS[protocol_idx].0.to_string();
            let tenant = self.tenant.clone();
            let endpoint = self.proof_endpoint.clone();
            let text_clone = text.clone();
            let cert = self.tls_cert_pem.clone();
            let (tx, rx) = mpsc::channel();
            thread::spawn(move || {
                let _ = tx.send(proofreader::submit_proofread(
                    &text_clone,
                    &protocol,
                    &tenant,
                    &endpoint,
                    cert.as_deref(),
                ));
            });
            self.state = ContentState::Submitting {
                original: text,
                protocol_idx,
                rx,
                wait_since: Instant::now(),
            };
            return CartridgeAction::Consumed;
        }

        // Ctrl-t → open a new tab (transition Input → MultiDraft)
        if key.code == KeyCode::Char('t') && key.modifiers.contains(KeyModifiers::CONTROL) {
            let lines: Vec<String> = self.textarea.lines().iter().map(String::from).collect();
            let tab1 = DraftTab {
                label: "Draft 1".into(),
                lines,
                protocol_idx,
            };
            let tab2 = DraftTab {
                label: "Draft 2".into(),
                lines: Vec::new(),
                protocol_idx: DEFAULT_PROTOCOL_IDX,
            };
            self.state = ContentState::MultiDraft {
                tabs: vec![tab1, tab2],
                active: 1,
            };
            let mut ta = TextArea::default();
            ta.set_placeholder_text(PLACEHOLDER);
            self.textarea = ta;
            return CartridgeAction::Consumed;
        }

        // Everything else → textarea
        self.textarea
            .input(tui_textarea::Input::from(event.clone()));
        // Auto-save draft after each keystroke.
        let content = self.textarea.lines().join("\n");
        self.draft_save.save(PROTOCOLS[protocol_idx].0, &content);
        CartridgeAction::Consumed
    }

    fn on_picker_key(
        &mut self,
        key: &crossterm::event::KeyEvent,
        selected: usize,
        saved: &[String],
    ) -> CartridgeAction {
        match key.code {
            KeyCode::Esc | KeyCode::BackTab => {
                let saved_clone = saved.to_vec();
                let mut ta = TextArea::from(saved_clone);
                ta.set_placeholder_text(PLACEHOLDER);
                self.textarea = ta;
                self.state = ContentState::Input {
                    protocol_idx: selected,
                };
            }
            KeyCode::Up => {
                if let ContentState::PickProtocol { selected: s, .. } = &mut self.state {
                    if *s > 0 {
                        *s -= 1;
                    }
                }
            }
            KeyCode::Down => {
                if let ContentState::PickProtocol { selected: s, .. } = &mut self.state {
                    if *s < PROTOCOLS.len() - 1 {
                        *s += 1;
                    }
                }
            }
            KeyCode::Enter => {
                let saved_clone = saved.to_vec();
                let mut ta = TextArea::from(saved_clone);
                ta.set_placeholder_text(PLACEHOLDER);
                self.textarea = ta;
                self.state = ContentState::Input {
                    protocol_idx: selected,
                };
            }
            _ => {}
        }
        CartridgeAction::Consumed
    }

    fn on_drafting_key(&mut self, key: &crossterm::event::KeyEvent) -> CartridgeAction {
        // Extract needed values without keeping a borrow across mutable calls
        let (done, title, protocol_idx, buffer) = match &self.state {
            ContentState::DraftingNew {
                done,
                title,
                protocol_idx,
                buffer,
                ..
            } => (*done, title.clone(), *protocol_idx, buffer.clone()),
            _ => return CartridgeAction::Consumed,
        };

        match key.code {
            KeyCode::Esc => {
                self.reset_textarea(protocol_idx);
            }
            KeyCode::Enter | KeyCode::Char('a') | KeyCode::Char('A') if done => {
                let protocol = PROTOCOLS[protocol_idx].0.to_string();
                let tenant = self.tenant.clone();
                let username = self.username.clone();
                let outdir = self.drafts_outbound_path.clone();
                thread::spawn(move || {
                    let _ = drafts_out::write_draft(
                        &title, &protocol, &buffer, &tenant, &username, &outdir,
                    );
                });
                self.reset_textarea(DEFAULT_PROTOCOL_IDX);
            }
            KeyCode::Up if done => {
                if let ContentState::DraftingNew { scroll, .. } = &mut self.state {
                    *scroll = scroll.saturating_sub(1);
                }
            }
            KeyCode::Down if done => {
                if let ContentState::DraftingNew { scroll, .. } = &mut self.state {
                    *scroll = scroll.saturating_add(1);
                }
            }
            _ => {}
        }
        CartridgeAction::Consumed
    }

    fn on_results_key(&mut self, key: &crossterm::event::KeyEvent) -> CartridgeAction {
        match key.code {
            KeyCode::Char('q') => {
                return CartridgeAction::None; // chassis quits
            }
            KeyCode::Char('a') | KeyCode::Char('A') | KeyCode::Enter | KeyCode::Esc => {
                if let ContentState::Results { response, .. } = &self.state {
                    let rid = response.request_id.clone();
                    let tenant = self.tenant.clone();
                    let endpoint = self.proof_endpoint.clone();
                    let cert = self.tls_cert_pem.clone();
                    thread::spawn(move || {
                        let _ = proofreader::post_verdict(
                            &rid, &tenant, "accept", &endpoint, cert.as_deref(),
                        );
                    });
                }
                self.reset_textarea(DEFAULT_PROTOCOL_IDX);
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                if let ContentState::Results { response, .. } = &self.state {
                    let rid = response.request_id.clone();
                    let tenant = self.tenant.clone();
                    let endpoint = self.proof_endpoint.clone();
                    let cert = self.tls_cert_pem.clone();
                    thread::spawn(move || {
                        let _ = proofreader::post_verdict(
                            &rid, &tenant, "reject", &endpoint, cert.as_deref(),
                        );
                    });
                }
                self.reset_textarea(DEFAULT_PROTOCOL_IDX);
            }
            KeyCode::Up => {
                if let ContentState::Results { scroll, .. } = &mut self.state {
                    *scroll = scroll.saturating_sub(1);
                }
            }
            KeyCode::Down => {
                if let ContentState::Results { scroll, .. } = &mut self.state {
                    *scroll = scroll.saturating_add(1);
                }
            }
            _ => {}
        }
        CartridgeAction::Consumed
    }

    fn on_search_key(&mut self, key: &crossterm::event::KeyEvent) -> CartridgeAction {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                if let ContentState::SearchResults { ref query, .. } = self.state {
                    self.last_search = query.clone();
                }
                self.reset_textarea(DEFAULT_PROTOCOL_IDX);
                self.save_session();
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if let ContentState::SearchResults {
                    selected, results, ..
                } = &mut self.state
                {
                    if *selected + 1 < results.len() {
                        *selected += 1;
                    }
                }
                self.save_session();
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if let ContentState::SearchResults { selected, .. } = &mut self.state {
                    *selected = selected.saturating_sub(1);
                }
                self.save_session();
            }
            _ => {}
        }
        CartridgeAction::Consumed
    }

    fn on_pdf_key(&mut self, key: &crossterm::event::KeyEvent) -> CartridgeAction {
        // Compute the requested page change, then kick off a render if it moved.
        let (path, page, total) = match &self.state {
            ContentState::PdfView {
                path,
                page,
                total_pages,
                ..
            } => (path.clone(), *page, *total_pages),
            _ => return CartridgeAction::Consumed,
        };

        let new_page = match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.reset_textarea(DEFAULT_PROTOCOL_IDX);
                return CartridgeAction::Consumed;
            }
            KeyCode::Char('j') | KeyCode::Down | KeyCode::PageDown => {
                if page + 1 < total {
                    page + 1
                } else {
                    page
                }
            }
            KeyCode::Char('k') | KeyCode::Up | KeyCode::PageUp => page.saturating_sub(1),
            _ => return CartridgeAction::Consumed,
        };

        if new_page != page {
            let path_clone = path.clone();
            let (tx, rx) = mpsc::channel();
            thread::spawn(move || {
                let _ = tx.send(pdf::render_page(&path_clone, new_page));
            });
            if let ContentState::PdfView {
                page,
                render_rx,
                proto_state,
                ..
            } = &mut self.state
            {
                *page = new_page;
                *render_rx = Some(rx);
                // Clear cached protocol so the new page image is picked up on next render
                *proto_state = None;
            }
        }
        CartridgeAction::Consumed
    }

    fn on_multidraft_key(&mut self, event: &Event, _active: usize) -> CartridgeAction {
        let Event::Key(key) = event else {
            return CartridgeAction::None;
        };
        if matches!(key.code, KeyCode::F(_)) {
            return CartridgeAction::None;
        }
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return CartridgeAction::None;
        }

        // Ctrl-t → add another tab
        if key.code == KeyCode::Char('t') && key.modifiers.contains(KeyModifiers::CONTROL) {
            if let ContentState::MultiDraft { tabs, active } = &mut self.state {
                let lines: Vec<String> = self.textarea.lines().iter().map(String::from).collect();
                tabs[*active].lines = lines;
                let n = tabs.len() + 1;
                tabs.push(DraftTab {
                    label: format!("Draft {}", n),
                    lines: Vec::new(),
                    protocol_idx: DEFAULT_PROTOCOL_IDX,
                });
                *active = tabs.len() - 1;
            }
            let mut ta = TextArea::default();
            ta.set_placeholder_text(PLACEHOLDER);
            self.textarea = ta;
            return CartridgeAction::Consumed;
        }

        // Ctrl-Left → previous tab
        if key.code == KeyCode::Left && key.modifiers.contains(KeyModifiers::CONTROL) {
            if let ContentState::MultiDraft { tabs, active } = &mut self.state {
                if *active > 0 {
                    let lines: Vec<String> =
                        self.textarea.lines().iter().map(String::from).collect();
                    tabs[*active].lines = lines;
                    *active -= 1;
                    let target = TextArea::from(tabs[*active].lines.clone());
                    self.textarea = target;
                    self.textarea.set_placeholder_text(PLACEHOLDER);
                }
            }
            return CartridgeAction::Consumed;
        }

        // Ctrl-Right → next tab
        if key.code == KeyCode::Right && key.modifiers.contains(KeyModifiers::CONTROL) {
            if let ContentState::MultiDraft { tabs, active } = &mut self.state {
                if *active + 1 < tabs.len() {
                    let lines: Vec<String> =
                        self.textarea.lines().iter().map(String::from).collect();
                    tabs[*active].lines = lines;
                    *active += 1;
                    let target = TextArea::from(tabs[*active].lines.clone());
                    self.textarea = target;
                    self.textarea.set_placeholder_text(PLACEHOLDER);
                }
            }
            return CartridgeAction::Consumed;
        }

        // Ctrl-w → close current tab
        if key.code == KeyCode::Char('w') && key.modifiers.contains(KeyModifiers::CONTROL) {
            let new_state = if let ContentState::MultiDraft { tabs, active } = &mut self.state {
                tabs.remove(*active);
                if tabs.len() == 1 {
                    let pidx = tabs[0].protocol_idx;
                    let lines = tabs[0].lines.clone();
                    let ta = TextArea::from(lines);
                    self.textarea = ta;
                    self.textarea.set_placeholder_text(PLACEHOLDER);
                    Some(ContentState::Input { protocol_idx: pidx })
                } else {
                    *active = (*active).min(tabs.len() - 1);
                    let lines = tabs[*active].lines.clone();
                    let ta = TextArea::from(lines);
                    self.textarea = ta;
                    self.textarea.set_placeholder_text(PLACEHOLDER);
                    None
                }
            } else {
                None
            };
            if let Some(s) = new_state {
                self.state = s;
            }
            return CartridgeAction::Consumed;
        }

        // Forward to textarea
        self.textarea
            .input(tui_textarea::Input::from(event.clone()));
        CartridgeAction::Consumed
    }

    fn render_multidraft(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        labels: &[String],
        active: usize,
        protocol_idx: usize,
    ) {
        let (slug, display) = PROTOCOLS[protocol_idx];

        // Split: 1 row tab bar + rest for content
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Fill(1),
                Constraint::Length(1),
            ])
            .split(area);

        // Tab bar
        let tab_line: Line = Line::from(
            labels
                .iter()
                .enumerate()
                .flat_map(|(i, label)| {
                    let (open, close) = if i == active {
                        ("▶[", "]")
                    } else {
                        ("[", "]")
                    };
                    let style = if i == active {
                        Style::default()
                            .fg(Color::Black)
                            .bg(self.selection_bg())
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    };
                    vec![
                        Span::raw(" "),
                        Span::styled(format!("{}{}{}", open, label, close), style),
                    ]
                })
                .chain(std::iter::once(Span::raw(
                    "   [Ctrl-t: new  Ctrl-←/→: switch  Ctrl-w: close]",
                )))
                .collect::<Vec<_>>(),
        );
        frame.render_widget(Paragraph::new(tab_line), chunks[0]);

        // Content area (same as render_input)
        let border_color = if self.offline {
            Color::DarkGray
        } else {
            self.accent_color()
        };
        let outer = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(format!(
                " {} ",
                labels.get(active).map(|s| s.as_str()).unwrap_or("Draft")
            ));
        let inner = outer.inner(chunks[1]);
        frame.render_widget(outer, chunks[1]);

        let content_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Fill(1), Constraint::Length(1)])
            .split(inner);
        frame.render_widget(&self.textarea, content_chunks[0]);

        let offline_note = if self.offline {
            "  [⚠ AI OFFLINE — /new disabled]"
        } else {
            ""
        };
        let hint = Paragraph::new(format!(
            " Protocol: {}  —  {}    [Ctrl-S: submit  /search: search  q/Ctrl-C: quit]{}",
            slug, display, offline_note
        ))
        .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(hint, chunks[2]);
    }

    // ── Multi-tab ─────────────────────────────────────────────────────────────

    fn tab_label(&self) -> String {
        match &self.state {
            ContentState::Input { .. } => "Content".into(),
            ContentState::SearchResults { query, .. } => {
                if query.is_empty() {
                    "Search".into()
                } else {
                    format!("/{}", &query[..query.len().min(10)])
                }
            }
            ContentState::DraftingNew { title, .. } => {
                let t: String = title.chars().take(10).collect();
                if t.is_empty() {
                    "Draft".into()
                } else {
                    t
                }
            }
            ContentState::Results { .. } => "Results".into(),
            ContentState::PdfView { path, .. } => path
                .split('/')
                .next_back()
                .unwrap_or("PDF")
                .chars()
                .take(10)
                .collect(),
            ContentState::MultiDraft { .. } => "Drafts".into(),
            _ => "Content".into(),
        }
    }

    fn serialize_current_to_slot(&mut self) {
        let label = self.tab_label();
        let idx = self.active_tab_idx;
        let snap = TabSnapshot {
            label,
            state: std::mem::replace(
                &mut self.state,
                ContentState::Input {
                    protocol_idx: DEFAULT_PROTOCOL_IDX,
                },
            ),
            textarea: std::mem::replace(&mut self.textarea, {
                let mut ta = TextArea::default();
                ta.set_placeholder_text(PLACEHOLDER);
                ta
            }),
            pending_hyperlinks: std::mem::take(&mut self.pending_hyperlinks),
            restored_hint: std::mem::replace(&mut self.restored_hint, false),
        };
        if idx < self.tabs.len() {
            self.tabs[idx] = snap;
        }
    }

    fn load_from_slot(&mut self, idx: usize) {
        let snap = std::mem::replace(&mut self.tabs[idx], TabSnapshot::fresh_input());
        self.state = snap.state;
        self.textarea = snap.textarea;
        self.pending_hyperlinks = snap.pending_hyperlinks;
        self.restored_hint = snap.restored_hint;
        self.active_tab_idx = idx;
    }

    fn switch_to_tab(&mut self, new_idx: usize) {
        if new_idx == self.active_tab_idx || new_idx >= self.tabs.len() {
            return;
        }
        self.serialize_current_to_slot();
        self.load_from_slot(new_idx);
    }

    fn new_tab(&mut self) {
        if self.tabs.len() >= 4 {
            return;
        }
        self.serialize_current_to_slot();
        let new_idx = self.tabs.len();
        self.tabs.push(TabSnapshot::fresh_input());
        self.state = ContentState::Input {
            protocol_idx: DEFAULT_PROTOCOL_IDX,
        };
        let mut ta = TextArea::default();
        ta.set_placeholder_text(PLACEHOLDER);
        self.textarea = ta;
        self.pending_hyperlinks = Vec::new();
        self.restored_hint = false;
        self.active_tab_idx = new_idx;
    }

    fn close_tab(&mut self) {
        if self.tabs.len() <= 1 {
            return;
        }
        // Discard current live fields; remove this tab's slot
        self.tabs.remove(self.active_tab_idx);
        let new_idx = if self.active_tab_idx >= self.tabs.len() {
            self.tabs.len() - 1
        } else {
            self.active_tab_idx
        };
        self.load_from_slot(new_idx);
    }

    fn cycle_tab(&mut self, forward: bool) {
        let n = self.tabs.len();
        if n <= 1 {
            return;
        }
        let new_idx = if forward {
            (self.active_tab_idx + 1) % n
        } else {
            (self.active_tab_idx + n - 1) % n
        };
        self.switch_to_tab(new_idx);
    }

    fn render_tab_bar(&self, frame: &mut Frame, area: Rect) {
        let n = self.tabs.len();
        let mut spans: Vec<Span> = Vec::new();
        for i in 0..n {
            let label = if i == self.active_tab_idx {
                self.tab_label()
            } else {
                self.tabs[i].label.clone()
            };
            let style = if i == self.active_tab_idx {
                Style::default()
                    .bg(Color::DarkGray)
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };
            spans.push(Span::styled(format!(" {} ", label), style));
            if i + 1 < n {
                spans.push(Span::styled("│", Style::default().fg(Color::DarkGray)));
            }
        }
        frame.render_widget(Paragraph::new(Line::from(spans)), area);
    }
}

impl Default for ContentCartridge {
    fn default() -> Self {
        Self::new()
    }
}

impl Cartridge for ContentCartridge {
    fn fkey(&self) -> FKey {
        FKey::F4
    }

    fn title(&self) -> &str {
        "Content"
    }

    fn tick(&mut self) {
        // Drain health poll — take the most recent availability status
        while let Ok(available) = self.health_rx.try_recv() {
            self.offline = !available;
        }
    }

    fn accept_transfer(&mut self, text: String) {
        let mut ta = TextArea::default();
        ta.set_placeholder_text(PLACEHOLDER);
        ta.insert_str(&text);
        self.textarea = ta;
        self.state = ContentState::Input {
            protocol_idx: DEFAULT_PROTOCOL_IDX,
        };
    }

    fn set_graphics_caps(
        &mut self,
        kitty: bool,
        sixel: bool,
        font_size: (u16, u16),
        truecolor: bool,
    ) {
        self.pdf_kitty = kitty;
        self.pdf_sixel = sixel;
        self.pdf_font_size = font_size;
        self.truecolor = truecolor;
    }

    fn render(&mut self, frame: &mut Frame, area: Rect) {
        // Poll for HTTP result (non-blocking)
        let new_state: Option<ContentState> =
            if let ContentState::Submitting { rx, original, .. } = &mut self.state {
                match rx.try_recv() {
                    Ok(Ok(resp)) => Some(ContentState::Results {
                        response: resp,
                        original: original.clone(),
                        scroll: 0,
                        born_at: Instant::now(),
                        witness_at: format_hhmm_utc(),
                    }),
                    Ok(Err(e)) => Some(ContentState::Error {
                        message: e.to_string(),
                    }),
                    Err(mpsc::TryRecvError::Disconnected) => Some(ContentState::Error {
                        message: "HTTP thread disconnected unexpectedly".into(),
                    }),
                    Err(mpsc::TryRecvError::Empty) => None,
                }
            } else {
                None
            };
        if let Some(ns) = new_state {
            self.state = ns;
        }

        // Drain SSE tokens for DraftingNew
        if let ContentState::DraftingNew {
            rx,
            buffer,
            done,
            error,
            ..
        } = &mut self.state
        {
            loop {
                match rx.try_recv() {
                    Ok(DraftEvent::Token(tok)) => buffer.push_str(&tok),
                    Ok(DraftEvent::Done) => {
                        *done = true;
                        break;
                    }
                    Ok(DraftEvent::Error(e)) => {
                        *error = Some(e);
                        *done = true;
                        break;
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        *done = true;
                        break;
                    }
                }
            }
        }

        // Patience ring: wait_since drives elapsed_ms at render time — no tick needed.

        // Drain search results — take() frees the borrow so we can act on self.state freely
        let search_rx_opt = if let ContentState::SearchResults { search_rx, .. } = &mut self.state {
            search_rx.take()
        } else {
            None
        };
        if let Some(rx) = search_rx_opt {
            match rx.try_recv() {
                Ok(Ok(found)) => {
                    if let ContentState::SearchResults { results, .. } = &mut self.state {
                        *results = found;
                    }
                }
                Ok(Err(e)) => {
                    self.state = ContentState::Error {
                        message: format!("Search error: {}", e),
                    };
                }
                Err(mpsc::TryRecvError::Empty) => {
                    // Not done yet — put receiver back
                    if let ContentState::SearchResults { search_rx, .. } = &mut self.state {
                        *search_rx = Some(rx);
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    // Thread done but results were already set (or errored above)
                }
            }
        }

        // Drain PDF render channel — take() frees the borrow
        let pdf_rx_opt = if let ContentState::PdfView { render_rx, .. } = &mut self.state {
            render_rx.take()
        } else {
            None
        };
        if let Some(rx) = pdf_rx_opt {
            match rx.try_recv() {
                Ok(Ok(data)) => {
                    if let ContentState::PdfView {
                        total_pages,
                        current_image,
                        proto_state,
                        error,
                        ..
                    } = &mut self.state
                    {
                        *total_pages = data.total_pages;
                        *current_image = Some(data.image);
                        *proto_state = None; // force rebuild for the new page
                        *error = None;
                    }
                }
                Ok(Err(e)) => {
                    if let ContentState::PdfView { error, .. } = &mut self.state {
                        *error = Some(e.to_string());
                    }
                }
                Err(mpsc::TryRecvError::Empty) => {
                    if let ContentState::PdfView { render_rx, .. } = &mut self.state {
                        *render_rx = Some(rx);
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => {}
            }
        }

        // Extract state data — borrow ends before calling render helpers
        enum Cmd {
            Input(usize),
            Picker(usize),
            Submitting(u64),
            Results(ProofreadResponse, String, u16, u64, String),
            Drafting(String, String, bool, Option<String>, u16),
            Search(String, Vec<SearchResult>, usize, u16, bool),
            Pdf(String, u32, u32),
            Error(String),
            MultiDraft(Vec<String>, usize, usize),
        }
        let cmd = match &self.state {
            ContentState::Input { protocol_idx } => Cmd::Input(*protocol_idx),
            ContentState::PickProtocol { selected, .. } => Cmd::Picker(*selected),
            ContentState::Submitting { wait_since, .. } => {
                Cmd::Submitting(wait_since.elapsed().as_millis() as u64)
            }
            ContentState::Results {
                response,
                original,
                scroll,
                born_at,
                witness_at,
            } => Cmd::Results(response.clone(), original.clone(), *scroll, born_at.elapsed().as_millis() as u64, witness_at.clone()),
            ContentState::DraftingNew {
                title,
                buffer,
                done,
                error,
                scroll,
                ..
            } => Cmd::Drafting(title.clone(), buffer.clone(), *done, error.clone(), *scroll),
            ContentState::SearchResults {
                query,
                results,
                search_rx,
                selected,
                scroll,
            } => Cmd::Search(
                query.clone(),
                results.clone(),
                *selected,
                *scroll,
                search_rx.is_some(),
            ),
            ContentState::PdfView {
                path,
                page,
                total_pages,
                ..
            } => Cmd::Pdf(path.clone(), *page, *total_pages),
            ContentState::Error { message } => Cmd::Error(message.clone()),
            ContentState::MultiDraft { tabs, active } => Cmd::MultiDraft(
                tabs.iter().map(|t| t.label.clone()).collect(),
                *active,
                tabs.get(*active)
                    .map(|t| t.protocol_idx)
                    .unwrap_or(DEFAULT_PROTOCOL_IDX),
            ),
        };

        self.pending_hyperlinks.clear();
        let render_area = if self.tabs.len() > 1 {
            let splits = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(1), Constraint::Min(0)])
                .split(area);
            self.render_tab_bar(frame, splits[0]);
            splits[1]
        } else {
            area
        };
        match cmd {
            Cmd::Input(pidx) => self.render_input(frame, render_area, pidx),
            Cmd::Picker(sel) => Self::render_picker(frame, render_area, sel, self.selection_bg()),
            Cmd::Submitting(elapsed_ms) => {
                Self::render_submitting(frame, render_area, elapsed_ms, self.truecolor)
            }
            Cmd::Results(resp, orig, sc, born_ms, wit) => {
                Self::render_results(frame, render_area, &resp, &orig, sc, born_ms, self.truecolor, &wit)
            }
            Cmd::Drafting(t, buf, done, err, sc) => {
                Self::render_drafting(frame, render_area, &t, &buf, done, err.as_deref(), sc)
            }
            Cmd::Search(q, results, sel, sc, loading) => {
                self.pending_hyperlinks = Self::render_search_results(
                    frame,
                    render_area,
                    &q,
                    &results,
                    sel,
                    sc,
                    loading,
                    self.selection_bg(),
                    &self.content_endpoint,
                );
            }
            Cmd::Pdf(path, page, total) => {
                self.render_pdf_view(frame, render_area, &path, page, total)
            }
            Cmd::Error(msg) => Self::render_error(frame, render_area, &msg),
            Cmd::MultiDraft(labels, active, pidx) => {
                self.render_multidraft(frame, render_area, &labels, active, pidx)
            }
        }
    }

    fn handle_event(&mut self, event: &Event) -> CartridgeAction {
        let Event::Key(key) = event else {
            return CartridgeAction::None;
        };

        // Extract state discriminant and data (borrow ends before we mutate)
        enum StateKind {
            Input(usize),
            Picker(usize, Vec<String>),
            Submitting,
            Results,
            Drafting,
            Search,
            Pdf,
            Error,
            MultiDraft(usize),
        }
        let kind = match &self.state {
            ContentState::Input { protocol_idx } => StateKind::Input(*protocol_idx),
            ContentState::PickProtocol {
                selected,
                saved_text,
            } => StateKind::Picker(*selected, saved_text.clone()),
            ContentState::Submitting { .. } => StateKind::Submitting,
            ContentState::Results { .. } => StateKind::Results,
            ContentState::DraftingNew { .. } => StateKind::Drafting,
            ContentState::SearchResults { .. } => StateKind::Search,
            ContentState::PdfView { .. } => StateKind::Pdf,
            ContentState::Error { .. } => StateKind::Error,
            ContentState::MultiDraft { active, .. } => StateKind::MultiDraft(*active),
        };

        // Tab management — intercept before state-specific handlers
        if key.modifiers == KeyModifiers::CONTROL {
            match key.code {
                KeyCode::Char('t') => {
                    self.new_tab();
                    return CartridgeAction::Consumed;
                }
                KeyCode::Char('w') if self.tabs.len() > 1 => {
                    self.close_tab();
                    return CartridgeAction::Consumed;
                }
                KeyCode::Tab => {
                    self.cycle_tab(true);
                    return CartridgeAction::Consumed;
                }
                KeyCode::BackTab => {
                    self.cycle_tab(false);
                    return CartridgeAction::Consumed;
                }
                _ => {}
            }
        }

        match kind {
            StateKind::Input(pidx) => self.on_input_key(event, pidx),
            StateKind::Picker(sel, saved) => self.on_picker_key(key, sel, &saved),
            StateKind::Submitting => CartridgeAction::Consumed,
            StateKind::Results => self.on_results_key(key),
            StateKind::Drafting => self.on_drafting_key(key),
            StateKind::Search => self.on_search_key(key),
            StateKind::Pdf => self.on_pdf_key(key),
            StateKind::Error => {
                self.reset_textarea(DEFAULT_PROTOCOL_IDX);
                CartridgeAction::Consumed
            }
            StateKind::MultiDraft(active) => self.on_multidraft_key(event, active),
        }
    }

    fn flush_hyperlinks(&self) {
        use crossterm::{cursor::MoveTo, style::Print, ExecutableCommand};
        let mut stdout = std::io::stdout();
        for h in &self.pending_hyperlinks {
            let osc8 = format!(
                "\x1b]8;;{}\x1b\\{}\x1b]8;;\x1b\\",
                h.url,
                &h.text[..h.text.len().min((h.text.len()).max(1))]
            );
            let _ = stdout
                .execute(MoveTo(h.col, h.row))
                .and_then(|s| s.execute(Print(osc8)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cartridge() -> ContentCartridge {
        ContentCartridge::new()
    }

    #[test]
    fn new_starts_with_one_tab() {
        let c = cartridge();
        assert_eq!(c.tabs.len(), 1);
        assert_eq!(c.active_tab_idx, 0);
    }

    #[test]
    fn new_tab_increments_count() {
        let mut c = cartridge();
        c.new_tab();
        assert_eq!(c.tabs.len(), 2);
        assert_eq!(c.active_tab_idx, 1);
    }

    #[test]
    fn new_tab_caps_at_four() {
        let mut c = cartridge();
        c.new_tab();
        c.new_tab();
        c.new_tab();
        // 4 tabs — should be rejected
        c.new_tab();
        assert_eq!(c.tabs.len(), 4);
        assert_eq!(c.active_tab_idx, 3);
    }

    #[test]
    fn close_tab_decrements_count() {
        let mut c = cartridge();
        c.new_tab();
        assert_eq!(c.tabs.len(), 2);
        c.close_tab();
        assert_eq!(c.tabs.len(), 1);
    }

    #[test]
    fn close_last_tab_is_noop() {
        let mut c = cartridge();
        c.close_tab();
        assert_eq!(c.tabs.len(), 1);
        assert_eq!(c.active_tab_idx, 0);
    }

    #[test]
    fn cycle_tab_wraps_forward() {
        let mut c = cartridge();
        c.new_tab(); // tabs=2, active=1
        c.cycle_tab(true); // should wrap to 0
        assert_eq!(c.active_tab_idx, 0);
    }

    #[test]
    fn cycle_tab_wraps_backward() {
        let mut c = cartridge();
        c.new_tab(); // tabs=2, active=1
        c.switch_to_tab(0); // active=0
        c.cycle_tab(false); // should wrap to 1
        assert_eq!(c.active_tab_idx, 1);
    }

    #[test]
    fn switch_to_tab_changes_active_idx() {
        let mut c = cartridge();
        c.new_tab();
        c.new_tab();
        c.switch_to_tab(0);
        assert_eq!(c.active_tab_idx, 0);
        c.switch_to_tab(2);
        assert_eq!(c.active_tab_idx, 2);
    }

    #[test]
    fn close_non_last_tab_adjusts_index() {
        let mut c = cartridge();
        c.new_tab();
        c.new_tab(); // 3 tabs, active=2
        c.switch_to_tab(0); // active=0
        c.close_tab(); // removes tab 0; new active should be 0 (shifted)
        assert_eq!(c.tabs.len(), 2);
        assert_eq!(c.active_tab_idx, 0);
    }
}
