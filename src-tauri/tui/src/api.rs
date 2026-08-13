//! NCM service: a typed façade over ncm-api-rs for the TUI. Every call
//! injects the persisted session cookie; anonymous calls degrade the same
//! way the desktop client does (standard quality, no personal data).

use std::path::PathBuf;
use std::sync::RwLock;

use anyhow::{anyhow, Result};
use ncm_api_rs::api::Query;
use ncm_api_rs::ApiClient;
use serde_json::Value;
use yesplaymusic_core::auth::{Session, SessionStore};
use yesplaymusic_core::cache::{AudioCodec, AudioQuality, CacheKey};

use crate::i18n::{self, Key};

#[derive(Clone, Debug)]
pub struct ResolvedTrack {
    pub id: i64,
    pub title: String,
    pub artist: String,
    pub url: String,
    pub kind: String,
    pub cache_key: CacheKey,
    pub codec: AudioCodec,
    pub actual_bitrate: u32,
    pub expected_bytes: Option<u64>,
    pub expected_md5: Option<[u8; 16]>,
    pub duration_ms: i64,
    pub pic_url: Option<String>,
}

#[derive(Debug, Eq, PartialEq)]
struct PlaybackSource {
    url: String,
    codec: AudioCodec,
    actual_bitrate: u32,
    expected_bytes: Option<u64>,
    expected_md5: Option<[u8; 16]>,
}

#[derive(Clone, Debug)]
pub struct SongRow {
    pub id: i64,
    pub title: String,
    pub artist: String,
    pub duration_ms: i64,
    pub pic_url: Option<String>,
}

/// Which library list is on screen / feeding the queue.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Liked,
    Daily,
    Fm,
    Cloud,
    Search,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QrStatus {
    Waiting,
    Scanned,
    Expired,
    Success(Session),
}

pub struct Ncm {
    client: ApiClient,
    store: SessionStore,
    session: RwLock<Option<Session>>,
    quality: AudioQuality,
}

impl Ncm {
    pub fn new(session_path: PathBuf, quality: AudioQuality) -> Self {
        let store = SessionStore::new(session_path);
        let session = RwLock::new(store.load());
        Self {
            client: ApiClient::new(None),
            store,
            session,
            quality,
        }
    }

    pub fn session_snapshot(&self) -> Option<Session> {
        self.session.read().ok().and_then(|session| session.clone())
    }

    pub(crate) fn quality(&self) -> AudioQuality {
        self.quality
    }

    pub fn commit_session(&self, session: &Session) -> Result<()> {
        self.store
            .save(session)
            .map_err(|error| anyhow!(i18n::t_api_failed(Key::OpPersistSession, error)))?;
        *self.session.write().expect("session lock") = Some(session.clone());
        Ok(())
    }

    fn query(&self) -> Query {
        let session = self.session_snapshot();
        Self::query_with_session(session.as_ref())
    }

    fn query_with_session(session: Option<&Session>) -> Query {
        let cookie = session.map(Session::cookie_header);
        match cookie {
            Some(cookie) => Query::new().cookie(&cookie),
            None => Query::new(),
        }
    }

    /// NCM `br` parameter for the configured quality; the server
    /// downgrades automatically when VIP or licensing says no.
    fn bitrate(&self) -> u32 {
        self.quality.bitrate()
    }

    // ── login ────────────────────────────────────────────────────────

    pub async fn qr_key(&self) -> Result<String> {
        let response = self
            .client
            .login_qr_key(&self.query())
            .await
            .map_err(|error| anyhow!(i18n::t_api_failed(Key::OpQrKey, error)))?;
        let body = &response.body;
        body["unikey"]
            .as_str()
            .or_else(|| body["data"]["unikey"].as_str())
            .map(str::to_owned)
            .ok_or_else(|| anyhow!(i18n::t(Key::ApiQrKeyMissing)))
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
            .map_err(|error| anyhow!(i18n::t_api_failed(Key::OpQrCheck, error)))?;
        parse_qr_status(&response.body, &response.cookie)
    }

    // ── account & library ────────────────────────────────────────────

    pub async fn account(&self, session: Option<&Session>) -> Result<(i64, String)> {
        let response = self
            .client
            .user_account(&Self::query_with_session(session))
            .await
            .map_err(|error| anyhow!(i18n::t_api_failed(Key::OpAccount, error)))?;
        parse_account(&response.body)
    }

    /// The user's "我喜欢的音乐" — by NCM convention the first playlist.
    pub async fn liked_songs(&self, uid: i64, session: Option<&Session>) -> Result<Vec<SongRow>> {
        let query = Self::query_with_session(session)
            .param("uid", &uid.to_string())
            .param("limit", "1");
        let response = self
            .client
            .user_playlist(&query)
            .await
            .map_err(|error| anyhow!(i18n::t_api_failed(Key::OpUserPlaylist, error)))?;
        let playlist_id = response.body["playlist"][0]["id"]
            .as_i64()
            .ok_or_else(|| anyhow!(i18n::t(Key::ApiLikedPlaylistMissing)))?;
        self.playlist_songs(playlist_id, session).await
    }

    pub async fn playlist_songs(
        &self,
        playlist_id: i64,
        session: Option<&Session>,
    ) -> Result<Vec<SongRow>> {
        let query = Self::query_with_session(session)
            .param("id", &playlist_id.to_string())
            .param("limit", "500");
        let response = self
            .client
            .playlist_track_all(&query)
            .await
            .map_err(|error| anyhow!(i18n::t_api_failed(Key::OpPlaylistTracks, error)))?;
        let songs = response.body["songs"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        Ok(songs.iter().map(song_row).collect())
    }

    pub async fn set_like(&self, id: i64, like: bool, session: Option<&Session>) -> Result<()> {
        let query = Self::query_with_session(session)
            .param("id", &id.to_string())
            .param("like", if like { "true" } else { "false" });
        let response = self
            .client
            .like(&query)
            .await
            .map_err(|error| anyhow!(i18n::t_api_failed(Key::OpSongUrl, error)))?;
        match response.body["code"].as_i64() {
            Some(200) => Ok(()),
            other => Err(anyhow!("{} ({other:?})", i18n::t(Key::LikeFailed))),
        }
    }

    pub async fn liked_ids(
        &self,
        uid: i64,
        session: Option<&Session>,
    ) -> Result<std::collections::HashSet<i64>> {
        let query = Self::query_with_session(session).param("uid", &uid.to_string());
        let response = self
            .client
            .likelist(&query)
            .await
            .map_err(|error| anyhow!(i18n::t_api_failed(Key::OpUserPlaylist, error)))?;
        Ok(response.body["ids"]
            .as_array()
            .map(|ids| ids.iter().filter_map(Value::as_i64).collect())
            .unwrap_or_default())
    }

    pub async fn daily_songs(&self, session: Option<&Session>) -> Result<Vec<SongRow>> {
        let response = self
            .client
            .recommend_songs(&Self::query_with_session(session))
            .await
            .map_err(|error| anyhow!(i18n::t_api_failed(Key::OpPlaylistTracks, error)))?;
        let songs = response.body["data"]["dailySongs"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        Ok(songs.iter().map(song_row_flex).collect())
    }

    pub async fn personal_fm(&self, session: Option<&Session>) -> Result<Vec<SongRow>> {
        let response = self
            .client
            .personal_fm(&Self::query_with_session(session))
            .await
            .map_err(|error| anyhow!(i18n::t_api_failed(Key::OpPlaylistTracks, error)))?;
        let songs = response.body["data"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        Ok(songs.iter().map(song_row_flex).collect())
    }

    pub async fn cloud_songs(&self, session: Option<&Session>) -> Result<Vec<SongRow>> {
        let response = self
            .client
            .user_cloud(&Self::query_with_session(session))
            .await
            .map_err(|error| anyhow!(i18n::t_api_failed(Key::OpPlaylistTracks, error)))?;
        let items = response.body["data"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        Ok(items
            .iter()
            .map(|item| {
                let mut row = song_row_flex(&item["simpleSong"]);
                if row.id == 0 {
                    row.id = item["songId"].as_i64().unwrap_or(0);
                }
                if row.title == "?" {
                    row.title = item["songName"].as_str().unwrap_or("?").to_owned();
                }
                if row.artist == "?" {
                    row.artist = item["artist"].as_str().unwrap_or("?").to_owned();
                }
                row
            })
            .collect())
    }

    // ── playback resolution ──────────────────────────────────────────

    async fn song_url(&self, id: i64) -> Result<PlaybackSource> {
        let bitrate = self.bitrate().to_string();
        let query = self
            .query()
            .param("id", &id.to_string())
            .param("br", &bitrate);
        let response = self
            .client
            .song_url(&query)
            .await
            .map_err(|error| anyhow!(i18n::t_api_failed(Key::OpSongUrl, error)))?;
        parse_playback_source(&response.body["data"][0])
    }

    /// Raw LRC text pair (original, translation) for a song.
    pub async fn lyrics(&self, id: i64) -> Result<(String, Option<String>)> {
        let query = self.query().param("id", &id.to_string());
        let response = self
            .client
            .lyric(&query)
            .await
            .map_err(|error| anyhow!(i18n::t_api_failed(Key::OpLyrics, error)))?;
        let body = &response.body;
        let lrc = body["lrc"]["lyric"].as_str().unwrap_or_default().to_owned();
        let tlyric = body["tlyric"]["lyric"]
            .as_str()
            .filter(|text| !text.trim().is_empty())
            .map(str::to_owned);
        Ok((lrc, tlyric))
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
            .map_err(|error| anyhow!(i18n::t_api_failed(Key::OpSearch, error)))?;
        Ok(response.body["result"]["songs"]
            .as_array()
            .cloned()
            .unwrap_or_default())
    }

    /// Song search mapped straight to typed rows.
    pub async fn search_rows(&self, keywords: &str, limit: u32) -> Result<Vec<SongRow>> {
        Ok(self
            .search_songs(keywords, limit)
            .await?
            .iter()
            .map(song_row)
            .collect())
    }

    /// Resolve a known song id straight to a playable track.
    pub async fn resolve_by_id(&self, row: &SongRow) -> Result<ResolvedTrack> {
        let source = self.song_url(row.id).await?;
        Ok(self.resolved_track(row.clone(), source))
    }

    /// Search by "title artist" and resolve the first *playable* match —
    /// top hits can be VIP-gated with a null URL, so walk the candidates.
    pub async fn resolve_for_play(&self, title: &str, artist: &str) -> Result<ResolvedTrack> {
        let keywords = format!("{title} {artist}");
        let songs = self.search_songs(keywords.trim(), 8).await?;
        if songs.is_empty() {
            return Err(anyhow!(i18n::t_search_not_found(&keywords)));
        }
        for song in &songs {
            let Some(id) = song["id"].as_i64() else {
                continue;
            };
            let Ok(source) = self.song_url(id).await else {
                continue;
            };
            let row = SongRow {
                id,
                title: song["name"].as_str().unwrap_or(title).to_owned(),
                artist: song["ar"][0]["name"].as_str().unwrap_or(artist).to_owned(),
                duration_ms: song["dt"].as_i64().unwrap_or(0),
                pic_url: song["al"]["picUrl"].as_str().map(str::to_owned),
            };
            return Ok(self.resolved_track(row, source));
        }
        Err(anyhow!(i18n::t_candidates_unavailable(&keywords)))
    }

    fn resolved_track(&self, row: SongRow, source: PlaybackSource) -> ResolvedTrack {
        ResolvedTrack {
            id: row.id,
            title: row.title,
            artist: row.artist,
            kind: source.codec.extension().to_owned(),
            cache_key: CacheKey::new(row.id, self.quality),
            codec: source.codec,
            actual_bitrate: source.actual_bitrate,
            expected_bytes: source.expected_bytes,
            expected_md5: source.expected_md5,
            url: source.url,
            duration_ms: row.duration_ms,
            pic_url: row.pic_url,
        }
    }
}

fn parse_playback_source(data: &Value) -> Result<PlaybackSource> {
    let url = data["url"]
        .as_str()
        .filter(|url| !url.is_empty())
        .ok_or_else(|| anyhow!(i18n::t(Key::ApiPlaybackUrlUnavailable)))?;
    let codec = data["type"]
        .as_str()
        .ok_or_else(|| anyhow!("playback response is missing its audio codec"))?
        .parse::<AudioCodec>()?;
    let actual_bitrate = data["br"]
        .as_u64()
        .and_then(|bitrate| u32::try_from(bitrate).ok())
        .ok_or_else(|| anyhow!("playback response is missing its actual bitrate"))?;
    let expected_bytes = data["size"].as_u64().filter(|size| *size > 0);
    let expected_md5 = parse_md5(data["md5"].as_str())?;

    Ok(PlaybackSource {
        url: url.to_owned(),
        codec,
        actual_bitrate,
        expected_bytes,
        expected_md5,
    })
}

fn parse_md5(value: Option<&str>) -> Result<Option<[u8; 16]>> {
    let Some(value) = value.filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    if value.len() != 32 || !value.is_ascii() {
        return Err(anyhow!("playback response contains an invalid MD5"));
    }

    let mut digest = [0_u8; 16];
    for (index, byte) in digest.iter_mut().enumerate() {
        let offset = index * 2;
        *byte = u8::from_str_radix(&value[offset..offset + 2], 16)
            .map_err(|_| anyhow!("playback response contains an invalid MD5"))?;
    }
    Ok(Some(digest))
}

fn parse_qr_status(body: &Value, cookies: &[String]) -> Result<QrStatus> {
    match body["code"].as_i64().unwrap_or(0) {
        800 => Ok(QrStatus::Expired),
        801 => Ok(QrStatus::Waiting),
        802 => Ok(QrStatus::Scanned),
        803 => Session::from_set_cookies(cookies)
            .map(QrStatus::Success)
            .ok_or_else(|| anyhow!(i18n::t(Key::ApiLoginCookieMissing))),
        other => Err(anyhow!(i18n::t_unknown_qr_status(other))),
    }
}

fn parse_account(body: &Value) -> Result<(i64, String)> {
    let uid = body["account"]["id"]
        .as_i64()
        .ok_or_else(|| anyhow!(i18n::t(Key::ApiInvalidSession)))?;
    let nickname = body["profile"]["nickname"]
        .as_str()
        .unwrap_or("")
        .to_owned();
    Ok((uid, nickname))
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

/// Tolerant mapping: daily/FM/cloud payloads use ar|artists, al|album,
/// dt|duration interchangeably.
fn song_row_flex(song: &Value) -> SongRow {
    let artist = song["ar"][0]["name"]
        .as_str()
        .or_else(|| song["artists"][0]["name"].as_str())
        .unwrap_or("?")
        .to_owned();
    let pic_url = song["al"]["picUrl"]
        .as_str()
        .or_else(|| song["album"]["picUrl"].as_str())
        .map(str::to_owned);
    let duration_ms = song["dt"]
        .as_i64()
        .or_else(|| song["duration"].as_i64())
        .unwrap_or(0);
    SongRow {
        id: song["id"].as_i64().unwrap_or(0),
        title: song["name"].as_str().unwrap_or("?").to_owned(),
        artist,
        duration_ms,
        pic_url,
    }
}

/// Small square cover JPEG from the NCM CDN (`?param=WxH` server-side crop).
pub async fn fetch_cover(pic_url: &str, edge: u32) -> Result<Vec<u8>> {
    let url = format!("{pic_url}?param={edge}y{edge}");
    let response = reqwest::get(&url)
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|error| anyhow!(i18n::t_api_failed(Key::OpFetchCover, error)))?;
    let bytes = response
        .bytes()
        .await
        .map_err(|error| anyhow!(i18n::t_api_failed(Key::OpReadCover, error)))?;
    Ok(bytes.to_vec())
}

/// Render a QR login link as terminal half-block art.
pub fn qr_unicode(url: &str) -> Result<String> {
    let code = qrcode::QrCode::new(url.as_bytes())
        .map_err(|error| anyhow!(i18n::t_api_failed(Key::OpBuildQr, error)))?;
    Ok(code
        .render::<qrcode::render::unicode::Dense1x2>()
        .dark_color(qrcode::render::unicode::Dense1x2::Light)
        .light_color(qrcode::render::unicode::Dense1x2::Dark)
        .build())
}

#[cfg(test)]
mod tests {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    use super::*;

    fn ncm(quality: AudioQuality) -> Ncm {
        let dir = tempfile::tempdir().unwrap();
        Ncm::new(dir.path().join("session.json"), quality)
    }

    #[test]
    fn every_quality_maps_to_its_exact_ncm_bitrate() {
        let cases = [
            (AudioQuality::Low128, 128_000),
            (AudioQuality::Medium192, 192_000),
            (AudioQuality::High320, 320_000),
            (AudioQuality::Lossless, 350_000),
            (AudioQuality::HiRes, 999_000),
        ];
        for (quality, expected) in cases {
            assert_eq!(ncm(quality).bitrate(), expected);
        }
    }

    #[test]
    fn playback_response_preserves_actual_cache_metadata() {
        let source = parse_playback_source(&serde_json::json!({
            "url": "https://example.test/audio.flac",
            "type": "FLAC",
            "br": 850_321,
            "size": 12_345_678,
            "md5": "00112233445566778899AABBCCDDEEFF"
        }))
        .unwrap();

        assert_eq!(source.url, "https://example.test/audio.flac");
        assert_eq!(source.codec, AudioCodec::Flac);
        assert_eq!(source.actual_bitrate, 850_321);
        assert_eq!(source.expected_bytes, Some(12_345_678));
        assert_eq!(
            source.expected_md5,
            Some([
                0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
                0xee, 0xff,
            ])
        );
    }

    #[test]
    fn resolved_track_keeps_requested_quality_separate_from_actual_audio() {
        let ncm = ncm(AudioQuality::HiRes);
        let track = ncm.resolved_track(
            SongRow {
                id: 42,
                title: "Track".into(),
                artist: "Artist".into(),
                duration_ms: 180_000,
                pic_url: None,
            },
            PlaybackSource {
                url: "https://example.test/audio.mp3".into(),
                codec: AudioCodec::Mp3,
                actual_bitrate: 320_000,
                expected_bytes: Some(7_654_321),
                expected_md5: Some([0x11; 16]),
            },
        );

        assert_eq!(track.cache_key, CacheKey::new(42, AudioQuality::HiRes));
        assert_eq!(track.codec, AudioCodec::Mp3);
        assert_eq!(track.kind, "mp3");
        assert_eq!(track.actual_bitrate, 320_000);
        assert_eq!(track.expected_bytes, Some(7_654_321));
        assert_eq!(track.expected_md5, Some([0x11; 16]));
    }

    #[test]
    fn playback_response_rejects_malformed_md5() {
        let result = parse_playback_source(&serde_json::json!({
            "url": "https://example.test/audio.mp3",
            "type": "mp3",
            "br": 320_000,
            "size": 1_024,
            "md5": "not-a-digest"
        }));

        assert!(result.is_err());
    }

    #[test]
    fn missing_session_means_logged_out_and_cookieless_queries() {
        let ncm = ncm(AudioQuality::High320);
        assert!(ncm.session_snapshot().is_none());
        assert!(ncm.query().cookie.is_none());
    }

    #[test]
    fn qr_login_url_and_unicode_rendering_hold_the_key() {
        let url = Ncm::qr_login_url("abc123");
        assert_eq!(url, "https://music.163.com/login?codekey=abc123");
        let art = qr_unicode(&url).unwrap();
        assert!(art.lines().count() > 10);
    }

    #[test]
    fn qr_success_returns_a_candidate_without_committing_it() {
        let ncm = ncm(AudioQuality::High320);
        let cookies = vec![
            "MUSIC_U=candidate-token; Path=/; HttpOnly".into(),
            "__csrf=candidate-csrf; Path=/".into(),
        ];

        let status = parse_qr_status(&serde_json::json!({ "code": 803 }), &cookies).unwrap();

        assert!(matches!(status, QrStatus::Success(_)));
        assert!(ncm.session_snapshot().is_none());
        assert!(ncm.query().cookie.is_none());
    }

    #[test]
    fn invalid_account_response_is_an_error_instead_of_uid_zero() {
        let error = parse_account(&serde_json::json!({
            "account": {},
            "profile": { "nickname": "unknown" }
        }));

        assert!(error.is_err());
    }

    #[tokio::test]
    async fn cover_fetch_accepts_success_and_rejects_http_error_status() {
        let success_url = serve_once("200 OK", b"image bytes").await;
        assert_eq!(fetch_cover(&success_url, 32).await.unwrap(), b"image bytes");

        let missing_url = serve_once("404 Not Found", b"missing").await;
        assert!(fetch_cover(&missing_url, 32).await.is_err());
    }

    async fn serve_once(status: &'static str, body: &'static [u8]) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = [0_u8; 1024];
            let _ = stream.read(&mut request).await.unwrap();
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            stream.write_all(response.as_bytes()).await.unwrap();
            stream.write_all(body).await.unwrap();
        });
        format!("http://{address}/cover")
    }
}
