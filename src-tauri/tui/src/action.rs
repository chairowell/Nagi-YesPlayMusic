//! Every input becomes an Action; the reducer is the only place state changes.

use std::time::Duration;

use yesplaymusic_core::auth::Session;
use yesplaymusic_core::cache::CacheLease;

use crate::api::{ResolvedTrack, SongRow, Source};
use crate::pixel::PixelCover;
use crate::player::PlayerEvent;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CoverRenderRequest {
    pub generation: u64,
    pub cells: (u16, u16),
}

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SessionStamp {
    pub epoch: u64,
    pub uid: i64,
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
    ToggleHelp,
    ToggleLike,
    SetVolumeTo(f32),
    OpenSource(usize),
    JumpBottom,
    ConfirmYes,
    Mouse(crossterm::event::MouseEvent),
    RawKey(crossterm::event::KeyEvent),
    Paste(String),
    Resize {
        cols: u16,
        rows: u16,
    },
    Player(PlayerEvent),
    TrackResolved {
        generation: u64,
        track: ResolvedTrack,
    },
    RowCacheReady {
        generation: u64,
        row: SongRow,
        lease: Option<CacheLease>,
    },
    ResolvedCacheReady {
        generation: u64,
        track: ResolvedTrack,
        lease: Option<CacheLease>,
    },
    CacheFallbackResolved {
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
        request: CoverRenderRequest,
        cover: PixelCover,
    },
    CoverDecoded {
        generation: u64,
        image: image::DynamicImage,
    },
    IdleArtBytes {
        bytes: Vec<u8>,
    },
    IdleArtLoaded {
        cells: (u16, u16),
        cover: PixelCover,
    },
    StartLogin,
    LoginQrReady {
        attempt: u64,
        art: String,
    },
    LoginProgress {
        attempt: u64,
        message: String,
    },
    LoginFailed {
        attempt: u64,
        message: String,
    },
    LoginSucceeded {
        attempt: u64,
        session: Session,
        uid: i64,
        nickname: String,
    },
    SessionRestored {
        epoch: u64,
        uid: i64,
        nickname: String,
    },
    SessionRestoreFailed {
        epoch: u64,
        message: String,
    },
    LibraryLoaded {
        session: SessionStamp,
        source: Source,
        rows: Vec<SongRow>,
    },
    FmMore {
        session: SessionStamp,
        rows: Vec<SongRow>,
    },
    FmLoadFailed {
        session: SessionStamp,
        message: String,
    },
    PrefetchReady {
        index: usize,
        track: ResolvedTrack,
    },
    SearchResults {
        seq: u64,
        query: String,
        rows: Vec<SongRow>,
    },
    SearchFailed {
        seq: u64,
        query: String,
        message: String,
    },
    LikedIds {
        session: SessionStamp,
        ids: std::collections::HashSet<i64>,
    },
    LikeFinished {
        session: SessionStamp,
        id: i64,
        mutation: u64,
        attempted_like: bool,
        error: Option<String>,
    },
    PersonalNotice {
        session: SessionStamp,
        message: String,
    },
    LyricsLoaded {
        generation: u64,
        lines: Vec<crate::lyrics::LyricLine>,
    },
}

pub const SEEK_STEP: Duration = Duration::from_secs(5);
