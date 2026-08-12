//! NCM service: a typed façade over ncm-api-rs for the TUI. Every call
//! injects the persisted session cookie; anonymous calls degrade the same
//! way the desktop client does (standard quality, no personal data).

use std::path::PathBuf;
use std::sync::RwLock;

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
    pub pic_url: Option<String>,
}

#[derive(Clone, Debug)]
pub struct SongRow {
    pub id: i64,
    pub title: String,
    pub artist: String,
    pub duration_ms: i64,
    pub pic_url: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QrStatus {
    Waiting,
    Scanned,
    Expired,
    Success,
}

pub struct Ncm {
    client: ApiClient,
    store: SessionStore,
    session: RwLock<Option<Session>>,
    quality: String,
}

impl Ncm {
    pub fn new(session_path: PathBuf, quality: String) -> Self {
        let store = SessionStore::new(session_path);
        let session = RwLock::new(store.load());
        Self {
            client: ApiClient::new(None),
            store,
            session,
            quality,
        }
    }

    pub fn is_logged_in(&self) -> bool {
        self.session.read().is_ok_and(|session| session.is_some())
    }

    pub fn logout(&self) -> Result<()> {
        *self.session.write().expect("session lock") = None;
        self.store.clear().context("clear session store")
    }

    fn query(&self) -> Query {
        let cookie = self
            .session
            .read()
            .ok()
            .and_then(|session| session.as_ref().map(Session::cookie_header));
        match cookie {
            Some(cookie) => Query::new().cookie(&cookie),
            None => Query::new(),
        }
    }

    /// NCM `br` parameter for the configured quality; the server
    /// downgrades automatically when VIP or licensing says no.
    fn bitrate(&self) -> &'static str {
        match self.quality.as_str() {
            "128" => "128000",
            "320" => "320000",
            "lossless" | "hires" => "999000",
            _ => "320000", // exhigh
        }
    }

    // ── login ────────────────────────────────────────────────────────

    pub async fn qr_key(&self) -> Result<String> {
        let response = self
            .client
            .login_qr_key(&self.query())
            .await
            .map_err(|error| anyhow!("login_qr_key failed: {error:?}"))?;
        let body = &response.body;
        body["unikey"]
            .as_str()
            .or_else(|| body["data"]["unikey"].as_str())
            .map(str::to_owned)
            .ok_or_else(|| anyhow!("二维码 key 响应缺少 unikey"))
    }

    pub fn qr_login_url(key: &str) -> String {
        format!("https://music.163.com/login?codekey={key}")
    }

    pub async fn qr_check(&self, key: &str) -> Result<QrStatus> {
        let query = self.query().param("key", key);
        let response = self
            .client
            .login_qr_check(&query)
            .await
            .map_err(|error| anyhow!("login_qr_check failed: {error:?}"))?;
        let code = response.body["code"].as_i64().unwrap_or(0);
        match code {
            800 => Ok(QrStatus::Expired),
            801 => Ok(QrStatus::Waiting),
            802 => Ok(QrStatus::Scanned),
            803 => {
                let session = Session::from_set_cookies(&response.cookie)
                    .ok_or_else(|| anyhow!("登录成功但响应里没有 MUSIC_U cookie"))?;
                self.store.save(&session).context("persist session")?;
                *self.session.write().expect("session lock") = Some(session);
                Ok(QrStatus::Success)
            }
            other => Err(anyhow!("二维码状态码未知：{other}")),
        }
    }

    // ── account & library ────────────────────────────────────────────

    pub async fn account(&self) -> Result<(i64, String)> {
        let response = self
            .client
            .user_account(&self.query())
            .await
            .map_err(|error| anyhow!("user_account failed: {error:?}"))?;
        let body = &response.body;
        let uid = body["account"]["id"]
            .as_i64()
            .ok_or_else(|| anyhow!("登录态无效（拿不到账号 id）"))?;
        let nickname = body["profile"]["nickname"].as_str().unwrap_or("").to_owned();
        Ok((uid, nickname))
    }

    /// The user's "我喜欢的音乐" — by NCM convention the first playlist.
    pub async fn liked_songs(&self, uid: i64) -> Result<Vec<SongRow>> {
        let query = self.query().param("uid", &uid.to_string()).param("limit", "1");
        let response = self
            .client
            .user_playlist(&query)
            .await
            .map_err(|error| anyhow!("user_playlist failed: {error:?}"))?;
        let playlist_id = response.body["playlist"][0]["id"]
            .as_i64()
            .ok_or_else(|| anyhow!("没有找到「我喜欢的音乐」歌单"))?;
        self.playlist_songs(playlist_id).await
    }

    pub async fn playlist_songs(&self, playlist_id: i64) -> Result<Vec<SongRow>> {
        let query = self
            .query()
            .param("id", &playlist_id.to_string())
            .param("limit", "500");
        let response = self
            .client
            .playlist_track_all(&query)
            .await
            .map_err(|error| anyhow!("playlist_track_all failed: {error:?}"))?;
        let songs = response.body["songs"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        Ok(songs.iter().map(song_row).collect())
    }

    // ── playback resolution ──────────────────────────────────────────

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

    /// Resolve a known song id straight to a playable track.
    pub async fn resolve_by_id(&self, row: &SongRow) -> Result<ResolvedTrack> {
        let (url, kind) = self.song_url(row.id).await?;
        Ok(ResolvedTrack {
            id: row.id,
            title: row.title.clone(),
            artist: row.artist.clone(),
            url,
            kind,
            duration_ms: row.duration_ms,
            pic_url: row.pic_url.clone(),
        })
    }

    /// Search by "title artist" and resolve the first *playable* match —
    /// top hits can be VIP-gated with a null URL, so walk the candidates.
    pub async fn resolve_for_play(&self, title: &str, artist: &str) -> Result<ResolvedTrack> {
        let keywords = format!("{title} {artist}");
        let songs = self.search_songs(keywords.trim(), 8).await?;
        if songs.is_empty() {
            return Err(anyhow!("搜索不到「{keywords}」"));
        }
        for song in &songs {
            let Some(id) = song["id"].as_i64() else { continue };
            let Ok((url, kind)) = self.song_url(id).await else {
                continue;
            };
            return Ok(ResolvedTrack {
                id,
                title: song["name"].as_str().unwrap_or(title).to_owned(),
                artist: song["ar"][0]["name"].as_str().unwrap_or(artist).to_owned(),
                url,
                kind,
                duration_ms: song["dt"].as_i64().unwrap_or(0),
                pic_url: song["al"]["picUrl"].as_str().map(str::to_owned),
            });
        }
        Err(anyhow!("「{keywords}」的候选都拿不到播放地址（可能需要登录）"))
    }
}

fn song_row(song: &Value) -> SongRow {
    SongRow {
        id: song["id"].as_i64().unwrap_or(0),
        title: song["name"].as_str().unwrap_or("?").to_owned(),
        artist: song["ar"][0]["name"].as_str().unwrap_or("?").to_owned(),
        duration_ms: song["dt"].as_i64().unwrap_or(0),
        pic_url: song["al"]["picUrl"].as_str().map(str::to_owned),
    }
}

/// Small square cover JPEG from the NCM CDN (`?param=WxH` server-side crop).
pub async fn fetch_cover(pic_url: &str, edge: u32) -> Result<Vec<u8>> {
    let url = format!("{pic_url}?param={edge}y{edge}");
    let response = reqwest::get(&url).await.context("fetch cover")?;
    let bytes = response.bytes().await.context("read cover body")?;
    Ok(bytes.to_vec())
}

/// Render a QR login link as terminal half-block art.
pub fn qr_unicode(url: &str) -> Result<String> {
    let code = qrcode::QrCode::new(url.as_bytes()).context("build QR code")?;
    Ok(code
        .render::<qrcode::render::unicode::Dense1x2>()
        .dark_color(qrcode::render::unicode::Dense1x2::Light)
        .light_color(qrcode::render::unicode::Dense1x2::Dark)
        .build())
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

    #[test]
    fn qr_login_url_and_unicode_rendering_hold_the_key() {
        let url = Ncm::qr_login_url("abc123");
        assert_eq!(url, "https://music.163.com/login?codekey=abc123");
        let art = qr_unicode(&url).unwrap();
        assert!(art.lines().count() > 10);
    }
}
