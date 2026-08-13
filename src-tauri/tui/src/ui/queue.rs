//! Queue view: the current listening context; ▶ marks the playing row.

use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::AppState;
use crate::i18n::{self, Key};
use crate::ui::text::pad_display;
use crate::ui::Hits;

pub fn draw(frame: &mut Frame, state: &AppState, area: Rect, hits: &mut Hits) {
    let theme = &state.theme;
    if state.queue.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                i18n::t(Key::EmptyQueue),
                Style::new().fg(theme.dim),
            )))
            .centered(),
            area,
        );
        return;
    }

    let visible = area.height as usize;
    let offset = super::scroll_offset(state.selected, state.queue.len(), visible);
    let mut lines = Vec::with_capacity(visible);
    for (index, row) in state.queue.iter().enumerate().skip(offset).take(visible) {
        hits.rows.push((
            Rect {
                x: area.x,
                y: area.y + (index - offset) as u16,
                width: area.width,
                height: 1,
            },
            index,
        ));
        let playing = state.queue_pos == Some(index);
        let selected = index == state.selected;
        let marker = if playing { "▶" } else { " " };
        let style = if selected {
            Style::new().fg(theme.selection_fg()).bg(theme.sel)
        } else if playing {
            Style::new().fg(theme.accent)
        } else {
            Style::new().fg(theme.fg)
        };
        lines.push(Line::from(Span::styled(
            format!(
                "  {marker} {:>3}  {} {} {:>5}",
                index + 1,
                pad_display(&row.title, 24),
                pad_display(&row.artist, 14),
                super::format_ms(row.duration_ms)
            ),
            style,
        )));
    }
    frame.render_widget(Paragraph::new(lines), area);
}
