//! Player-first main view: cover · title/lyrics · progress. The pixel-art
//! cover widget replaces the placeholder in the visual stage.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::Frame;

use crate::app::AppState;
use crate::ui::format_duration;

// Cover art travels as a 26×13 cell grid (each cell is two vertical pixels,
// so the pixel grid is square); the frame must hug exactly that grid or the
// border reads as a stretched rectangle.
const COVER_GRID: (u16, u16) = (26, 13);

pub fn draw(frame: &mut Frame, state: &AppState, area: Rect) {
    let [main, progress_area] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(2)]).areas(area);

    let frame_width = (COVER_GRID.0 + 2).min(main.width);
    let frame_height = (COVER_GRID.1 + 2).min(main.height);
    let [cover_column, meta_area] =
        Layout::horizontal([Constraint::Length(frame_width), Constraint::Min(0)]).areas(main);
    // Fixed square frame, top-aligned with the title column.
    let cover_area = Rect {
        x: cover_column.x,
        y: cover_column.y,
        width: frame_width,
        height: frame_height,
    };

    draw_cover(frame, state, cover_area);
    draw_meta(frame, state, meta_area);
    draw_progress(frame, state, progress_area);
}

fn draw_cover(frame: &mut Frame, state: &AppState, area: Rect) {
    let theme = &state.theme;
    let block = Block::new()
        .borders(Borders::ALL)
        .border_type(BorderType::QuadrantOutside)
        .border_style(Style::new().fg(theme.faint));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 {
        return;
    }
    if let Some(cover) = &state.cover {
        frame.render_widget(cover, inner);
        return;
    }
    let tag = Line::from(Span::styled("▚ 封面", Style::new().fg(theme.faint)));
    let [_, middle, _] = Layout::vertical([
        Constraint::Percentage(45),
        Constraint::Length(1),
        Constraint::Min(0),
    ])
    .areas(inner);
    frame.render_widget(Paragraph::new(tag).centered(), middle);
}

fn draw_meta(frame: &mut Frame, state: &AppState, area: Rect) {
    let theme = &state.theme;
    let mut lines = Vec::new();
    lines.push(Line::default());
    match &state.now {
        Some(now) => {
            lines.push(Line::from(Span::styled(
                format!("  {}", now.title),
                Style::new().fg(theme.fg).add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(Span::styled(
                format!("  {}", now.artist),
                Style::new().fg(theme.dim),
            )));
            if !now.album.is_empty() {
                lines.push(Line::from(Span::styled(
                    format!("  {}", now.album),
                    Style::new().fg(theme.faint),
                )));
            }
            if !state.lyrics.is_empty() {
                lines.push(Line::default());
                let reserved = lines.len() as u16;
                lines.extend(lyric_window(state, area.height.saturating_sub(reserved)));
            }
        }
        None => {
            lines.push(Line::from(Span::styled(
                "  还没有在播的歌",
                Style::new().fg(theme.fg).add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(Span::styled(
                "  按 2 打开曲库，Enter 播放",
                Style::new().fg(theme.dim),
            )));
            if state.nickname.is_none() {
                lines.push(Line::from(Span::styled(
                    "  按 g 扫码登录，听你自己的歌单",
                    Style::new().fg(theme.accent2),
                )));
            }
        }
    }
    if let Some(status) = &state.status {
        lines.push(Line::default());
        lines.push(Line::from(Span::styled(
            format!("  {status}"),
            Style::new().fg(theme.accent2),
        )));
    }
    frame.render_widget(Paragraph::new(lines), area);
}

/// Rows of synced lyrics with the current line pinned mid-window:
/// context dimmed, current line accented, its translation right below.
fn lyric_window(state: &AppState, height: u16) -> Vec<Line<'static>> {
    let theme = &state.theme;
    let current = crate::lyrics::line_index_at(&state.lyrics, state.position);
    let rows = height.max(1) as usize;
    let anchor = current.unwrap_or(0);
    let above = rows / 2;
    let start = anchor.saturating_sub(above);

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
        let marker = if is_current { "▸ " } else { "  " };
        lines.push(Line::from(Span::styled(
            format!("  {marker}{}", lyric.text),
            style,
        )));
        used += 1;
        if is_current {
            if let Some(translation) = &lyric.translation {
                if used < rows {
                    lines.push(Line::from(Span::styled(
                        format!("    {translation}"),
                        Style::new().fg(theme.faint),
                    )));
                    used += 1;
                }
            }
        }
    }
    lines
}

fn draw_progress(frame: &mut Frame, state: &AppState, area: Rect) {
    let theme = &state.theme;
    let icon = if state.paused { "⏸" } else { "▶" };
    let elapsed = format_duration(state.position);
    let total = state
        .duration
        .map(format_duration)
        .unwrap_or_else(|| "--:--".into());

    let fixed = icon.len() + elapsed.len() + total.len() + 12;
    let bar_width = (area.width as usize).saturating_sub(fixed).max(8);
    let ratio = match state.duration {
        Some(duration) if !duration.is_zero() => {
            (state.position.as_secs_f64() / duration.as_secs_f64()).clamp(0.0, 1.0)
        }
        _ => 0.0,
    };
    let head = ((bar_width as f64 - 1.0) * ratio).round() as usize;

    let line = Line::from(vec![
        Span::styled(format!(" {icon} "), Style::new().fg(theme.fg)),
        Span::styled(elapsed, Style::new().fg(theme.dim)),
        Span::raw(" "),
        Span::styled("━".repeat(head), Style::new().fg(theme.fg)),
        Span::styled("●", Style::new().fg(theme.accent)),
        Span::styled(
            "─".repeat(bar_width.saturating_sub(head + 1)),
            Style::new().fg(theme.faint),
        ),
        Span::raw(" "),
        Span::styled(total, Style::new().fg(theme.dim)),
        Span::styled(
            format!("  vol {:>3.0}%", state.volume * 100.0),
            Style::new().fg(theme.faint),
        ),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}
