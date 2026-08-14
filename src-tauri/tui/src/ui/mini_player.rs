//! The compact one-line player used by very short terminals.

use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::AppState;
use crate::i18n::{self, Key};
use crate::ui::text::{display_width, pad_or_marquee};
use crate::ui::{format_duration, Hits};

const VOLUME_CELLS: usize = 5;

pub(crate) fn marquee_needed(state: &AppState, width: u16) -> bool {
    let Some(now) = &state.now else {
        return false;
    };
    display_width(&format!("{} — {}", now.title, now.artist)) > title_width(state, width)
}

pub(super) fn draw(frame: &mut Frame, state: &AppState, area: Rect, hits: &mut Hits) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    if state.confirm_quit {
        draw_quit_confirm(frame, state, area, hits);
        return;
    }

    let theme = &state.theme;
    let icons = crate::icons::for_style(state.config.icons);
    let play_icon = if state.paused {
        icons.play
    } else {
        icons.pause
    };
    let title = state
        .now
        .as_ref()
        .map(|now| format!("{} — {}", now.title, now.artist))
        .unwrap_or_else(|| "—".into());
    let elapsed = format_duration(state.position);
    let total = state
        .duration
        .map(format_duration)
        .unwrap_or_else(|| "--:--".into());
    let time = format!("{elapsed}/{total}");
    let liked = state
        .current_track_id
        .is_some_and(|id| state.liked.contains(&id));

    // The title owns the flexible centre so controls never drift off-screen.
    let title_width = title_width(state, area.width);
    let title_text = pad_or_marquee(&title, title_width, true, state.marquee_frame());
    let play_x = area.x;
    let heart_x =
        area.x + (display_width(play_icon) + 1 + title_width + 1 + display_width(&time) + 1) as u16;
    let volume_x =
        heart_x + display_width(icons.heart) as u16 + 2 + display_width(icons.volume) as u16 + 1;

    hits.play.push((
        Rect::new(play_x, area.y, display_width(play_icon) as u16, 1),
        (),
    ));
    hits.heart.push((
        Rect::new(heart_x, area.y, display_width(icons.heart) as u16, 1),
        (),
    ));
    hits.volume
        .push((Rect::new(volume_x, area.y, VOLUME_CELLS as u16, 1), ()));

    let filled = (state.volume.clamp(0.0, 1.0) * VOLUME_CELLS as f32).round() as usize;
    let line = Line::from(vec![
        Span::styled(play_icon, Style::new().fg(theme.fg)),
        Span::raw(" "),
        Span::styled(title_text, Style::new().fg(theme.fg)),
        Span::raw(" "),
        Span::styled(time, Style::new().fg(theme.dim)),
        Span::raw(" "),
        Span::styled(
            icons.heart,
            Style::new().fg(if liked { theme.accent2 } else { theme.faint }),
        ),
        Span::raw("  "),
        Span::styled(icons.volume, Style::new().fg(theme.faint)),
        Span::raw(" "),
        Span::styled(icons.volume_full.repeat(filled), Style::new().fg(theme.dim)),
        Span::styled(
            icons
                .volume_empty
                .repeat(VOLUME_CELLS.saturating_sub(filled)),
            Style::new().fg(theme.faint),
        ),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

fn draw_quit_confirm(frame: &mut Frame, state: &AppState, area: Rect, hits: &mut Hits) {
    let question = i18n::t(Key::QuitQuestion);
    let confirm = format!("[y {}]", i18n::t(Key::Quit));
    let cancel = format!("[n {}]", i18n::t(Key::Cancel));
    let total_width =
        display_width(question) + display_width(&confirm) + display_width(&cancel) + 4;
    let x = area.x + area.width.saturating_sub(total_width as u16) / 2;
    let confirm_x = x + display_width(question) as u16 + 2;
    let cancel_x = confirm_x + display_width(&confirm) as u16 + 2;

    for (button_x, label, accepted) in [
        (confirm_x, confirm.as_str(), true),
        (cancel_x, cancel.as_str(), false),
    ] {
        let width = display_width(label) as u16;
        if button_x.saturating_add(width) <= area.right() {
            hits.confirm
                .push((Rect::new(button_x, area.y, width, 1), accepted));
        }
    }

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(question, Style::new().fg(state.theme.fg)),
            Span::raw("  "),
            Span::styled(confirm, Style::new().fg(state.theme.accent)),
            Span::raw("  "),
            Span::styled(cancel, Style::new().fg(state.theme.dim)),
        ])),
        Rect::new(x, area.y, area.right().saturating_sub(x), 1),
    );
}

fn title_width(state: &AppState, width: u16) -> usize {
    let icons = crate::icons::for_style(state.config.icons);
    let fixed_width = display_width(icons.play)
        .max(display_width(icons.pause))
        + 11 // mm:ss/mm:ss
        + display_width(icons.heart)
        + display_width(icons.volume)
        + VOLUME_CELLS
        + 8;
    (width as usize).saturating_sub(fixed_width)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    use ratatui::backend::TestBackend;
    use ratatui::layout::Rect;
    use ratatui::Terminal;

    use super::{draw, marquee_needed};
    use crate::action::Action;
    use crate::app::{AppState, NowPlaying};
    use crate::config::Config;
    use crate::event;
    use crate::ui::Hits;

    fn click(rect: Rect) -> MouseEvent {
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: rect.x,
            row: rect.y,
            modifiers: KeyModifiers::NONE,
        }
    }

    #[test]
    fn short_terminal_draws_only_a_single_compact_player_row() {
        let mut state = AppState::new(&Config::default());
        state.now = Some(NowPlaying {
            title: "A deliberately long title for the compact player".into(),
            artist: "Artist".into(),
            album: String::new(),
        });
        state.duration = Some(Duration::from_secs(245));
        state.position = Duration::from_secs(61);
        state.paused = true;
        state.current_track_id = Some(7);
        state.liked.insert(7);

        let backend = TestBackend::new(80, 6);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut hits = Hits::default();
        terminal
            .draw(|frame| draw(frame, &state, frame.area(), &mut hits))
            .unwrap();

        let buffer = terminal.backend().buffer();
        let first = (0..80).map(|x| buffer[(x, 0)].symbol()).collect::<String>();
        assert!(first.contains('▶'));
        assert!(first.contains("01:01/04:05"));
        assert!(first.contains('♥'));
        for y in 1..6 {
            assert!((0..80).all(|x| buffer[(x, y)].symbol() == " "));
        }
        assert_eq!(hits.play.len(), 1);
        assert_eq!(hits.heart.len(), 1);
        assert_eq!(hits.volume.len(), 1);
        assert!(matches!(
            event::mouse_action(click(hits.play[0].0), &hits, 0),
            Some(Action::TogglePlay)
        ));
        assert!(matches!(
            event::mouse_action(click(hits.heart[0].0), &hits, 0),
            Some(Action::ToggleLike)
        ));
        assert!(marquee_needed(&state, 80));
    }

    #[test]
    fn short_terminal_keeps_quit_confirmation_visible_and_clickable() {
        let mut state = AppState::new(&Config::default());
        state.confirm_quit = true;
        let backend = TestBackend::new(80, 6);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut hits = Hits::default();

        terminal
            .draw(|frame| draw(frame, &state, frame.area(), &mut hits))
            .unwrap();

        let first = (0..80)
            .map(|x| terminal.backend().buffer()[(x, 0)].symbol())
            .collect::<String>();
        assert!(first.contains("ypm"));
        assert_eq!(hits.confirm.len(), 2);
        assert!(hits.confirm[0].1);
        assert!(!hits.confirm[1].1);
    }
}
