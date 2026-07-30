use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::Paragraph,
    Frame,
};

use crate::fkey::FKey;

#[derive(Debug, Clone, PartialEq)]
pub enum MbaStatus {
    Active,
    Inactive(String),
    Pending,
}

impl MbaStatus {
    fn label(&self) -> String {
        match self {
            MbaStatus::Active => "MBA LINK ACTIVE".into(),
            MbaStatus::Inactive(reason) => format!("MBA LINK INACTIVE: {reason}"),
            MbaStatus::Pending => "MBA LINK PENDING".into(),
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn render(
    frame: &mut Frame,
    area: Rect,
    username: &str,
    tenant: &str,
    mba: &MbaStatus,
    active: FKey,
    elapsed_secs: u64,
    pending_pairs: u16,
) {
    let h = elapsed_secs / 3600;
    let m = (elapsed_secs % 3600) / 60;
    let s = elapsed_secs % 60;

    let badge = if pending_pairs > 0 {
        format!("  │  [{} pending]", pending_pairs)
    } else {
        String::new()
    };

    let text = format!(
        " {}@{}  │  {}  │  {}  │  {:02}:{:02}:{:02}{} ",
        username,
        tenant,
        mba.label(),
        active.label(),
        h,
        m,
        s,
        badge,
    );

    let (bg, fg) = match mba {
        MbaStatus::Active => (Color::DarkGray, Color::White),
        MbaStatus::Inactive(_) => (Color::Yellow, Color::Black),
        MbaStatus::Pending => (Color::Blue, Color::White),
    };

    let style = Style::default().fg(fg).bg(bg).add_modifier(Modifier::BOLD);
    frame.render_widget(Paragraph::new(text).style(style), area);
}
