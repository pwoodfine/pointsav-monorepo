use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::Paragraph,
    Frame,
};
use serde::Deserialize;

use crate::session::User;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct HealthState {
    #[serde(default)]
    pub ready: bool,
    #[serde(default)]
    pub has_doorman: bool,
    #[serde(default)]
    pub has_lt: bool,
}

pub async fn fetch_health() -> HealthState {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .unwrap_or_default();
    match client
        .get("http://127.0.0.1:9092/v1/health/ready")
        .send()
        .await
    {
        Ok(resp) => resp.json::<HealthState>().await.unwrap_or_default(),
        Err(_) => HealthState::default(),
    }
}

pub fn render(frame: &mut Frame, area: Rect, user: &User, health: &HealthState, elapsed_secs: u64) {
    let tier = if health.has_doorman {
        "Tier A (OLMo 3 7B)"
    } else {
        "Doorman offline"
    };
    let status = if health.ready { "Ready" } else { "Degraded" };
    let h = elapsed_secs / 3600;
    let m = (elapsed_secs % 3600) / 60;
    let s = elapsed_secs % 60;

    let text = format!(
        " {}@{}  │  {}  │  {}  │  {:02}:{:02}:{:02} ",
        user.username,
        user.tenant.as_str(),
        tier,
        status,
        h,
        m,
        s,
    );

    let bg = if health.ready { Color::DarkGray } else { Color::Yellow };
    let fg = if health.ready { Color::White } else { Color::Black };
    let style = Style::default().fg(fg).bg(bg).add_modifier(Modifier::BOLD);
    frame.render_widget(Paragraph::new(text).style(style), area);
}
