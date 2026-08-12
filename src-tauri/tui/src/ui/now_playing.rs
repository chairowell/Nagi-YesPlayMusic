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

pub fn draw(frame: &mut Frame, state: &AppState, area: Rect, hits: &mut Hits) {
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

    let footer = match (&state.nickname, state.library.len()) {
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
    entries.push((i18n::t(Key::Search).to_owned(), "f", MenuEntry::Search));
    entries.push(match &state.nickname {
        Some(_) => (i18n::t(Key::Relogin).to_owned(), "i", MenuEntry::Login),
        None => (i18n::t(Key::ScanLogin).to_owned(), "i", MenuEntry::Login),
    });
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

fn draw_cover(frame: &mut Frame, state: &AppState, area: Rect) {
    if area.height == 0 {
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
            Style::new().fg(theme.fg).add_modifier(Modifier::BOLD),
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

/// Rows of synced lyrics with the current line pinned mid-window:
/// context dimmed, current line accented, its translation right below.
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
        let style = if is_current {
            Style::new().fg(theme.accent).add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(theme.dim)
        };
        let marker = if is_current { "> " } else { "  " };
        lines.push(Line::from(Span::styled(
            format!("{marker}{}", lyric.text),
            style,
        )));
        used += 1;
        // Every line carries its translation; the current pair reads brighter.
        if let Some(translation) = &lyric.translation {
            if used < rows && !translation.is_empty() {
                let translation_style = if is_current {
                    Style::new().fg(theme.dim)
                } else {
                    Style::new().fg(theme.faint)
                };
                lines.push(Line::from(Span::styled(
                    format!("  {translation}"),
                    translation_style,
                )));
                used += 1;
            }
        }
    }
    lines
}

fn draw_progress(frame: &mut Frame, state: &AppState, area: Rect, hits: &mut Hits) {
    let theme = &state.theme;
    let icon = if state.paused { "⏸" } else { "▶" };
    let mode_icon = state.play_mode.icon();
    let liked = state
        .current_track_id
        .map(|id| state.liked.contains(&id))
        .unwrap_or(false);
    let elapsed = format_duration(state.position);
    let total = state
        .duration
        .map(format_duration)
        .unwrap_or_else(|| "--:--".into());

    let fixed = icon.len() + elapsed.len() + total.len() + display_width(mode_icon) + 22;
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
    let mut spans = vec![
        Span::styled(format!(" {icon} "), Style::new().fg(theme.fg)),
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
            width: 1,
            height: 1,
        },
        (),
    ));
    spans.push(Span::raw("  "));
    // Filled+bright = liked, hollow+faint = not — glyph AND color both signal.
    if liked {
        spans.push(Span::styled("♥", Style::new().fg(theme.accent2)));
    } else {
        spans.push(Span::styled("♡", Style::new().fg(theme.faint)));
    }
    spans.push(Span::styled(
        format!("  {mode_icon}"),
        Style::new().fg(theme.dim),
    ));
    // Battery-style volume: click or drag inside the bracket to set.
    const VOLUME_CELLS: usize = 10;
    let filled = (state.volume.clamp(0.0, 1.0) * VOLUME_CELLS as f32).round() as usize;
    let volume_x = spans
        .iter()
        .map(|span| display_width(&span.content) as u16)
        .sum::<u16>()
        + area.x
        + 3; // skip the "  [" prefix
    hits.volume.push((
        Rect {
            x: volume_x,
            y: area.y,
            width: VOLUME_CELLS as u16,
            height: 1,
        },
        (),
    ));
    spans.push(Span::styled("  [", Style::new().fg(theme.faint)));
    spans.push(Span::styled("▮".repeat(filled), Style::new().fg(theme.dim)));
    spans.push(Span::styled(
        "▯".repeat(VOLUME_CELLS - filled),
        Style::new().fg(theme.faint),
    ));
    spans.push(Span::styled("]", Style::new().fg(theme.faint)));
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}
