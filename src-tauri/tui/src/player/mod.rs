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
use std::time::{Duration, Instant};

use rodio::{Decoder, Player};
use stream_download::storage::temp::TempStorageProvider;
use stream_download::{Settings, StreamDownload};
use tokio::sync::mpsc;

#[derive(Debug)]
pub enum PlayerCommand {
    /// Local-file playback: reserved for the core cache integration.
    #[allow(dead_code)]
    PlayFile {
        generation: u64,
        path: PathBuf,
    },
    PlayUrl {
        generation: u64,
        url: String,
    },
    TogglePause,
    SeekTo(Duration),
    SetVolume(f32),
    #[allow(dead_code)] // stop control lands with the command palette
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
    Paused {
        generation: u64,
        paused: bool,
    },
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
    wake: std_mpsc::Sender<()>,
}

impl PlayerHandle {
    pub fn send(&self, command: PlayerCommand) {
        if self.commands.send(command).is_ok() {
            let _ = self.wake.send(());
        }
    }
}

pub fn spawn(
    runtime: tokio::runtime::Handle,
) -> (PlayerHandle, mpsc::UnboundedReceiver<PlayerEvent>) {
    let (command_tx, command_rx) = std_mpsc::channel();
    let (wake_tx, wake_rx) = std_mpsc::channel();
    let (opened_tx, opened_rx) = std_mpsc::channel();
    let (event_tx, event_rx) = mpsc::unbounded_channel();
    let actor_wake = wake_tx.clone();
    std::thread::Builder::new()
        .name("ypm-player".into())
        .spawn(move || {
            actor(
                runtime, command_rx, wake_rx, actor_wake, opened_tx, opened_rx, event_tx,
            )
        })
        .expect("spawn player thread");
    (
        PlayerHandle {
            commands: command_tx,
            wake: wake_tx,
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

struct Opened {
    request: u64,
    generation: u64,
    result: anyhow::Result<Media>,
}

struct PendingOpen {
    request: u64,
    generation: u64,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for PendingOpen {
    fn drop(&mut self) {
        self.task.abort();
    }
}

const TICK: Duration = Duration::from_millis(250);

struct Engine {
    _device: rodio::MixerDeviceSink,
    player: Player,
}

fn actor(
    runtime: tokio::runtime::Handle,
    commands: std_mpsc::Receiver<PlayerCommand>,
    wake: std_mpsc::Receiver<()>,
    wake_tx: std_mpsc::Sender<()>,
    opened_tx: std_mpsc::Sender<Opened>,
    opened_rx: std_mpsc::Receiver<Opened>,
    events: mpsc::UnboundedSender<PlayerEvent>,
) {
    let mut engine: Option<Engine> = None;
    let mut active_generation: Option<u64> = None;
    let mut volume = 1.0_f32;
    let mut pending: Option<PendingOpen> = None;
    let mut next_request = 1_u64;
    let mut last_tick = Instant::now();

    loop {
        let wait = TICK.saturating_sub(last_tick.elapsed());
        let _ = wake.recv_timeout(wait);

        let mut disconnected = false;
        loop {
            let command = match commands.try_recv() {
                Ok(command) => command,
                Err(std_mpsc::TryRecvError::Empty) => break,
                Err(std_mpsc::TryRecvError::Disconnected) => {
                    disconnected = true;
                    break;
                }
            };

            match command {
                PlayerCommand::PlayFile {
                    generation: g,
                    path,
                } => {
                    drop(pending.take());
                    stop(&engine);
                    active_generation =
                        start(&mut engine, volume, g, &events, || open_file(&path)).then_some(g);
                }
                PlayerCommand::PlayUrl { generation: g, url } => {
                    drop(pending.take());
                    stop(&engine);
                    active_generation = None;
                    let request = next_request;
                    next_request += 1;
                    pending = Some(spawn_open(
                        &runtime,
                        request,
                        g,
                        url,
                        opened_tx.clone(),
                        wake_tx.clone(),
                    ));
                }
                PlayerCommand::TogglePause => {
                    if let (Some(engine), Some(generation)) = (&engine, active_generation) {
                        let paused = !engine.player.is_paused();
                        if paused {
                            engine.player.pause();
                        } else {
                            engine.player.play();
                        }
                        let _ = events.send(PlayerEvent::Paused { generation, paused });
                    }
                }
                PlayerCommand::SeekTo(position) => {
                    if let (Some(engine), Some(generation)) = (&engine, active_generation) {
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
                    drop(pending.take());
                    stop(&engine);
                    active_generation = None;
                }
            }
        }

        if disconnected {
            break;
        }

        while let Ok(opened) = opened_rx.try_recv() {
            let is_current = pending.as_ref().is_some_and(|candidate| {
                candidate.request == opened.request && candidate.generation == opened.generation
            });
            if !is_current {
                continue;
            }

            drop(pending.take());
            active_generation = start(&mut engine, volume, opened.generation, &events, || {
                opened.result
            })
            .then_some(opened.generation);
        }

        if last_tick.elapsed() >= TICK {
            last_tick = Instant::now();
            if let (Some(engine), Some(generation)) = (&engine, active_generation) {
                if engine.player.empty() {
                    active_generation = None;
                    let _ = events.send(PlayerEvent::Ended { generation });
                } else if !engine.player.is_paused() {
                    let _ = events.send(PlayerEvent::Position {
                        generation,
                        position: engine.player.get_pos(),
                    });
                }
            }
        }
    }
}

fn stop(engine: &Option<Engine>) {
    if let Some(engine) = engine {
        engine.player.stop();
    }
}

fn spawn_open(
    runtime: &tokio::runtime::Handle,
    request: u64,
    generation: u64,
    url: String,
    opened: std_mpsc::Sender<Opened>,
    wake: std_mpsc::Sender<()>,
) -> PendingOpen {
    let task = runtime.spawn(async move {
        let result = open_url(&url).await;
        let _ = opened.send(Opened {
            request,
            generation,
            result,
        });
        let _ = wake.send(());
    });
    PendingOpen {
        request,
        generation,
        task,
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

async fn open_url(url: &str) -> anyhow::Result<Media> {
    let reader = StreamDownload::new_http(
        url.parse()?,
        TempStorageProvider::new(),
        Settings::default(),
    )
    .await?;
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
    let decoder = open().and_then(|media| {
        let mut builder = Decoder::builder()
            .with_data(media.reader)
            .with_seekable(true);
        if let Some(byte_len) = media.byte_len {
            builder = builder.with_byte_len(byte_len);
        }
        builder.build().map_err(Into::into)
    });
    let decoder = match decoder {
        Ok(decoder) => decoder,
        Err(error) => {
            let _ = events.send(PlayerEvent::Failed {
                generation,
                message: error.to_string(),
            });
            return false;
        }
    };

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
    let total = rodio::Source::total_duration(&decoder);
    engine.player.append(decoder);
    engine.player.play();
    let _ = events.send(PlayerEvent::Started { generation, total });
    true
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

#[cfg(test)]
mod tests {
    use std::io::Read;
    use std::net::TcpListener;
    use std::sync::mpsc as std_mpsc;
    use std::thread;
    use std::time::Duration;

    use tokio::sync::oneshot;

    use super::{spawn, PlayerCommand, PlayerEvent};

    struct StalledServer {
        url: String,
        accepted: Option<oneshot::Receiver<()>>,
        closed: std_mpsc::Receiver<()>,
        thread: thread::JoinHandle<()>,
    }

    impl StalledServer {
        fn spawn() -> Self {
            let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind stalled server");
            let address = listener.local_addr().expect("server address");
            let (accepted_tx, accepted) = oneshot::channel();
            let (closed_tx, closed) = std_mpsc::channel();
            let thread = thread::spawn(move || {
                let (mut socket, _) = listener.accept().expect("accept player request");
                socket
                    .set_read_timeout(Some(Duration::from_millis(20)))
                    .expect("set socket timeout");
                let _ = accepted_tx.send(());

                let mut buffer = [0_u8; 1024];
                loop {
                    match socket.read(&mut buffer) {
                        Ok(0) => {
                            let _ = closed_tx.send(());
                            break;
                        }
                        Ok(_) => {}
                        Err(error)
                            if matches!(
                                error.kind(),
                                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                            ) => {}
                        Err(_) => {
                            let _ = closed_tx.send(());
                            break;
                        }
                    }
                }
            });
            Self {
                url: format!("http://{address}/audio"),
                accepted: Some(accepted),
                closed,
                thread,
            }
        }

        async fn wait_until_requested(&mut self) {
            let accepted = self.accepted.take().expect("server request awaited once");
            tokio::time::timeout(Duration::from_secs(1), accepted)
                .await
                .expect("player should connect to local server")
                .expect("stalled server should report the connection");
        }

        fn wait_until_closed(&self) {
            self.closed
                .recv_timeout(Duration::from_millis(500))
                .expect("cancelling the open should close its HTTP connection");
        }

        fn join(self) {
            self.thread.join().expect("join stalled server");
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn stop_cancels_a_stalled_initial_connection() {
        let mut server = StalledServer::spawn();
        let (player, mut events) = spawn(tokio::runtime::Handle::current());
        player.send(PlayerCommand::PlayUrl {
            generation: 1,
            url: server.url.clone(),
        });
        server.wait_until_requested().await;

        player.send(PlayerCommand::Stop);
        server.wait_until_closed();
        assert!(
            tokio::time::timeout(Duration::from_millis(100), events.recv())
                .await
                .is_err(),
            "the cancelled track must not emit a playback event"
        );

        drop(player);
        server.join();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_new_play_replaces_a_stalled_open_without_starting_it() {
        let mut server = StalledServer::spawn();
        let (player, mut events) = spawn(tokio::runtime::Handle::current());
        player.send(PlayerCommand::PlayUrl {
            generation: 1,
            url: server.url.clone(),
        });
        server.wait_until_requested().await;

        player.send(PlayerCommand::PlayUrl {
            generation: 2,
            url: "not a url".into(),
        });
        let event = tokio::time::timeout(Duration::from_millis(500), events.recv())
            .await
            .expect("replacement should complete without waiting for the first connection")
            .expect("player event channel should remain open");
        assert!(matches!(event, PlayerEvent::Failed { generation: 2, .. }));
        server.wait_until_closed();
        assert!(
            tokio::time::timeout(Duration::from_millis(100), events.recv())
                .await
                .is_err(),
            "the replaced track must not emit a playback event"
        );

        drop(player);
        server.join();
    }
}
