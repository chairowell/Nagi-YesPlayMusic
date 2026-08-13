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

use super::text::pad_display;

const SIDEBAR_WIDTH: u16 = 16;
pub const COLLAPSE_BELOW: u16 = 50;

pub fn draw(frame: &mut Frame, state: &AppState, area: Rect, hits: &mut Hits) {
    if area.width >= COLLAPSE_BELOW {
        let [sidebar, list] =
            Layout::horizontal([Constraint::Length(SIDEBAR_WIDTH), Constraint::Min(0)]).areas(area);
        draw_sidebar(frame, state, sidebar, hits);
        draw_list(frame, state, list, hits);
    } else {
        draw_list(frame, state, area, hits);
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
        let selected = visible_index == state.selected && !state.filter.input;
        let style = if selected {
            Style::new().fg(theme.selection_fg()).bg(theme.sel)
        } else {
            Style::new().fg(theme.fg)
        };
        lines.push(Line::from(Span::styled(
            format!(
                "  {:>3}  {} {} {:>5}",
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
