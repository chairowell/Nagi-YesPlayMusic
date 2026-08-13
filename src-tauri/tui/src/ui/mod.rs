//! Pure view layer: reads AppState, writes the frame. No side effects.

mod cover_preview;
pub(crate) mod library;
mod login;
mod now_playing;
mod queue;
mod search;
mod settings;
pub(crate) mod text;

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
    pub sidebar: Vec<(Rect, usize)>,
    pub heart: Vec<(Rect, ())>,
    pub play: Vec<(Rect, ())>,
    pub playback_mode: Vec<(Rect, ())>,
    pub volume: Vec<(Rect, ())>,
    /// Quit-confirm buttons: true = 确定退出, false = 点错了.
    pub confirm: Vec<(Rect, bool)>,
    pub settings_rows: Vec<(Rect, usize)>,
    pub settings_adjust: Vec<(Rect, i32)>,
    pub settings_save: Vec<Rect>,
    pub settings_cancel: Vec<Rect>,
}

pub fn draw(frame: &mut Frame, state: &mut AppState, hits: &mut Hits) {
    hits.tabs.clear();
    hits.rows.clear();
    hits.menu.clear();
    hits.sidebar.clear();
    hits.heart.clear();
    hits.play.clear();
    hits.playback_mode.clear();
    hits.volume.clear();
    hits.confirm.clear();
    hits.settings_rows.clear();
    hits.settings_adjust.clear();
    hits.settings_save.clear();
    hits.settings_cancel.clear();

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
            View::Search => search::draw(frame, state, body, hits),
            View::Queue => queue::draw(frame, state, body, hits),
            View::Login => login::draw(frame, state, body),
            View::Settings => settings::draw(frame, state, body, hits),
        }
        draw_hints(frame, state, hints_area);
    }

    if state.show_help {
        draw_help(frame, state, area);
    }
    if state.confirm_quit {
        draw_quit_confirm(frame, state, area, hits);
    }
}

fn draw_help(frame: &mut Frame, state: &AppState, area: Rect) {
    let theme = &state.theme;
    let rows: Vec<(&str, Key)> = vec![
        ("1-5", Key::NowPlaying),
        ("j/k ↑/↓", Key::Select),
        ("PgUp/PgDn", Key::Page),
        ("Home/End · gg/G", Key::TopBottom),
        ("l/Enter", Key::Play),
        ("a", Key::AddToQueue),
        ("h/Esc", Key::Back),
        ("Tab", Key::LibraryFocus),
        ("Space", Key::Pause),
        ("n / p", Key::ChangeTrack),
        ("←/→ 5s · Shift+←/→ 30s", Key::Seek),
        ("- / +", Key::Volume),
        ("m", Key::Mute),
        ("s", Key::Shuffle),
        ("r", Key::Repeat),
        ("*", Key::LabelLike),
        ("/", Key::Filter),
        ("z", Key::Zen),
        ("?", Key::LabelHelp),
        (",", Key::Settings),
        ("q", Key::Quit),
    ];
    let height = (rows.len() as u16 + 3).min(area.height);
    let width = 72_u16.min(area.width);
    let modal = Rect {
        x: area.x + (area.width.saturating_sub(width)) / 2,
        y: area.y + (area.height.saturating_sub(height)) / 2,
        width,
        height,
    };
    frame.render_widget(ratatui::widgets::Clear, modal);
    let block = Block::bordered()
        .style(Style::new().bg(theme.bg))
        .border_style(Style::new().fg(theme.accent))
        .title(format!(" {} ", i18n::t(Key::HelpTitle)));
    let inner = block.inner(modal);
    frame.render_widget(block, modal);

    let mut lines = Vec::new();
    for (keys, label) in rows {
        lines.push(Line::from(vec![
            Span::styled(format!("  {keys:<28}"), Style::new().fg(theme.accent)),
            Span::styled(i18n::t(label), Style::new().fg(theme.fg)),
        ]));
    }
    lines.push(Line::from(Span::styled(
        format!("  {}", i18n::t(Key::HelpAnyKey)),
        Style::new().fg(theme.faint),
    )));
    frame.render_widget(Paragraph::new(lines), inner);
}

const TABS: [(&str, Key, View); 5] = [
    ("1", Key::NowPlaying, View::NowPlaying),
    ("2", Key::Library, View::Library),
    ("3", Key::Search, View::Search),
    ("4", Key::Queue, View::Queue),
    ("5", Key::Settings, View::Settings),
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
            Style::new().fg(theme.selection_fg()).bg(theme.accent),
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
    if state.filter.is_active() {
        let cursor = if state.filter.input { "▎" } else { "" };
        let mut spans = vec![
            Span::styled("/ ", Style::new().fg(theme.accent)),
            Span::styled(
                format!("{}{cursor}", state.filter.query),
                Style::new().fg(theme.fg),
            ),
        ];
        if state.filter.input {
            spans.push(Span::styled(
                format!("   Enter {}", i18n::t(Key::FinishFilter)),
                Style::new().fg(theme.dim),
            ));
        } else {
            spans.push(Span::styled(
                format!(
                    "   Enter {}   a {}",
                    i18n::t(Key::Play),
                    i18n::t(Key::AddToQueue)
                ),
                Style::new().fg(theme.dim),
            ));
        }
        spans.push(Span::styled(
            format!("   Esc {}", i18n::t(Key::ClearFilter)),
            Style::new().fg(theme.dim),
        ));
        frame.render_widget(Paragraph::new(Line::from(spans)), area);
        return;
    }
    let hints: &[(&str, Key)] = match state.view {
        View::Library => &[
            ("l/Enter", Key::Play),
            ("a", Key::AddToQueue),
            ("Tab", Key::LibraryFocus),
            ("/", Key::Filter),
            ("j/k", Key::Select),
            ("PgUp/PgDn", Key::Page),
            ("h", Key::Back),
        ],
        View::Queue => &[
            ("l/Enter", Key::JumpToTrack),
            ("a", Key::AddToQueue),
            ("/", Key::Filter),
            ("j/k", Key::Select),
            ("PgUp/PgDn", Key::Page),
            ("h", Key::Back),
        ],
        View::Login => &[("h/Esc", Key::Back), ("q", Key::Quit)],
        View::Search if state.search.input => &[
            ("Enter", Key::Search),
            ("Tab/↓", Key::Select),
            ("Esc", Key::Back),
        ],
        View::Search => &[
            ("l/Enter", Key::Play),
            ("a", Key::AddToQueue),
            ("/", Key::Filter),
            ("PgUp/PgDn", Key::Page),
            ("h", Key::Back),
        ],
        View::Settings => &[
            ("j/k", Key::Select),
            ("h/l", Key::SettingAdjust),
            ("Enter", Key::Save),
            ("Esc", Key::Cancel),
        ],
        _ => &[
            ("Space", Key::Pause),
            ("←/→ 5s · ⇧ 30s", Key::Seek),
            ("-/+", Key::Volume),
            ("m", Key::Mute),
            ("s", Key::Shuffle),
            ("r", Key::Repeat),
            ("*", Key::LabelLike),
            ("3", Key::Search),
            ("n/p", Key::ChangeTrack),
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

#[cfg(test)]
mod tests {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    use super::draw_help;
    use crate::app::AppState;
    use crate::config::Config;

    #[test]
    fn help_overlay_renders_the_refactored_key_map() {
        let state = AppState::new(&Config::default());
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| draw_help(frame, &state, frame.area()))
            .unwrap();

        let buffer = terminal.backend().buffer();
        let rendered = (0..24)
            .map(|y| (0..80).map(|x| buffer[(x, y)].symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n");
        for keys in [
            "PgUp/PgDn",
            "Home/End · gg/G",
            "a",
            "Tab",
            "←/→ 5s · Shift+←/→ 30s",
            "m",
            "s",
            "r",
            "*",
            "/",
        ] {
            assert!(rendered.contains(&format!("  {keys:<28}")), "{keys}");
        }
    }
}
