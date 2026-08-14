//! QR login view: half-block QR art centered, status line underneath.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::AppState;
use crate::i18n::{self, Key};

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
    let qr_height = qr_lines.len() as u16;

    let [_, title_area, qr_area, message_area, _] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(2),
        Constraint::Length(qr_height),
        Constraint::Length(2),
        Constraint::Min(0),
    ])
    .areas(area);

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            i18n::t(Key::LoginTitle),
            Style::new().fg(theme.accent),
        )))
        .centered(),
        title_area,
    );
    if !qr_lines.is_empty() {
        frame.render_widget(Paragraph::new(qr_lines).centered(), qr_area);
    }
    if let Some(message) = &state.session.login_message {
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(Span::styled(message.clone(), Style::new().fg(theme.dim))),
                Line::from(Span::styled(
                    i18n::t(Key::LoginInstruction),
                    Style::new().fg(theme.faint),
                )),
            ])
            .centered(),
            message_area,
        );
    }
}
