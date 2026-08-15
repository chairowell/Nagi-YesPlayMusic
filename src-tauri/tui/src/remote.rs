//! Remote control socket: `ypm pause` etc. drive a running player.
//!
//! JSON lines over a Unix socket. The TUI serves the full protocol here;
//! the GUI serves the command subset inline in src-tauri/src/main.rs
//! (keep the wire words in sync). The client half lives in ctl.rs.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{mpsc, watch};

use crate::action::Action;

/// Now-playing state published by the app loop for `status` replies.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub playing: bool,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub position_ms: u64,
    pub duration_ms: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "lowercase")]
pub enum Command {
    Status,
    Pause,
    Resume,
    Toggle,
    Next,
    Prev,
}

pub fn socket_path() -> PathBuf {
    crate::config::state_dir().join("ctl.sock")
}

/// Bind the control socket, replacing a stale file left by a dead process.
/// If another live TUI already listens, that one keeps the socket.
pub fn bind(path: &PathBuf) -> std::io::Result<Option<UnixListener>> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    match UnixListener::bind(path) {
        Ok(listener) => Ok(Some(listener)),
        Err(error) if error.kind() == std::io::ErrorKind::AddrInUse => {
            if std::os::unix::net::UnixStream::connect(path).is_ok() {
                return Ok(None);
            }
            std::fs::remove_file(path)?;
            Ok(Some(UnixListener::bind(path)?))
        }
        Err(error) => Err(error),
    }
}

pub async fn serve(
    listener: UnixListener,
    actions: mpsc::UnboundedSender<Action>,
    snapshots: watch::Receiver<Snapshot>,
) {
    loop {
        let Ok((stream, _)) = listener.accept().await else {
            return;
        };
        let actions = actions.clone();
        let snapshots = snapshots.clone();
        tokio::spawn(async move {
            let _ = handle(stream, actions, snapshots).await;
        });
    }
}

async fn handle(
    stream: UnixStream,
    actions: mpsc::UnboundedSender<Action>,
    snapshots: watch::Receiver<Snapshot>,
) -> std::io::Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut line = String::new();
    BufReader::new(reader).read_line(&mut line).await?;
    let reply = match serde_json::from_str::<Command>(&line) {
        Ok(command) => {
            let snapshot = snapshots.borrow().clone();
            match command {
                Command::Status => serde_json::to_string(&snapshot).expect("snapshot serializes"),
                other => {
                    let action = match other {
                        Command::Toggle => Some(Action::TogglePlay),
                        Command::Pause => snapshot.playing.then_some(Action::TogglePlay),
                        Command::Resume => (!snapshot.playing).then_some(Action::TogglePlay),
                        Command::Next => Some(Action::NextTrack),
                        Command::Prev => Some(Action::PrevTrack),
                        Command::Status => unreachable!(),
                    };
                    if let Some(action) = action {
                        let _ = actions.send(action);
                    }
                    r#"{"ok":true}"#.to_owned()
                }
            }
        }
        Err(error) => format!(r#"{{"ok":false,"error":"{error}"}}"#),
    };
    writer.write_all(reply.as_bytes()).await?;
    writer.write_all(b"\n").await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_use_stable_wire_words() {
        // The GUI parses these words independently; renaming breaks it.
        for (command, wire) in [
            (Command::Status, r#"{"cmd":"status"}"#),
            (Command::Pause, r#"{"cmd":"pause"}"#),
            (Command::Resume, r#"{"cmd":"resume"}"#),
            (Command::Toggle, r#"{"cmd":"toggle"}"#),
            (Command::Next, r#"{"cmd":"next"}"#),
            (Command::Prev, r#"{"cmd":"prev"}"#),
        ] {
            assert_eq!(serde_json::to_string(&command).unwrap(), wire);
        }
    }

    #[tokio::test]
    async fn pause_only_fires_while_playing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ctl.sock");
        let listener = bind(&path).unwrap().unwrap();
        let (actions, mut received) = mpsc::unbounded_channel();
        let (publish, snapshots) = watch::channel(Snapshot {
            playing: true,
            ..Snapshot::default()
        });
        tokio::spawn(serve(listener, actions, snapshots));

        let roundtrip = |cmd: Command| {
            let path = path.clone();
            async move {
                let mut stream = UnixStream::connect(&path).await.unwrap();
                let mut payload = serde_json::to_string(&cmd).unwrap();
                payload.push('\n');
                stream.write_all(payload.as_bytes()).await.unwrap();
                let mut reply = String::new();
                BufReader::new(stream).read_line(&mut reply).await.unwrap();
                reply
            }
        };

        assert_eq!(roundtrip(Command::Pause).await, "{\"ok\":true}\n");
        assert!(matches!(received.recv().await, Some(Action::TogglePlay)));

        publish.send_replace(Snapshot::default());
        assert_eq!(roundtrip(Command::Pause).await, "{\"ok\":true}\n");
        assert_eq!(roundtrip(Command::Next).await, "{\"ok\":true}\n");
        // Pause while paused sent nothing; Next arrived instead.
        assert!(matches!(received.recv().await, Some(Action::NextTrack)));

        let status: Snapshot =
            serde_json::from_str(roundtrip(Command::Status).await.trim()).unwrap();
        assert!(!status.playing);
    }
}
