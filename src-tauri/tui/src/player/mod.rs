//! Player actor: rodio lives on one dedicated thread; the UI only talks
//! Command in / Event out. Generation stamps let the reducer drop stale
//! events after a track switch.
//!
//! Spike-proven rules (see design charter appendix):
//! - every Decoder must know its byte length, or backward seeks fail;
//! - rodio is pinned to 0.22.x (0.20 panics opening M4A).

mod cache_stream;

use std::io::{Read, Seek};
use std::sync::mpsc as std_mpsc;
use std::time::{Duration, Instant};

use rodio::{Decoder, Player};
use stream_download::{Settings, StreamDownload, StreamHandle, StreamPhase};
use tokio::sync::{mpsc, oneshot};
use yesplaymusic_core::cache::{CacheLease, CacheMetadata};

pub use cache_stream::CacheWritePlan;
use cache_stream::{CacheImportReader, CacheStreamProvider};

pub enum PlayerCommand {
    PlayCached {
        generation: u64,
        lease: CacheLease,
    },
    PlayUrl {
        generation: u64,
        url: String,
        cache: Option<CacheWritePlan>,
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
        cached: Option<CacheMetadata>,
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
    cached: Option<CacheMetadata>,
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
                PlayerCommand::PlayCached {
                    generation: g,
                    lease,
                } => {
                    drop(pending.take());
                    stop(&engine);
                    active_generation =
                        start(&mut engine, volume, g, &events, || Ok(open_cached(lease)))
                            .then_some(g);
                }
                PlayerCommand::PlayUrl {
                    generation: g,
                    url,
                    cache,
                } => {
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
                        cache,
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
                                cached: None,
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
    cache: Option<CacheWritePlan>,
    opened: std_mpsc::Sender<Opened>,
    wake: std_mpsc::Sender<()>,
) -> PendingOpen {
    let task = runtime.spawn(async move {
        let result = open_url(&url, cache).await;
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

fn open_cached(lease: CacheLease) -> Media {
    let metadata = *lease.metadata();
    Media {
        reader: Box::new(lease),
        byte_len: Some(metadata.bytes),
        cached: Some(metadata),
    }
}

async fn open_url(url: &str, cache: Option<CacheWritePlan>) -> anyhow::Result<Media> {
    let (provider, import) = CacheStreamProvider::new()?;
    let (complete_tx, complete_rx) = oneshot::channel();
    let mut complete_tx = cache.as_ref().map(|_| complete_tx);
    let settings = Settings::default().on_progress(move |_, state, _| {
        if matches!(state.phase, StreamPhase::Complete) {
            if let Some(complete) = complete_tx.take() {
                let _ = complete.send(());
            }
        }
    });
    let reader = StreamDownload::new_http(url.parse()?, provider, settings).await?;
    let byte_len = reader.content_length();
    if let Some(plan) = cache {
        spawn_cache_publish(reader.handle(), complete_rx, import, plan);
    }
    Ok(Media {
        reader: Box::new(reader),
        byte_len,
        cached: None,
    })
}

fn spawn_cache_publish(
    handle: StreamHandle,
    complete: oneshot::Receiver<()>,
    import: CacheImportReader,
    plan: CacheWritePlan,
) {
    tokio::spawn(async move {
        if complete.await.is_err() {
            return;
        }
        handle.wait_for_completion().await;
        match tokio::task::spawn_blocking(move || cache_stream::publish(import, plan)).await {
            Ok(Ok(())) => {}
            Ok(Err(error)) => tracing::warn!(%error, "audio cache write failed"),
            Err(error) => tracing::warn!(%error, "audio cache worker failed"),
        }
    });
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
    let media = match open() {
        Ok(media) => media,
        Err(error) => {
            let _ = events.send(PlayerEvent::Failed {
                generation,
                message: error.to_string(),
                cached: None,
            });
            return false;
        }
    };
    let cached = media.cached;
    let mut builder = Decoder::builder()
        .with_data(media.reader)
        .with_seekable(true);
    if let Some(byte_len) = media.byte_len {
        builder = builder.with_byte_len(byte_len);
    }
    let decoder = builder.build().map_err(anyhow::Error::from);
    let decoder = match decoder {
        Ok(decoder) => decoder,
        Err(error) => {
            let _ = events.send(PlayerEvent::Failed {
                generation,
                message: error.to_string(),
                cached,
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
                    cached: None,
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
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::path::Path;
    use std::sync::mpsc as std_mpsc;
    use std::thread;
    use std::time::Duration;

    use tokio::sync::{mpsc, oneshot};
    use yesplaymusic_core::cache::{
        AudioCodec, AudioQuality, CacheKey, CacheWriteRequest, TrackCache,
    };

    use super::{open_cached, open_url, spawn, start, CacheWritePlan, PlayerCommand, PlayerEvent};

    const AUDIO_BODY: &[u8] = b"complete cache body";

    fn cache_request(track_id: i64, expected_bytes: u64) -> CacheWriteRequest {
        CacheWriteRequest::new(
            CacheKey::new(track_id, AudioQuality::High320),
            AudioCodec::Mp3,
            320_000,
        )
        .with_expected_bytes(expected_bytes)
        .with_expected_md5([
            0xe8, 0xa9, 0x92, 0x1b, 0xe8, 0x6b, 0xc2, 0x3f, 0x73, 0x2f, 0xa2, 0x62, 0x13, 0xec,
            0x6e, 0x05,
        ])
    }

    struct HttpServer {
        url: String,
        thread: thread::JoinHandle<()>,
    }

    impl HttpServer {
        fn complete(body: &'static [u8]) -> Self {
            let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind complete server");
            let address = listener.local_addr().expect("server address");
            let thread = thread::spawn(move || {
                let (mut socket, _) = listener.accept().expect("accept player request");
                read_request(&mut socket);
                write!(
                    socket,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )
                .expect("write response headers");
                socket.write_all(body).expect("write response body");
            });
            Self {
                url: format!("http://{address}/audio"),
                thread,
            }
        }

        fn stalled_prefix(
            prefix: &'static [u8],
            content_length: usize,
        ) -> (Self, oneshot::Receiver<()>, std_mpsc::Receiver<()>) {
            let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind partial server");
            let address = listener.local_addr().expect("server address");
            let (prefix_tx, prefix_sent) = oneshot::channel();
            let (closed_tx, closed) = std_mpsc::channel();
            let thread = thread::spawn(move || {
                let (mut socket, _) = listener.accept().expect("accept player request");
                read_request(&mut socket);
                write!(
                    socket,
                    "HTTP/1.1 200 OK\r\nContent-Length: {content_length}\r\nConnection: close\r\n\r\n"
                )
                .expect("write response headers");
                socket.write_all(prefix).expect("write response prefix");
                socket.flush().expect("flush response prefix");
                let _ = prefix_tx.send(());
                let mut buffer = [0_u8; 256];
                while socket.read(&mut buffer).is_ok_and(|read| read != 0) {}
                let _ = closed_tx.send(());
            });
            (
                Self {
                    url: format!("http://{address}/audio"),
                    thread,
                },
                prefix_sent,
                closed,
            )
        }

        fn join(self) {
            self.thread.join().expect("join HTTP server");
        }
    }

    fn read_request(socket: &mut std::net::TcpStream) {
        socket
            .set_read_timeout(Some(Duration::from_secs(1)))
            .expect("set request timeout");
        let mut request = Vec::new();
        let mut buffer = [0_u8; 256];
        while !request.windows(4).any(|window| window == b"\r\n\r\n") {
            let read = socket.read(&mut buffer).expect("read request");
            assert_ne!(read, 0, "request ended before headers");
            request.extend_from_slice(&buffer[..read]);
        }
    }

    async fn wait_for_cache(root: &Path, key: CacheKey) {
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if TrackCache::open(root)
                    .expect("open cache")
                    .lookup(key)
                    .expect("lookup cache")
                    .is_some()
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("completed stream should be cached");
    }

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
            cache: None,
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
            cache: None,
        });
        server.wait_until_requested().await;

        player.send(PlayerCommand::PlayUrl {
            generation: 2,
            url: "not a url".into(),
            cache: None,
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

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn naturally_completed_stream_is_published_with_validated_bytes_and_md5() {
        let cache_dir = tempfile::tempdir().expect("cache directory");
        let server = HttpServer::complete(AUDIO_BODY);
        let request = cache_request(31, AUDIO_BODY.len() as u64);
        let key = request.key;
        let media = open_url(
            &server.url,
            Some(CacheWritePlan {
                root: cache_dir.path().to_path_buf(),
                request,
            }),
        )
        .await
        .expect("open complete stream");

        let downloaded = tokio::task::spawn_blocking(move || {
            let mut reader = media.reader;
            let mut downloaded = Vec::new();
            reader.read_to_end(&mut downloaded).expect("read stream");
            downloaded
        })
        .await
        .expect("join stream reader");
        assert_eq!(downloaded, AUDIO_BODY);
        wait_for_cache(cache_dir.path(), key).await;

        let cache = TrackCache::open(cache_dir.path()).expect("open published cache");
        let mut lease = cache
            .lookup(key)
            .expect("lookup published cache")
            .expect("published cache entry");
        assert_eq!(lease.metadata().bytes, AUDIO_BODY.len() as u64);
        let mut cached = Vec::new();
        lease.read_to_end(&mut cached).expect("read cached bytes");
        assert_eq!(cached, AUDIO_BODY);
        server.join();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn stopping_a_partial_stream_does_not_publish_it() {
        let cache_dir = tempfile::tempdir().expect("cache directory");
        let (server, prefix_sent, closed) = HttpServer::stalled_prefix(b"part", AUDIO_BODY.len());
        let request = cache_request(32, AUDIO_BODY.len() as u64);
        let key = request.key;
        let media = open_url(
            &server.url,
            Some(CacheWritePlan {
                root: cache_dir.path().to_path_buf(),
                request,
            }),
        )
        .await
        .expect("open partial stream");
        tokio::time::timeout(Duration::from_secs(1), prefix_sent)
            .await
            .expect("server should send a prefix")
            .expect("prefix signal should arrive");

        drop(media);
        closed
            .recv_timeout(Duration::from_millis(500))
            .expect("stopping the stream should close the response");
        tokio::time::sleep(Duration::from_millis(50)).await;

        let cache = TrackCache::open(cache_dir.path()).expect("open cache");
        assert!(cache.lookup(key).expect("lookup cache").is_none());
        server.join();
    }

    #[test]
    fn cached_media_keeps_its_lease_until_the_decoder_source_is_dropped() {
        let cache_dir = tempfile::tempdir().expect("cache directory");
        let key = CacheKey::new(33, AudioQuality::High320);
        let cache = TrackCache::open(cache_dir.path()).expect("open cache");
        let mut writer = cache
            .begin_write(
                CacheWriteRequest::new(key, AudioCodec::Mp3, 320_000)
                    .with_expected_bytes(AUDIO_BODY.len() as u64),
            )
            .expect("begin cache write");
        writer.write_all(AUDIO_BODY).expect("write cache entry");
        writer.finish().expect("finish cache entry");
        let lease = cache
            .lookup(key)
            .expect("lookup cache")
            .expect("cache lease");
        let media = open_cached(lease);

        cache.set_max_bytes(0).expect("evict leased cache");
        assert_eq!(cache.total_bytes().expect("leased cache size"), 19);

        drop(media);
        cache.set_max_bytes(0).expect("evict released cache");
        assert_eq!(cache.total_bytes().expect("released cache size"), 0);
    }

    #[test]
    fn cached_decoder_failure_reports_the_entry_that_must_be_invalidated() {
        let cache_dir = tempfile::tempdir().expect("cache directory");
        let key = CacheKey::new(34, AudioQuality::High320);
        let cache = TrackCache::open(cache_dir.path()).expect("open cache");
        let mut writer = cache
            .begin_write(CacheWriteRequest::new(key, AudioCodec::Mp3, 320_000))
            .expect("begin cache write");
        writer.write_all(b"not audio").expect("write cache entry");
        let metadata = writer.finish().expect("finish cache entry");
        let lease = cache
            .lookup(key)
            .expect("lookup cache")
            .expect("cache lease");
        let (events, mut received) = mpsc::unbounded_channel();
        let mut engine = None;

        assert!(!start(&mut engine, 1.0, 8, &events, || {
            Ok(open_cached(lease))
        }));
        let event = received.try_recv().expect("decoder failure event");
        assert!(matches!(
            event,
            PlayerEvent::Failed {
                generation: 8,
                cached: Some(failed),
                ..
            } if failed == metadata
        ));
    }
}
