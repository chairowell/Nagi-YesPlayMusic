//! Library view: collapsible sidebar + track list. Sidebar entries become
//! real NCM playlists in the service stage.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::AppState;
use crate::ui::Hits;

use super::text::pad_display;

const SIDEBAR_WIDTH: u16 = 16;
const COLLAPSE_BELOW: u16 = 50;

pub fn draw(frame: &mut Frame, state: &AppState, area: Rect, hits: &mut Hits) {
    if area.width >= COLLAPSE_BELOW {
        let [sidebar, list] =
            Layout::horizontal([Constraint::Length(SIDEBAR_WIDTH), Constraint::Min(0)])
                .areas(area);
        draw_sidebar(frame, state, sidebar);
        draw_list(frame, state, list, hits);
    } else {
        draw_list(frame, state, area, hits);
    }
}

fn draw_sidebar(frame: &mut Frame, state: &AppState, area: Rect) {
    let theme = &state.theme;
    let account = match &state.nickname {
        Some(nickname) => Line::from(Span::styled(
            format!("♪ {nickname}"),
            Style::new().fg(theme.accent2),
        )),
        None => Line::from(Span::styled("未登录 · 按 g", Style::new().fg(theme.accent2))),
    };
    let lines = vec![
        account,
        Line::default(),
        Line::from(Span::styled("▸ 我喜欢的音乐", Style::new().fg(theme.accent))),
        Line::from(Span::styled("  每日推荐", Style::new().fg(theme.dim))),
        Line::from(Span::styled("  私人FM", Style::new().fg(theme.dim))),
        Line::from(Span::styled("  云盘", Style::new().fg(theme.dim))),
    ];
    frame.render_widget(Paragraph::new(lines), area);
}

fn draw_list(frame: &mut Frame, state: &AppState, area: Rect, hits: &mut Hits) {
    let theme = &state.theme;
    let visible = area.height.saturating_sub(1) as usize; // header row
    let offset = super::scroll_offset(state.selected, state.library.len(), visible);

    let mut lines = Vec::with_capacity(visible + 1);
    lines.push(Line::from(Span::styled(
        format!(
            "  {:>3}  {} {} {:>5}",
            "#",
            pad_display("歌名", 24),
            pad_display("歌手", 14),
            "时长"
        ),
        Style::new().fg(theme.faint),
    )));
    for (index, row) in state.library.iter().enumerate().skip(offset).take(visible) {
        hits.rows.push((
            Rect {
                x: area.x,
                y: area.y + 1 + (index - offset) as u16,
                width: area.width,
                height: 1,
            },
            index,
        ));
        let selected = index == state.selected;
        let style = if selected {
            Style::new().fg(theme.bg).bg(theme.sel)
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
