//! Single state source: input becomes Action, update() is the only writer,
//! ui::draw() only reads.

use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use futures_util::StreamExt;
use tokio::sync::mpsc;

use crate::action::{Action, View, SEEK_STEP};
use crate::api::{self, Ncm, QrStatus, SongRow};
use crate::config::{self, Config};
use crate::event;
use crate::pixel::{self, PixelCover};
use crate::player::{self, PlayerCommand, PlayerEvent, PlayerHandle};
use crate::theme::Theme;
use crate::ui;

/// Side-effect handles the reducer may use; state itself stays plain data.
pub struct Effects {
    pub player: PlayerHandle,
    pub ncm: Arc<Ncm>,
    pub actions: mpsc::UnboundedSender<Action>,
}

/// Cover art cell grid fetched per track (rendering clips or centers).
const COVER_CELLS: (u16, u16) = (26, 13);
const COVER_SOURCE_EDGE: u32 = 300;

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
    pub nickname: Option<String>,
    pub login_qr: Option<String>,
    pub login_message: Option<String>,
    pub now: Option<NowPlaying>,
    pub cover: Option<PixelCover>,
    pub position: Duration,
    pub duration: Option<Duration>,
    pub paused: bool,
    pub volume: f32,
    pub status: Option<String>,
    pub generation: u64,
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
            nickname: None,
            login_qr: None,
            login_message: None,
            now: None,
            cover: None,
            position: Duration::ZERO,
            duration: None,
            paused: false,
            volume: 1.0,
            status: None,
            generation: 0,
            should_quit: false,
        }
    }

    fn update(&mut self, action: Action, fx: &Effects) {
        match action {
            Action::Quit => self.should_quit = true,
            Action::SwitchView(view) => self.view = view,
            Action::Back => self.view = View::NowPlaying,
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
                if self.view == View::Library && !self.library.is_empty() {
                    let last = self.library.len() as i32 - 1;
                    let next = (self.selected as i32 + delta).clamp(0, last);
                    self.selected = next as usize;
                }
            }
            Action::Activate => {
                if self.view == View::Library {
                    if let Some(row) = self.library.get(self.selected) {
                        self.generation += 1;
                        self.now = Some(NowPlaying {
                            title: row.title.clone(),
                            artist: row.artist.clone(),
                            album: String::new(),
                        });
                        self.cover = None;
                        self.position = Duration::ZERO;
                        self.duration = None;
                        self.status = Some("解析中…".into());
                        self.view = View::NowPlaying;
                        spawn_resolve(fx, self.generation, row.clone());
                    }
                }
            }
            Action::StartLogin => {
                if self.nickname.is_some() {
                    self.status = Some("已经登录了".into());
                } else {
                    self.view = View::Login;
                    self.login_qr = None;
                    self.login_message = Some("正在获取二维码…".into());
                    spawn_login(fx);
                }
            }
            Action::LoginQrReady { art } => {
                if self.view == View::Login {
                    self.login_qr = Some(art);
                    self.login_message = Some("用网易云音乐 App 扫码".into());
                }
            }
            Action::LoginProgress { message } | Action::LoginFailed { message } => {
                if self.view == View::Login {
                    self.login_message = Some(message);
                }
            }
            Action::LoggedIn { nickname } => {
                self.nickname = Some(nickname.clone());
                self.status = Some(format!("欢迎，{nickname}"));
                if self.view == View::Login {
                    self.view = View::Library;
                }
                self.login_qr = None;
                spawn_fetch_library(fx);
            }
            Action::LibraryLoaded { rows } => {
                self.status = Some(format!("我喜欢的音乐 · {} 首", rows.len()));
                self.library = rows;
                self.selected = 0;
            }
            Action::Notice { message } => self.status = Some(message),
            Action::TrackResolved { generation, track } => {
                if generation == self.generation {
                    self.now = Some(NowPlaying {
                        title: track.title.clone(),
                        artist: track.artist.clone(),
                        album: String::new(),
                    });
                    self.duration = (track.duration_ms > 0)
                        .then(|| Duration::from_millis(track.duration_ms as u64));
                    self.status = Some(format!("播放中 · {}", track.kind));
                    fx.player.send(PlayerCommand::PlayUrl {
                        generation,
                        url: track.url.clone(),
                    });
                    if let Some(pic_url) = track.pic_url {
                        spawn_cover_fetch(fx, generation, pic_url, self.theme.palette);
                    }
                }
            }
            Action::ResolveFailed { generation, message } => {
                if generation == self.generation {
                    self.status = Some(message);
                }
            }
            Action::CoverLoaded { generation, cover } => {
                if generation == self.generation {
                    self.cover = Some(cover);
                }
            }
            Action::Player(event) => self.apply_player_event(event),
            Action::Resize => {}
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
            PlayerEvent::Position { generation, position } => {
                if generation == self.generation {
                    self.position = position;
                }
            }
            PlayerEvent::Paused(paused) => self.paused = paused,
            PlayerEvent::Ended { generation } => {
                if generation == self.generation {
                    self.status = Some("播放结束".into());
                }
            }
            PlayerEvent::Failed { generation, message } => {
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
        loop {
            tokio::time::sleep(Duration::from_secs(2)).await;
            match ncm.qr_check(&key).await {
                Ok(QrStatus::Waiting) => {}
                Ok(QrStatus::Scanned) => {
                    if actions
                        .send(Action::LoginProgress {
                            message: "已扫码，在手机上确认…".into(),
                        })
                        .is_err()
                    {
                        return;
                    }
                }
                Ok(QrStatus::Expired) => {
                    let _ = actions.send(Action::LoginFailed {
                        message: "二维码已过期，按 g 重新获取".into(),
                    });
                    return;
                }
                Ok(QrStatus::Success) => {
                    let nickname = ncm
                        .account()
                        .await
                        .map(|(_, nickname)| nickname)
                        .unwrap_or_default();
                    let _ = actions.send(Action::LoggedIn { nickname });
                    return;
                }
                Err(error) => {
                    let _ = actions.send(Action::LoginFailed {
                        message: error.to_string(),
                    });
                    return;
                }
            }
        }
    });
}

fn spawn_fetch_library(fx: &Effects) {
    let ncm = fx.ncm.clone();
    let actions = fx.actions.clone();
    tokio::spawn(async move {
        let result = async {
            let (uid, _) = ncm.account().await?;
            ncm.liked_songs(uid).await
        }
        .await;
        let action = match result {
            Ok(rows) => Action::LibraryLoaded { rows },
            Err(error) => Action::Notice {
                message: format!("歌单加载失败：{error}"),
            },
        };
        let _ = actions.send(action);
    });
}

fn spawn_cover_fetch(
    fx: &Effects,
    generation: u64,
    pic_url: String,
    palette: &'static [(u8, u8, u8)],
) {
    let actions = fx.actions.clone();
    tokio::spawn(async move {
        let Ok(bytes) = api::fetch_cover(&pic_url, COVER_SOURCE_EDGE).await else {
            return; // a missing cover is cosmetic; the placeholder stays
        };
        let cover = tokio::task::spawn_blocking(move || {
            pixel::from_image_bytes(&bytes, palette, COVER_CELLS.0, COVER_CELLS.1)
        })
        .await;
        if let Ok(Ok(cover)) = cover {
            let _ = actions.send(Action::CoverLoaded { generation, cover });
        }
    });
}

pub async fn run(config: Config) -> Result<()> {
    let mut terminal = ratatui::init();
    let result = event_loop(&mut terminal, &config).await;
    ratatui::restore();
    result
}

async fn event_loop(terminal: &mut ratatui::DefaultTerminal, config: &Config) -> Result<()> {
    let (player, mut player_events) = player::spawn(tokio::runtime::Handle::current());
    let (actions_tx, mut actions) = mpsc::unbounded_channel();

    let input_tx = actions_tx.clone();
    tokio::spawn(async move {
        let mut stream = crossterm::event::EventStream::new();
        while let Some(Ok(event)) = stream.next().await {
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
        actions: actions_tx,
    };
    // Restore a persisted session: greet + load 我喜欢的音乐.
    if fx.ncm.is_logged_in() {
        let ncm = fx.ncm.clone();
        let actions = fx.actions.clone();
        tokio::spawn(async move {
            let action = match ncm.account().await {
                Ok((_, nickname)) => Action::LoggedIn { nickname },
                Err(_) => Action::Notice {
                    message: "登录态已失效，按 g 重新扫码".into(),
                },
            };
            let _ = actions.send(action);
        });
    }
    let mut state = AppState::new(config);
    terminal.draw(|frame| ui::draw(frame, &state))?;

    while let Some(action) = actions.recv().await {
        state.update(action, &fx);
        // Coalesce whatever queued up so one draw covers the burst.
        while let Ok(action) = actions.try_recv() {
            state.update(action, &fx);
        }
        if state.should_quit {
            break;
        }
        terminal.draw(|frame| ui::draw(frame, &state))?;
    }
    Ok(())
}
