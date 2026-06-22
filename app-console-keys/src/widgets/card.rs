//! Card + padding helpers (Phase C-1). Consistent chrome so every cartridge
//! gets the same air: rounded cards for human surfaces, square blocks for
//! machine-of-record surfaces (ledgers, audit), and an inner-padding rect helper
//! for the 8×1 spacing rhythm (cards use `pad(area, 2, 1)`).

use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, BorderType, Borders};

/// A rounded card block in the given accent color — for human-facing surfaces.
pub fn card(title: &str, accent: Color) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(accent))
        .title(Line::from(format!(" {title} ")))
}

/// A square-cornered block — for machine-of-record surfaces (audit ledger, WORM
/// ribbon) where the chrome should read as a register, not a card.
pub fn record_block(title: &str, accent: Color) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(accent))
        .title(Line::from(format!(" {title} ")))
}

/// Shrink a rect by `h` cells horizontally and `v` cells vertically on each
/// side. Saturating, so it never underflows on tiny terminals.
pub fn pad(area: Rect, h: u16, v: u16) -> Rect {
    Rect {
        x: area.x.saturating_add(h),
        y: area.y.saturating_add(v),
        width: area.width.saturating_sub(h.saturating_mul(2)),
        height: area.height.saturating_sub(v.saturating_mul(2)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pad_shrinks_symmetrically() {
        let r = Rect {
            x: 10,
            y: 10,
            width: 40,
            height: 20,
        };
        let p = pad(r, 2, 1);
        assert_eq!((p.x, p.y, p.width, p.height), (12, 11, 36, 18));
    }

    #[test]
    fn pad_saturates_on_tiny_rect() {
        let r = Rect {
            x: 0,
            y: 0,
            width: 2,
            height: 1,
        };
        let p = pad(r, 4, 4);
        assert_eq!((p.width, p.height), (0, 0));
    }
}
