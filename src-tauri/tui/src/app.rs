//! Single state source: input becomes Action, update() is the only writer,
//! ui::draw() only reads.

use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use futures_util::StreamExt;
use tokio::sync::mpsc;

use crate::action::{Action, View, SEEK_STEP};
use crate::api::{self, Ncm, QrStatus, SongRow, Source};
use crate::config::{self, Config};
use crate::event;
use crate::i18n::{self, Key};
use crate::pixel::{self, PixelCover};
use crate::player::{self, PlayerCommand, PlayerEvent, PlayerHandle};
use crate::theme::Theme;
use crate::ui;

/// Side-effect handles the reducer may use; state itself stays plain data.
pub struct Effects {
    pub player: PlayerHandle,
    pub ncm: Arc<Ncm>,
    pub store: Arc<crate::store::LibraryStore>,
    pub actions: mpsc::UnboundedSender<Action>,
}

const COVER_SOURCE_EDGE: u32 = 500;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayMode {
    Sequential,
    Shuffle,
    RepeatOne,
}

impl PlayMode {
    fn next(self) -> Self {
        match self {
            PlayMode::Sequential => PlayMode::Shuffle,
            PlayMode::Shuffle => PlayMode::RepeatOne,
            PlayMode::RepeatOne => PlayMode::Sequential,
        }
    }

    /// Progress-row glyphs (the artifact mockup style), not words.
    pub fn icon(self) -> &'static str {
        match self {
            PlayMode::Sequential => "»",
            PlayMode::Shuffle => "⇆",
            PlayMode::RepeatOne => "↺¹",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayLayout {
    /// Cover fills the height, lyrics beside it.
    Side,
    /// Cover centered on top, lyrics below.
    Stacked,
}

impl PlayLayout {
    fn from_config(value: &str) -> Self {
        match value {
            "stacked" => Self::Stacked,
            _ => Self::Side,
        }
    }
}

pub struct NowPlaying {
    pub title: String,
    pub artist: String,
    pub album: String,
}

pub struct AppState {
    pub view: View,
    pub zen: bool,
    pub theme: Theme,
    pub library: Vec<SongRow>,
    pub selected: usize,
    pub queue: Vec<SongRow>,
    pub queue_pos: Option<usize>,
    queue_source: Source,
    pub play_mode: PlayMode,
    pub liked: std::collections::HashSet<i64>,
    pub current_track_id: Option<i64>,
    pub library_source: Source,
    pub sidebar_focus: bool,
    pub sidebar_selected: usize,
    pending_fm_next: bool,
    cover_prefetched: bool,
    /// Next queue item resolved ahead of time — track switches feel instant.
    prefetched: Option<(usize, api::ResolvedTrack)>,
    enter_replaces_queue: bool,
    pub nickname: Option<String>,
    uid: Option<i64>,
    pub search_query: String,
    pub search_results: Vec<SongRow>,
    pub search_input: bool,
    pub searching: bool,
    search_seq: u64,
    pub login_qr: Option<String>,
    pub login_message: Option<String>,
    pub now: Option<NowPlaying>,
    pub cover: Option<PixelCover>,
    pub layout: PlayLayout,
    pub thick_progress: bool,
    pixel_scale: f32,
    pub idle_art: PixelCover,
    pub library_synced: bool,
    /// Idle art pre-rendered at the playing-cover size, so the loading
    /// placeholder swaps to the real cover without a size jump.
    pub placeholder: Option<PixelCover>,
    /// Source image for the idle art; None = procedural vinyl.
    idle_bytes: Option<Vec<u8>>,
    cover_bytes: Option<Vec<u8>>,
    pub lyrics: Vec<crate::lyrics::LyricLine>,
    pub position: Duration,
    pub duration: Option<Duration>,
    pub paused: bool,
    pub volume: f32,
    pub status: Option<String>,
    pub generation: u64,
    pub confirm_quit: bool,
    pub show_help: bool,
    pending_g: bool,
    pending_auto_next: bool,
    should_quit: bool,
}

impl AppState {
    pub fn new(config: &Config) -> Self {
        Self {
            view: View::NowPlaying,
            zen: false,
            theme: Theme::by_name(&config.theme),
            // Demo rows for the logged-out state; replaced by 我喜欢的音乐
            // right after login (id 0 = resolve via search).
            library: vec![
                SongRow {
                    id: 0,
                    title: "反方向的钟".into(),
                    artist: String::new(),
                    duration_ms: 0,
                    pic_url: None,
                },
                SongRow {
                    id: 0,
                    title: "海阔天空".into(),
                    artist: "Beyond".into(),
                    duration_ms: 0,
                    pic_url: None,
                },
            ],
            selected: 0,
            queue: Vec::new(),
            queue_pos: None,
            queue_source: Source::Liked,
            play_mode: PlayMode::Sequential,
            liked: std::collections::HashSet::new(),
            current_track_id: None,
            library_source: Source::Liked,
            sidebar_focus: false,
            sidebar_selected: 0,
            pending_fm_next: false,
            cover_prefetched: false,
            prefetched: None,
            enter_replaces_queue: config.enter_replaces_queue,
            nickname: None,
            uid: None,
            search_query: String::new(),
            search_results: Vec::new(),
            search_input: true,
            searching: false,
            search_seq: 0,
            login_qr: None,
            login_message: None,
            now: None,
            cover: None,
            layout: PlayLayout::from_config(&config.layout),
            thick_progress: config.progress_style == "bar",
            pixel_scale: config.pixel_scale.clamp(0.5, 2.0),
            idle_art: render_idle_art(
                idle_art_bytes(config).as_deref(),
                Theme::by_name(&config.theme).palette,
                scale_cells(desired_idle_cells(), config.pixel_scale.clamp(0.5, 2.0)),
            ),
            library_synced: false,
            placeholder: None,
            idle_bytes: idle_art_bytes(config),
            cover_bytes: None,
            lyrics: Vec::new(),
            position: Duration::ZERO,
            duration: None,
            paused: false,
            volume: 1.0,
            status: None,
            generation: 0,
            confirm_quit: false,
            show_help: false,
            pending_g: false,
            pending_auto_next: false,
            should_quit: false,
        }
    }

    /// The cover cell grid that fits the current terminal and layout.
    /// Height-driven in Side layout, width-bounded in Stacked.
    fn desired_cover_cells(&self) -> (u16, u16) {
        let (cols, rows) = crossterm::terminal::size().unwrap_or((80, 24));
        let body_rows = rows.saturating_sub(if self.zen { 2 } else { 4 });
        let height = match self.layout {
            PlayLayout::Side => body_rows.saturating_sub(2),
            PlayLayout::Stacked => body_rows / 2,
        };
        let height = height.clamp(8, 40);
        let width = (height * 2).min(match self.layout {
            PlayLayout::Side => cols.saturating_sub(30).max(16),
            PlayLayout::Stacked => cols.saturating_sub(4).max(16),
        });
        scale_cells((width, width / 2), self.pixel_scale)
    }

    fn ensure_placeholder(&mut self) {
        let desired = self.desired_cover_cells();
        let current = self.placeholder.as_ref().map(|art| (art.width, art.height));
        if current != Some(desired) {
            self.placeholder = Some(render_idle_art(
                self.idle_bytes.as_deref(),
                self.theme.palette,
                desired,
            ));
        }
    }

    fn handle_search_key(&mut self, fx: &Effects, key: crossterm::event::KeyEvent) {
        use crossterm::event::{KeyCode, KeyModifiers};
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            if key.code == KeyCode::Char('c') {
                self.confirm_quit = true;
            }
            return;
        }
        match key.code {
            KeyCode::Char(c) => self.search_query.push(c),
            KeyCode::Backspace => {
                self.search_query.pop();
            }
            KeyCode::Enter => {
                let query = self.search_query.trim().to_owned();
                if !query.is_empty() {
                    self.searching = true;
                    self.search_seq += 1;
                    spawn_search(fx, self.search_seq, query);
                }
            }
            KeyCode::Esc => {
                if self.search_query.is_empty() {
                    self.view = View::NowPlaying;
                } else {
                    self.search_query.clear();
                }
            }
            KeyCode::Down | KeyCode::Tab if !self.search_results.is_empty() => {
                self.search_input = false;
                self.selected = 0;
            }
            _ => {}
        }
    }

    fn apply_resolved(&mut self, fx: &Effects, generation: u64, track: api::ResolvedTrack) {
        self.current_track_id = Some(track.id);
        self.now = Some(NowPlaying {
            title: track.title.clone(),
            artist: track.artist.clone(),
            album: String::new(),
        });
        self.duration =
            (track.duration_ms > 0).then(|| Duration::from_millis(track.duration_ms as u64));
        self.status = Some(i18n::t_playing(&track.kind));
        fx.player.send(PlayerCommand::PlayUrl {
            generation,
            url: track.url.clone(),
        });
        if !self.cover_prefetched {
            if let Some(pic_url) = track.pic_url.clone() {
                spawn_cover_fetch(fx, generation, pic_url);
            }
        }
        spawn_fetch_lyrics(fx, generation, track.id);
        self.prefetch_next(fx);
    }

    /// Resolve the sequential next queue item ahead of time (shuffle is
    /// unpredictable, so it opts out).
    fn prefetch_next(&mut self, fx: &Effects) {
        if self.play_mode == PlayMode::Shuffle {
            return;
        }
        let Some(position) = self.queue_pos else {
            return;
        };
        let next = position + 1;
        if self.prefetched.as_ref().is_some_and(|(i, _)| *i == next) {
            return;
        }
        if let Some(row) = self.queue.get(next).cloned() {
            if row.id > 0 {
                spawn_prefetch(fx, next, row);
            }
        }
    }

    /// Reset the now-playing surface and kick off resolution for a row.
    fn play_row(&mut self, fx: &Effects, row: SongRow) {
        self.ensure_placeholder();
        self.generation += 1;
        self.now = Some(NowPlaying {
            title: row.title.clone(),
            artist: row.artist.clone(),
            album: String::new(),
        });
        // Keep the previous cover on screen until the new one lands —
        // no placeholder flash between tracks.
        self.lyrics.clear();
        self.position = Duration::ZERO;
        self.duration = None;
        self.status = Some(i18n::t(Key::Resolving).into());
        // Cover art is independent of URL resolution: start fetching now
        // (queue rows carry pic_url) instead of waiting for TrackResolved.
        self.cover_prefetched = row.pic_url.is_some();
        if let Some(pic_url) = row.pic_url.clone() {
            spawn_cover_fetch(fx, self.generation, pic_url);
        }
        // Prefetched? Play instantly, skip the resolve round-trip.
        if let Some((_, track)) = self
            .prefetched
            .take_if(|(_, track)| track.id == row.id && row.id > 0)
        {
            let generation = self.generation;
            self.apply_resolved(fx, generation, track);
            return;
        }
        spawn_resolve(fx, self.generation, row);
    }

    pub fn source_index(&self) -> usize {
        match self.library_source {
            Source::Liked => 0,
            Source::Daily => 1,
            Source::Fm => 2,
            Source::Cloud | Source::Search => 3,
        }
    }

    fn open_source(&mut self, fx: &Effects, index: usize) {
        let source = match index {
            1 => Source::Daily,
            2 => Source::Fm,
            3 => Source::Cloud,
            _ => Source::Liked,
        };
        self.library_source = source;
        self.sidebar_selected = index;
        self.sidebar_focus = false;
        self.selected = 0;
        self.library_synced = false;
        self.library = self
            .uid
            .zip(cache_name(source))
            .and_then(|(uid, name)| fx.store.load(uid, name))
            .map(|rows| rows.into_iter().map(|row| row.into_song_row()).collect())
            .unwrap_or_default();
        spawn_fetch_source(fx, source);
    }

    /// Move within the queue (manual n/p and end-of-track auto-advance).
    fn step_queue(&mut self, fx: &Effects, delta: i32, auto: bool) {
        let Some(position) = self.queue_pos else {
            return;
        };
        if self.queue.is_empty() {
            return;
        }
        if auto && self.play_mode == PlayMode::RepeatOne {
            if let Some(row) = self.queue.get(position).cloned() {
                self.play_row(fx, row);
            }
            return;
        }
        let next = if self.play_mode == PlayMode::Shuffle && self.queue.len() > 1 {
            random_index(self.queue.len(), position) as i32
        } else {
            position as i32 + delta
        };
        if next < 0 {
            return;
        }
        match self.queue.get(next as usize).cloned() {
            Some(row) => {
                self.queue_pos = Some(next as usize);
                self.play_row(fx, row);
            }
            None if self.queue_source == Source::Fm && delta > 0 => {
                // FM is an endless stream: pull the next batch, then play.
                self.pending_fm_next = true;
                spawn_fm_more(fx);
            }
            None => self.status = Some(i18n::t(Key::QueueFinished).into()),
        }
    }

    fn update(&mut self, action: Action, fx: &Effects) {
        // The quit-confirm dialog is modal: y/Enter/q confirm, n/Esc cancel.
        if self.confirm_quit {
            match action {
                Action::ConfirmYes | Action::Quit | Action::Activate => self.should_quit = true,
                Action::Back | Action::NextTrack => self.confirm_quit = false,
                Action::Player(event) => self.apply_player_event(event),
                _ => {}
            }
            return;
        }
        // The help overlay is modal: any key dismisses it.
        if self.show_help && matches!(action, Action::RawKey(_) | Action::Mouse(_)) {
            self.show_help = false;
            return;
        }
        // Text-input mode: the search box owns the keyboard.
        if let Action::RawKey(key) = &action {
            if self.view == View::Search && self.search_input && !self.confirm_quit {
                self.handle_search_key(fx, *key);
                return;
            }
            let Some(mapped) = event::key_action(*key) else {
                return;
            };
            self.update(mapped, fx);
            return;
        }
        if let Action::Paste(text) = &action {
            if self.view == View::Search && self.search_input {
                self.search_query.push_str(&text.replace(['\n', '\r'], " "));
            }
            return;
        }
        // vim gg: a second bare `g` right after the first jumps to the top.
        let was_pending_g = self.pending_g;
        self.pending_g = false;
        match action {
            Action::GKey => {
                if was_pending_g {
                    self.selected = 0;
                } else {
                    self.pending_g = true;
                }
            }
            Action::JumpBottom => {
                let len = match self.view {
                    View::Library => self.library.len(),
                    View::Queue => self.queue.len(),
                    _ => 0,
                };
                self.selected = len.saturating_sub(1);
            }
            Action::ConfirmYes => {}
            Action::Quit => self.confirm_quit = true,
            Action::SwitchView(view) => {
                self.view = view;
                if view == View::Search {
                    self.search_input = true;
                }
            }
            Action::Back => {
                if self.view == View::Search && !self.search_input {
                    self.search_input = true;
                } else if self.view == View::Library && !self.sidebar_focus {
                    self.sidebar_focus = true;
                    self.sidebar_selected = self.source_index();
                } else {
                    self.sidebar_focus = false;
                    self.view = View::NowPlaying;
                }
            }
            Action::ToggleZen => {
                self.zen = !self.zen;
                if self.zen {
                    self.view = View::NowPlaying;
                }
            }
            Action::TogglePlay => fx.player.send(PlayerCommand::TogglePause),
            Action::SeekBy(sign) => {
                let target = if sign >= 0 {
                    self.position.saturating_add(SEEK_STEP)
                } else {
                    self.position.saturating_sub(SEEK_STEP)
                };
                fx.player.send(PlayerCommand::SeekTo(target));
            }
            Action::VolumeBy(delta) => {
                self.volume = (self.volume + delta).clamp(0.0, 1.5);
                fx.player.send(PlayerCommand::SetVolume(self.volume));
            }
            Action::MoveSelection(delta) => {
                if self.view == View::Library && self.sidebar_focus {
                    let next = (self.sidebar_selected as i32 + delta.signum()).clamp(0, 3);
                    self.sidebar_selected = next as usize;
                    return;
                }
                let len = match self.view {
                    View::Library => self.library.len(),
                    View::Queue => self.queue.len(),
                    View::Search => self.search_results.len(),
                    _ => 0,
                };
                if len > 0 {
                    let last = len as i32 - 1;
                    let next = (self.selected as i32 + delta).clamp(0, last);
                    self.selected = next as usize;
                }
            }
            Action::Activate => match self.view {
                View::Library if self.sidebar_focus => {
                    self.open_source(fx, self.sidebar_selected);
                }
                View::Library => {
                    if let Some(row) = self.library.get(self.selected).cloned() {
                        if self.enter_replaces_queue {
                            // Desktop/NCM semantics: the list becomes the
                            // listening context from this song onward.
                            self.queue = self.library.clone();
                            self.queue_pos = Some(self.selected);
                        } else {
                            self.queue = vec![row.clone()];
                            self.queue_pos = Some(0);
                        }
                        self.queue_source = self.library_source;
                        self.play_row(fx, row);
                        self.view = View::NowPlaying;
                    }
                }
                View::Queue => {
                    if let Some(row) = self.queue.get(self.selected).cloned() {
                        self.queue_pos = Some(self.selected);
                        self.play_row(fx, row);
                        self.view = View::NowPlaying;
                    }
                }
                View::Search if !self.search_input => {
                    if let Some(row) = self.search_results.get(self.selected).cloned() {
                        self.queue = self.search_results.clone();
                        self.queue_pos = Some(self.selected);
                        self.queue_source = Source::Search;
                        self.play_row(fx, row);
                        self.view = View::NowPlaying;
                    }
                }
                _ => {}
            },
            Action::NextTrack => self.step_queue(fx, 1, false),
            Action::PrevTrack => self.step_queue(fx, -1, false),
            Action::ToggleHelp => self.show_help = true,
            Action::CycleMode => {
                self.play_mode = self.play_mode.next();
            }
            Action::SetVolumeTo(ratio) => {
                self.volume = ratio.clamp(0.0, 1.0);
                fx.player.send(PlayerCommand::SetVolume(self.volume));
            }
            Action::ToggleLike => {
                if let Some(id) = self.current_track_id {
                    let like = !self.liked.contains(&id);
                    if like {
                        self.liked.insert(id);
                    } else {
                        self.liked.remove(&id);
                    }
                    self.status = Some(
                        crate::i18n::t(if like {
                            crate::i18n::Key::Liked
                        } else {
                            crate::i18n::Key::Unliked
                        })
                        .to_owned(),
                    );
                    spawn_toggle_like(fx, id, like);
                }
            }
            Action::OpenSource(index) => self.open_source(fx, index),
            Action::LikedIds { ids } => self.liked = ids,
            Action::FmMore { rows } => {
                if self.queue_source == Source::Fm {
                    self.queue.extend(rows.iter().cloned());
                }
                if self.library_source == Source::Fm {
                    self.library.extend(rows.iter().cloned());
                    self.library_synced = true;
                }
                if self.pending_fm_next {
                    self.pending_fm_next = false;
                    self.step_queue(fx, 1, true);
                }
            }
            Action::SelectIndex(index) => {
                let len = match self.view {
                    View::Library => self.library.len(),
                    View::Queue => self.queue.len(),
                    _ => 0,
                };
                if index < len {
                    self.selected = index;
                }
            }
            Action::Mouse(_) => {} // resolved against Hits in the event loop
            Action::RawKey(_) | Action::Paste(_) => {} // handled before this match
            Action::StartLogin => {
                if self.nickname.is_some() {
                    self.status = Some(i18n::t(Key::AlreadyLoggedIn).into());
                } else {
                    self.view = View::Login;
                    self.login_qr = None;
                    self.login_message = Some(i18n::t(Key::FetchingQr).into());
                    spawn_login(fx);
                }
            }
            Action::LoginQrReady { art } => {
                if self.view == View::Login {
                    self.login_qr = Some(art);
                    self.login_message = Some(i18n::t(Key::ScanQr).into());
                }
            }
            Action::LoginProgress { message } | Action::LoginFailed { message } => {
                if self.view == View::Login {
                    self.login_message = Some(message);
                }
            }
            Action::LoggedIn { uid, nickname } => {
                self.uid = Some(uid);
                self.nickname = Some(nickname.clone());
                self.status = Some(i18n::t_welcome(&nickname));
                if self.view == View::Login {
                    self.view = View::Library;
                }
                self.login_qr = None;
                // Demo rows are a logged-out affordance; a real library is
                // on its way now.
                self.selected = 0;
                self.library_synced = false;
                // Snapshot first: the last known list paints instantly,
                // the fresh fetch swaps it in when it lands.
                self.library = fx
                    .store
                    .load(uid, "liked")
                    .map(|rows| rows.into_iter().map(|row| row.into_song_row()).collect())
                    .unwrap_or_default();
                spawn_fetch_library(fx, uid);
            }
            Action::LibraryLoaded { source, rows } => {
                if let (Some(uid), Some(name)) = (self.uid, cache_name(source)) {
                    spawn_save_snapshot(fx, uid, name, rows.clone());
                }
                if source == self.library_source {
                    if source == Source::Liked {
                        self.status = Some(i18n::t_liked_songs_count(rows.len()));
                    }
                    self.library = rows;
                    self.selected = 0;
                    self.library_synced = true;
                }
            }
            Action::SearchResults { seq, rows } => {
                if seq == self.search_seq {
                    self.searching = false;
                    self.search_results = rows;
                    if !self.search_results.is_empty() {
                        self.search_input = false;
                        self.selected = 0;
                    }
                }
            }
            Action::Notice { message } => self.status = Some(message),
            Action::LyricsLoaded { generation, lines } => {
                if generation == self.generation {
                    self.lyrics = lines;
                }
            }
            Action::TrackResolved { generation, track } => {
                if generation == self.generation {
                    self.apply_resolved(fx, generation, track);
                }
            }
            Action::PrefetchReady { index, track } => {
                // Guard against a rebuilt queue: only keep it if the row
                // at that index is still the same song.
                if self.queue.get(index).is_some_and(|row| row.id == track.id) {
                    self.prefetched = Some((index, track));
                }
            }
            Action::ResolveFailed {
                generation,
                message,
            } => {
                if generation == self.generation {
                    self.status = Some(message);
                }
            }
            Action::CoverBytes { generation, bytes } => {
                if generation == self.generation {
                    self.cover_bytes = Some(bytes.clone());
                    spawn_render_cover(
                        fx,
                        generation,
                        bytes,
                        self.theme.palette,
                        self.desired_cover_cells(),
                    );
                }
            }
            Action::CoverLoaded { generation, cover } => {
                if generation == self.generation {
                    self.cover = Some(cover);
                }
            }
            Action::Player(event) => {
                self.apply_player_event(event);
                if self.pending_auto_next {
                    self.pending_auto_next = false;
                    self.step_queue(fx, 1, true);
                }
            }
            Action::Resize => {
                // Layout-dependent resolution: re-render cover and idle art
                // from their kept source bytes when the desired grid changed.
                if let (Some(bytes), Some(cover)) = (&self.cover_bytes, &self.cover) {
                    let desired = self.desired_cover_cells();
                    if (cover.width, cover.height) != desired {
                        spawn_render_cover(
                            fx,
                            self.generation,
                            bytes.clone(),
                            self.theme.palette,
                            desired,
                        );
                    }
                }
                let desired = scale_cells(desired_idle_cells(), self.pixel_scale);
                if (self.idle_art.width, self.idle_art.height) != desired {
                    self.idle_art =
                        render_idle_art(self.idle_bytes.as_deref(), self.theme.palette, desired);
                }
                if self.now.is_some() {
                    self.ensure_placeholder();
                }
            }
        }
    }

    fn apply_player_event(&mut self, event: PlayerEvent) {
        match event {
            PlayerEvent::Started { generation, total } => {
                if generation == self.generation {
                    self.position = Duration::ZERO;
                    self.duration = total;
                    self.paused = false;
                }
            }
            PlayerEvent::Position {
                generation,
                position,
            } => {
                if generation == self.generation {
                    self.position = position;
                }
            }
            PlayerEvent::Paused(paused) => self.paused = paused,
            PlayerEvent::Ended { generation } => {
                if generation == self.generation {
                    self.pending_auto_next = true;
                }
            }
            PlayerEvent::Failed {
                generation,
                message,
            } => {
                if generation == self.generation {
                    self.status = Some(message);
                }
            }
        }
    }
}

fn spawn_resolve(fx: &Effects, generation: u64, row: SongRow) {
    let ncm = fx.ncm.clone();
    let actions = fx.actions.clone();
    tokio::spawn(async move {
        let resolved = if row.id > 0 {
            ncm.resolve_by_id(&row).await
        } else {
            ncm.resolve_for_play(&row.title, &row.artist).await
        };
        let action = match resolved {
            Ok(track) => Action::TrackResolved { generation, track },
            Err(error) => Action::ResolveFailed {
                generation,
                message: error.to_string(),
            },
        };
        let _ = actions.send(action);
    });
}

fn spawn_login(fx: &Effects) {
    let ncm = fx.ncm.clone();
    let actions = fx.actions.clone();
    tokio::spawn(async move {
        let key = match ncm.qr_key().await {
            Ok(key) => key,
            Err(error) => {
                let _ = actions.send(Action::LoginFailed {
                    message: error.to_string(),
                });
                return;
            }
        };
        let art = match api::qr_unicode(&Ncm::qr_login_url(&key)) {
            Ok(art) => art,
            Err(error) => {
                let _ = actions.send(Action::LoginFailed {
                    message: error.to_string(),
                });
                return;
            }
        };
        if actions.send(Action::LoginQrReady { art }).is_err() {
            return;
        }
        // One flaky poll must not orphan the QR key — the phone-side scan
        // would then report "invalid" against a dead session. Tolerate
        // transient errors and only give up after several in a row.
        let mut consecutive_errors = 0_u32;
        loop {
            tokio::time::sleep(Duration::from_secs(2)).await;
            match ncm.qr_check(&key).await {
                Ok(QrStatus::Waiting) => consecutive_errors = 0,
                Ok(QrStatus::Scanned) => {
                    consecutive_errors = 0;
                    if actions
                        .send(Action::LoginProgress {
                            message: i18n::t(Key::QrScannedConfirm).into(),
                        })
                        .is_err()
                    {
                        return;
                    }
                }
                Ok(QrStatus::Expired) => {
                    let _ = actions.send(Action::LoginFailed {
                        message: i18n::t(Key::QrExpired).into(),
                    });
                    return;
                }
                Ok(QrStatus::Success) => {
                    let (uid, nickname) = ncm.account().await.unwrap_or((0, String::new()));
                    let _ = actions.send(Action::LoggedIn { uid, nickname });
                    return;
                }
                Err(error) => {
                    consecutive_errors += 1;
                    if consecutive_errors >= 3 {
                        let _ = actions.send(Action::LoginFailed {
                            message: i18n::t_login_interrupted(error),
                        });
                        return;
                    }
                    if actions
                        .send(Action::LoginProgress {
                            message: i18n::t(Key::NetworkRetrying).into(),
                        })
                        .is_err()
                    {
                        return;
                    }
                }
            }
        }
    });
}

fn spawn_fetch_lyrics(fx: &Effects, generation: u64, song_id: i64) {
    let ncm = fx.ncm.clone();
    let actions = fx.actions.clone();
    tokio::spawn(async move {
        let Ok((lrc, tlyric)) = ncm.lyrics(song_id).await else {
            return; // missing lyrics are cosmetic
        };
        let lines = crate::lyrics::parse_lrc(&lrc, tlyric.as_deref());
        if !lines.is_empty() {
            let _ = actions.send(Action::LyricsLoaded { generation, lines });
        }
    });
}

fn spawn_fetch_library(fx: &Effects, uid: i64) {
    let ncm = fx.ncm.clone();
    let actions = fx.actions.clone();
    tokio::spawn(async move {
        let result = ncm.liked_songs(uid).await;
        let action = match result {
            Ok(rows) => Action::LibraryLoaded {
                source: Source::Liked,
                rows,
            },
            Err(error) => Action::Notice {
                message: i18n::t_library_load_failed(error),
            },
        };
        let _ = actions.send(action);
    });
    let ncm = fx.ncm.clone();
    let actions = fx.actions.clone();
    tokio::spawn(async move {
        if let Ok(ids) = ncm.liked_ids(uid).await {
            let _ = actions.send(Action::LikedIds { ids });
        }
    });
}

fn spawn_fetch_source(fx: &Effects, source: Source) {
    if source == Source::Liked {
        // Liked needs the uid; reuse the account-carrying path.
        let ncm = fx.ncm.clone();
        let actions = fx.actions.clone();
        tokio::spawn(async move {
            let result = async {
                let (uid, _) = ncm.account().await?;
                ncm.liked_songs(uid).await
            }
            .await;
            let action = match result {
                Ok(rows) => Action::LibraryLoaded {
                    source: Source::Liked,
                    rows,
                },
                Err(error) => Action::Notice {
                    message: i18n::t_library_load_failed(error),
                },
            };
            let _ = actions.send(action);
        });
        return;
    }
    let ncm = fx.ncm.clone();
    let actions = fx.actions.clone();
    tokio::spawn(async move {
        let result = match source {
            Source::Daily => ncm.daily_songs().await,
            Source::Fm => ncm.personal_fm().await,
            Source::Cloud => ncm.cloud_songs().await,
            Source::Liked | Source::Search => unreachable!("handled above"),
        };
        let action = match result {
            Ok(rows) => Action::LibraryLoaded { source, rows },
            Err(error) => Action::Notice {
                message: i18n::t_library_load_failed(error),
            },
        };
        let _ = actions.send(action);
    });
}

fn spawn_search(fx: &Effects, seq: u64, query: String) {
    let ncm = fx.ncm.clone();
    let actions = fx.actions.clone();
    tokio::spawn(async move {
        let rows = ncm.search_rows(&query, 30).await.unwrap_or_default();
        let _ = actions.send(Action::SearchResults { seq, rows });
    });
}

fn spawn_prefetch(fx: &Effects, index: usize, row: SongRow) {
    let ncm = fx.ncm.clone();
    let actions = fx.actions.clone();
    tokio::spawn(async move {
        if let Ok(track) = ncm.resolve_by_id(&row).await {
            let _ = actions.send(Action::PrefetchReady { index, track });
        }
    });
}

/// FM is a live stream and search is transient; the rest snapshot to disk.
fn cache_name(source: Source) -> Option<&'static str> {
    match source {
        Source::Liked => Some("liked"),
        Source::Daily => Some("daily"),
        Source::Cloud => Some("cloud"),
        Source::Fm | Source::Search => None,
    }
}

fn spawn_save_snapshot(fx: &Effects, uid: i64, source: &'static str, rows: Vec<SongRow>) {
    let store = fx.store.clone();
    tokio::spawn(async move {
        let stored: Vec<crate::store::StoredSong> =
            rows.iter().map(crate::store::StoredSong::from).collect();
        let _ = tokio::task::spawn_blocking(move || store.save(uid, source, &stored)).await;
    });
}

fn spawn_fm_more(fx: &Effects) {
    let ncm = fx.ncm.clone();
    let actions = fx.actions.clone();
    tokio::spawn(async move {
        if let Ok(rows) = ncm.personal_fm().await {
            let _ = actions.send(Action::FmMore { rows });
        }
    });
}

fn spawn_toggle_like(fx: &Effects, id: i64, like: bool) {
    let ncm = fx.ncm.clone();
    let actions = fx.actions.clone();
    tokio::spawn(async move {
        if let Err(error) = ncm.set_like(id, like).await {
            let _ = actions.send(Action::Notice {
                message: error.to_string(),
            });
        }
    });
}

/// Cheap non-repeating pick for shuffle; nanos beat rand-crate weight here.
fn random_index(len: usize, current: usize) -> usize {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as usize)
        .unwrap_or(0);
    let mut pick = nanos % len;
    if pick == current {
        pick = (pick + 1) % len;
    }
    pick
}

fn spawn_cover_fetch(fx: &Effects, generation: u64, pic_url: String) {
    let actions = fx.actions.clone();
    tokio::spawn(async move {
        let Ok(bytes) = api::fetch_cover(&pic_url, COVER_SOURCE_EDGE).await else {
            return; // a missing cover is cosmetic; the idle art stays
        };
        let _ = actions.send(Action::CoverBytes { generation, bytes });
    });
}

fn spawn_render_cover(
    fx: &Effects,
    generation: u64,
    bytes: Vec<u8>,
    palette: &'static [(u8, u8, u8)],
    cells: (u16, u16),
) {
    let actions = fx.actions.clone();
    tokio::spawn(async move {
        let cover = tokio::task::spawn_blocking(move || {
            pixel::from_image_bytes(&bytes, palette, cells.0, cells.1)
        })
        .await;
        if let Ok(Ok(cover)) = cover {
            let _ = actions.send(Action::CoverLoaded { generation, cover });
        }
    });
}

/// The project's own logo (MIT, ships with the repo) — the default
/// dashboard art, pixelated through the same pipeline as covers.
const LOGO_BYTES: &[u8] = include_bytes!("../../../images/logo.png");

/// The idle-art source: the user's configured image, else the logo.
fn idle_art_bytes(config: &Config) -> Option<Vec<u8>> {
    if let Some(path) = &config.idle_art {
        if let Ok(bytes) = std::fs::read(shellexpand_home(path)) {
            return Some(bytes);
        }
    }
    Some(LOGO_BYTES.to_vec())
}

fn render_idle_art(
    bytes: Option<&[u8]>,
    palette: &'static [(u8, u8, u8)],
    cells: (u16, u16),
) -> PixelCover {
    bytes
        .and_then(|bytes| pixel::from_image_bytes(bytes, palette, cells.0, cells.1).ok())
        .unwrap_or_else(|| pixel::vinyl(palette, cells.0, cells.1))
}

/// Idle art scales with the terminal like covers do.
fn desired_idle_cells() -> (u16, u16) {
    let (cols, rows) = crossterm::terminal::size().unwrap_or((80, 24));
    let height = (rows * 2 / 5).clamp(12, 24);
    let width = (height * 2).min(cols.saturating_sub(4).max(16));
    (width, width / 2)
}

/// Cell-grid scaling for the pixel-density setting; the widget itself
/// still clips to the drawing area, so scale only changes granularity.
fn scale_cells(cells: (u16, u16), factor: f32) -> (u16, u16) {
    let width = ((cells.0 as f32 * factor).round() as u16).clamp(8, 120);
    (width, width / 2)
}

fn shellexpand_home(path: &str) -> std::path::PathBuf {
    match path.strip_prefix("~/") {
        Some(rest) => dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(rest),
        None => std::path::PathBuf::from(path),
    }
}

pub async fn run(config: Config) -> Result<()> {
    let mut terminal = ratatui::init();
    let _ = crossterm::execute!(
        std::io::stdout(),
        crossterm::event::EnableMouseCapture,
        crossterm::event::EnableBracketedPaste
    );
    let result = event_loop(&mut terminal, &config).await;
    let _ = crossterm::execute!(
        std::io::stdout(),
        crossterm::event::DisableMouseCapture,
        crossterm::event::DisableBracketedPaste
    );
    ratatui::restore();
    result
}

async fn event_loop(terminal: &mut ratatui::DefaultTerminal, config: &Config) -> Result<()> {
    let (player, mut player_events) = player::spawn(tokio::runtime::Handle::current());
    let (actions_tx, mut actions) = mpsc::unbounded_channel();

    let input_tx = actions_tx.clone();
    tokio::spawn(async move {
        let mut stream = crossterm::event::EventStream::new();
        while let Some(result) = stream.next().await {
            // A single unparseable sequence (e.g. an exotic drag-and-drop
            // payload) must not kill the input loop.
            let Ok(event) = result else { continue };
            if let Some(action) = event::action_for(event) {
                if input_tx.send(action).is_err() {
                    break;
                }
            }
        }
    });

    let player_tx = actions_tx.clone();
    tokio::spawn(async move {
        while let Some(event) = player_events.recv().await {
            if player_tx.send(Action::Player(event)).is_err() {
                break;
            }
        }
    });

    let fx = Effects {
        player,
        ncm: Arc::new(Ncm::new(config::session_path(), config.quality.clone())),
        store: Arc::new(crate::store::LibraryStore::new(
            config::cache_dir().join("library"),
        )),
        actions: actions_tx,
    };
    // Restore a persisted session: greet + load 我喜欢的音乐.
    if fx.ncm.is_logged_in() {
        let ncm = fx.ncm.clone();
        let actions = fx.actions.clone();
        tokio::spawn(async move {
            let action = match ncm.account().await {
                Ok((uid, nickname)) => Action::LoggedIn { uid, nickname },
                Err(_) => Action::Notice {
                    message: i18n::t(Key::SessionExpired).into(),
                },
            };
            let _ = actions.send(action);
        });
    }
    let mut state = AppState::new(config);
    let mut hits = ui::Hits::default();
    terminal.draw(|frame| ui::draw(frame, &state, &mut hits))?;

    while let Some(action) = actions.recv().await {
        apply(&mut state, action, &fx, &hits);
        // Coalesce whatever queued up so one draw covers the burst.
        while let Ok(action) = actions.try_recv() {
            apply(&mut state, action, &fx, &hits);
        }
        if state.should_quit {
            break;
        }
        terminal.draw(|frame| ui::draw(frame, &state, &mut hits))?;
    }
    Ok(())
}

/// Mouse events need the draw-time geometry, so they resolve here and
/// everything else goes straight to the reducer.
fn apply(state: &mut AppState, action: Action, fx: &Effects, hits: &ui::Hits) {
    match action {
        Action::Mouse(mouse) => {
            if let Some(resolved) = event::mouse_action(mouse, hits, state.selected) {
                state.update(resolved, fx);
            }
        }
        other => state.update(other, fx),
    }
}
