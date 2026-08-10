use std::{
    sync::mpsc::{self, Sender},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

use discord_rich_presence::{
    activity::{Activity, ActivityType, Assets, StatusDisplayType, Timestamps},
    DiscordIpc, DiscordIpcClient,
};
use serde::Deserialize;

const DISCORD_CLIENT_ID: &str = "818936529484906596";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscordPresencePayload {
    title: String,
    artists: Vec<String>,
    album: String,
    cover_url: String,
    duration_ms: i64,
    position_seconds: f64,
    playing: bool,
}

enum DiscordPresenceCommand {
    Configure(bool),
    Update(DiscordPresencePayload),
}

pub struct DiscordPresenceHandle(Sender<DiscordPresenceCommand>);

impl Default for DiscordPresenceHandle {
    fn default() -> Self {
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let mut enabled = false;
            let mut client = None;
            while let Ok(command) = receiver.recv() {
                match command {
                    DiscordPresenceCommand::Configure(next) => {
                        enabled = next;
                        if !enabled {
                            close_client(&mut client);
                        }
                    }
                    DiscordPresenceCommand::Update(payload) if enabled => {
                        update_activity(&mut client, &payload);
                    }
                    DiscordPresenceCommand::Update(_) => {}
                }
            }
            close_client(&mut client);
        });
        Self(sender)
    }
}

impl DiscordPresenceHandle {
    pub fn configure(&self, enabled: bool) -> Result<(), String> {
        self.0
            .send(DiscordPresenceCommand::Configure(enabled))
            .map_err(|error| error.to_string())
    }

    pub fn update(&self, payload: DiscordPresencePayload) -> Result<(), String> {
        self.0
            .send(DiscordPresenceCommand::Update(payload))
            .map_err(|error| error.to_string())
    }
}

fn close_client(client: &mut Option<DiscordIpcClient>) {
    if let Some(mut active) = client.take() {
        let _ = active.clear_activity();
        let _ = active.close();
    }
}

fn update_activity(client: &mut Option<DiscordIpcClient>, payload: &DiscordPresencePayload) {
    if client.is_none() {
        let mut next = DiscordIpcClient::new(DISCORD_CLIENT_ID);
        if let Err(error) = next.connect() {
            eprintln!("[discord] connection failed: {error}");
            return;
        }
        *client = Some(next);
    }

    let Some(active) = client.as_mut() else {
        return;
    };
    if let Err(error) = active.set_activity(build_activity(payload)) {
        eprintln!("[discord] activity update failed: {error}");
        close_client(client);
    }
}

fn build_activity(payload: &DiscordPresencePayload) -> Activity<'_> {
    let artists = payload.artists.join(", ");
    let details = if artists.is_empty() {
        payload.title.clone()
    } else {
        format!("{} - {artists}", payload.title)
    };
    let mut assets = Assets::new()
        .large_text(&payload.title)
        .small_image(if payload.playing { "play" } else { "pause" })
        .small_text(if payload.playing { "Playing" } else { "Paused" });
    if !payload.cover_url.is_empty() {
        assets = assets.large_image(&payload.cover_url);
    }

    let mut activity = Activity::new()
        .activity_type(ActivityType::Listening)
        .status_display_type(StatusDisplayType::Details)
        .details(details)
        .state(&payload.album)
        .assets(assets);
    if payload.playing && payload.duration_ms > 0 {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis() as i64)
            .unwrap_or_default();
        let elapsed_ms = (payload.position_seconds.max(0.0) * 1000.0) as i64;
        let start_ms = now_ms.saturating_sub(elapsed_ms);
        activity = activity.timestamps(
            Timestamps::new()
                .start(start_ms)
                .end(start_ms.saturating_add(payload.duration_ms)),
        );
    }
    activity
}

#[cfg(test)]
mod tests {
    use super::{build_activity, DiscordPresencePayload};

    fn payload(playing: bool) -> DiscordPresencePayload {
        DiscordPresencePayload {
            title: "Track".to_string(),
            artists: vec!["Artist".to_string()],
            album: "Album".to_string(),
            cover_url: "https://example.com/cover.jpg".to_string(),
            duration_ms: 180_000,
            position_seconds: 30.0,
            playing,
        }
    }

    #[test]
    fn playing_activity_contains_track_timing() {
        let value = serde_json::to_value(build_activity(&payload(true))).unwrap();
        assert_eq!(value["details"], "Track - Artist");
        assert_eq!(value["state"], "Album");
        assert!(value["timestamps"]["start"].is_i64());
        assert!(value["timestamps"]["end"].is_i64());
        assert_eq!(
            value["assets"]["large_image"],
            "https://example.com/cover.jpg"
        );
    }

    #[test]
    fn paused_activity_omits_timing() {
        let value = serde_json::to_value(build_activity(&payload(false))).unwrap();
        assert!(value.get("timestamps").is_none());
        assert_eq!(value["assets"]["small_image"], "pause");
    }
}
