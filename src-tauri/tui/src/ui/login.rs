//! QR login view: half-block QR art centered, status line underneath.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
use ratatui::Frame;

use crate::app::AppState;
use crate::i18n::{self, Key};

use super::{panel_block, text::display_width, PANEL_PADDING_X};

/// Login copy and QR art stay compact on wide terminals without touching narrow edges.
const LOGIN_PANEL_MAX_WIDTH: u16 = 80;
/// A composed signed-out state keeps enough measure for its title and short status.
const LOGIN_MIN_CONTENT_WIDTH: u16 = 32;
/// Even before a QR arrives, the framed state retains a small body beneath its title.
const LOGIN_PANEL_MIN_HEIGHT: u16 = 5;
/// Rounded borders occupy one column on each side of a panel.
const PANEL_BORDER_COLUMNS: u16 = 2;
/// Rounded borders occupy one row above and below panel content.
const PANEL_BORDER_ROWS: u16 = 2;
pub fn draw(frame: &mut Frame, state: &AppState, area: Rect) {
    let theme = &state.theme;
    let qr_lines: Vec<Line> = state
        .session
        .login_qr
        .as_deref()
        .map(|art| {
            art.lines()
                .map(|line| Line::from(Span::styled(line.to_owned(), Style::new().fg(theme.fg))))
                .collect()
        })
        .unwrap_or_default();
    let qr_height = u16::try_from(qr_lines.len()).unwrap_or(u16::MAX);
    let qr_width = state
        .session
        .login_qr
        .as_deref()
        .map(|art| art.lines().map(safe_display_width).max().unwrap_or(0))
        .unwrap_or(0);
    let message_width = state.session.login_message.as_deref().map_or(0, |message| {
        safe_display_width(message).max(safe_display_width(i18n::t(Key::LoginInstruction)))
    });
    let content_width = qr_width
        .max(message_width)
        .max(safe_display_width(i18n::t(Key::LoginTitle)))
        .max(LOGIN_MIN_CONTENT_WIDTH);
    let panel_width = content_width
        .saturating_add(PANEL_BORDER_COLUMNS + PANEL_PADDING_X * 2)
        .min(LOGIN_PANEL_MAX_WIDTH)
        .min(area.width);
    let inner_width = panel_width
        .saturating_sub(PANEL_BORDER_COLUMNS + PANEL_PADDING_X * 2)
        .max(1);
    let desired_message_height = state.session.login_message.as_deref().map_or(0, |message| {
        wrapped_rows(message, inner_width)
            .saturating_add(wrapped_rows(i18n::t(Key::LoginInstruction), inner_width))
    });
    let message_lines = state
        .session
        .login_message
        .as_deref()
        .map(|message| {
            vec![
                Line::from(Span::styled(message.to_owned(), Style::new().fg(theme.dim))),
                Line::from(Span::styled(
                    i18n::t(Key::LoginInstruction),
                    Style::new().fg(theme.dim),
                )),
            ]
        })
        .unwrap_or_default();
    let message = Paragraph::new(message_lines)
        .centered()
        .wrap(Wrap { trim: true });
    let panel_height = qr_height
        .saturating_add(desired_message_height)
        .saturating_add(PANEL_BORDER_ROWS)
        .max(LOGIN_PANEL_MIN_HEIGHT)
        .min(area.height);
    let panel = Rect::new(
        area.x + area.width.saturating_sub(panel_width) / 2,
        area.y + area.height.saturating_sub(panel_height) / 2,
        panel_width,
        panel_height,
    );
    let block = panel_block(theme, i18n::t(Key::LoginTitle), None);
    let inner = block.inner(panel);
    frame.render_widget(block, panel);
    if inner.is_empty() {
        return;
    }

    // Preserve a complete, scannable QR first; guidance uses any remaining row on short screens.
    let visible_qr_height = qr_height.min(inner.height);
    let visible_message_height = desired_message_height.min(inner.height - visible_qr_height);
    let used_height = visible_qr_height + visible_message_height;
    let leading_space = inner.height.saturating_sub(used_height) / 2;
    let [_, qr_area, message_area, _] = Layout::vertical([
        Constraint::Length(leading_space),
        Constraint::Length(visible_qr_height),
        Constraint::Length(visible_message_height),
        Constraint::Min(0),
    ])
    .areas(inner);

    if visible_qr_height > 0 {
        frame.render_widget(Paragraph::new(qr_lines).centered(), qr_area);
    }
    if visible_message_height > 0 {
        frame.render_widget(message, message_area);
    }
}

fn safe_display_width(text: &str) -> u16 {
    u16::try_from(display_width(text)).unwrap_or(u16::MAX)
}

fn wrapped_rows(text: &str, width: u16) -> u16 {
    text.lines()
        .map(|line| safe_display_width(line).max(1).div_ceil(width.max(1)))
        .fold(0, u16::saturating_add)
}

#[cfg(test)]
mod tests {
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::Terminal;

    use super::*;
    use crate::config::Config;

    const SKELETON_SIZES: [(u16, u16); 3] = [(80, 24), (120, 40), (200, 60)];

    fn rendered_login(width: u16, height: u16) -> Buffer {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = AppState::new(&Config::default());
        state.session.login_qr = Some("████████\n█      █\n████████".into());
        state.session.login_message = Some("案".repeat(120));
        terminal
            .draw(|frame| draw(frame, &state, frame.area()))
            .unwrap();
        terminal.backend().buffer().clone()
    }

    fn symbol_position(buffer: &Buffer, symbol: &str) -> (u16, u16) {
        let area = buffer.area;
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                if buffer[(x, y)].symbol() == symbol {
                    return (x, y);
                }
            }
        }
        panic!("missing symbol {symbol}");
    }

    #[test]
    fn login_panel_wraps_copy_inside_a_complete_centered_frame_at_all_sizes() {
        for (width, height) in SKELETON_SIZES {
            let buffer = rendered_login(width, height);
            let top_left = symbol_position(&buffer, "╭");
            let top_right = symbol_position(&buffer, "╮");
            let bottom_left = symbol_position(&buffer, "╰");
            let bottom_right = symbol_position(&buffer, "╯");
            let theme = crate::theme::Theme::db16();
            let instruction_start = i18n::t(Key::LoginInstruction)
                .chars()
                .next()
                .unwrap()
                .to_string();

            assert_eq!(top_left.1, top_right.1);
            assert_eq!(bottom_left.1, bottom_right.1);
            assert_eq!(top_left.0, bottom_left.0);
            assert_eq!(top_right.0, bottom_right.0);
            assert!(top_left.0.abs_diff(width - top_right.0 - 1) <= 1);
            assert!(top_left.1.abs_diff(height - bottom_left.1 - 1) <= 1);
            assert_eq!(buffer[top_left].fg, theme.faint);
            assert!((top_left.0..=top_right.0).any(|x| buffer[(x, top_left.1)].fg == theme.accent));

            for y in top_left.1 + 1..bottom_left.1 {
                assert_eq!(buffer[(top_left.0, y)].symbol(), "│");
                assert_eq!(buffer[(top_right.0, y)].symbol(), "│");
                assert_eq!(buffer[(top_right.0, y)].fg, theme.faint);
            }
            let wrapped_message_rows = (top_left.1 + 1..bottom_left.1)
                .filter(|y| (top_left.0 + 1..top_right.0).any(|x| buffer[(x, *y)].symbol() == "案"))
                .count();
            assert!(wrapped_message_rows >= 2);
            let instruction_cell = (top_left.1 + 1..bottom_left.1)
                .find_map(|y| {
                    (top_left.0 + 1..top_right.0)
                        .find(|x| buffer[(*x, y)].symbol() == instruction_start)
                        .map(|x| (x, y))
                })
                .expect("login instruction should remain visible inside the panel");
            assert_eq!(buffer[instruction_cell].fg, theme.dim);
        }
    }

    #[test]
    fn wrapped_row_estimate_accounts_for_each_localized_copy_line() {
        let japanese_instruction =
            "NetEase Cloud Musicアプリのスキャン機能を使用してください（カメラアプリは使用不可）";

        assert!(wrapped_rows(japanese_instruction, 76) >= 2);
        assert_eq!(wrapped_rows("status", 76), 1);
    }
}
