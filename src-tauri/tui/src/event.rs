//! Terminal input → Action. Arrow keys and vim keys coexist; numbers jump
//! straight to a view (cmus model).

use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEvent, MouseEventKind,
};

use crate::action::{Action, View};
use crate::ui::Hits;

pub fn action_for(event: Event) -> Option<Action> {
    match event {
        // Keys route through the reducer, which knows whether a text
        // input owns the keyboard right now.
        Event::Key(key) if key.kind != KeyEventKind::Release => Some(Action::RawKey(key)),
        Event::Mouse(mouse) => Some(Action::Mouse(mouse)),
        Event::Resize(_, _) => Some(Action::Resize),
        Event::Paste(text) => Some(Action::Paste(text)),
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
                    return Some(if *is_confirm {
                        Action::Quit
                    } else {
                        Action::Back
                    });
                }
            }
        }
        return None;
    }
    // Battery-style volume bar: click or drag anywhere inside sets the level.
    if matches!(
        mouse.kind,
        MouseEventKind::Down(crossterm::event::MouseButton::Left)
            | MouseEventKind::Drag(crossterm::event::MouseButton::Left)
    ) {
        for (rect, _) in &hits.volume {
            if rect.contains(position)
                || matches!(mouse.kind, MouseEventKind::Drag(_)) && mouse.row == rect.y
            {
                let ratio = (mouse.column.saturating_sub(rect.x) as f32 + 0.5) / rect.width as f32;
                return Some(Action::SetVolumeTo(ratio.clamp(0.0, 1.0)));
            }
        }
    }
    match mouse.kind {
        MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
            for (rect, view) in &hits.tabs {
                if rect.contains(position) {
                    return Some(Action::SwitchView(*view));
                }
            }
            for (rect, _) in &hits.heart {
                if rect.contains(position) {
                    return Some(Action::ToggleLike);
                }
            }
            for (rect, index) in &hits.sidebar {
                if rect.contains(position) {
                    return Some(Action::OpenSource(*index));
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

pub fn key_action(key: KeyEvent) -> Option<Action> {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        return match key.code {
            KeyCode::Char('c') => Some(Action::Quit),
            // vim half-page jumps
            KeyCode::Char('d') => Some(Action::MoveSelection(10)),
            KeyCode::Char('u') => Some(Action::MoveSelection(-10)),
            _ => None,
        };
    }
    match key.code {
        KeyCode::Char('q') => Some(Action::Quit),
        KeyCode::Char('1') => Some(Action::SwitchView(View::NowPlaying)),
        KeyCode::Char('2') => Some(Action::SwitchView(View::Library)),
        KeyCode::Char('3') | KeyCode::Char('/') | KeyCode::Char('f') => {
            Some(Action::SwitchView(View::Search))
        }
        KeyCode::Char('4') => Some(Action::SwitchView(View::Queue)),
        // vim: h backs out, l dives in (Backspace/Esc keep working)
        KeyCode::Backspace | KeyCode::Esc | KeyCode::Char('h') => Some(Action::Back),
        KeyCode::Char('l') | KeyCode::Enter => Some(Action::Activate),
        KeyCode::Char('g') => Some(Action::GKey),
        KeyCode::Char('G') => Some(Action::JumpBottom),
        KeyCode::Char('i') => Some(Action::StartLogin),
        KeyCode::Char('y') => Some(Action::ConfirmYes),
        KeyCode::Char('?') => Some(Action::ToggleHelp),
        KeyCode::Char('z') => Some(Action::ToggleZen),
        KeyCode::Char('s') => Some(Action::CycleMode),
        KeyCode::Char('x') => Some(Action::ToggleLike),
        KeyCode::Char(' ') => Some(Action::TogglePlay),
        KeyCode::Char('n') => Some(Action::NextTrack),
        KeyCode::Char('p') => Some(Action::PrevTrack),
        KeyCode::Right => Some(Action::SeekBy(1)),
        KeyCode::Left => Some(Action::SeekBy(-1)),
        KeyCode::Char('+') | KeyCode::Char('=') => Some(Action::VolumeBy(0.05)),
        KeyCode::Char('-') => Some(Action::VolumeBy(-0.05)),
        KeyCode::Down | KeyCode::Char('j') => Some(Action::MoveSelection(1)),
        KeyCode::Up | KeyCode::Char('k') => Some(Action::MoveSelection(-1)),
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
    fn keys_arrive_raw_and_map_through_key_action() {
        // action_for defers mapping so text inputs can own the keyboard.
        assert!(matches!(
            action_for(key(KeyCode::Down)),
            Some(Action::RawKey(_))
        ));
        let map = |code| key_action(KeyEvent::new(code, KeyModifiers::NONE));
        assert!(matches!(map(KeyCode::Down), Some(Action::MoveSelection(1))));
        assert!(matches!(
            map(KeyCode::Char('j')),
            Some(Action::MoveSelection(1))
        ));
        assert!(matches!(
            map(KeyCode::Char('2')),
            Some(Action::SwitchView(View::Library))
        ));
        assert!(matches!(map(KeyCode::Char('z')), Some(Action::ToggleZen)));
    }
}
