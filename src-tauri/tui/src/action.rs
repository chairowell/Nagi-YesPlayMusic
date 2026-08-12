//! Every input becomes an Action; the reducer is the only place state changes.

use std::time::Duration;

use crate::api::{ResolvedTrack, SongRow, Source};
use crate::pixel::PixelCover;
use crate::player::PlayerEvent;

/// Idle-dashboard menu entries; resolved to Actions at the input layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuEntry {
    Library,
    Search,
    Login,
    Quit,
}

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
    SelectIndex(usize),
    /// vim `g` prefix: two in a row jump to the list top.
    GKey,
    CycleMode,
    ToggleLike,
    SetVolumeTo(f32),
    OpenSource(usize),
    JumpBottom,
    ConfirmYes,
    Mouse(crossterm::event::MouseEvent),
    RawKey(crossterm::event::KeyEvent),
    Paste(String),
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
    CoverBytes {
        generation: u64,
        bytes: Vec<u8>,
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
        uid: i64,
        nickname: String,
    },
    LibraryLoaded {
        source: Source,
        rows: Vec<SongRow>,
    },
    FmMore {
        rows: Vec<SongRow>,
    },
    PrefetchReady {
        index: usize,
        track: ResolvedTrack,
    },
    SearchResults {
        seq: u64,
        rows: Vec<SongRow>,
    },
    LikedIds {
        ids: std::collections::HashSet<i64>,
    },
    Notice {
        message: String,
    },
    LyricsLoaded {
        generation: u64,
        lines: Vec<crate::lyrics::LyricLine>,
    },
}

pub const SEEK_STEP: Duration = Duration::from_secs(5);
