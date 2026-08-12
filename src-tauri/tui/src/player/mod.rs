//! Player actor: rodio lives on one dedicated thread; the UI only talks
//! Command in / Event out. Generation stamps let the reducer drop stale
//! events after a track switch.
//!
//! Spike-proven rules (see design charter appendix):
//! - every Decoder must know its byte length, or backward seeks fail;
//! - rodio is pinned to 0.22.x (0.20 panics opening M4A).

use std::fs::File;
use std::io::{BufReader, Read, Seek};
use std::path::PathBuf;
use std::sync::mpsc as std_mpsc;
use std::time::Duration;

use rodio::{Decoder, Player};
use stream_download::storage::temp::TempStorageProvider;
use stream_download::{Settings, StreamDownload};
use tokio::sync::mpsc;

#[derive(Debug)]
pub enum PlayerCommand {
    PlayFile { generation: u64, path: PathBuf },
    PlayUrl { generation: u64, url: String },
    TogglePause,
    SeekTo(Duration),
    SetVolume(f32),
    Stop,
}

#[derive(Debug)]
pub enum PlayerEvent {
    Started {
        generation: u64,
        total: Option<Duration>,
    },
    Position {
        generation: u64,
        position: Duration,
    },
    Paused(bool),
    Ended {
        generation: u64,
    },
    Failed {
        generation: u64,
        message: String,
    },
}

#[derive(Clone)]
pub struct PlayerHandle {
    commands: std_mpsc::Sender<PlayerCommand>,
}

impl PlayerHandle {
    pub fn send(&self, command: PlayerCommand) {
        let _ = self.commands.send(command);
    }
}

pub fn spawn(
    runtime: tokio::runtime::Handle,
) -> (PlayerHandle, mpsc::UnboundedReceiver<PlayerEvent>) {
    let (command_tx, command_rx) = std_mpsc::channel();
    let (event_tx, event_rx) = mpsc::unbounded_channel();
    std::thread::Builder::new()
        .name("ypm-player".into())
        .spawn(move || actor(runtime, command_rx, event_tx))
        .expect("spawn player thread");
    (
        PlayerHandle {
            commands: command_tx,
        },
        event_rx,
    )
}

/// Anything rodio can decode from: a file or a stream-download reader.
trait MediaSource: Read + Seek + Send + Sync {}
impl<T: Read + Seek + Send + Sync> MediaSource for T {}

/// Reader plus its byte length — carrying the length is what makes
/// backward seeks work, so the two travel together by construction.
struct Media {
    reader: Box<dyn MediaSource>,
    byte_len: Option<u64>,
}

const TICK: Duration = Duration::from_millis(250);

struct Engine {
    _device: rodio::MixerDeviceSink,
    player: Player,
}

fn actor(
    runtime: tokio::runtime::Handle,
    commands: std_mpsc::Receiver<PlayerCommand>,
    events: mpsc::UnboundedSender<PlayerEvent>,
) {
    let mut engine: Option<Engine> = None;
    let mut generation = 0_u64;
    let mut active = false;
    let mut volume = 1.0_f32;

    loop {
        match commands.recv_timeout(TICK) {
            Ok(command) => match command {
                PlayerCommand::PlayFile { generation: g, path } => {
                    generation = g;
                    active = start(&mut engine, volume, g, &events, || open_file(&path));
                }
                PlayerCommand::PlayUrl { generation: g, url } => {
                    generation = g;
                    let runtime = runtime.clone();
                    active = start(&mut engine, volume, g, &events, move || {
                        open_url(&runtime, &url)
                    });
                }
                PlayerCommand::TogglePause => {
                    if let Some(engine) = &engine {
                        let paused = !engine.player.is_paused();
                        if paused {
                            engine.player.pause();
                        } else {
                            engine.player.play();
                        }
                        let _ = events.send(PlayerEvent::Paused(paused));
                    }
                }
                PlayerCommand::SeekTo(position) => {
                    if let Some(engine) = &engine {
                        if let Err(error) = engine.player.try_seek(position) {
                            let _ = events.send(PlayerEvent::Failed {
                                generation,
                                message: format!("seek failed: {error}"),
                            });
                        }
                    }
                }
                PlayerCommand::SetVolume(value) => {
                    volume = value.clamp(0.0, 1.5);
                    if let Some(engine) = &engine {
                        engine.player.set_volume(volume);
                    }
                }
                PlayerCommand::Stop => {
                    if let Some(engine) = &engine {
                        engine.player.stop();
                    }
                    active = false;
                }
            },
            Err(std_mpsc::RecvTimeoutError::Timeout) => {
                if let Some(engine) = &engine {
                    if active && engine.player.empty() {
                        active = false;
                        let _ = events.send(PlayerEvent::Ended { generation });
                    } else if active && !engine.player.is_paused() {
                        let _ = events.send(PlayerEvent::Position {
                            generation,
                            position: engine.player.get_pos(),
                        });
                    }
                }
            }
            Err(std_mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}

fn open_file(path: &PathBuf) -> anyhow::Result<Media> {
    let file = File::open(path)?;
    let byte_len = file.metadata().ok().map(|meta| meta.len());
    Ok(Media {
        reader: Box::new(BufReader::new(file)),
        byte_len,
    })
}

fn open_url(runtime: &tokio::runtime::Handle, url: &str) -> anyhow::Result<Media> {
    let reader = runtime.block_on(StreamDownload::new_http(
        url.parse()?,
        TempStorageProvider::new(),
        Settings::default(),
    ))?;
    let byte_len = reader.content_length();
    Ok(Media {
        reader: Box::new(reader),
        byte_len,
    })
}

fn start<F>(
    engine: &mut Option<Engine>,
    volume: f32,
    generation: u64,
    events: &mpsc::UnboundedSender<PlayerEvent>,
    open: F,
) -> bool
where
    F: FnOnce() -> anyhow::Result<Media>,
{
    let engine = match engine {
        Some(engine) => engine,
        None => match open_engine(volume) {
            Ok(opened) => engine.insert(opened),
            Err(error) => {
                let _ = events.send(PlayerEvent::Failed {
                    generation,
                    message: format!("audio device unavailable: {error}"),
                });
                return false;
            }
        },
    };

    engine.player.stop();
    let decoder = open().and_then(|media| {
        let mut builder = Decoder::builder()
            .with_data(media.reader)
            .with_seekable(true);
        if let Some(byte_len) = media.byte_len {
            builder = builder.with_byte_len(byte_len);
        }
        builder.build().map_err(Into::into)
    });
    match decoder {
        Ok(decoder) => {
            let total = rodio::Source::total_duration(&decoder);
            engine.player.append(decoder);
            engine.player.play();
            let _ = events.send(PlayerEvent::Started { generation, total });
            true
        }
        Err(error) => {
            let _ = events.send(PlayerEvent::Failed {
                generation,
                message: error.to_string(),
            });
            false
        }
    }
}

fn open_engine(volume: f32) -> anyhow::Result<Engine> {
    let device = rodio::DeviceSinkBuilder::open_default_sink()
        .map_err(|error| anyhow::anyhow!("{error}"))?;
    let player = Player::connect_new(device.mixer());
    player.set_volume(volume);
    Ok(Engine {
        _device: device,
        player,
    })
}
