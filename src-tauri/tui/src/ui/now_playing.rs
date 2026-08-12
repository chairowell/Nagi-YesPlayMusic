//! The main view has two moods: an idle dashboard (pixel art + menu,
//! dashboard-nvim style) and the playing layout (cover · lyrics · progress).

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::Frame;

use crate::action::MenuEntry;
use crate::app::AppState;
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
    let (cover_w, cover_h) = state
        .cover
        .as_ref()
        .map(|cover| (cover.width, cover.height))
        .unwrap_or(COVER_GRID);

    match state.layout {
        crate::app::PlayLayout::Side => {
            let frame_width = (cover_w + 2).min(main.width);
            let frame_height = (cover_h + 2).min(main.height);
            let [cover_column, meta_area] =
                Layout::horizontal([Constraint::Length(frame_width), Constraint::Min(0)])
                    .areas(main);
            let cover_area = Rect {
                x: cover_column.x,
                y: cover_column.y,
                width: frame_width,
                height: frame_height,
            };
            draw_cover(frame, state, cover_area);
            draw_meta(frame, state, meta_area, false);
        }
        crate::app::PlayLayout::Stacked => {
            let frame_height = (cover_h + 2).min(main.height.saturating_sub(4));
            let [cover_row, meta_area] =
                Layout::vertical([Constraint::Length(frame_height), Constraint::Min(0)])
                    .areas(main);
            let cover_area = centered(cover_row, cover_w + 2, frame_height);
            draw_cover(frame, state, cover_area);
            draw_meta(frame, state, meta_area, true);
        }
    }
    draw_progress(frame, state, progress_area);
}

// ── idle dashboard ──────────────────────────────────────────────────

fn draw_dashboard(frame: &mut Frame, state: &AppState, area: Rect, hits: &mut Hits) {
    let theme = &state.theme;
    let menu = menu_entries(state);
    let menu_height = menu.len() as u16;
    let art_height = state.idle_art.height.min(area.height.saturating_sub(menu_height + 4));

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
        let pad = (MENU_WIDTH as usize)
            .saturating_sub(display_width(label) + display_width(key) + 2);
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
        (Some(nickname), n) if n > 0 => format!("♪ {nickname} · {n} 首已就绪"),
        (Some(nickname), _) => format!("♪ {nickname}"),
        (None, _) => "未登录 · 扫码后同步你的音乐".into(),
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
    let mut entries = vec![("我喜欢的音乐".to_owned(), "2", MenuEntry::Library)];
    entries.push(("搜索".to_owned(), "/", MenuEntry::Search));
    entries.push(match &state.nickname {
        Some(_) => ("重新扫码登录".to_owned(), "i", MenuEntry::Login),
        None => ("扫码登录".to_owned(), "i", MenuEntry::Login),
    });
    entries.push(("退出".to_owned(), "q", MenuEntry::Quit));
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
    // Cover still loading: keep the frame quiet, no label.
    frame.render_widget(&state.idle_art, inner);
}

fn draw_meta(frame: &mut Frame, state: &AppState, area: Rect, centered_text: bool) {
    let theme = &state.theme;
    let indent = if centered_text { "" } else { "  " };
    let mut lines = Vec::new();
    lines.push(Line::default());
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
        // Every line carries its translation; the current pair reads brighter.
        if let Some(translation) = &lyric.translation {
            if used < rows && !translation.is_empty() {
                let translation_style = if is_current {
                    Style::new().fg(theme.dim)
                } else {
                    Style::new().fg(theme.faint)
                };
                lines.push(Line::from(Span::styled(
                    format!("    {translation}"),
                    translation_style,
                )));
                used += 1;
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
    spans.push(Span::styled(
        format!("  vol {:>3.0}%", state.volume * 100.0),
        Style::new().fg(theme.faint),
    ));
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}
