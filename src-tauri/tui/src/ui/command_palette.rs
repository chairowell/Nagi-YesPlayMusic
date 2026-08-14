use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};
use ratatui::Frame;

use crate::app::AppState;
use crate::i18n::{self, Key};

use super::{panel_block, scroll_offset, text};

pub(crate) const WIDTH: u16 = 60;
const MAX_HEIGHT: u16 = 20;

pub(crate) fn dim_background(frame: &mut Frame, faint: ratatui::style::Color) {
    for cell in &mut frame.buffer_mut().content {
        cell.fg = faint;
    }
}

pub(crate) fn draw(frame: &mut Frame, state: &AppState, area: Rect) {
    if area.is_empty() {
        return;
    }
    let theme = &state.theme;
    let commands = state.command_palette.filtered();
    let width = WIDTH.min(area.width);
    let height = MAX_HEIGHT.min(area.height);
    let modal = Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    );

    frame.render_widget(Clear, modal);
    let block = panel_block(
        theme,
        i18n::t(Key::CommandPalette),
        Some(i18n::t_result_count(commands.len())),
    )
    .style(Style::new().bg(theme.bg));
    let inner = block.inner(modal);
    frame.render_widget(block, modal);
    if inner.is_empty() {
        return;
    }

    let [input_area, _, list_area, hint_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(inner);
    draw_input(frame, state, input_area);
    draw_commands(frame, state, &commands, list_area);

    let hint = state
        .command_feedback
        .as_deref()
        .unwrap_or_else(|| i18n::t(Key::CommandPaletteHint));
    let hint_color = if state.command_feedback_error {
        theme.accent2
    } else if state.command_feedback.is_some() {
        theme.accent
    } else {
        theme.dim
    };
    frame.render_widget(
        Paragraph::new(text::pad_display(hint, usize::from(hint_area.width)))
            .style(Style::new().fg(hint_color)),
        hint_area,
    );
}

fn draw_input(frame: &mut Frame, state: &AppState, area: Rect) {
    let theme = &state.theme;
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(": ", Style::new().fg(theme.accent)),
            Span::styled(
                state.command_palette.query.as_str(),
                Style::new().fg(theme.fg),
            ),
            Span::styled("▎", Style::new().fg(theme.accent)),
        ]))
        .style(Style::new().bg(theme.selection_bg())),
        area,
    );
}

fn draw_commands(
    frame: &mut Frame,
    state: &AppState,
    commands: &[&crate::app::command_palette::CommandSpec],
    area: Rect,
) {
    let theme = &state.theme;
    if commands.is_empty() {
        frame.render_widget(
            Paragraph::new(i18n::t(Key::CommandNoMatches)).style(Style::new().fg(theme.dim)),
            area,
        );
        return;
    }

    let visible = usize::from(area.height);
    let offset = scroll_offset(state.command_palette.selected, commands.len(), visible);
    for (row, command) in commands.iter().skip(offset).take(visible).enumerate() {
        let index = offset + row;
        let selected = index == state.command_palette.selected;
        let background = selected.then_some(theme.selection_bg());
        let base = Style::new().bg(background.unwrap_or(theme.bg));
        let marker_width = 2_usize.min(usize::from(area.width));
        let content_width = usize::from(area.width).saturating_sub(marker_width);
        let alias_width = text::display_width(command.aliases)
            .min(18)
            .min(content_width / 2);
        let usage_width = content_width.saturating_sub(alias_width);
        let marker = if selected { "› " } else { "  " };
        let line = Line::from(vec![
            Span::styled(
                text::pad_display(marker, marker_width),
                base.fg(if selected { theme.accent } else { theme.faint }),
            ),
            Span::styled(
                text::pad_display(command.usage, usage_width),
                base.fg(if selected { theme.fg } else { theme.dim }),
            ),
            Span::styled(
                text::pad_display_right(command.aliases, alias_width),
                base.fg(if selected { theme.dim } else { theme.faint }),
            ),
        ]);
        frame.render_widget(
            Paragraph::new(line),
            Rect::new(area.x, area.y + row as u16, area.width, 1),
        );
    }
}

#[cfg(test)]
mod tests {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    use super::*;
    use crate::config::Config;
    use crate::ui::{self, Hits};

    #[test]
    fn palette_is_centered_sixty_columns_and_dims_only_the_background() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = AppState::new(&Config::default());
        state.command_palette.open();
        let mut hits = Hits::default();

        terminal
            .draw(|frame| ui::draw(frame, &mut state, &mut hits))
            .unwrap();
        let buffer = terminal.backend().buffer();
        let modal_x = (100 - WIDTH) / 2;
        let modal_y = (30 - MAX_HEIGHT) / 2;

        assert_eq!(buffer[(modal_x, modal_y)].symbol(), "╭");
        assert_eq!(buffer[(modal_x + WIDTH - 1, modal_y)].symbol(), "╮");
        assert_eq!(buffer[(0, 0)].fg, state.theme.faint);
        let title_start = i18n::t(Key::CommandPalette)
            .chars()
            .next()
            .unwrap()
            .to_string();
        let title_x = (modal_x..modal_x + WIDTH)
            .find(|x| buffer[(*x, modal_y)].symbol() == title_start)
            .unwrap();
        assert_eq!(buffer[(title_x, modal_y)].fg, state.theme.accent);
    }

    #[test]
    fn chinese_filter_and_selected_row_are_projected() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = AppState::new(&Config::default());
        state.command_palette.open();
        state.command_palette.paste("主题");
        let mut hits = Hits::default();

        terminal
            .draw(|frame| ui::draw(frame, &mut state, &mut hits))
            .unwrap();
        let rendered = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        let compact = rendered
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect::<String>();

        assert!(rendered.contains("theme <name>"));
        assert!(compact.contains("主题<名称>"));
        assert!(!rendered.contains("volume <0-100>"));
    }

    #[test]
    fn mini_player_still_renders_the_modal_and_footer_feedback_is_temporary_slot_content() {
        let backend = TestBackend::new(80, 6);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = AppState::new(&Config::default());
        state.command_palette.open();
        let mut hits = Hits::default();
        terminal
            .draw(|frame| ui::draw(frame, &mut state, &mut hits))
            .unwrap();
        let buffer = terminal.backend().buffer();
        assert_eq!(buffer[((80 - WIDTH) / 2, 0)].symbol(), "╭");
        assert_eq!(buffer[(0, 0)].fg, state.theme.faint);

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        state.command_palette.close();
        state.command_feedback = Some("Command executed: next".into());
        terminal
            .draw(|frame| ui::draw(frame, &mut state, &mut hits))
            .unwrap();
        let footer = (0..80)
            .map(|x| terminal.backend().buffer()[(x, 23)].symbol())
            .collect::<String>();
        assert!(footer.contains("Command executed: next"));

        state.command_feedback = None;
        terminal
            .draw(|frame| ui::draw(frame, &mut state, &mut hits))
            .unwrap();
        let footer = (0..80)
            .map(|x| terminal.backend().buffer()[(x, 23)].symbol())
            .collect::<String>();
        assert!(footer.contains("Space"));
    }
}
