//! Pure view layer: reads AppState, writes the frame. No side effects.

mod library;
mod login;
mod now_playing;
mod queue;
mod text;

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;

use crate::action::View;
use crate::app::AppState;
use crate::i18n::{self, Key};

/// Geometry recorded at draw time so mouse events can be resolved
/// against what is actually on screen.
#[derive(Default)]
pub struct Hits {
    pub tabs: Vec<(Rect, View)>,
    pub rows: Vec<(Rect, usize)>,
    pub menu: Vec<(Rect, crate::action::MenuEntry)>,
    /// Quit-confirm buttons: true = 确定退出, false = 点错了.
    pub confirm: Vec<(Rect, bool)>,
}

pub fn draw(frame: &mut Frame, state: &AppState, hits: &mut Hits) {
    hits.tabs.clear();
    hits.rows.clear();
    hits.menu.clear();
    hits.confirm.clear();

    let theme = &state.theme;
    let area = frame.area();
    frame.render_widget(Block::new().style(Style::new().bg(theme.bg)), area);

    if state.zen {
        now_playing::draw(frame, state, area, hits);
    } else {
        let [tabs_area, body, hints_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .areas(area);

        draw_tabs(frame, state, tabs_area, hits);
        match state.view {
            View::NowPlaying => now_playing::draw(frame, state, body, hits),
            View::Library => library::draw(frame, state, body, hits),
            View::Search => placeholder(frame, state, body, Key::SearchPlaceholder),
            View::Queue => queue::draw(frame, state, body, hits),
            View::Login => login::draw(frame, state, body),
        }
        draw_hints(frame, state, hints_area);
    }

    if state.confirm_quit {
        draw_quit_confirm(frame, state, area, hits);
    }
}

const TABS: [(&str, Key, View); 4] = [
    ("1", Key::NowPlaying, View::NowPlaying),
    ("2", Key::Library, View::Library),
    ("3", Key::Search, View::Search),
    ("4", Key::Queue, View::Queue),
];

fn draw_tabs(frame: &mut Frame, state: &AppState, area: Rect, hits: &mut Hits) {
    let theme = &state.theme;
    let mut spans = Vec::new();
    let mut x = area.x;
    for (number, label, view) in TABS {
        let text = format!("[{number} {}] ", i18n::t(label));
        let width = text::display_width(&text) as u16;
        let style = if state.view == view {
            Style::new().fg(theme.accent)
        } else {
            Style::new().fg(theme.dim)
        };
        hits.tabs.push((
            Rect {
                x,
                y: area.y,
                width,
                height: 1,
            },
            view,
        ));
        x = x.saturating_add(width);
        spans.push(Span::styled(text, style));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_quit_confirm(frame: &mut Frame, state: &AppState, area: Rect, hits: &mut Hits) {
    let theme = &state.theme;
    let width = 34_u16.min(area.width);
    let height = 5_u16.min(area.height);
    let modal = Rect {
        x: area.x + (area.width.saturating_sub(width)) / 2,
        y: area.y + (area.height.saturating_sub(height)) / 2,
        width,
        height,
    };
    frame.render_widget(ratatui::widgets::Clear, modal);
    let block = Block::bordered()
        .style(Style::new().bg(theme.bg))
        .border_style(Style::new().fg(theme.accent));
    let inner = block.inner(modal);
    frame.render_widget(block, modal);

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            i18n::t(Key::QuitQuestion),
            Style::new().fg(theme.fg),
        )))
        .centered(),
        Rect { height: 1, ..inner },
    );

    let confirm_label = format!("[ y {} ]", i18n::t(Key::Quit));
    let cancel_label = format!("[ n {} ]", i18n::t(Key::Cancel));
    let gap = 3_u16;
    let confirm_width = text::display_width(&confirm_label) as u16;
    let cancel_width = text::display_width(&cancel_label) as u16;
    let total = confirm_width + gap + cancel_width;
    let buttons_y = inner.y + inner.height.saturating_sub(1);
    let start_x = inner.x + (inner.width.saturating_sub(total)) / 2;

    let confirm_rect = Rect {
        x: start_x,
        y: buttons_y,
        width: confirm_width,
        height: 1,
    };
    let cancel_rect = Rect {
        x: start_x + confirm_width + gap,
        y: buttons_y,
        width: cancel_width,
        height: 1,
    };
    hits.confirm.push((confirm_rect, true));
    hits.confirm.push((cancel_rect, false));

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            confirm_label,
            Style::new().fg(theme.bg).bg(theme.accent),
        ))),
        confirm_rect,
    );
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            cancel_label,
            Style::new().fg(theme.fg),
        ))),
        cancel_rect,
    );
}

fn draw_hints(frame: &mut Frame, state: &AppState, area: Rect) {
    let theme = &state.theme;
    let hints: &[(&str, Key)] = match state.view {
        View::Library => &[
            ("l/Enter", Key::Play),
            ("j/k", Key::Select),
            ("gg/G", Key::TopBottom),
            ("h", Key::Back),
            ("q", Key::Quit),
        ],
        View::Queue => &[
            ("l/Enter", Key::JumpToTrack),
            ("j/k", Key::Select),
            ("n/p", Key::ChangeTrack),
            ("h", Key::Back),
        ],
        View::Login => &[
            ("h/Esc", Key::Back),
            ("i", Key::RefreshQr),
            ("q", Key::Quit),
        ],
        _ => &[
            ("Space", Key::Pause),
            ("←/→", Key::Seek),
            ("-/+", Key::Volume),
            ("n/p", Key::ChangeTrack),
            ("z", Key::Zen),
            ("q", Key::Quit),
        ],
    };
    let mut spans = Vec::new();
    for (key, label) in hints {
        spans.push(Span::styled(*key, Style::new().fg(theme.fg)));
        spans.push(Span::styled(
            format!(" {}   ", i18n::t(*label)),
            Style::new().fg(theme.dim),
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn placeholder(frame: &mut Frame, state: &AppState, area: Rect, key: Key) {
    let line = Line::from(Span::styled(i18n::t(key), Style::new().fg(state.theme.dim)));
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

/// First visible index for a list viewport that keeps the selection
/// centered where possible.
pub fn scroll_offset(selected: usize, len: usize, visible: usize) -> usize {
    if visible == 0 || len <= visible {
        return 0;
    }
    selected.saturating_sub(visible / 2).min(len - visible)
}

pub fn format_ms(ms: i64) -> String {
    if ms <= 0 {
        return "--:--".into();
    }
    format_duration(std::time::Duration::from_millis(ms as u64))
}
