//! Library view: collapsible sidebar + track list. Sidebar entries become
//! real NCM playlists in the service stage.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::AppState;

const SIDEBAR_WIDTH: u16 = 16;
const COLLAPSE_BELOW: u16 = 50;

pub fn draw(frame: &mut Frame, state: &AppState, area: Rect) {
    if area.width >= COLLAPSE_BELOW {
        let [sidebar, list] =
            Layout::horizontal([Constraint::Length(SIDEBAR_WIDTH), Constraint::Min(0)])
                .areas(area);
        draw_sidebar(frame, state, sidebar);
        draw_list(frame, state, list);
    } else {
        draw_list(frame, state, area);
    }
}

fn draw_sidebar(frame: &mut Frame, state: &AppState, area: Rect) {
    let theme = &state.theme;
    let lines = vec![
        Line::from(Span::styled("▸ 我喜欢的音乐", Style::new().fg(theme.accent))),
        Line::from(Span::styled("  每日推荐", Style::new().fg(theme.dim))),
        Line::from(Span::styled("  私人FM", Style::new().fg(theme.dim))),
        Line::from(Span::styled("  云盘", Style::new().fg(theme.dim))),
    ];
    frame.render_widget(Paragraph::new(lines), area);
}

fn draw_list(frame: &mut Frame, state: &AppState, area: Rect) {
    let theme = &state.theme;
    let mut lines = Vec::with_capacity(state.library.len() + 1);
    lines.push(Line::from(Span::styled(
        format!("  {:>3}  {:<24} {:<14} {:>5}", "#", "歌名", "歌手", "时长"),
        Style::new().fg(theme.faint),
    )));
    for (index, row) in state.library.iter().enumerate() {
        let selected = index == state.selected;
        let style = if selected {
            Style::new().fg(theme.bg).bg(theme.sel)
        } else {
            Style::new().fg(theme.fg)
        };
        lines.push(Line::from(Span::styled(
            format!(
                "  {:>3}  {:<24} {:<14} {:>5}",
                index + 1,
                row.title,
                row.artist,
                row.duration
            ),
            style,
        )));
    }
    frame.render_widget(Paragraph::new(lines), area);
}
