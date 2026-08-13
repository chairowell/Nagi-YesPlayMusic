//! Single state source: input becomes Action, update() is the only writer,
//! ui::draw() only reads.

mod reducer;
mod search;
mod session;

use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use futures_util::StreamExt;
use image::{DynamicImage, Rgba};
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui_image::picker::{Picker, ProtocolType};
use ratatui_image::thread::{ResizeRequest, ResizeResponse, ThreadProtocol};
use ratatui_image::StatefulImage;
use tokio::sync::mpsc;

use crate::action::{Action, CoverRenderRequest, View};
use crate::api::{self, Ncm, SongRow, Source};
use crate::config::{self, Config, CoverMode};
use crate::event;
use crate::i18n::{self, Key};
use crate::pixel::{self, PixelCover};
use crate::player::{self, PlayerCommand, PlayerEvent, PlayerHandle};
use crate::theme::Theme;
use crate::ui;

use self::search::SearchState;
use self::session::SessionState;

/// Side-effect handles the reducer may use; state itself stays plain data.
pub struct Effects {
    pub player: PlayerHandle,
    pub ncm: Arc<Ncm>,
    pub store: Arc<crate::store::LibraryStore>,
    pub actions: mpsc::UnboundedSender<Action>,
}

const COVER_SOURCE_EDGE: u32 = 500;

struct OriginalCover {
    picker: Picker,
    protocol: ThreadProtocol,
    generation: Option<u64>,
}

impl OriginalCover {
    fn new(picker: Picker, requests: mpsc::UnboundedSender<ResizeRequest>) -> Self {
        Self {
            picker,
            protocol: ThreadProtocol::new(requests, None),
            generation: None,
        }
    }

    fn clear(&mut self) {
        self.generation = None;
        self.protocol.empty_protocol();
    }

    fn replace(&mut self, generation: u64, image: DynamicImage) {
        self.protocol
            .replace_protocol(self.picker.new_resize_protocol(image));
        self.generation = Some(generation);
    }
}

fn select_graphics_picker(mode: CoverMode, picker: Option<Picker>) -> Option<Picker> {
    if mode != CoverMode::Original {
        return None;
    }
    picker.filter(|picker| picker.protocol_type() != ProtocolType::Halfblocks)
}

fn query_graphics_picker(mode: CoverMode, background: Color) -> Option<Picker> {
    if mode != CoverMode::Original {
        return None;
    }
    let mut picker = select_graphics_picker(mode, Picker::from_query_stdio().ok())?;
    if let Color::Rgb(red, green, blue) = background {
        picker.set_background_color(Some(Rgba([red, green, blue, 255])));
    }
    Some(picker)
}

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
    pub session: SessionState,
    pub search: SearchState,
    pub now: Option<NowPlaying>,
    pub cover: Option<PixelCover>,
    pub layout: PlayLayout,
    pub thick_progress: bool,
    pixel_detail_scale: f32,
    original_cover: Option<OriginalCover>,
    pub idle_art: PixelCover,
    pub library_synced: bool,
    /// Idle art pre-rendered at the playing-cover size, so the loading
    /// placeholder swaps to the real cover without a size jump.
    pub placeholder: Option<PixelCover>,
    /// Source image for the idle art; None = procedural vinyl.
    idle_bytes: Option<Vec<u8>>,
    idle_path: Option<std::path::PathBuf>,
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
        let theme = Theme::by_name(&config.theme);
        let idle_cells = desired_idle_cells();
        let idle_path = config.idle_art.as_deref().map(shellexpand_home);
        let idle_bytes = idle_path.is_none().then(|| LOGO_BYTES.to_vec());
        Self {
            view: View::NowPlaying,
            zen: false,
            theme,
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
            session: SessionState::default(),
            search: SearchState::new(),
            now: None,
            cover: None,
            layout: PlayLayout::from_config(&config.layout),
            thick_progress: config.progress_style == "bar",
            pixel_detail_scale: config.pixel_scale.clamp(0.5, 2.0),
            original_cover: None,
            idle_art: pixel::vinyl(theme.palette, theme.bg, idle_cells.0, idle_cells.1),
            library_synced: false,
            placeholder: None,
            idle_bytes,
            idle_path,
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
        (width, width / 2)
    }

    fn ensure_placeholder(&mut self) {
        let desired = self.desired_cover_cells();
        let current = self.placeholder.as_ref().map(|art| (art.width, art.height));
        if current != Some(desired) {
            self.placeholder = Some(pixel::vinyl(
                self.theme.palette,
                self.theme.bg,
                desired.0,
                desired.1,
            ));
        }
    }

    fn clear_cover(&mut self) {
        self.cover = None;
        self.cover_bytes = None;
        if let Some(original) = &mut self.original_cover {
            original.clear();
        }
    }

    fn load_idle_art(&mut self, fx: &Effects) {
        if let Some(bytes) = self.idle_bytes.clone() {
            spawn_render_idle(
                fx,
                bytes,
                self.theme.palette,
                self.theme.bg,
                desired_idle_cells(),
                self.pixel_detail_scale,
            );
        } else if let Some(path) = self.idle_path.clone() {
            spawn_idle_load(fx, path);
        }
    }

    pub fn original_cover_is_current(&self) -> bool {
        self.original_cover
            .as_ref()
            .is_some_and(|cover| cover.generation == Some(self.generation))
    }

    pub fn render_original_cover(&mut self, frame: &mut ratatui::Frame, area: Rect) {
        if let Some(original) = &mut self.original_cover {
            frame.render_stateful_widget(StatefulImage::new(), area, &mut original.protocol);
        }
    }

    fn apply_original_resize(&mut self, response: ResizeResponse) {
        if let Some(original) = &mut self.original_cover {
            original.protocol.update_resized_protocol(response);
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
        fx.player.send(PlayerCommand::Stop);
        self.current_track_id = None;
        self.paused = false;
        self.clear_cover();
        self.ensure_placeholder();
        self.generation += 1;
        self.now = Some(NowPlaying {
            title: row.title.clone(),
            artist: row.artist.clone(),
            album: String::new(),
        });
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
                self.fetch_fm_more(fx);
            }
            None => self.status = Some(i18n::t(Key::QueueFinished).into()),
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
            PlayerEvent::Paused { generation, paused } => {
                if generation == self.generation {
                    self.paused = paused;
                }
            }
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

fn spawn_prefetch(fx: &Effects, index: usize, row: SongRow) {
    let ncm = fx.ncm.clone();
    let actions = fx.actions.clone();
    tokio::spawn(async move {
        if let Ok(track) = ncm.resolve_by_id(&row).await {
            let _ = actions.send(Action::PrefetchReady { index, track });
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
    request: CoverRenderRequest,
    bytes: Vec<u8>,
    palette: &'static [(u8, u8, u8)],
    background: Color,
    detail_scale: f32,
) {
    let actions = fx.actions.clone();
    tokio::spawn(async move {
        let cover = tokio::task::spawn_blocking(move || {
            pixel::from_image_bytes(
                &bytes,
                palette,
                background,
                request.cells.0,
                request.cells.1,
                detail_scale,
            )
        })
        .await;
        if let Ok(Ok(cover)) = cover {
            let _ = actions.send(Action::CoverLoaded { request, cover });
        }
    });
}

fn spawn_decode_cover(fx: &Effects, generation: u64, bytes: Vec<u8>) {
    let actions = fx.actions.clone();
    tokio::spawn(async move {
        let image = tokio::task::spawn_blocking(move || image::load_from_memory(&bytes)).await;
        if let Ok(Ok(image)) = image {
            let _ = actions.send(Action::CoverDecoded { generation, image });
        }
    });
}

fn apply_pixel_cover(
    current: &mut Option<PixelCover>,
    generation: u64,
    desired_cells: (u16, u16),
    request: CoverRenderRequest,
    cover: PixelCover,
) {
    if request.generation == generation && request.cells == desired_cells {
        *current = Some(cover);
    }
}

/// The project's own logo (MIT, ships with the repo) — the default
/// dashboard art, pixelated through the same pipeline as covers.
const LOGO_BYTES: &[u8] = include_bytes!("../../../images/logo.png");

fn spawn_render_idle(
    fx: &Effects,
    bytes: Vec<u8>,
    palette: &'static [(u8, u8, u8)],
    background: Color,
    cells: (u16, u16),
    detail_scale: f32,
) {
    let actions = fx.actions.clone();
    tokio::spawn(async move {
        let cover = tokio::task::spawn_blocking(move || {
            pixel::from_image_bytes(&bytes, palette, background, cells.0, cells.1, detail_scale)
        })
        .await;
        if let Ok(Ok(cover)) = cover {
            let _ = actions.send(Action::IdleArtLoaded { cells, cover });
        }
    });
}

fn spawn_idle_load(fx: &Effects, path: std::path::PathBuf) {
    let actions = fx.actions.clone();
    tokio::spawn(async move {
        let bytes = tokio::task::spawn_blocking(move || std::fs::read(path)).await;
        if let Ok(Ok(bytes)) = bytes {
            let _ = actions.send(Action::IdleArtBytes { bytes });
        }
    });
}

/// Idle art scales with the terminal like covers do.
fn desired_idle_cells() -> (u16, u16) {
    let (cols, rows) = crossterm::terminal::size().unwrap_or((80, 24));
    let height = (rows * 2 / 5).clamp(12, 24);
    let width = (height * 2).min(cols.saturating_sub(4).max(16));
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
    let (resize_requests_tx, mut resize_requests) = mpsc::unbounded_channel();
    let (resize_responses_tx, mut resize_responses) = mpsc::unbounded_channel();

    // Graphics protocol queries must finish before EventStream starts reading
    // the same terminal response bytes.
    let theme = Theme::by_name(&config.theme);
    let original_cover = query_graphics_picker(config.cover_mode, theme.bg)
        .map(|picker| OriginalCover::new(picker, resize_requests_tx));

    tokio::spawn(async move {
        while let Some(request) = resize_requests.recv().await {
            let response = tokio::task::spawn_blocking(move || request.resize_encode()).await;
            let Ok(Ok(response)) = response else { continue };
            if resize_responses_tx.send(response).is_err() {
                break;
            }
        }
    });

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
        ncm: Arc::new(Ncm::new(config::session_path(), config.quality)),
        store: Arc::new(crate::store::LibraryStore::new(
            config::cache_dir().join("library"),
        )),
        actions: actions_tx,
    };
    let mut state = AppState::new(config);
    state.original_cover = original_cover;
    state.load_idle_art(&fx);
    // Restore a persisted session: greet + load 我喜欢的音乐.
    if let Some(session) = fx.ncm.session_snapshot() {
        state.begin_session_restore(&fx, session);
    }
    let mut hits = ui::Hits::default();
    terminal.draw(|frame| ui::draw(frame, &mut state, &mut hits))?;

    loop {
        tokio::select! {
            action = actions.recv() => {
                let Some(action) = action else { break };
                apply(&mut state, action, &fx, &hits);
                // Coalesce whatever queued up so one draw covers the burst.
                while let Ok(action) = actions.try_recv() {
                    apply(&mut state, action, &fx, &hits);
                }
            }
            response = resize_responses.recv(), if state.original_cover.is_some() => {
                let Some(response) = response else { break };
                state.apply_original_resize(response);
            }
        }
        if state.should_quit {
            break;
        }
        terminal.draw(|frame| ui::draw(frame, &mut state, &mut hits))?;
    }
    Ok(())
}

/// Mouse events need the draw-time geometry, so they resolve here and
/// everything else goes straight to the reducer.
fn apply(state: &mut AppState, action: Action, fx: &Effects, hits: &ui::Hits) {
    match action {
        Action::Mouse(mouse) => {
            let selected = if state.view == View::Search && state.search.input {
                usize::MAX
            } else {
                state.selected
            };
            if let Some(resolved) = event::mouse_action(mouse, hits, selected) {
                state.update(resolved, fx);
            }
        }
        other => state.update(other, fx),
    }
}

#[cfg(test)]
mod tests;
