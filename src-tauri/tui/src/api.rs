//! NCM service: a typed façade over ncm-api-rs for the TUI. Every call
//! injects the persisted session cookie; anonymous calls degrade the same
//! way the desktop client does (standard quality, no personal data).

use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use ncm_api_rs::api::Query;
use ncm_api_rs::ApiClient;
use serde_json::Value;
use yesplaymusic_core::auth::{Session, SessionStore};

#[derive(Clone, Debug)]
pub struct ResolvedTrack {
    pub id: i64,
    pub title: String,
    pub artist: String,
    pub url: String,
    pub kind: String,
    pub duration_ms: i64,
}

pub struct Ncm {
    client: ApiClient,
    store: SessionStore,
    session: Option<Session>,
    quality: String,
}

impl Ncm {
    pub fn new(session_path: PathBuf, quality: String) -> Self {
        let store = SessionStore::new(session_path);
        let session = store.load();
        Self {
            client: ApiClient::new(None),
            store,
            session,
            quality,
        }
    }

    pub fn is_logged_in(&self) -> bool {
        self.session.is_some()
    }

    pub fn logout(&mut self) -> Result<()> {
        self.session = None;
        self.store.clear().context("clear session store")
    }

    fn query(&self) -> Query {
        let query = Query::new();
        match &self.session {
            Some(session) => query.cookie(&session.cookie_header()),
            None => query,
        }
    }

    /// NCM `level` parameter for the configured quality; the server
    /// downgrades automatically when VIP or licensing says no.
    fn bitrate(&self) -> &'static str {
        match self.quality.as_str() {
            "128" => "128000",
            "320" => "320000",
            "lossless" | "hires" => "999000",
            _ => "320000", // exhigh
        }
    }

    pub async fn search_songs(&self, keywords: &str, limit: u32) -> Result<Vec<Value>> {
        let query = self
            .query()
            .param("keywords", keywords)
            .param("type", "1")
            .param("limit", &limit.to_string());
        let response = self
            .client
            .cloudsearch(&query)
            .await
            .map_err(|error| anyhow!("cloudsearch failed: {error:?}"))?;
        Ok(response.body["result"]["songs"]
            .as_array()
            .cloned()
            .unwrap_or_default())
    }

    pub async fn song_url(&self, id: i64) -> Result<(String, String)> {
        let query = self
            .query()
            .param("id", &id.to_string())
            .param("br", self.bitrate());
        let response = self
            .client
            .song_url(&query)
            .await
            .map_err(|error| anyhow!("song_url failed: {error:?}"))?;
        let data = &response.body["data"][0];
        let url = data["url"]
            .as_str()
            .filter(|url| !url.is_empty())
            .ok_or_else(|| anyhow!("这首歌暂时拿不到播放地址（可能需要登录或 VIP）"))?;
        let kind = data["type"].as_str().unwrap_or("mp3").to_lowercase();
        Ok((url.to_owned(), kind))
    }

    /// Search by "title artist" and resolve the best match to a playable
    /// URL — the bridge that lets the UI play before playlists land.
    pub async fn resolve_for_play(&self, title: &str, artist: &str) -> Result<ResolvedTrack> {
        let keywords = format!("{title} {artist}");
        let songs = self.search_songs(&keywords, 8).await?;
        let song = songs
            .first()
            .ok_or_else(|| anyhow!("搜索不到「{keywords}」"))?;
        let id = song["id"].as_i64().ok_or_else(|| anyhow!("搜索结果缺少 id"))?;
        let (url, kind) = self.song_url(id).await?;
        Ok(ResolvedTrack {
            id,
            title: song["name"].as_str().unwrap_or(title).to_owned(),
            artist: song["ar"][0]["name"].as_str().unwrap_or(artist).to_owned(),
            url,
            kind,
            duration_ms: song["dt"].as_i64().unwrap_or(0),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ncm(quality: &str) -> Ncm {
        let dir = tempfile::tempdir().unwrap();
        Ncm::new(dir.path().join("session.json"), quality.into())
    }

    #[test]
    fn quality_maps_to_ncm_bitrates_with_exhigh_default() {
        assert_eq!(ncm("128").bitrate(), "128000");
        assert_eq!(ncm("exhigh").bitrate(), "320000");
        assert_eq!(ncm("lossless").bitrate(), "999000");
        assert_eq!(ncm("weird").bitrate(), "320000");
    }

    #[test]
    fn missing_session_means_logged_out_and_cookieless_queries() {
        let ncm = ncm("exhigh");
        assert!(!ncm.is_logged_in());
        assert!(ncm.query().cookie.is_none());
    }
}
