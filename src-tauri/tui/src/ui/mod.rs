//! Pure view layer: reads AppState, writes the frame. No side effects.

mod library;
mod now_playing;

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;

use crate::action::View;
use crate::app::AppState;

pub fn draw(frame: &mut Frame, state: &AppState) {
    let theme = &state.theme;
    let area = frame.area();
    frame.render_widget(Block::new().style(Style::new().bg(theme.bg)), area);

    if state.zen {
        now_playing::draw(frame, state, area);
        return;
    }

    let [tabs_area, body, hints_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(area);

    draw_tabs(frame, state, tabs_area);
    match state.view {
        View::NowPlaying => now_playing::draw(frame, state, body),
        View::Library => library::draw(frame, state, body),
        View::Search => placeholder(frame, state, body, "搜索（NCM 服务阶段接入）"),
        View::Queue => placeholder(frame, state, body, "播放队列（NCM 服务阶段接入）"),
    }
    draw_hints(frame, state, hints_area);
}

fn draw_tabs(frame: &mut Frame, state: &AppState, area: Rect) {
    let theme = &state.theme;
    let tab = |label: &str, view: View| {
        let style = if state.view == view {
            Style::new().fg(theme.accent)
        } else {
            Style::new().fg(theme.dim)
        };
        Span::styled(format!("[{label}] "), style)
    };
    let line = Line::from(vec![
        tab("1 正在播放", View::NowPlaying),
        tab("2 曲库", View::Library),
        tab("3 搜索", View::Search),
        tab("4 队列", View::Queue),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

fn draw_hints(frame: &mut Frame, state: &AppState, area: Rect) {
    let theme = &state.theme;
    let hints: &[(&str, &str)] = match state.view {
        View::Library => &[
            ("Enter", "播放"),
            ("j/k", "选择"),
            ("z", "纯净"),
            ("q", "退出"),
        ],
        _ => &[
            ("Space", "暂停"),
            ("←/→", "seek"),
            ("-/+", "音量"),
            ("z", "纯净"),
            ("q", "退出"),
        ],
    };
    let mut spans = Vec::new();
    for (key, label) in hints {
        spans.push(Span::styled(*key, Style::new().fg(theme.fg)));
        spans.push(Span::styled(
            format!(" {label}   "),
            Style::new().fg(theme.dim),
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn placeholder(frame: &mut Frame, state: &AppState, area: Rect, text: &str) {
    let line = Line::from(Span::styled(
        text.to_owned(),
        Style::new().fg(state.theme.dim),
    ));
    let [_, middle, _] = Layout::vertical([
        Constraint::Percentage(45),
        Constraint::Length(1),
        Constraint::Min(0),
    ])
    .areas(area);
    frame.render_widget(Paragraph::new(line).centered(), middle);
}

pub fn format_duration(duration: std::time::Duration) -> String {
    let total = duration.as_secs();
    format!("{:02}:{:02}", total / 60, total % 60)
}
