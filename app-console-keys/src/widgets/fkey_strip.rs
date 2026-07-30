use std::collections::BTreeSet;

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::fkey::FKey;

pub fn render(frame: &mut Frame, area: Rect, active: FKey, installed: &BTreeSet<FKey>) {
    let mut spans: Vec<Span> = Vec::new();
    for fkey in FKey::all() {
        let label = format!(" {} ", fkey.short());
        let style = if fkey == active {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else if installed.contains(&fkey) {
            Style::default().fg(Color::White).bg(Color::DarkGray)
        } else {
            Style::default().fg(Color::DarkGray).bg(Color::Black)
        };
        spans.push(Span::styled(label, style));
        if fkey != FKey::F12 {
            spans.push(Span::raw(" "));
        }
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}
