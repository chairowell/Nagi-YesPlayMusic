//! Search view: one input line, then the same list language as the library.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::AppState;
use crate::i18n::{self, Key};
use crate::ui::text::pad_or_marquee;
use crate::ui::Hits;

use super::cover_preview;

const MIN_LIST_WIDTH: u16 = 53;

pub fn draw(frame: &mut Frame, state: &mut AppState, area: Rect, hits: &mut Hits) {
    let theme = &state.theme;
    let [input_area, _, list_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(0),
    ])
    .areas(area);

    let cursor = if state.search.input { "▎" } else { "" };
    let query_style = if state.search.input {
        Style::new().fg(theme.fg)
    } else {
        Style::new().fg(theme.dim)
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("  / ", Style::new().fg(theme.accent)),
            Span::styled(format!("{}{cursor}", state.search.query), query_style),
            Span::styled(
                if state.search.query.is_empty() && state.search.input {
                    format!("  {}", i18n::t(Key::TypeToSearch))
                } else {
                    String::new()
                },
                Style::new().fg(theme.faint),
            ),
        ])),
        input_area,
    );

    if state.search.searching {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                i18n::t(Key::Searching),
                Style::new().fg(theme.dim),
            )))
            .centered(),
            list_area,
        );
        return;
    }
    if let Some(message) = &state.search.error {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                message.as_str(),
                Style::new().fg(theme.accent),
            )))
            .centered(),
            list_area,
        );
        return;
    }
    let rows = state.visible_rows(&state.search.results);
    if rows.is_empty() {
        let message = if state.search.query.is_empty() {
            i18n::t(Key::SearchPrompt)
        } else {
            i18n::t(Key::NoResults)
        };
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                message,
                Style::new().fg(theme.faint),
            )))
            .centered(),
            list_area,
        );
        return;
    }

    let (list_area, preview_area) = if state.search.input || state.filter.input {
        (list_area, None)
    } else {
        cover_preview::split_preview(list_area, MIN_LIST_WIDTH)
    };

    let visible = list_area.height as usize;
    let offset = super::scroll_offset(state.selected, rows.len(), visible);
    let marquee_frame = state.marquee_frame();
    let mut lines = Vec::with_capacity(visible);
    for (visible_index, (_, row)) in rows.iter().enumerate().skip(offset).take(visible) {
        hits.rows.push((
            Rect {
                x: list_area.x,
                y: list_area.y + (visible_index - offset) as u16,
                width: list_area.width,
                height: 1,
            },
            visible_index,
        ));
        let selected =
            visible_index == state.selected && !state.search.input && !state.filter.input;
        let style = if selected {
            Style::new().fg(theme.selection_fg()).bg(theme.sel)
        } else {
            Style::new().fg(theme.fg)
        };
        let liked = if state.liked.contains(&row.id) {
            "♥"
        } else {
            " "
        };
        let mut line_style = style;
        if selected {
            line_style = line_style.add_modifier(Modifier::BOLD);
        }
        lines.push(Line::from(Span::styled(
            format!(
                "  {liked} {} {} {:>5}",
                pad_or_marquee(&row.title, 26, selected, marquee_frame),
                pad_or_marquee(&row.artist, 16, selected, marquee_frame),
                super::format_ms(row.duration_ms)
            ),
            line_style,
        )));
    }
    frame.render_widget(Paragraph::new(lines), list_area);
    if let Some(preview_area) = preview_area {
        cover_preview::draw(frame, state, preview_area);
    }
}

#[cfg(test)]
mod tests {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    use super::*;
    use crate::api::SongRow;
    use crate::config::Config;

    fn rendered_search(width: u16) -> (ratatui::buffer::Buffer, Hits) {
        let backend = TestBackend::new(width, 15);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = AppState::new(&Config::default());
        state.search.input = false;
        state.search.results.push(SongRow {
            id: 1,
            title: "Track".into(),
            artist: "Artist".into(),
            duration_ms: 180_000,
            pic_url: None,
        });
        let mut hits = Hits::default();
        terminal
            .draw(|frame| draw(frame, &mut state, frame.area(), &mut hits))
            .unwrap();
        (terminal.backend().buffer().clone(), hits)
    }

    #[test]
    fn preview_and_list_hits_share_search_at_width_81() {
        let (buffer, hits) = rendered_search(81);

        assert_eq!(hits.rows.len(), 1);
        assert_eq!(hits.rows[0].0, Rect::new(0, 2, 53, 1));
        assert_eq!(buffer[(55, 2)].symbol(), "▀");
    }

    #[test]
    fn width_80_keeps_the_whole_search_row_clickable() {
        let (buffer, hits) = rendered_search(80);

        assert_eq!(hits.rows.len(), 1);
        assert_eq!(hits.rows[0].0, Rect::new(0, 2, 80, 1));
        assert_ne!(buffer[(55, 2)].symbol(), "▀");
    }
}
