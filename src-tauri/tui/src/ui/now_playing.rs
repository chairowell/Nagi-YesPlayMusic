//! The main view has two moods: an idle dashboard (pixel art + menu,
//! dashboard-nvim style) and the playing layout (cover · lyrics · progress).

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::action::MenuEntry;
use crate::app::AppState;
use crate::i18n::{self, Key};
use crate::ui::text::display_width;
use crate::ui::{format_duration, Hits};

// Cover art travels as a 26×13 cell grid (each cell is two vertical pixels,
// so the pixel grid is square); the frame must hug exactly that grid or the
// border reads as a stretched rectangle.
const COVER_GRID: (u16, u16) = (26, 13);

pub fn draw(frame: &mut Frame, state: &mut AppState, area: Rect, hits: &mut Hits) {
    if state.now.is_none() {
        draw_dashboard(frame, state, area, hits);
        return;
    }

    let [main, progress_area] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(2)]).areas(area);
    let progress_hits = progress_area;
    let (cover_w, cover_h) = state
        .cover
        .as_ref()
        .or(state.placeholder.as_ref())
        .map(|art| (art.width, art.height))
        .unwrap_or(COVER_GRID);

    match state.layout {
        crate::app::PlayLayout::Side => {
            // Borderless pixel art + one column of breathing room.
            let [cover_column, _, meta_area] = Layout::horizontal([
                Constraint::Length(cover_w.min(main.width)),
                Constraint::Length(2),
                Constraint::Min(0),
            ])
            .areas(main);
            let cover_area = Rect {
                x: cover_column.x,
                y: cover_column.y,
                width: cover_column.width,
                height: cover_h.min(main.height),
            };
            draw_cover(frame, state, cover_area);
            draw_meta(frame, state, meta_area, false);
        }
        crate::app::PlayLayout::Stacked => {
            let art_height = cover_h.min(main.height.saturating_sub(4));
            let [cover_row, _, meta_area] = Layout::vertical([
                Constraint::Length(art_height),
                Constraint::Length(1),
                Constraint::Min(0),
            ])
            .areas(main);
            let cover_area = centered(cover_row, cover_w, art_height);
            draw_cover(frame, state, cover_area);
            draw_meta(frame, state, meta_area, true);
        }
    }
    draw_progress(frame, state, progress_hits, hits);
}

// ── idle dashboard ──────────────────────────────────────────────────

fn draw_dashboard(frame: &mut Frame, state: &AppState, area: Rect, hits: &mut Hits) {
    let theme = &state.theme;
    let menu = menu_entries(state);
    let menu_height = menu.len() as u16;
    let art_height = state
        .idle_art
        .height
        .min(area.height.saturating_sub(menu_height + 4));

    let [_, art_area, _, menu_area, _, footer_area, _] = Layout::vertical([
        Constraint::Fill(2),
        Constraint::Length(art_height),
        Constraint::Length(1),
        Constraint::Length(menu_height),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Fill(3),
    ])
    .areas(area);

    let art_rect = centered(art_area, state.idle_art.width, art_height);
    frame.render_widget(&state.idle_art, art_rect);

    const MENU_WIDTH: u16 = 30;
    for (index, (label, key, entry)) in menu.iter().enumerate() {
        let row = Rect {
            x: area.x + (area.width.saturating_sub(MENU_WIDTH)) / 2,
            y: menu_area.y + index as u16,
            width: MENU_WIDTH.min(area.width),
            height: 1,
        };
        hits.menu.push((row, *entry));
        let pad =
            (MENU_WIDTH as usize).saturating_sub(display_width(label) + display_width(key) + 2);
        let line = Line::from(vec![
            Span::styled(
                format!(" {label}{}", " ".repeat(pad)),
                Style::new().fg(theme.fg),
            ),
            Span::styled((*key).to_owned(), Style::new().fg(theme.accent)),
            Span::raw(" "),
        ]);
        frame.render_widget(Paragraph::new(line), row);
    }

    let footer = match (&state.session.nickname, state.library.len()) {
        (Some(nickname), n) if state.library_synced => {
            format!("♪ {nickname} · {}", i18n::t_songs_ready(n))
        }
        (Some(nickname), _) => {
            format!("♪ {nickname} · {}", i18n::t(Key::SyncingLibrary))
        }
        (None, _) => i18n::t(Key::NotLoggedInSync).into(),
    };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            footer,
            Style::new().fg(theme.faint),
        )))
        .centered(),
        footer_area,
    );
}

fn menu_entries(state: &AppState) -> Vec<(String, &'static str, MenuEntry)> {
    let mut entries = vec![(i18n::t(Key::LikedSongs).to_owned(), "2", MenuEntry::Library)];
    entries.push((i18n::t(Key::Search).to_owned(), "3", MenuEntry::Search));
    entries.push(match &state.session.nickname {
        Some(_) => (i18n::t(Key::Relogin).to_owned(), "", MenuEntry::Login),
        None => (i18n::t(Key::ScanLogin).to_owned(), "", MenuEntry::Login),
    });
    entries.push((i18n::t(Key::Settings).to_owned(), ",", MenuEntry::Settings));
    entries.push((i18n::t(Key::Quit).to_owned(), "q", MenuEntry::Quit));
    entries
}

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    Rect {
        x: area.x + (area.width.saturating_sub(width)) / 2,
        y: area.y,
        width: width.min(area.width),
        height: height.min(area.height),
    }
}

// ── playing layout ──────────────────────────────────────────────────

fn draw_cover(frame: &mut Frame, state: &mut AppState, area: Rect) {
    if area.height == 0 {
        return;
    }
    if state.original_cover_is_current() {
        frame.render_widget(
            ratatui::widgets::Block::new().style(Style::new().bg(state.theme.bg)),
            area,
        );
        state.render_original_cover(frame, area);
        return;
    }
    match (&state.cover, &state.placeholder) {
        (Some(cover), _) => frame.render_widget(cover, area),
        // Cover still loading: idle art pre-rendered at cover size, so
        // the swap keeps position and scale.
        (None, Some(placeholder)) => frame.render_widget(placeholder, area),
        (None, None) => frame.render_widget(&state.idle_art, area),
    }
}

fn draw_meta(frame: &mut Frame, state: &AppState, area: Rect, centered_text: bool) {
    let theme = &state.theme;
    let indent = if centered_text { "" } else { "  " };
    let mut lines = Vec::new();
    if centered_text {
        lines.push(Line::default());
    }
    if let Some(now) = &state.now {
        lines.push(Line::from(Span::styled(
            format!("{indent}{}", now.title),
            Style::new().fg(theme.accent).add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(Span::styled(
            format!("{indent}{} ", now.artist),
            Style::new().fg(theme.dim),
        )));
        if !now.album.is_empty() {
            lines.push(Line::from(Span::styled(
                format!("{indent}{}", now.album),
                Style::new().fg(theme.faint),
            )));
        }
        if !state.lyrics.is_empty() {
            lines.push(Line::default());
            let reserved = lines.len() as u16;
            lines.extend(lyric_window(state, area.height.saturating_sub(reserved)));
        } else if let Some(status) = &state.status {
            lines.push(Line::default());
            lines.push(Line::from(Span::styled(
                format!("{indent}{status}"),
                Style::new().fg(theme.accent2),
            )));
        }
    }
    let paragraph = if centered_text {
        Paragraph::new(lines).centered()
    } else {
        Paragraph::new(lines)
    };
    frame.render_widget(paragraph, area);
}

/// Rows of synced lyrics with the current pair pinned mid-window.
fn lyric_window(state: &AppState, height: u16) -> Vec<Line<'static>> {
    let theme = &state.theme;
    let current = crate::lyrics::line_index_at(&state.lyrics, state.position);
    let rows = height.max(1) as usize;
    let anchor = current.unwrap_or(0);
    // Walk backwards in *display rows* (a translated line costs two),
    // so the anchor really sits mid-window instead of drifting bottom.
    let target_above = rows / 2;
    let mut start = anchor;
    let mut used_above = 0_usize;
    while start > 0 {
        let previous = start - 1;
        let cost = 1 + state.lyrics[previous]
            .translation
            .as_ref()
            .is_some_and(|text| !text.is_empty()) as usize;
        if used_above + cost > target_above {
            break;
        }
        used_above += cost;
        start = previous;
    }

    let mut lines = Vec::new();
    let mut used = 0_usize;
    for (index, lyric) in state.lyrics.iter().enumerate().skip(start) {
        if used >= rows {
            break;
        }
        let is_current = Some(index) == current;
        let original_style = if is_current {
            Style::new().fg(theme.fg).add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(theme.dim)
        };
        let marker_style = if is_current {
            Style::new().fg(theme.accent)
        } else {
            original_style
        };
        lines.push(Line::from(vec![
            Span::styled(if is_current { "▎ " } else { "  " }, marker_style),
            Span::styled(lyric.text.clone(), original_style),
        ]));
        used += 1;
        // Every line carries its translation; the current pair reads brighter.
        if let Some(translation) = &lyric.translation {
            if used < rows && !translation.is_empty() {
                let translation_style = if is_current {
                    Style::new().fg(theme.fg)
                } else {
                    Style::new().fg(theme.faint)
                }
                .add_modifier(Modifier::ITALIC);
                let marker_style = if is_current {
                    Style::new().fg(theme.accent)
                } else {
                    translation_style
                };
                lines.push(Line::from(vec![
                    Span::styled(if is_current { "▎ " } else { "  " }, marker_style),
                    Span::styled(translation.clone(), translation_style),
                ]));
                used += 1;
            }
        }
    }
    lines
}

fn draw_progress(frame: &mut Frame, state: &AppState, area: Rect, hits: &mut Hits) {
    let theme = &state.theme;
    let icons = crate::icons::for_style(state.config.icons);
    let play_icon = if state.paused {
        icons.play
    } else {
        icons.pause
    };
    let mode = crate::app::PlaybackModeSlot::from_parts(state.shuffle, state.play_mode);
    let mode_icon = match mode {
        crate::app::PlaybackModeSlot::Sequential => icons.sequential,
        crate::app::PlaybackModeSlot::RepeatList => icons.repeat_list,
        crate::app::PlaybackModeSlot::RepeatOne => icons.repeat_one,
        crate::app::PlaybackModeSlot::Shuffle => icons.shuffle,
    };
    let liked = state
        .current_track_id
        .map(|id| state.liked.contains(&id))
        .unwrap_or(false);
    let elapsed = format_duration(state.position);
    let total = state
        .duration
        .map(format_duration)
        .unwrap_or_else(|| "--:--".into());

    let play_slot_width = display_width(icons.play).max(display_width(icons.pause));
    let heart_slot_width = display_width(icons.heart);
    let mode_slot_width = [
        icons.sequential,
        icons.repeat_list,
        icons.repeat_one,
        icons.shuffle,
    ]
    .into_iter()
    .map(display_width)
    .max()
    .unwrap_or(1);
    const VOLUME_CELLS: usize = 10;
    let fixed = play_slot_width
        + display_width(&elapsed)
        + display_width(&total)
        + heart_slot_width
        + mode_slot_width
        + display_width(icons.volume)
        + VOLUME_CELLS
        + 21;
    let bar_width = (area.width as usize).saturating_sub(fixed).max(8);
    let ratio = match state.duration {
        Some(duration) if !duration.is_zero() => {
            (state.position.as_secs_f64() / duration.as_secs_f64()).clamp(0.0, 1.0)
        }
        _ => 0.0,
    };
    let head = ((bar_width as f64 - 1.0) * ratio).round() as usize;

    let bar_spans = if state.thick_progress {
        let filled = ((bar_width as f64) * ratio).round() as usize;
        vec![
            Span::styled("█".repeat(filled), Style::new().fg(theme.accent)),
            Span::styled(
                "▁".repeat(bar_width.saturating_sub(filled)),
                Style::new().fg(theme.faint),
            ),
        ]
    } else {
        vec![
            Span::styled("━".repeat(head), Style::new().fg(theme.fg)),
            Span::styled("●", Style::new().fg(theme.accent)),
            Span::styled(
                "─".repeat(bar_width.saturating_sub(head + 1)),
                Style::new().fg(theme.faint),
            ),
        ]
    };
    hits.play.push((
        Rect {
            x: area.x + 1,
            y: area.y,
            width: display_width(play_icon) as u16,
            height: 1,
        },
        (),
    ));
    let mut spans = vec![
        Span::raw(" "),
        Span::styled(play_icon, Style::new().fg(theme.fg)),
        Span::raw(" ".repeat(play_slot_width.saturating_sub(display_width(play_icon)) + 1)),
        Span::styled(elapsed, Style::new().fg(theme.dim)),
        Span::raw(" "),
    ];
    spans.extend(bar_spans);
    spans.push(Span::raw(" "));
    spans.push(Span::styled(total, Style::new().fg(theme.dim)));
    // Clickable heart: its cell position is the rendered width so far + 2.
    let heart_x = spans
        .iter()
        .map(|span| display_width(&span.content) as u16)
        .sum::<u16>()
        + area.x
        + 2;
    hits.heart.push((
        Rect {
            x: heart_x,
            y: area.y,
            width: display_width(icons.heart) as u16,
            height: 1,
        },
        (),
    ));
    spans.push(Span::raw("  "));
    spans.push(Span::styled(
        icons.heart,
        Style::new().fg(if liked { theme.accent2 } else { theme.faint }),
    ));
    spans.push(Span::raw("  "));
    let mode_x = area.x
        + spans
            .iter()
            .map(|span| display_width(&span.content) as u16)
            .sum::<u16>();
    hits.playback_mode.push((
        Rect {
            x: mode_x,
            y: area.y,
            width: display_width(mode_icon) as u16,
            height: 1,
        },
        (),
    ));
    spans.push(Span::styled(
        mode_icon,
        Style::new().fg(if mode == crate::app::PlaybackModeSlot::Sequential {
            theme.faint
        } else {
            theme.accent
        }),
    ));
    spans.push(Span::raw(
        " ".repeat(mode_slot_width.saturating_sub(display_width(mode_icon))),
    ));
    // Click or drag across the dot meter to set the volume.
    let filled = (state.volume.clamp(0.0, 1.0) * VOLUME_CELLS as f32).round() as usize;
    spans.push(Span::raw("  "));
    spans.push(Span::styled(icons.volume, Style::new().fg(theme.faint)));
    spans.push(Span::raw(" "));
    let volume_x = spans
        .iter()
        .map(|span| display_width(&span.content) as u16)
        .sum::<u16>()
        + area.x;
    hits.volume.push((
        Rect {
            x: volume_x,
            y: area.y,
            width: VOLUME_CELLS as u16,
            height: 1,
        },
        (),
    ));
    spans.push(Span::styled(
        icons.volume_full.repeat(filled),
        Style::new().fg(theme.dim),
    ));
    spans.push(Span::styled(
        icons.volume_empty.repeat(VOLUME_CELLS - filled),
        Style::new().fg(theme.faint),
    ));
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use ratatui::style::Modifier;
    use ratatui::Terminal;

    use super::{draw_meta, draw_progress, lyric_window, menu_entries};
    use crate::action::{Action, MenuEntry};
    use crate::app::{AppState, NowPlaying, PlayMode};
    use crate::config::Config;
    use crate::event;
    use crate::lyrics::LyricLine;
    use crate::ui::Hits;

    fn rect_text(buffer: &Buffer, rect: Rect) -> String {
        (rect.x..rect.right())
            .map(|x| buffer[(x, rect.y)].symbol())
            .collect()
    }

    fn click(rect: Rect) -> MouseEvent {
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: rect.x,
            row: rect.y,
            modifiers: KeyModifiers::NONE,
        }
    }

    #[test]
    fn lyrics_use_four_distinct_original_and_translation_styles() {
        let mut state = AppState::new(&Config::default());
        state.position = Duration::from_secs(1);
        state.lyrics = vec![
            LyricLine {
                time: Duration::ZERO,
                text: "Current".into(),
                translation: Some("当前翻译".into()),
            },
            LyricLine {
                time: Duration::from_secs(10),
                text: "Context".into(),
                translation: Some("上下文翻译".into()),
            },
        ];

        let lines = lyric_window(&state, 4);
        assert_eq!(lines[0].spans[0].content, "▎ ");
        assert_eq!(lines[0].spans[0].style.fg, Some(state.theme.accent));
        assert_eq!(lines[0].spans[1].style.fg, Some(state.theme.fg));
        assert!(lines[0].spans[1]
            .style
            .add_modifier
            .contains(Modifier::BOLD));

        assert_eq!(lines[1].spans[0].content, "▎ ");
        assert_eq!(lines[1].spans[0].style.fg, Some(state.theme.accent));
        assert_eq!(lines[1].spans[1].style.fg, Some(state.theme.fg));
        assert!(lines[1].spans[1]
            .style
            .add_modifier
            .contains(Modifier::ITALIC));

        assert_eq!(lines[2].spans[1].style.fg, Some(state.theme.dim));
        assert!(!lines[2].spans[1]
            .style
            .add_modifier
            .contains(Modifier::ITALIC));
        assert_eq!(lines[3].spans[1].style.fg, Some(state.theme.faint));
        assert!(lines[3].spans[1]
            .style
            .add_modifier
            .contains(Modifier::ITALIC));
    }

    #[test]
    fn metadata_and_liked_progress_render_the_new_visual_hierarchy() {
        let mut state = AppState::new(&Config::default());
        state.now = Some(NowPlaying {
            title: "Title".into(),
            artist: "Artist".into(),
            album: "Album".into(),
        });
        state.current_track_id = Some(42);
        state.liked.insert(42);
        let backend = TestBackend::new(80, 8);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut hits = Hits::default();

        terminal
            .draw(|frame| {
                draw_meta(frame, &state, Rect::new(0, 0, 80, 5), false);
                draw_progress(frame, &state, Rect::new(0, 6, 80, 1), &mut hits);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        assert_eq!(buffer[(2, 0)].fg, state.theme.accent);
        assert!(buffer[(2, 0)].modifier.contains(Modifier::BOLD));
        assert_eq!(buffer[(2, 1)].fg, state.theme.dim);
        assert_eq!(buffer[(2, 2)].fg, state.theme.faint);
        let progress = (0..80).map(|x| buffer[(x, 6)].symbol()).collect::<String>();
        assert!(progress.contains('♥'));
        assert!(!progress.contains('♡'));
    }

    #[test]
    fn progress_heart_uses_color_as_the_only_liked_state_signal() {
        for liked in [false, true] {
            let mut state = AppState::new(&Config::default());
            state.current_track_id = Some(42);
            if liked {
                state.liked.insert(42);
            }
            let backend = TestBackend::new(100, 1);
            let mut terminal = Terminal::new(backend).unwrap();
            let mut hits = Hits::default();

            terminal
                .draw(|frame| draw_progress(frame, &state, frame.area(), &mut hits))
                .unwrap();

            let rect = hits.heart[0].0;
            let buffer = terminal.backend().buffer();
            assert_eq!(rect_text(buffer, rect), "♥");
            assert_eq!(
                buffer[(rect.x, rect.y)].fg,
                if liked {
                    state.theme.accent2
                } else {
                    state.theme.faint
                }
            );
        }
    }

    #[test]
    fn progress_play_button_shows_the_action_and_uses_its_drawn_hit_target() {
        for (paused, expected) in [(true, "▶"), (false, "⏸")] {
            let mut state = AppState::new(&Config::default());
            state.paused = paused;
            let backend = TestBackend::new(100, 1);
            let mut terminal = Terminal::new(backend).unwrap();
            let mut hits = Hits::default();

            terminal
                .draw(|frame| draw_progress(frame, &state, frame.area(), &mut hits))
                .unwrap();

            let rect = hits.play[0].0;
            assert_eq!(rect_text(terminal.backend().buffer(), rect), expected);
            assert!(matches!(
                event::mouse_action(click(rect), &hits, 0),
                Some(Action::TogglePlay)
            ));
        }
    }

    #[test]
    fn progress_projects_repeat_and_shuffle_into_one_clickable_slot() {
        let cases = [
            (false, PlayMode::Off, "→", false),
            (false, PlayMode::List, "↺", true),
            (false, PlayMode::One, "↺¹", true),
            (true, PlayMode::Off, "⇆", true),
            (true, PlayMode::List, "⇆", true),
            (true, PlayMode::One, "⇆", true),
        ];
        for (shuffle, repeat, expected, active) in cases {
            let mut state = AppState::new(&Config::default());
            state.shuffle = shuffle;
            state.play_mode = repeat;
            state.volume = 0.4;
            let backend = TestBackend::new(100, 1);
            let mut terminal = Terminal::new(backend).unwrap();
            let mut hits = Hits::default();

            terminal
                .draw(|frame| draw_progress(frame, &state, frame.area(), &mut hits))
                .unwrap();

            let buffer = terminal.backend().buffer();
            let rect = hits.playback_mode[0].0;
            assert_eq!(rect_text(buffer, rect), expected);
            assert_eq!(
                buffer[(rect.x, rect.y)].fg,
                if active {
                    state.theme.accent
                } else {
                    state.theme.faint
                }
            );
            let rendered = (0..100)
                .map(|x| buffer[(x, 0)].symbol())
                .collect::<String>();
            assert!(!rendered.contains('×'));
            assert!(rendered.contains("●●●●○○○○○○"));
            assert!(matches!(
                event::mouse_action(click(rect), &hits, 0),
                Some(Action::CyclePlaybackMode)
            ));
        }
    }

    #[test]
    fn nerd_setting_switches_the_progress_controls_to_the_nerd_table() {
        let config = Config {
            icons: crate::config::IconStyle::Nerd,
            ..Config::default()
        };
        let mut state = AppState::new(&config);
        state.paused = true;
        state.current_track_id = Some(42);
        state.liked.insert(42);
        state.shuffle = true;
        state.volume = 0.5;
        let icons = crate::icons::for_style(crate::config::IconStyle::Nerd);
        let backend = TestBackend::new(100, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut hits = Hits::default();

        terminal
            .draw(|frame| draw_progress(frame, &state, frame.area(), &mut hits))
            .unwrap();

        let buffer = terminal.backend().buffer();
        assert_eq!(rect_text(buffer, hits.play[0].0), icons.play);
        assert_eq!(rect_text(buffer, hits.heart[0].0), icons.heart);
        assert_eq!(rect_text(buffer, hits.playback_mode[0].0), icons.shuffle);
        let rendered = (0..100)
            .map(|x| buffer[(x, 0)].symbol())
            .collect::<String>();
        assert!(rendered.contains(icons.volume));
        assert!(rendered.contains(&icons.volume_full.repeat(5)));
        assert!(rendered.contains(&icons.volume_empty.repeat(5)));
    }

    #[test]
    fn dashboard_menu_only_advertises_shortcuts_that_still_exist() {
        let state = AppState::new(&Config::default());
        let entries = menu_entries(&state);

        assert_eq!(
            entries
                .iter()
                .find(|(_, _, entry)| *entry == MenuEntry::Search)
                .map(|(_, key, _)| *key),
            Some("3")
        );
        assert_eq!(
            entries
                .iter()
                .find(|(_, _, entry)| *entry == MenuEntry::Login)
                .map(|(_, key, _)| *key),
            Some("")
        );
    }
}
