//! Terminal input → Action. Arrow keys and vim keys coexist; numbers jump
//! straight to a view (cmus model).

use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEvent, MouseEventKind,
};

use crate::action::{Action, View};
use crate::ui::Hits;

pub fn action_for(event: Event) -> Option<Action> {
    match event {
        Event::Key(key) if key.kind != KeyEventKind::Release => key_action(key),
        Event::Mouse(mouse) => Some(Action::Mouse(mouse)),
        Event::Resize(_, _) => Some(Action::Resize),
        _ => None,
    }
}

/// Resolve a mouse event against the geometry recorded at draw time.
/// Click a tab to switch; click a row to select, click it again to play;
/// the wheel moves the selection. An open quit-confirm dialog is modal:
/// only its buttons respond.
pub fn mouse_action(mouse: MouseEvent, hits: &Hits, selected: usize) -> Option<Action> {
    let position = ratatui::layout::Position {
        x: mouse.column,
        y: mouse.row,
    };
    if !hits.confirm.is_empty() {
        if let MouseEventKind::Down(crossterm::event::MouseButton::Left) = mouse.kind {
            for (rect, is_confirm) in &hits.confirm {
                if rect.contains(position) {
                    return Some(if *is_confirm { Action::Quit } else { Action::Back });
                }
            }
        }
        return None;
    }
    match mouse.kind {
        MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
            for (rect, view) in &hits.tabs {
                if rect.contains(position) {
                    return Some(Action::SwitchView(*view));
                }
            }
            for (rect, entry) in &hits.menu {
                if rect.contains(position) {
                    return Some(match entry {
                        crate::action::MenuEntry::Library => Action::SwitchView(View::Library),
                        crate::action::MenuEntry::Search => Action::SwitchView(View::Search),
                        crate::action::MenuEntry::Login => Action::StartLogin,
                        crate::action::MenuEntry::Quit => Action::Quit,
                    });
                }
            }
            for (rect, index) in &hits.rows {
                if rect.contains(position) {
                    return Some(if *index == selected {
                        Action::Activate
                    } else {
                        Action::SelectIndex(*index)
                    });
                }
            }
            None
        }
        MouseEventKind::ScrollDown => Some(Action::MoveSelection(1)),
        MouseEventKind::ScrollUp => Some(Action::MoveSelection(-1)),
        _ => None,
    }
}

fn key_action(key: KeyEvent) -> Option<Action> {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return Some(Action::Quit);
    }
    match key.code {
        KeyCode::Char('q') => Some(Action::Quit),
        KeyCode::Char('1') => Some(Action::SwitchView(View::NowPlaying)),
        KeyCode::Char('2') | KeyCode::Char('l') => Some(Action::SwitchView(View::Library)),
        KeyCode::Char('3') | KeyCode::Char('/') => Some(Action::SwitchView(View::Search)),
        KeyCode::Char('4') => Some(Action::SwitchView(View::Queue)),
        KeyCode::Backspace | KeyCode::Esc => Some(Action::Back),
        KeyCode::Char('g') => Some(Action::StartLogin),
        KeyCode::Char('z') => Some(Action::ToggleZen),
        KeyCode::Char(' ') => Some(Action::TogglePlay),
        KeyCode::Char('n') => Some(Action::NextTrack),
        KeyCode::Char('p') => Some(Action::PrevTrack),
        KeyCode::Right => Some(Action::SeekBy(1)),
        KeyCode::Left => Some(Action::SeekBy(-1)),
        KeyCode::Char('+') | KeyCode::Char('=') => Some(Action::VolumeBy(0.05)),
        KeyCode::Char('-') => Some(Action::VolumeBy(-0.05)),
        KeyCode::Down | KeyCode::Char('j') => Some(Action::MoveSelection(1)),
        KeyCode::Up | KeyCode::Char('k') => Some(Action::MoveSelection(-1)),
        KeyCode::Enter => Some(Action::Activate),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    #[test]
    fn arrows_and_vim_keys_map_to_the_same_actions() {
        assert!(matches!(
            action_for(key(KeyCode::Down)),
            Some(Action::MoveSelection(1))
        ));
        assert!(matches!(
            action_for(key(KeyCode::Char('j'))),
            Some(Action::MoveSelection(1))
        ));
        assert!(matches!(
            action_for(key(KeyCode::Char('2'))),
            Some(Action::SwitchView(View::Library))
        ));
        assert!(matches!(
            action_for(key(KeyCode::Char('z'))),
            Some(Action::ToggleZen)
        ));
    }
}
