//! Library view: collapsible sidebar + track list. Sidebar entries become
//! real NCM playlists in the service stage.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::AppState;
use crate::i18n::{self, Key};
use crate::ui::Hits;

use super::cover_preview;
use super::text::{pad_display, pad_or_marquee};

const SIDEBAR_WIDTH: u16 = 16;
const MIN_LIST_WIDTH: u16 = 52;
pub const COLLAPSE_BELOW: u16 = 50;

pub fn draw(frame: &mut Frame, state: &mut AppState, area: Rect, hits: &mut Hits) {
    let content = if area.width >= COLLAPSE_BELOW {
        let [sidebar, content] =
            Layout::horizontal([Constraint::Length(SIDEBAR_WIDTH), Constraint::Min(0)]).areas(area);
        draw_sidebar(frame, state, sidebar, hits);
        content
    } else {
        area
    };

    let has_selected_row = !state.sidebar_focus
        && !state.filter.input
        && state.library.iter().any(|row| state.filter.matches(row));
    let (list, preview) = if has_selected_row {
        cover_preview::split_preview(content, MIN_LIST_WIDTH)
    } else {
        (content, None)
    };
    draw_list(frame, state, list, hits);
    if let Some(preview) = preview {
        cover_preview::draw(frame, state, preview);
    }
}

pub const SOURCES: [Key; 4] = [
    Key::LikedSongs,
    Key::DailyRecommendations,
    Key::PersonalFm,
    Key::CloudDrive,
];

fn draw_sidebar(frame: &mut Frame, state: &AppState, area: Rect, hits: &mut Hits) {
    let theme = &state.theme;
    let account = match &state.session.nickname {
        Some(nickname) => Line::from(Span::styled(
            format!("♪ {nickname}"),
            Style::new().fg(theme.accent2),
        )),
        None => Line::from(Span::styled(
            i18n::t(Key::NotLoggedInMenu),
            Style::new().fg(theme.accent2),
        )),
    };
    let mut lines = vec![account, Line::default()];
    for (index, key) in SOURCES.iter().enumerate() {
        let y = area.y + 2 + index as u16;
        if y < area.y + area.height {
            hits.sidebar.push((
                Rect {
                    x: area.x,
                    y,
                    width: area.width,
                    height: 1,
                },
                index,
            ));
        }
        let is_current = index == state.source_index();
        let is_cursor = state.sidebar_focus && index == state.sidebar_selected;
        let marker = if is_current { "▸" } else { " " };
        let style = if is_cursor {
            Style::new().fg(theme.selection_fg()).bg(theme.sel)
        } else if is_current {
            Style::new().fg(theme.accent)
        } else {
            Style::new().fg(theme.dim)
        };
        lines.push(Line::from(Span::styled(
            format!("{marker} {}", i18n::t(*key)),
            style,
        )));
    }
    frame.render_widget(Paragraph::new(lines), area);
}

fn draw_list(frame: &mut Frame, state: &AppState, area: Rect, hits: &mut Hits) {
    let theme = &state.theme;
    let rows = state.visible_rows(&state.library);
    if rows.is_empty() {
        let message = if !state.filter.query.is_empty() && !state.library.is_empty() {
            i18n::t(Key::NoResults)
        } else if state.session.nickname.is_some() && !state.library_synced {
            i18n::t(Key::SyncingLibrary)
        } else {
            i18n::t(Key::EmptyLibrary)
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
    let visible = area.height.saturating_sub(1) as usize; // header row
    let offset = super::scroll_offset(state.selected, rows.len(), visible);
    let marquee_frame = state.marquee_frame();

    let mut lines = Vec::with_capacity(visible + 1);
    lines.push(Line::from(Span::styled(
        format!(
            "  {:>3}  {} {} {:>5}",
            "#",
            pad_display(i18n::t(Key::ColumnTitle), 24),
            pad_display(i18n::t(Key::ColumnArtist), 14),
            i18n::t(Key::ColumnDuration)
        ),
        Style::new().fg(theme.faint),
    )));
    for (visible_index, (index, row)) in rows.iter().enumerate().skip(offset).take(visible) {
        hits.rows.push((
            Rect {
                x: area.x,
                y: area.y + 1 + (visible_index - offset) as u16,
                width: area.width,
                height: 1,
            },
            visible_index,
        ));
        let selected =
            visible_index == state.selected && !state.filter.input && !state.sidebar_focus;
        let style = if selected {
            Style::new().fg(theme.selection_fg()).bg(theme.sel)
        } else {
            Style::new().fg(theme.fg)
        };
        lines.push(Line::from(Span::styled(
            format!(
                "  {:>3}  {} {} {:>5}",
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

#[cfg(test)]
mod tests {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    use super::*;
    use crate::config::Config;

    fn rendered_library(width: u16) -> (ratatui::buffer::Buffer, Hits) {
        let backend = TestBackend::new(width, 15);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = AppState::new(&Config::default());
        let mut hits = Hits::default();
        terminal
            .draw(|frame| draw(frame, &mut state, frame.area(), &mut hits))
            .unwrap();
        (terminal.backend().buffer().clone(), hits)
    }

    #[test]
    fn preview_and_list_hits_share_the_library_at_width_96() {
        let (buffer, hits) = rendered_library(96);

        assert!(!hits.rows.is_empty());
        assert!(hits
            .rows
            .iter()
            .all(|(area, _)| area.x == 16 && area.width == 52));
        assert_eq!(buffer[(70, 0)].symbol(), "▀");
    }

    #[test]
    fn width_95_keeps_the_whole_content_area_clickable() {
        let (buffer, hits) = rendered_library(95);

        assert!(!hits.rows.is_empty());
        assert!(hits
            .rows
            .iter()
            .all(|(area, _)| area.x == 16 && area.width == 79));
        assert_ne!(buffer[(70, 0)].symbol(), "▀");
    }
}
