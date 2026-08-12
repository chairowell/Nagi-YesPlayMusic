//! Queue view: the current listening context; ▶ marks the playing row.

use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::AppState;
use crate::ui::text::pad_display;
use crate::ui::Hits;

pub fn draw(frame: &mut Frame, state: &AppState, area: Rect, hits: &mut Hits) {
    let theme = &state.theme;
    for (index, _) in state.queue.iter().enumerate() {
        let y = area.y + index as u16;
        if y >= area.y + area.height {
            break;
        }
        hits.rows.push((
            Rect {
                x: area.x,
                y,
                width: area.width,
                height: 1,
            },
            index,
        ));
    }
    if state.queue.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "队列是空的——去曲库按 Enter，整列表就会成为播放队列",
                Style::new().fg(theme.dim),
            )))
            .centered(),
            area,
        );
        return;
    }

    let mut lines = Vec::with_capacity(state.queue.len());
    for (index, row) in state.queue.iter().enumerate() {
        let playing = state.queue_pos == Some(index);
        let selected = index == state.selected;
        let marker = if playing { "▶" } else { " " };
        let style = if selected {
            Style::new().fg(theme.bg).bg(theme.sel)
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
