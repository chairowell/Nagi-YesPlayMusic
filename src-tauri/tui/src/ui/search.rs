//! Search view: one input line, then the same list language as the library.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::AppState;
use crate::i18n::{self, Key};
use crate::ui::text::{needs_marquee, pad_or_marquee};
use crate::ui::Hits;

use super::cover_preview;

/// Showing a cover keeps at least a 58-column padded result list.
const PREVIEW_MIN_LIST_PANEL_WIDTH: u16 = 62;
/// Full shell width required for the result panel, gap, and framed preview.
pub(crate) const PREVIEW_MIN_TERMINAL_WIDTH: u16 =
    PREVIEW_MIN_LIST_PANEL_WIDTH + super::PANEL_GAP_X + cover_preview::WIDTH;
/// Search input occupies exactly one terminal row.
const SEARCH_INPUT_HEIGHT: u16 = 1;
/// One blank row separates the query from tabular results.
const SEARCH_INPUT_GAP: u16 = 1;
/// The solid heart state occupies one terminal cell.
const HEART_WIDTH: usize = 1;
/// Keep the heart distinct from the primary title.
const HEART_TITLE_GAP: usize = 1;
/// Artist metadata keeps a stable scan width.
const ARTIST_WIDTH: usize = 14;
/// Terminal playback durations always use `mm:ss`.
const DURATION_WIDTH: usize = 5;
/// Metadata columns are separated by one blank terminal cell.
const COLUMN_GAP: usize = 1;

pub fn draw(frame: &mut Frame, state: &mut AppState, area: Rect, hits: &mut Hits) {
    let has_selected_row = !state.search.input
        && !state.filter.input
        && state
            .search
            .results
            .iter()
            .any(|row| state.filter.matches(row));
    let (panel_area, preview_area) = if has_selected_row {
        cover_preview::split_preview(area, PREVIEW_MIN_LIST_PANEL_WIDTH)
    } else {
        (area, None)
    };
    draw_panel(frame, state, panel_area, hits);
    if let Some(preview_area) = preview_area {
        cover_preview::draw(frame, state, preview_area);
    }
}

pub(crate) fn marquee_needed(
    row: &crate::api::SongRow,
    area_width: u16,
    preview_visible: bool,
) -> bool {
    let area = Rect::new(0, 0, area_width, cover_preview::HEIGHT);
    let (panel, preview) = if preview_visible {
        cover_preview::split_preview(area, PREVIEW_MIN_LIST_PANEL_WIDTH)
    } else {
        (area, None)
    };
    let columns = SearchColumns::for_width(super::panel_inner_width(panel.width));
    needs_marquee(&row.title, columns.title)
        || needs_marquee(&row.artist, ARTIST_WIDTH)
        || preview.is_some() && cover_preview::metadata_needs_marquee(row)
}

fn draw_panel(frame: &mut Frame, state: &AppState, area: Rect, hits: &mut Hits) {
    let theme = &state.theme;
    let icons = crate::icons::for_style(state.config.icons);
    let rows = state.visible_rows(&state.search.results);
    let block = super::panel_block(
        theme,
        i18n::t(Key::Search),
        Some(i18n::t_track_count(rows.len())),
    )
    .title_bottom(super::filter_title(state));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.is_empty() {
        return;
    }
    let [input_area, _, list_area] = Layout::vertical([
        Constraint::Length(SEARCH_INPUT_HEIGHT),
        Constraint::Length(SEARCH_INPUT_GAP),
        Constraint::Min(0),
    ])
    .areas(inner);

    let cursor = if state.search.input { "▎" } else { "" };
    let query_style = if state.search.input {
        Style::new().fg(theme.fg)
    } else {
        Style::new().fg(theme.dim)
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(format!("{} ", icons.search), Style::new().fg(theme.faint)),
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
                Style::new().fg(theme.accent2),
            )))
            .centered(),
            list_area,
        );
        return;
    }
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

    let visible = list_area.height.saturating_sub(1) as usize; // header row
    let offset = super::scroll_offset(state.selected, rows.len(), visible);
    let marquee_frame = state.marquee_frame();
    let columns = SearchColumns::for_width(list_area.width as usize);
    let mut lines = Vec::with_capacity(visible + 1);
    lines.push(columns.header(theme));
    for (visible_index, (_, row)) in rows.iter().enumerate().skip(offset).take(visible) {
        hits.rows.push((
            Rect {
                x: list_area.x,
                y: list_area.y + 1 + (visible_index - offset) as u16,
                width: list_area.width,
                height: 1,
            },
            visible_index,
        ));
        let selected =
            visible_index == state.selected && !state.search.input && !state.filter.input;
        let liked = state.liked.contains(&row.id);
        lines.push(columns.row(theme, icons.heart, row, liked, selected, marquee_frame));
    }
    frame.render_widget(Paragraph::new(lines), list_area);
}

#[derive(Clone, Copy)]
struct SearchColumns {
    title: usize,
}

impl SearchColumns {
    fn for_width(width: usize) -> Self {
        let fixed = HEART_WIDTH + HEART_TITLE_GAP + ARTIST_WIDTH + DURATION_WIDTH + COLUMN_GAP * 2;
        Self {
            title: width.saturating_sub(fixed),
        }
    }

    fn header(self, theme: &crate::theme::Theme) -> Line<'static> {
        self.header_with_duration(theme, i18n::t(Key::ColumnDuration))
    }

    fn header_with_duration(
        self,
        theme: &crate::theme::Theme,
        duration_label: &str,
    ) -> Line<'static> {
        let style = Style::new().fg(theme.faint);
        Line::from(vec![
            Span::styled(" ".repeat(HEART_WIDTH + HEART_TITLE_GAP), style),
            Span::styled(
                super::text::pad_display(i18n::t(Key::ColumnTitle), self.title),
                style,
            ),
            Span::styled(" ".repeat(COLUMN_GAP), style),
            Span::styled(
                super::text::pad_display(i18n::t(Key::ColumnArtist), ARTIST_WIDTH),
                style,
            ),
            Span::styled(" ".repeat(COLUMN_GAP), style),
            Span::styled(
                super::text::pad_display_right(duration_label, DURATION_WIDTH),
                style,
            ),
        ])
    }

    fn row(
        self,
        theme: &crate::theme::Theme,
        heart: &'static str,
        row: &crate::api::SongRow,
        liked: bool,
        selected: bool,
        marquee_frame: u64,
    ) -> Line<'static> {
        let base = if selected {
            Style::new().bg(theme.selection_bg())
        } else {
            Style::new()
        };
        let title_style = if selected {
            base.fg(theme.fg).add_modifier(Modifier::BOLD)
        } else {
            base.fg(theme.fg)
        };
        Line::from(vec![
            Span::styled(
                heart,
                base.fg(if liked { theme.accent2 } else { theme.faint }),
            ),
            Span::styled(" ".repeat(HEART_TITLE_GAP), base),
            Span::styled(
                pad_or_marquee(&row.title, self.title, selected, marquee_frame),
                title_style,
            ),
            Span::styled(" ".repeat(COLUMN_GAP), base),
            Span::styled(
                pad_or_marquee(&row.artist, ARTIST_WIDTH, selected, marquee_frame),
                base.fg(theme.dim),
            ),
            Span::styled(" ".repeat(COLUMN_GAP), base),
            Span::styled(
                format!("{:>DURATION_WIDTH$}", super::format_ms(row.duration_ms)),
                base.fg(theme.faint),
            ),
        ])
    }
}

#[cfg(test)]
mod tests {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    use super::*;
    use crate::api::SongRow;
    use crate::config::Config;

    #[test]
    fn cjk_duration_header_fits_every_supported_search_width() {
        let state = AppState::new(&Config::default());
        let minimum =
            HEART_WIDTH + HEART_TITLE_GAP + ARTIST_WIDTH + DURATION_WIDTH + COLUMN_GAP * 2;

        for width in minimum..=200 {
            let header = SearchColumns::for_width(width).header_with_duration(&state.theme, "时长");
            assert_eq!(header.width(), width, "width {width}");

            let backend = TestBackend::new(width as u16, 1);
            let mut terminal = Terminal::new(backend).unwrap();
            terminal
                .draw(|frame| frame.render_widget(Paragraph::new(header.clone()), frame.area()))
                .unwrap();
            let buffer = terminal.backend().buffer();
            assert_eq!(buffer[((width - 4) as u16, 0)].symbol(), "时");
            assert_eq!(buffer[((width - 2) as u16, 0)].symbol(), "长");
        }
    }

    fn rendered_search(width: u16, height: u16) -> (ratatui::buffer::Buffer, Hits, AppState) {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = AppState::new(&Config::default());
        state.search.input = false;
        state.search.results.push(SongRow {
            id: 1,
            title: "Track".into(),
            artist: "Artist".into(),
            album: "Album".into(),
            duration_ms: 180_000,
            pic_url: None,
        });
        let mut hits = Hits::default();
        terminal
            .draw(|frame| draw(frame, &mut state, frame.area(), &mut hits))
            .unwrap();
        (terminal.backend().buffer().clone(), hits, state)
    }

    #[test]
    fn eighty_columns_keep_a_complete_search_panel_without_preview() {
        let (buffer, hits, _state) = rendered_search(80, 24);

        assert_eq!(hits.rows.len(), 1);
        assert_eq!(hits.rows[0].0, Rect::new(2, 4, 76, 1));
        for (position, symbol) in [
            ((0, 0), "╭"),
            ((79, 0), "╮"),
            ((0, 23), "╰"),
            ((79, 23), "╯"),
        ] {
            assert_eq!(buffer[position].symbol(), symbol);
        }
    }

    #[test]
    fn wide_search_keeps_the_cover_two_columns_after_the_panel() {
        let (buffer, hits, _state) = rendered_search(120, 40);

        assert_eq!(hits.rows.len(), 1);
        assert_eq!(hits.rows[0].0, Rect::new(2, 4, 88, 1));
        assert_eq!(buffer[(91, 0)].symbol(), "╮");
        for (position, symbol) in [
            ((94, 0), "╭"),
            ((119, 0), "╮"),
            ((94, 14), "╰"),
            ((119, 14), "╯"),
            ((96, 1), "▀"),
        ] {
            assert_eq!(buffer[position].symbol(), symbol);
        }
    }

    #[test]
    fn selected_search_row_uses_subtle_background_and_three_text_tiers() {
        let (buffer, _hits, state) = rendered_search(80, 24);

        let title = &buffer[(4, 4)];
        assert_eq!(title.fg, state.theme.fg);
        assert_eq!(title.bg, state.theme.selection_bg());
        assert!(title.modifier.contains(Modifier::BOLD));
        assert_eq!(buffer[(59, 4)].fg, state.theme.dim);
        assert_eq!(buffer[(73, 4)].fg, state.theme.faint);
    }

    #[test]
    fn search_rows_render_solid_hearts_in_state_colors() {
        for liked in [false, true] {
            let backend = TestBackend::new(80, 15);
            let mut terminal = Terminal::new(backend).unwrap();
            let mut state = AppState::new(&Config::default());
            state.search.input = false;
            state.search.results.push(SongRow {
                id: 1,
                title: "Track".into(),
                artist: "Artist".into(),
                album: "Album".into(),
                duration_ms: 180_000,
                pic_url: None,
            });
            if liked {
                state.liked.insert(1);
            }
            let mut hits = Hits::default();

            terminal
                .draw(|frame| draw(frame, &mut state, frame.area(), &mut hits))
                .unwrap();

            let cell = &terminal.backend().buffer()[(2, 4)];
            assert_eq!(cell.symbol(), "♥");
            assert_eq!(
                cell.fg,
                if liked {
                    state.theme.accent2
                } else {
                    state.theme.faint
                }
            );
        }
    }
}
