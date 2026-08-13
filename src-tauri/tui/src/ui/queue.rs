//! Queue view: the current listening context; the play glyph marks its row.

use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::AppState;
use crate::i18n::{self, Key};
use crate::ui::text::pad_or_marquee;
use crate::ui::Hits;

pub fn draw(frame: &mut Frame, state: &AppState, area: Rect, hits: &mut Hits) {
    let theme = &state.theme;
    let icons = crate::icons::for_style(state.config.icons);
    let rows = state.visible_rows(&state.queue);
    if rows.is_empty() {
        let message = if !state.filter.query.is_empty() && !state.queue.is_empty() {
            i18n::t(Key::NoResults)
        } else {
            i18n::t(Key::EmptyQueue)
        };
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                message,
                Style::new().fg(theme.dim),
            )))
            .centered(),
            area,
        );
        return;
    }

    let visible = area.height as usize;
    let offset = super::scroll_offset(state.selected, rows.len(), visible);
    let marquee_frame = state.marquee_frame();
    let mut lines = Vec::with_capacity(visible);
    for (visible_index, (index, row)) in rows.iter().enumerate().skip(offset).take(visible) {
        hits.rows.push((
            Rect {
                x: area.x,
                y: area.y + (visible_index - offset) as u16,
                width: area.width,
                height: 1,
            },
            visible_index,
        ));
        let playing = state.queue_pos == Some(*index);
        let selected = visible_index == state.selected && !state.filter.input;
        let marker = if playing { icons.play } else { " " };
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
                pad_or_marquee(&row.title, 24, selected, marquee_frame),
                pad_or_marquee(&row.artist, 14, selected, marquee_frame),
                super::format_ms(row.duration_ms)
            ),
            style,
        )));
    }
    frame.render_widget(Paragraph::new(lines), area);
}
