//! Terminal input → Action. Arrow keys and vim keys coexist; numbers jump
//! straight to a view (cmus model).

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::action::{Action, View};

pub fn action_for(event: Event) -> Option<Action> {
    match event {
        Event::Key(key) if key.kind != KeyEventKind::Release => key_action(key),
        Event::Resize(_, _) => Some(Action::Resize),
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
        KeyCode::Char('z') => Some(Action::ToggleZen),
        KeyCode::Char(' ') => Some(Action::TogglePlay),
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
