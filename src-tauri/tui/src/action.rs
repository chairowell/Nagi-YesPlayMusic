//! Every input becomes an Action; the reducer is the only place state changes.

use std::time::Duration;

use crate::api::{ResolvedTrack, SongRow};
use crate::pixel::PixelCover;
use crate::player::PlayerEvent;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    NowPlaying,
    Library,
    Search,
    Queue,
    Login,
}

#[derive(Debug)]
pub enum Action {
    Quit,
    SwitchView(View),
    Back,
    ToggleZen,
    TogglePlay,
    NextTrack,
    PrevTrack,
    SeekBy(i64),
    VolumeBy(f32),
    MoveSelection(i32),
    Activate,
    Resize,
    Player(PlayerEvent),
    TrackResolved {
        generation: u64,
        track: ResolvedTrack,
    },
    ResolveFailed {
        generation: u64,
        message: String,
    },
    CoverLoaded {
        generation: u64,
        cover: PixelCover,
    },
    StartLogin,
    LoginQrReady {
        art: String,
    },
    LoginProgress {
        message: String,
    },
    LoginFailed {
        message: String,
    },
    LoggedIn {
        nickname: String,
    },
    LibraryLoaded {
        rows: Vec<SongRow>,
    },
    Notice {
        message: String,
    },
}

pub const SEEK_STEP: Duration = Duration::from_secs(5);
