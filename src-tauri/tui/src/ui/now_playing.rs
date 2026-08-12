//! Player-first main view: cover · title/lyrics · progress. The pixel-art
//! cover widget replaces the placeholder in the visual stage.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::Frame;

use crate::app::AppState;
use crate::ui::format_duration;

pub fn draw(frame: &mut Frame, state: &AppState, area: Rect) {
    let [main, progress_area] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(2)]).areas(area);

    let cover_width = (main.height.saturating_mul(2)).clamp(12, 28);
    let [cover_area, meta_area] = Layout::horizontal([
        Constraint::Length(cover_width),
        Constraint::Min(0),
    ])
    .areas(main);

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
