use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;

use crate::app::settings::SettingField;
use crate::app::AppState;
use crate::i18n::{self, Key};

use super::{text::display_width, Hits};

pub fn draw(frame: &mut Frame, state: &AppState, area: Rect, hits: &mut Hits) {
    let theme = &state.theme;
    let width = 62_u16.min(area.width);
    let height = (SettingField::ALL.len() as u16 + 8).min(area.height);
    let panel = Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    };
    let block = Block::bordered()
        .title(format!(" {} ", i18n::t(Key::Settings)))
        .style(Style::new().bg(theme.bg))
        .border_style(Style::new().fg(theme.accent));
    let inner = block.inner(panel);
    frame.render_widget(block, panel);
    if inner.is_empty() {
        return;
    }

    let [hint_area, rows_area, status_area, buttons_area] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(SettingField::ALL.len() as u16),
        Constraint::Length(1),
        Constraint::Length(2),
    ])
    .areas(inner);
    frame.render_widget(
        Paragraph::new(i18n::t(Key::SettingsHint)).style(Style::new().fg(theme.dim)),
        hint_area,
    );

    let visible_rows = usize::from(rows_area.height).min(SettingField::ALL.len());
    let max_offset = SettingField::ALL.len().saturating_sub(visible_rows);
    let offset = state
        .settings
        .selected
        .saturating_add(1)
        .saturating_sub(visible_rows)
        .min(max_offset);
    for (visible_index, (index, field)) in SettingField::ALL
        .iter()
        .copied()
        .enumerate()
        .skip(offset)
        .take(visible_rows)
        .enumerate()
    {
        let row = Rect {
            y: rows_area.y + visible_index as u16,
            height: 1,
            ..rows_area
        };
        let selected = index == state.settings.selected;
        let row_style = if selected {
            Style::new()
                .fg(theme.selection_fg())
                .bg(theme.sel)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(theme.fg)
        };
        let label = i18n::t(field.label());
        let label_width = display_width(label);
        let value = state.setting_value(field);
        let value_width = display_width(&value) as u16;
        let available = row.width as usize;
        let content_width = label_width + usize::from(value_width) + 8;
        let pad = available.saturating_sub(content_width);
        let line = Line::from(vec![
            Span::styled(if selected { " › " } else { "   " }, row_style),
            Span::styled(label, row_style),
            Span::styled(" ".repeat(pad), row_style),
            Span::styled("‹ ", row_style),
            Span::styled(value, row_style),
            Span::styled(" › ", row_style),
        ]);
        frame.render_widget(Paragraph::new(line), row);
        hits.settings_rows.push((row, index));
        if selected && available >= content_width {
            let next = Rect::new(row.right().saturating_sub(2), row.y, 2, 1);
            let previous_x = next.x.saturating_sub(value_width.saturating_add(3));
            hits.settings_adjust
                .push((Rect::new(previous_x, row.y, 2, 1), -1));
            hits.settings_adjust.push((next, 1));
        }
    }

    if let Some(status) = &state.status {
        frame.render_widget(
            Paragraph::new(status.as_str()).style(Style::new().fg(theme.accent2)),
            status_area,
        );
    }

    let save = format!("[ Enter · {} ]", i18n::t(Key::Save));
    let cancel = format!("[ Esc · {} ]", i18n::t(Key::Cancel));
    let gap = 3_u16;
    let save_width = display_width(&save) as u16;
    let cancel_width = display_width(&cancel) as u16;
    let total = save_width.saturating_add(gap).saturating_add(cancel_width);
    let start = buttons_area.x + buttons_area.width.saturating_sub(total) / 2;
    let save_rect = Rect::new(start, buttons_area.y, save_width.min(buttons_area.width), 1);
    let cancel_rect = Rect::new(
        save_rect.right().saturating_add(gap),
        buttons_area.y,
        cancel_width.min(buttons_area.right().saturating_sub(save_rect.right() + gap)),
        1,
    );
    frame.render_widget(
        Paragraph::new(save).style(Style::new().fg(theme.selection_fg()).bg(theme.accent)),
        save_rect,
    );
    frame.render_widget(
        Paragraph::new(cancel).style(Style::new().fg(theme.fg)),
        cancel_rect,
    );
    hits.settings_save.push(save_rect);
    hits.settings_cancel.push(cancel_rect);
}

#[cfg(test)]
mod tests {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    use super::*;
    use crate::config::Config;

    fn rendered_settings(
        width: u16,
        height: u16,
        selected: usize,
    ) -> (ratatui::buffer::Buffer, Hits) {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = AppState::new(&Config::default());
        state.settings.selected = selected;
        let mut hits = Hits::default();
        terminal
            .draw(|frame| draw(frame, &state, frame.area(), &mut hits))
            .unwrap();
        (terminal.backend().buffer().clone(), hits)
    }

    #[test]
    fn arrow_hits_cover_the_drawn_glyphs() {
        let (buffer, hits) = rendered_settings(80, 24, 0);
        let (previous, _) = hits
            .settings_adjust
            .iter()
            .find(|(_, delta)| *delta < 0)
            .unwrap();
        let (next, _) = hits
            .settings_adjust
            .iter()
            .find(|(_, delta)| *delta > 0)
            .unwrap();

        assert_eq!(buffer[(previous.x, previous.y)].symbol(), "‹");
        assert_eq!(buffer[(next.x, next.y)].symbol(), "›");
    }

    #[test]
    fn a_short_terminal_scrolls_the_selected_setting_into_view() {
        let (_buffer, hits) = rendered_settings(60, 12, SettingField::ALL.len() - 1);

        assert!(hits
            .settings_rows
            .iter()
            .any(|(_, index)| *index == SettingField::ALL.len() - 1));
    }

    #[test]
    fn a_clipped_row_does_not_register_invisible_arrow_hits() {
        let (_buffer, hits) = rendered_settings(16, 24, 0);

        assert!(hits.settings_adjust.is_empty());
    }
}
