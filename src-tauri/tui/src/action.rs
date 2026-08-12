//! Every input becomes an Action; the reducer is the only place state changes.

use std::time::Duration;

use crate::player::PlayerEvent;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    NowPlaying,
    Library,
    Search,
    Queue,
}

#[derive(Debug)]
pub enum Action {
    Quit,
    SwitchView(View),
    Back,
    ToggleZen,
    TogglePlay,
    SeekBy(i64),
    VolumeBy(f32),
    MoveSelection(i32),
    Activate,
    Resize,
    Player(PlayerEvent),
}

pub const SEEK_STEP: Duration = Duration::from_secs(5);
