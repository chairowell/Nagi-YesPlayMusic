//! NCM service: a typed façade over ncm-api-rs for the TUI. Every call
//! injects the persisted session cookie; anonymous calls degrade the same
//! way the desktop client does (standard quality, no personal data).

use std::path::PathBuf;
use std::sync::{Arc, OnceLock, RwLock};
use std::time::Duration;

use anyhow::{anyhow, Result};
use futures_util::future::BoxFuture;
use ncm_api_rs::api::Query;
use ncm_api_rs::ApiClient;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use yesplaymusic_core::auth::{Session, SessionStore};
use yesplaymusic_core::cache::{AudioCodec, AudioQuality, CacheKey};
use yesplaymusic_core::unm::UnmState;

use crate::i18n::{self, Key};

const PLAYLIST_PAGE_SIZE: usize = 500;
const UNM_RESOLVE_TIMEOUT: Duration = Duration::from_secs(30);
/// Every outbound HTTP call this crate owns is bounded: a dead CDN or a
/// captive portal must not hang a cover fetch forever.
const HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const HTTP_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Shared, bounded HTTP client for the plain (non-NCM-signed) requests.
/// Reusing one client also keeps the connection pool warm across covers.
pub(crate) fn http_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| build_http_client(HTTP_CONNECT_TIMEOUT, HTTP_REQUEST_TIMEOUT))
}

fn build_http_client(connect: Duration, total: Duration) -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(connect)
        .timeout(total)
        .build()
        // Builder failure means no TLS backend at all; an unbounded default
        // client still beats refusing to show any cover.
        .unwrap_or_default()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedMedia {
    NeteaseUrl(String),
    UnmUrl(String),
    UnmBytes(Vec<u8>),
}

impl ResolvedMedia {
    pub const fn is_unm(&self) -> bool {
        matches!(self, Self::UnmUrl(_) | Self::UnmBytes(_))
    }
}

#[derive(Clone, Debug)]
pub struct ResolvedTrack {
    pub id: i64,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub media: ResolvedMedia,
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

#[derive(Debug)]
enum SongUrlFailure {
    /// NCM answered `code: 200` with no playable URL — really no rights.
    Unavailable,
    /// NCM refused the request itself (expired cookie, rate limit, risk
    /// control). Distinct from `Unavailable` because UNM must not run: it
    /// would hide a fixable sign-in problem behind "no copyright" and burn
    /// one round trip per track.
    Rejected(Option<i64>),
    Other(anyhow::Error),
}

#[derive(Debug, thiserror::Error)]
#[error("track has no playable source")]
pub(crate) struct TrackUnavailable;

#[derive(Clone, Debug, Eq, PartialEq)]
struct UnmResolution {
    provider: String,
    url: String,
}

trait UnmResolver: Send + Sync {
    fn resolve<'a>(&'a self, payload: &'a Value) -> BoxFuture<'a, Result<Option<UnmResolution>>>;
}

impl UnmResolver for UnmState {
    fn resolve<'a>(&'a self, payload: &'a Value) -> BoxFuture<'a, Result<Option<UnmResolution>>> {
        Box::pin(async move {
            Ok(UnmState::resolve(self, payload)
                .await
                .map_err(|_| anyhow!("UNM rejected the track payload"))?
                .map(|retrieved| UnmResolution {
                    provider: retrieved.source.into_owned(),
                    url: retrieved.url,
                }))
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SongRow {
    pub id: i64,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_ms: i64,
    pub pic_url: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LyricsPayload {
    pub lrc: String,
    pub tlyric: Option<String>,
    pub yrc: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum SearchChannel {
    #[default]
    Songs,
    Artists,
    Albums,
    Playlists,
}

impl SearchChannel {
    pub const ALL: [Self; 4] = [Self::Songs, Self::Artists, Self::Albums, Self::Playlists];

    pub const fn index(self) -> usize {
        match self {
            Self::Songs => 0,
            Self::Artists => 1,
            Self::Albums => 2,
            Self::Playlists => 3,
        }
    }

    pub fn cycle(self, delta: i32) -> Self {
        let index = (self.index() as i32 + delta).rem_euclid(Self::ALL.len() as i32) as usize;
        Self::ALL[index]
    }

    const fn api_type(self) -> &'static str {
        match self {
            Self::Songs => "1",
            Self::Artists => "100",
            Self::Albums => "10",
            Self::Playlists => "1000",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchPage<T> {
    pub items: Vec<T>,
    pub total: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtistHit {
    pub id: i64,
    pub name: String,
    pub pic_url: Option<String>,
    pub album_count: usize,
    pub song_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AlbumHit {
    pub id: i64,
    pub name: String,
    pub artist: String,
    pub pic_url: Option<String>,
    pub song_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlaylistHit {
    pub id: i64,
    pub name: String,
    pub creator: String,
    pub cover_url: Option<String>,
    pub track_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SearchPayload {
    Songs(SearchPage<SongRow>),
    Artists(SearchPage<ArtistHit>),
    Albums(SearchPage<AlbumHit>),
    Playlists(SearchPage<PlaylistHit>),
}

/// Which library list is on screen / feeding the queue.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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
    quality: RwLock<AudioQuality>,
    unm_enabled: bool,
    unm: Arc<dyn UnmResolver>,
    unm_timeout: Duration,
}

impl Ncm {
    pub fn new(session_path: PathBuf, quality: AudioQuality, unm_enabled: bool) -> Self {
        Self::with_unm(
            session_path,
            quality,
            unm_enabled,
            Arc::new(UnmState::new()),
            UNM_RESOLVE_TIMEOUT,
        )
    }

    fn with_unm(
        session_path: PathBuf,
        quality: AudioQuality,
        unm_enabled: bool,
        unm: Arc<dyn UnmResolver>,
        unm_timeout: Duration,
    ) -> Self {
        let store = SessionStore::new(session_path);
        let session = RwLock::new(store.load());
        Self {
            client: ApiClient::new(None),
            store,
            session,
            quality: RwLock::new(quality),
            unm_enabled,
            unm,
            unm_timeout,
        }
    }

    pub fn session_snapshot(&self) -> Option<Session> {
        self.session.read().ok().and_then(|session| session.clone())
    }

    pub(crate) fn quality(&self) -> AudioQuality {
        *self.quality.read().expect("quality lock")
    }

    pub(crate) fn set_quality(&self, quality: AudioQuality) {
        *self.quality.write().expect("quality lock") = quality;
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
        collect_playlist_pages(|offset| self.playlist_songs_page(playlist_id, session, offset))
            .await
    }

    async fn playlist_songs_page(
        &self,
        playlist_id: i64,
        session: Option<&Session>,
        offset: usize,
    ) -> Result<Vec<SongRow>> {
        let query = Self::query_with_session(session)
            .param("id", &playlist_id.to_string())
            .param("limit", &PLAYLIST_PAGE_SIZE.to_string())
            .param("offset", &offset.to_string());
        let response = self
            .client
            .playlist_track_all(&query)
            .await
            .map_err(|error| anyhow!(i18n::t_api_failed(Key::OpPlaylistTracks, error)))?;
        parse_song_collection(&response.body, &["songs"])
    }

    pub async fn set_like(&self, id: i64, like: bool, session: Option<&Session>) -> Result<()> {
        let query = Self::query_with_session(session)
            .param("id", &id.to_string())
            .param("like", if like { "true" } else { "false" });
        let response = self.client.like(&query).await.map_err(|_| like_error())?;
        match response.body["code"].as_i64() {
            Some(200) => Ok(()),
            _ => Err(like_error()),
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
        Ok(response_array(&response.body, &["ids"])?
            .iter()
            .filter_map(Value::as_i64)
            .collect())
    }

    pub async fn daily_songs(&self, session: Option<&Session>) -> Result<Vec<SongRow>> {
        let response = self
            .client
            .recommend_songs(&Self::query_with_session(session))
            .await
            .map_err(|error| anyhow!(i18n::t_api_failed(Key::OpPlaylistTracks, error)))?;
        let songs = response_array(&response.body, &["data", "dailySongs"])?;
        Ok(songs.iter().map(song_row_flex).collect())
    }

    pub async fn personal_fm(&self, session: Option<&Session>) -> Result<Vec<SongRow>> {
        let response = self
            .client
            .personal_fm(&Self::query_with_session(session))
            .await
            .map_err(|error| anyhow!(i18n::t_api_failed(Key::OpPlaylistTracks, error)))?;
        let songs = response_array(&response.body, &["data"])?;
        Ok(songs.iter().map(song_row_flex).collect())
    }

    pub async fn cloud_songs(&self, session: Option<&Session>) -> Result<Vec<SongRow>> {
        collect_cloud_pages(|offset| self.cloud_songs_page(session, offset)).await
    }

    async fn cloud_songs_page(
        &self,
        session: Option<&Session>,
        offset: usize,
    ) -> Result<(Vec<SongRow>, Option<bool>)> {
        let query = Self::query_with_session(session)
            .param("limit", &PLAYLIST_PAGE_SIZE.to_string())
            .param("offset", &offset.to_string());
        let response = self
            .client
            .user_cloud(&query)
            .await
            .map_err(|error| anyhow!(i18n::t_api_failed(Key::OpPlaylistTracks, error)))?;
        let has_more = response.body["hasMore"].as_bool();
        let items = response_array(&response.body, &["data"])?;
        let rows = items
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
            .collect();
        Ok((rows, has_more))
    }

    // ── playback resolution ──────────────────────────────────────────

    async fn song_url(
        &self,
        id: i64,
    ) -> std::result::Result<(AudioQuality, PlaybackSource), SongUrlFailure> {
        let requested_quality = self.quality();
        let bitrate = requested_quality.bitrate().to_string();
        let query = self
            .query()
            .param("id", &id.to_string())
            .param("br", &bitrate);
        let response = self.client.song_url(&query).await.map_err(|error| {
            SongUrlFailure::Other(anyhow!(i18n::t_api_failed(Key::OpSongUrl, error)))
        })?;
        Ok((requested_quality, classify_song_url(&response.body)?))
    }

    /// Raw line-, translation-, and word-synchronised lyrics for a song.
    pub async fn lyrics(&self, id: i64) -> Result<LyricsPayload> {
        let query = self.query().param("id", &id.to_string());
        let response = self
            .client
            // `lyric_new` sends yv/ytv/yrv, the API's YRC request flags.
            .lyric_new(&query)
            .await
            .map_err(|error| anyhow!(i18n::t_api_failed(Key::OpLyrics, error)))?;
        parse_lyrics_payload(&response.body)
    }

    pub async fn search_channel(
        &self,
        keywords: &str,
        channel: SearchChannel,
        limit: u32,
    ) -> Result<SearchPayload> {
        let query = self
            .query()
            .param("keywords", keywords)
            .param("type", channel.api_type())
            .param("limit", &limit.to_string());
        let response = self
            .client
            .cloudsearch(&query)
            .await
            .map_err(|error| anyhow!(i18n::t_api_failed(Key::OpSearch, error)))?;
        parse_search_payload(&response.body, channel)
    }

    pub async fn artist_top_songs(&self, artist_id: i64) -> Result<Vec<SongRow>> {
        let query = self.query().param("id", &artist_id.to_string());
        let response = self
            .client
            .artist_top_song(&query)
            .await
            .map_err(|error| anyhow!(i18n::t_api_failed(Key::OpPlaylistTracks, error)))?;
        parse_song_collection(&response.body, &["songs"])
    }

    pub async fn album_songs(&self, album_id: i64) -> Result<Vec<SongRow>> {
        let query = self.query().param("id", &album_id.to_string());
        let response = self
            .client
            .album(&query)
            .await
            .map_err(|error| anyhow!(i18n::t_api_failed(Key::OpPlaylistTracks, error)))?;
        parse_song_collection(&response.body, &["songs"])
    }

    pub async fn playlist_detail_songs(&self, playlist_id: i64) -> Result<Vec<SongRow>> {
        let query = self.query().param("id", &playlist_id.to_string());
        let response = self
            .client
            .playlist_detail(&query)
            .await
            .map_err(|error| anyhow!(i18n::t_api_failed(Key::OpPlaylistTracks, error)))?;
        let (embedded, total) = parse_playlist_detail(&response.body)?;
        let session = self.session_snapshot();
        complete_playlist_detail(embedded, total, || {
            self.playlist_songs(playlist_id, session.as_ref())
        })
        .await
    }

    pub async fn search_songs(&self, keywords: &str, limit: u32) -> Result<Vec<Value>> {
        let query = self
            .query()
            .param("keywords", keywords)
            .param("type", SearchChannel::Songs.api_type())
            .param("limit", &limit.to_string());
        let response = self
            .client
            .cloudsearch(&query)
            .await
            .map_err(|error| anyhow!(i18n::t_api_failed(Key::OpSearch, error)))?;
        require_success(&response.body)?;
        Ok(response_array(&response.body, &["result", "songs"])?.to_vec())
    }

    /// Resolve a known song id straight to a playable track.
    pub async fn resolve_by_id(&self, row: &SongRow) -> Result<ResolvedTrack> {
        let (requested_quality, source) =
            self.song_url(row.id).await.map_err(|error| match error {
                SongUrlFailure::Unavailable => anyhow!(i18n::t(Key::ApiPlaybackUrlUnavailable)),
                SongUrlFailure::Rejected(code) => anyhow!(i18n::t_song_url_rejected(code)),
                SongUrlFailure::Other(error) => error,
            })?;
        Ok(self.resolved_track(row.clone(), requested_quality, source))
    }

    /// Resolve for active playback. UNM is only consulted after the NCM
    /// endpoint explicitly returns no URL, never for transport failures.
    pub async fn resolve_for_playback(&self, row: &SongRow) -> Result<ResolvedTrack> {
        if row.id <= 0 {
            return self.resolve_for_play(&row.title, &row.artist).await;
        }
        let requested_quality = self.quality();
        let native = self.song_url(row.id).await.map(|(_, source)| source);
        self.resolve_after_native(row, requested_quality, native)
            .await
    }

    async fn resolve_after_native(
        &self,
        row: &SongRow,
        requested_quality: AudioQuality,
        native: std::result::Result<PlaybackSource, SongUrlFailure>,
    ) -> Result<ResolvedTrack> {
        match native {
            Ok(source) => Ok(self.resolved_track(row.clone(), requested_quality, source)),
            Err(SongUrlFailure::Other(error)) => Err(error),
            // Auth/rate-limit refusals are not a copyright problem — say so
            // instead of spending a UNM round trip on every track.
            Err(SongUrlFailure::Rejected(code)) => Err(anyhow!(i18n::t_song_url_rejected(code))),
            Err(SongUrlFailure::Unavailable) => {
                self.resolve_unm_track(row, requested_quality).await
            }
        }
    }

    async fn resolve_unm_track(
        &self,
        row: &SongRow,
        requested_quality: AudioQuality,
    ) -> Result<ResolvedTrack> {
        if !self.unm_enabled {
            return Err(TrackUnavailable.into());
        }
        let payload = json!({
            "track": {
                "id": row.id,
                "name": row.title,
                "dt": row.duration_ms,
                "ar": [{ "id": 0, "name": row.artist }]
            },
            "context": {}
        });
        let resolution =
            match tokio::time::timeout(self.unm_timeout, self.unm.resolve(&payload)).await {
                Ok(Ok(Some(resolution))) => resolution,
                Ok(Ok(None)) => return Err(TrackUnavailable.into()),
                Ok(Err(error)) => {
                    tracing::warn!(%error, "UNM resolution failed");
                    return Err(TrackUnavailable.into());
                }
                Err(_) => {
                    tracing::warn!("UNM resolution timed out");
                    return Err(TrackUnavailable.into());
                }
            };

        if resolution.url.trim().is_empty() {
            return Err(TrackUnavailable.into());
        }
        let media = if resolution.provider.eq_ignore_ascii_case("bilibili") {
            let bytes = decode_base64(&resolution.url).map_err(|error| {
                tracing::warn!(%error, "UNM returned invalid Bilibili audio");
                TrackUnavailable
            })?;
            ResolvedMedia::UnmBytes(bytes)
        } else {
            ResolvedMedia::UnmUrl(resolution.url)
        };
        let codec = match &media {
            ResolvedMedia::UnmUrl(url) => codec_from_url(url),
            ResolvedMedia::UnmBytes(_) => AudioCodec::Mp3,
            ResolvedMedia::NeteaseUrl(_) => unreachable!(),
        };
        Ok(ResolvedTrack {
            id: row.id,
            title: row.title.clone(),
            artist: row.artist.clone(),
            album: row.album.clone(),
            media,
            kind: codec.extension().to_owned(),
            cache_key: CacheKey::new(row.id, requested_quality),
            codec,
            actual_bitrate: 128_000,
            expected_bytes: None,
            expected_md5: None,
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
            return Err(anyhow!(i18n::t_search_not_found(&keywords)));
        }
        for song in &songs {
            let Some(id) = song["id"].as_i64() else {
                continue;
            };
            let (requested_quality, source) = match self.song_url(id).await {
                Ok(resolved) => resolved,
                // A refusal is per-account, not per-track: the remaining
                // candidates would all fail the same way.
                Err(SongUrlFailure::Rejected(code)) => {
                    return Err(anyhow!(i18n::t_song_url_rejected(code)))
                }
                Err(_) => continue,
            };
            let row = SongRow {
                id,
                title: song["name"].as_str().unwrap_or(title).to_owned(),
                artist: song["ar"][0]["name"].as_str().unwrap_or(artist).to_owned(),
                album: song["al"]["name"].as_str().unwrap_or("").to_owned(),
                duration_ms: song["dt"].as_i64().unwrap_or(0),
                pic_url: song["al"]["picUrl"].as_str().map(str::to_owned),
            };
            return Ok(self.resolved_track(row, requested_quality, source));
        }
        Err(anyhow!(i18n::t_candidates_unavailable(&keywords)))
    }

    fn resolved_track(
        &self,
        row: SongRow,
        requested_quality: AudioQuality,
        source: PlaybackSource,
    ) -> ResolvedTrack {
        ResolvedTrack {
            id: row.id,
            title: row.title,
            artist: row.artist,
            album: row.album,
            kind: source.codec.extension().to_owned(),
            cache_key: CacheKey::new(row.id, requested_quality),
            codec: source.codec,
            actual_bitrate: source.actual_bitrate,
            expected_bytes: source.expected_bytes,
            expected_md5: source.expected_md5,
            media: ResolvedMedia::NeteaseUrl(source.url),
            duration_ms: row.duration_ms,
            pic_url: row.pic_url,
        }
    }
}

fn parse_lyrics_payload(body: &Value) -> Result<LyricsPayload> {
    require_success(body)?;
    Ok(LyricsPayload {
        lrc: lyric_text(body, "lrc")?.unwrap_or_default(),
        tlyric: lyric_text(body, "tlyric")?,
        yrc: lyric_text(body, "yrc")?,
    })
}

fn lyric_text(body: &Value, field: &str) -> Result<Option<String>> {
    let Value::Object(body) = body else {
        return Err(invalid_payload("$"));
    };
    let section = match body.get(field) {
        None | Some(Value::Null) => return Ok(None),
        Some(Value::Object(section)) => section,
        Some(_) => return Err(invalid_payload(&format!("$.{field}"))),
    };
    let text = match section.get("lyric") {
        None | Some(Value::Null) => return Ok(None),
        Some(Value::String(text)) => text,
        Some(_) => return Err(invalid_payload(&format!("$.{field}.lyric"))),
    };
    Ok((!text.trim().is_empty()).then(|| text.clone()))
}

async fn collect_playlist_pages<F, Fut>(mut fetch: F) -> Result<Vec<SongRow>>
where
    F: FnMut(usize) -> Fut,
    Fut: std::future::Future<Output = Result<Vec<SongRow>>>,
{
    let mut rows = Vec::new();
    loop {
        let page = fetch(rows.len()).await?;
        let complete = page.len() < PLAYLIST_PAGE_SIZE;
        rows.extend(page);
        if complete {
            return Ok(rows);
        }
    }
}

async fn collect_cloud_pages<F, Fut>(mut fetch: F) -> Result<Vec<SongRow>>
where
    F: FnMut(usize) -> Fut,
    Fut: std::future::Future<Output = Result<(Vec<SongRow>, Option<bool>)>>,
{
    let mut rows = Vec::new();
    loop {
        let (page, has_more) = fetch(rows.len()).await?;
        let page_len = page.len();
        rows.extend(page);
        if page_len == 0 || !has_more.unwrap_or(page_len == PLAYLIST_PAGE_SIZE) {
            return Ok(rows);
        }
    }
}

fn like_error() -> anyhow::Error {
    anyhow!(i18n::t(Key::LikeFailed))
}

fn response_array<'a>(body: &'a Value, path: &[&str]) -> Result<&'a [Value]> {
    if body
        .get("code")
        .is_some_and(|code| code.as_i64() != Some(200))
    {
        return Err(anyhow!(i18n::t(Key::ApiLibraryPayloadMissing)));
    }
    let mut value = body;
    for segment in path {
        value = value
            .get(*segment)
            .ok_or_else(|| anyhow!(i18n::t(Key::ApiLibraryPayloadMissing)))?;
    }
    value
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| anyhow!(i18n::t(Key::ApiLibraryPayloadMissing)))
}

fn parse_search_payload(body: &Value, channel: SearchChannel) -> Result<SearchPayload> {
    require_success(body)?;
    match channel {
        SearchChannel::Songs => {
            let total = required_usize(body, &["result", "songCount"])?;
            Ok(SearchPayload::Songs(SearchPage {
                items: search_result_array(body, &["result", "songs"], total)?
                    .iter()
                    .map(parse_song_row)
                    .collect::<Result<_>>()?,
                total,
            }))
        }
        SearchChannel::Artists => {
            let total = required_usize(body, &["result", "artistCount"])?;
            Ok(SearchPayload::Artists(SearchPage {
                items: search_result_array(body, &["result", "artists"], total)?
                    .iter()
                    .map(parse_artist_hit)
                    .collect::<Result<_>>()?,
                total,
            }))
        }
        SearchChannel::Albums => {
            let total = required_usize(body, &["result", "albumCount"])?;
            Ok(SearchPayload::Albums(SearchPage {
                items: search_result_array(body, &["result", "albums"], total)?
                    .iter()
                    .map(parse_album_hit)
                    .collect::<Result<_>>()?,
                total,
            }))
        }
        SearchChannel::Playlists => {
            let total = required_usize(body, &["result", "playlistCount"])?;
            Ok(SearchPayload::Playlists(SearchPage {
                items: search_result_array(body, &["result", "playlists"], total)?
                    .iter()
                    .map(parse_playlist_hit)
                    .collect::<Result<_>>()?,
                total,
            }))
        }
    }
}

fn search_result_array<'a>(value: &'a Value, path: &[&str], total: usize) -> Result<&'a [Value]> {
    let Some((field, parent_path)) = path.split_last() else {
        return Err(invalid_payload("$"));
    };
    let parent = required_value(value, parent_path)?;
    match parent.get(*field) {
        Some(Value::Array(items)) => Ok(items),
        None if total == 0 => Ok(&[]),
        _ => Err(invalid_payload_path(path)),
    }
}

fn parse_song_collection(body: &Value, path: &[&str]) -> Result<Vec<SongRow>> {
    require_success(body)?;
    required_array(body, path)?
        .iter()
        .map(parse_song_row)
        .collect()
}

fn parse_playlist_detail(body: &Value) -> Result<(Vec<SongRow>, usize)> {
    let rows = parse_song_collection(body, &["playlist", "tracks"])?;
    let total = required_usize(body, &["playlist", "trackCount"])?;
    Ok((rows, total))
}

async fn complete_playlist_detail<F, Fut>(
    embedded: Vec<SongRow>,
    total: usize,
    fetch_all: F,
) -> Result<Vec<SongRow>>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<Vec<SongRow>>>,
{
    if embedded.len() >= total {
        Ok(embedded)
    } else {
        fetch_all().await
    }
}

fn parse_song_row(song: &Value) -> Result<SongRow> {
    let artists = required_array(song, &["ar"])?;
    let artist = artists.first().ok_or_else(|| invalid_payload("$.ar[0]"))?;
    let album = required_value(song, &["al"])?;
    let id = required_i64(song, &["id"])?;
    if id <= 0 {
        return Err(invalid_payload("$.id"));
    }
    let duration_ms = required_i64(song, &["dt"])?;
    if duration_ms < 0 {
        return Err(invalid_payload("$.dt"));
    }

    Ok(SongRow {
        id,
        title: required_string(song, &["name"])?,
        artist: required_string(artist, &["name"])?,
        album: required_string(album, &["name"])?,
        duration_ms,
        pic_url: optional_string(album, "picUrl")?,
    })
}

fn parse_artist_hit(artist: &Value) -> Result<ArtistHit> {
    let id = required_i64(artist, &["id"])?;
    if id <= 0 {
        return Err(invalid_payload("$.id"));
    }
    let pic_url = match optional_string(artist, "picUrl")? {
        Some(pic_url) => Some(pic_url),
        None => optional_string(artist, "img1v1Url")?,
    };
    Ok(ArtistHit {
        id,
        name: required_string(artist, &["name"])?,
        pic_url,
        album_count: required_usize(artist, &["albumSize"])?,
        song_count: required_usize(artist, &["musicSize"])?,
    })
}

fn parse_album_hit(album: &Value) -> Result<AlbumHit> {
    let id = required_i64(album, &["id"])?;
    if id <= 0 {
        return Err(invalid_payload("$.id"));
    }
    Ok(AlbumHit {
        id,
        name: required_string(album, &["name"])?,
        artist: required_string(album, &["artist", "name"])?,
        pic_url: optional_string(album, "picUrl")?,
        song_count: required_usize(album, &["size"])?,
    })
}

fn parse_playlist_hit(playlist: &Value) -> Result<PlaylistHit> {
    let id = required_i64(playlist, &["id"])?;
    if id <= 0 {
        return Err(invalid_payload("$.id"));
    }
    Ok(PlaylistHit {
        id,
        name: required_string(playlist, &["name"])?,
        creator: required_string(playlist, &["creator", "nickname"])?,
        cover_url: optional_string(playlist, "coverImgUrl")?,
        track_count: required_usize(playlist, &["trackCount"])?,
    })
}

fn require_success(body: &Value) -> Result<()> {
    match body.get("code").and_then(Value::as_i64) {
        Some(200) => Ok(()),
        _ => Err(invalid_payload("$.code")),
    }
}

fn required_value<'a>(value: &'a Value, path: &[&str]) -> Result<&'a Value> {
    let mut current = value;
    for segment in path {
        current = current
            .get(*segment)
            .ok_or_else(|| invalid_payload_path(path))?;
    }
    Ok(current)
}

fn required_array<'a>(value: &'a Value, path: &[&str]) -> Result<&'a [Value]> {
    required_value(value, path)?
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| invalid_payload_path(path))
}

fn required_string(value: &Value, path: &[&str]) -> Result<String> {
    required_value(value, path)?
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| invalid_payload_path(path))
}

fn optional_string(value: &Value, field: &str) -> Result<Option<String>> {
    match value.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => Ok((!text.is_empty()).then_some(text.clone())),
        Some(_) => Err(invalid_payload(&format!("$.{field}"))),
    }
}

fn required_i64(value: &Value, path: &[&str]) -> Result<i64> {
    required_value(value, path)?
        .as_i64()
        .ok_or_else(|| invalid_payload_path(path))
}

fn required_usize(value: &Value, path: &[&str]) -> Result<usize> {
    required_value(value, path)?
        .as_u64()
        .and_then(|number| usize::try_from(number).ok())
        .ok_or_else(|| invalid_payload_path(path))
}

fn invalid_payload_path(path: &[&str]) -> anyhow::Error {
    invalid_payload(&format!("$.{}", path.join(".")))
}

fn invalid_payload(path: &str) -> anyhow::Error {
    anyhow!("invalid NCM response at {path}")
}

/// Separate "the account may not ask" from "the track has no rights".
/// NCM answers `code: 200` with a null url for the second case and a
/// non-200 code (301 signed out, -462 risk control, 400 rate limited…)
/// for the first — reading only `data[0].url` conflates them.
fn classify_song_url(body: &Value) -> std::result::Result<PlaybackSource, SongUrlFailure> {
    let code = body.get("code").and_then(Value::as_i64);
    if code != Some(200) {
        return Err(SongUrlFailure::Rejected(code));
    }
    let data = &body["data"][0];
    if data["url"].as_str().is_none_or(str::is_empty) {
        return Err(SongUrlFailure::Unavailable);
    }
    parse_playback_source(data).map_err(SongUrlFailure::Other)
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

fn codec_from_url(url: &str) -> AudioCodec {
    url.split(['?', '#'])
        .next()
        .and_then(|path| path.rsplit('/').next())
        .and_then(|name| name.rsplit_once('.').map(|(_, extension)| extension))
        .and_then(|extension| extension.parse().ok())
        .unwrap_or(AudioCodec::Mp3)
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

fn decode_base64(input: &str) -> Result<Vec<u8>> {
    let bytes = input.as_bytes();
    if !bytes.len().is_multiple_of(4) {
        return Err(anyhow!("invalid base64 length"));
    }
    let mut decoded = Vec::with_capacity(bytes.len() / 4 * 3);
    let chunks = bytes.chunks_exact(4);
    let chunk_count = chunks.len();
    for (index, chunk) in chunks.enumerate() {
        let last = index + 1 == chunk_count;
        let high = base64_value(chunk[0]).ok_or_else(|| anyhow!("invalid base64 digit"))?;
        let low = base64_value(chunk[1]).ok_or_else(|| anyhow!("invalid base64 digit"))?;
        decoded.push((high << 2) | (low >> 4));
        match (chunk[2], chunk[3]) {
            (b'=', b'=') if last && low & 0x0f == 0 => {}
            (third, b'=') if last => {
                let third = base64_value(third).ok_or_else(|| anyhow!("invalid base64 digit"))?;
                if third & 0x03 != 0 {
                    return Err(anyhow!("invalid base64 padding"));
                }
                decoded.push((low << 4) | (third >> 2));
            }
            (third, fourth) if third != b'=' && fourth != b'=' => {
                let third = base64_value(third).ok_or_else(|| anyhow!("invalid base64 digit"))?;
                let fourth = base64_value(fourth).ok_or_else(|| anyhow!("invalid base64 digit"))?;
                decoded.push((low << 4) | (third >> 2));
                decoded.push((third << 6) | fourth);
            }
            _ => return Err(anyhow!("invalid base64 padding")),
        }
    }
    Ok(decoded)
}

fn base64_value(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
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

/// Tolerant mapping: daily/FM/cloud payloads use ar|artists, al|album,
/// dt|duration interchangeably.
fn song_row_flex(song: &Value) -> SongRow {
    let artist = song["ar"][0]["name"]
        .as_str()
        .or_else(|| song["artists"][0]["name"].as_str())
        .unwrap_or("?")
        .to_owned();
    let album = song["al"]["name"]
        .as_str()
        .or_else(|| song["album"]["name"].as_str())
        .unwrap_or("")
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
        album,
        duration_ms,
        pic_url,
    }
}

/// Small square cover JPEG from the NCM CDN (`?param=WxH` server-side crop).
pub async fn fetch_cover(pic_url: &str, edge: u32) -> Result<Vec<u8>> {
    let url = format!("{pic_url}?param={edge}y{edge}");
    let response = http_client()
        .get(&url)
        .send()
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
    use std::sync::atomic::{AtomicUsize, Ordering};

    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    use super::*;

    #[test]
    fn lyric_payload_keeps_all_supported_timeline_kinds() {
        let payload = parse_lyrics_payload(&serde_json::json!({
            "code": 200,
            "lrc": { "lyric": "[00:01]line" },
            "tlyric": { "lyric": "[00:01]翻译" },
            "yrc": { "lyric": "[1000,500](1000,500,0)line" }
        }))
        .unwrap();

        assert_eq!(payload.lrc, "[00:01]line");
        assert_eq!(payload.tlyric.as_deref(), Some("[00:01]翻译"));
        assert_eq!(payload.yrc.as_deref(), Some("[1000,500](1000,500,0)line"));
    }

    #[test]
    fn lyric_payload_accepts_missing_null_and_blank_optional_text() {
        let payload = parse_lyrics_payload(&serde_json::json!({
            "code": 200,
            "lrc": { "lyric": null },
            "tlyric": null
        }))
        .unwrap();

        assert_eq!(payload, LyricsPayload::default());

        let whitespace = parse_lyrics_payload(&serde_json::json!({
            "code": 200,
            "lrc": { "lyric": "  \n" },
            "tlyric": {},
            "yrc": null
        }))
        .unwrap();
        assert_eq!(whitespace, LyricsPayload::default());
    }

    #[test]
    fn lyric_payload_rejects_wrong_dynamic_types_and_unsuccessful_codes() {
        let malformed = [
            serde_json::json!({ "code": 200, "lrc": [] }),
            serde_json::json!({ "code": 200, "tlyric": { "lyric": 42 } }),
            serde_json::json!({ "code": 200, "yrc": ["not", "an", "object"] }),
            serde_json::json!({ "code": 500, "lrc": { "lyric": "ignored" } }),
            serde_json::json!({ "lrc": { "lyric": "missing code" } }),
        ];

        for body in malformed {
            assert!(parse_lyrics_payload(&body).is_err());
        }
    }

    fn ncm(quality: AudioQuality) -> Ncm {
        let dir = tempfile::tempdir().unwrap();
        Ncm::new(dir.path().join("session.json"), quality, true)
    }

    #[derive(Clone)]
    enum FakeUnmOutcome {
        Found(UnmResolution),
        Missing,
    }

    struct FakeUnm {
        outcome: FakeUnmOutcome,
        delay: Duration,
        calls: Arc<AtomicUsize>,
    }

    impl UnmResolver for FakeUnm {
        fn resolve<'a>(
            &'a self,
            _payload: &'a Value,
        ) -> BoxFuture<'a, Result<Option<UnmResolution>>> {
            Box::pin(async move {
                self.calls.fetch_add(1, Ordering::Relaxed);
                tokio::time::sleep(self.delay).await;
                Ok(match &self.outcome {
                    FakeUnmOutcome::Found(resolution) => Some(resolution.clone()),
                    FakeUnmOutcome::Missing => None,
                })
            })
        }
    }

    fn ncm_with_unm(
        enabled: bool,
        outcome: FakeUnmOutcome,
        delay: Duration,
        timeout: Duration,
        calls: Arc<AtomicUsize>,
    ) -> Ncm {
        let dir = tempfile::tempdir().unwrap();
        Ncm::with_unm(
            dir.path().join("session.json"),
            AudioQuality::High320,
            enabled,
            Arc::new(FakeUnm {
                outcome,
                delay,
                calls,
            }),
            timeout,
        )
    }

    fn unavailable_row() -> SongRow {
        SongRow {
            id: 42,
            title: "Unavailable".into(),
            artist: "Artist".into(),
            album: "Album".into(),
            duration_ms: 180_000,
            pic_url: None,
        }
    }

    fn song_payload() -> Value {
        serde_json::json!({
            "id": 186_016,
            "name": "晴天",
            "ar": [{ "id": 6_452, "name": "周杰伦" }],
            "al": {
                "id": 18_905,
                "name": "叶惠美",
                "picUrl": "https://example.test/cover.jpg"
            },
            "dt": 269_000
        })
    }

    #[test]
    fn search_channels_use_the_documented_ncm_types() {
        assert_eq!(
            SearchChannel::ALL.map(SearchChannel::api_type),
            ["1", "100", "10", "1000"]
        );
        assert_eq!(SearchChannel::Songs.cycle(-1), SearchChannel::Playlists);
        assert_eq!(SearchChannel::Playlists.cycle(1), SearchChannel::Songs);
        assert_eq!(SearchChannel::Albums.index(), 2);
    }

    #[test]
    fn search_payloads_narrow_all_four_channel_shapes() {
        let songs = parse_search_payload(
            &serde_json::json!({
                "code": 200,
                "result": { "songCount": 1, "songs": [song_payload()] }
            }),
            SearchChannel::Songs,
        )
        .unwrap();
        let SearchPayload::Songs(songs) = songs else {
            panic!("song search returned the wrong variant");
        };
        assert_eq!(songs.total, 1);
        assert_eq!(songs.items[0].title, "晴天");
        assert_eq!(songs.items[0].album, "叶惠美");

        let artists = parse_search_payload(
            &serde_json::json!({
                "code": 200,
                "result": {
                    "artistCount": 83,
                    "artists": [{
                        "id": 6_452,
                        "name": "周杰伦",
                        "picUrl": null,
                        "img1v1Url": "https://example.test/artist.jpg",
                        "albumSize": 41,
                        "musicSize": 568
                    }]
                }
            }),
            SearchChannel::Artists,
        )
        .unwrap();
        let SearchPayload::Artists(artists) = artists else {
            panic!("artist search returned the wrong variant");
        };
        assert_eq!(artists.total, 83);
        assert_eq!(artists.items[0].album_count, 41);
        assert_eq!(artists.items[0].song_count, 568);
        assert_eq!(
            artists.items[0].pic_url.as_deref(),
            Some("https://example.test/artist.jpg")
        );

        let albums = parse_search_payload(
            &serde_json::json!({
                "code": 200,
                "result": {
                    "albumCount": 12,
                    "albums": [{
                        "id": 18_905,
                        "name": "叶惠美",
                        "artist": { "id": 6_452, "name": "周杰伦" },
                        "picUrl": "https://example.test/album.jpg",
                        "size": 11
                    }]
                }
            }),
            SearchChannel::Albums,
        )
        .unwrap();
        let SearchPayload::Albums(albums) = albums else {
            panic!("album search returned the wrong variant");
        };
        assert_eq!(albums.total, 12);
        assert_eq!(albums.items[0].artist, "周杰伦");
        assert_eq!(albums.items[0].song_count, 11);

        let playlists = parse_search_payload(
            &serde_json::json!({
                "code": 200,
                "result": {
                    "playlistCount": 9,
                    "playlists": [{
                        "id": 19_723_756,
                        "name": "飙升榜",
                        "creator": { "nickname": "网易云音乐" },
                        "coverImgUrl": "https://example.test/playlist.jpg",
                        "trackCount": 100
                    }]
                }
            }),
            SearchChannel::Playlists,
        )
        .unwrap();
        let SearchPayload::Playlists(playlists) = playlists else {
            panic!("playlist search returned the wrong variant");
        };
        assert_eq!(playlists.total, 9);
        assert_eq!(playlists.items[0].creator, "网易云音乐");
        assert_eq!(playlists.items[0].track_count, 100);
    }

    #[test]
    fn search_payloads_accept_explicit_empty_results() {
        let cases = [
            (
                SearchChannel::Songs,
                serde_json::json!({
                    "code": 200,
                    "result": { "songCount": 0, "songs": [] }
                }),
            ),
            (
                SearchChannel::Artists,
                serde_json::json!({
                    "code": 200,
                    "result": { "artistCount": 0, "artists": [] }
                }),
            ),
            (
                SearchChannel::Albums,
                serde_json::json!({
                    "code": 200,
                    "result": { "albumCount": 0, "albums": [] }
                }),
            ),
            (
                SearchChannel::Playlists,
                serde_json::json!({
                    "code": 200,
                    "result": { "playlistCount": 0, "playlists": [] }
                }),
            ),
        ];

        for (channel, payload) in cases {
            let result = parse_search_payload(&payload, channel).unwrap();
            let (len, total) = match result {
                SearchPayload::Songs(page) => (page.items.len(), page.total),
                SearchPayload::Artists(page) => (page.items.len(), page.total),
                SearchPayload::Albums(page) => (page.items.len(), page.total),
                SearchPayload::Playlists(page) => (page.items.len(), page.total),
            };
            assert_eq!((len, total), (0, 0));
        }
    }

    #[test]
    fn search_payloads_accept_omitted_arrays_when_the_count_is_zero() {
        let cases = [
            (
                SearchChannel::Songs,
                serde_json::json!({ "code": 200, "result": { "songCount": 0 } }),
            ),
            (
                SearchChannel::Artists,
                serde_json::json!({ "code": 200, "result": { "artistCount": 0 } }),
            ),
            (
                SearchChannel::Albums,
                serde_json::json!({ "code": 200, "result": { "albumCount": 0 } }),
            ),
            (
                SearchChannel::Playlists,
                serde_json::json!({ "code": 200, "result": { "playlistCount": 0 } }),
            ),
        ];

        for (channel, payload) in cases {
            let result = parse_search_payload(&payload, channel).unwrap();
            let (len, total) = match result {
                SearchPayload::Songs(page) => (page.items.len(), page.total),
                SearchPayload::Artists(page) => (page.items.len(), page.total),
                SearchPayload::Albums(page) => (page.items.len(), page.total),
                SearchPayload::Playlists(page) => (page.items.len(), page.total),
            };
            assert_eq!((len, total), (0, 0));
        }
    }

    #[test]
    fn search_payloads_reject_missing_or_wrongly_typed_fields() {
        let cases = [
            (
                SearchChannel::Songs,
                serde_json::json!({
                    "code": 200,
                    "result": { "songCount": 1 }
                }),
            ),
            (
                SearchChannel::Artists,
                serde_json::json!({
                    "code": 200,
                    "result": {
                        "artistCount": 1,
                        "artists": [{
                            "id": "6452",
                            "name": "周杰伦",
                            "albumSize": 41,
                            "musicSize": 568
                        }]
                    }
                }),
            ),
            (
                SearchChannel::Albums,
                serde_json::json!({
                    "code": 200,
                    "result": {
                        "albumCount": 1,
                        "albums": [{
                            "id": 18_905,
                            "name": "叶惠美",
                            "artist": {},
                            "size": 11
                        }]
                    }
                }),
            ),
            (
                SearchChannel::Playlists,
                serde_json::json!({
                    "code": 200,
                    "result": {
                        "playlistCount": -1,
                        "playlists": []
                    }
                }),
            ),
        ];

        for (channel, payload) in cases {
            assert!(parse_search_payload(&payload, channel).is_err());
        }
        assert!(parse_search_payload(
            &serde_json::json!({ "result": { "songCount": 0, "songs": [] } }),
            SearchChannel::Songs,
        )
        .is_err());
        assert!(parse_artist_hit(&serde_json::json!({
            "id": 6_452,
            "name": "周杰伦",
            "picUrl": 42,
            "albumSize": 41,
            "musicSize": 568
        }))
        .is_err());
    }

    #[test]
    fn detail_payloads_narrow_artist_album_and_playlist_tracks() {
        let song = song_payload();
        let artist = parse_song_collection(
            &serde_json::json!({ "code": 200, "songs": [song.clone()] }),
            &["songs"],
        )
        .unwrap();
        let album = parse_song_collection(
            &serde_json::json!({ "code": 200, "songs": [song.clone()] }),
            &["songs"],
        )
        .unwrap();
        let playlist = parse_song_collection(
            &serde_json::json!({
                "code": 200,
                "playlist": { "tracks": [song] }
            }),
            &["playlist", "tracks"],
        )
        .unwrap();

        assert_eq!(artist, album);
        assert_eq!(album, playlist);
        assert_eq!(
            playlist[0].pic_url.as_deref(),
            Some("https://example.test/cover.jpg")
        );
    }

    #[test]
    fn playlist_detail_reports_when_embedded_tracks_need_paged_completion() {
        let (rows, total) = parse_playlist_detail(&serde_json::json!({
            "code": 200,
            "playlist": {
                "trackCount": 2,
                "tracks": [song_payload()]
            }
        }))
        .unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(total, 2);
        assert!(rows.len() < total);
    }

    #[tokio::test]
    async fn complete_playlist_detail_keeps_complete_embedded_tracks_without_fetching() {
        let embedded = rows(1..3);
        let expected = embedded.clone();
        let fetch_calls = AtomicUsize::new(0);

        let result = complete_playlist_detail(embedded, expected.len(), || {
            fetch_calls.fetch_add(1, Ordering::Relaxed);
            std::future::ready(Ok(rows(100..101)))
        })
        .await
        .unwrap();

        assert_eq!(result, expected);
        assert_eq!(fetch_calls.load(Ordering::Relaxed), 0);
    }

    #[tokio::test]
    async fn complete_playlist_detail_fetches_full_tracks_once_when_embedded_is_partial() {
        let full = rows(1..4);
        let expected = full.clone();
        let fetch_calls = AtomicUsize::new(0);

        let result = complete_playlist_detail(rows(1..2), expected.len(), || {
            fetch_calls.fetch_add(1, Ordering::Relaxed);
            std::future::ready(Ok(full))
        })
        .await
        .unwrap();

        assert_eq!(result, expected);
        assert_eq!(fetch_calls.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn detail_payloads_reject_unknown_song_shapes_instead_of_defaulting() {
        let missing_duration = serde_json::json!({
            "code": 200,
            "songs": [{
                "id": 186_016,
                "name": "晴天",
                "ar": [{ "name": "周杰伦" }],
                "al": { "name": "叶惠美" }
            }]
        });
        let wrong_artists = serde_json::json!({
            "code": 200,
            "songs": [{
                "id": 186_016,
                "name": "晴天",
                "ar": { "name": "周杰伦" },
                "al": { "name": "叶惠美" },
                "dt": 269_000
            }]
        });

        assert!(parse_song_collection(&missing_duration, &["songs"]).is_err());
        assert!(parse_song_collection(&wrong_artists, &["songs"]).is_err());
        assert!(parse_song_collection(
            &serde_json::json!({ "code": 200, "playlist": {} }),
            &["playlist", "tracks"],
        )
        .is_err());
    }

    #[test]
    fn bilibili_base64_decoder_handles_complete_and_padded_groups() {
        assert_eq!(decode_base64("YQ==").unwrap(), b"a");
        assert_eq!(decode_base64("YWI=").unwrap(), b"ab");
        assert_eq!(decode_base64("YWJj").unwrap(), b"abc");
        assert!(decode_base64("YQ=A").is_err());
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
            assert_eq!(ncm(quality).quality().bitrate(), expected);
        }
    }

    #[tokio::test]
    async fn unavailable_native_audio_uses_unm_and_decodes_bilibili_bytes() {
        let calls = Arc::new(AtomicUsize::new(0));
        let ncm = ncm_with_unm(
            true,
            FakeUnmOutcome::Found(UnmResolution {
                provider: "bilibili".into(),
                url: "YXVkaW8gYnl0ZXM=".into(),
            }),
            Duration::ZERO,
            Duration::from_secs(1),
            calls.clone(),
        );

        let track = ncm
            .resolve_after_native(
                &unavailable_row(),
                AudioQuality::High320,
                Err(SongUrlFailure::Unavailable),
            )
            .await
            .unwrap();

        assert_eq!(calls.load(Ordering::Relaxed), 1);
        assert_eq!(
            track.media,
            ResolvedMedia::UnmBytes(b"audio bytes".to_vec())
        );
        assert_eq!(track.codec, AudioCodec::Mp3);
        assert_eq!(track.actual_bitrate, 128_000);
    }

    #[tokio::test]
    async fn unavailable_native_audio_uses_a_regular_unm_url() {
        let calls = Arc::new(AtomicUsize::new(0));
        let url = "https://audio.example/recovered.FLAC?token=secret";
        let ncm = ncm_with_unm(
            true,
            FakeUnmOutcome::Found(UnmResolution {
                provider: "kugou".into(),
                url: url.into(),
            }),
            Duration::ZERO,
            Duration::from_secs(1),
            calls.clone(),
        );

        let track = ncm
            .resolve_after_native(
                &unavailable_row(),
                AudioQuality::High320,
                Err(SongUrlFailure::Unavailable),
            )
            .await
            .unwrap();

        assert_eq!(calls.load(Ordering::Relaxed), 1);
        assert_eq!(track.media, ResolvedMedia::UnmUrl(url.into()));
        assert_eq!(track.codec, AudioCodec::Flac);
    }

    #[tokio::test]
    async fn disabled_unm_does_not_call_the_resolver() {
        let calls = Arc::new(AtomicUsize::new(0));
        let ncm = ncm_with_unm(
            false,
            FakeUnmOutcome::Missing,
            Duration::ZERO,
            Duration::from_secs(1),
            calls.clone(),
        );

        let error = ncm
            .resolve_after_native(
                &unavailable_row(),
                AudioQuality::High320,
                Err(SongUrlFailure::Unavailable),
            )
            .await
            .unwrap_err();

        assert!(error.is::<TrackUnavailable>());
        assert_eq!(calls.load(Ordering::Relaxed), 0);
    }

    #[tokio::test]
    async fn unm_timeout_is_bounded_and_returns_unavailable() {
        let calls = Arc::new(AtomicUsize::new(0));
        let ncm = ncm_with_unm(
            true,
            FakeUnmOutcome::Missing,
            Duration::from_secs(1),
            Duration::from_millis(10),
            calls.clone(),
        );

        let error = ncm
            .resolve_after_native(
                &unavailable_row(),
                AudioQuality::High320,
                Err(SongUrlFailure::Unavailable),
            )
            .await
            .unwrap_err();

        assert!(error.is::<TrackUnavailable>());
        assert_eq!(calls.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn native_transport_errors_never_trigger_unm() {
        let calls = Arc::new(AtomicUsize::new(0));
        let ncm = ncm_with_unm(
            true,
            FakeUnmOutcome::Missing,
            Duration::ZERO,
            Duration::from_secs(1),
            calls.clone(),
        );

        let error = ncm
            .resolve_after_native(
                &unavailable_row(),
                AudioQuality::High320,
                Err(SongUrlFailure::Other(anyhow!("offline"))),
            )
            .await
            .unwrap_err();

        assert_eq!(error.to_string(), "offline");
        assert_eq!(calls.load(Ordering::Relaxed), 0);
    }

    fn rows(range: std::ops::Range<usize>) -> Vec<SongRow> {
        range
            .map(|id| SongRow {
                id: id as i64,
                title: format!("Track {id}"),
                artist: "Artist".into(),
                album: "Album".into(),
                duration_ms: 180_000,
                pic_url: None,
            })
            .collect()
    }

    #[tokio::test]
    async fn playlist_paging_keeps_all_rows_in_order() {
        let pages = [rows(0..500), rows(500..1000), Vec::new()];
        let mut calls = Vec::new();

        let result = collect_playlist_pages(|offset| {
            calls.push(offset);
            let page = pages[calls.len() - 1].clone();
            async move { Ok(page) }
        })
        .await
        .unwrap();

        assert_eq!(calls, vec![0, 500, 1000]);
        assert_eq!(result.len(), 1000);
        assert!(result
            .iter()
            .enumerate()
            .all(|(index, row)| row.id == index as i64));
    }

    #[tokio::test]
    async fn playlist_paging_fetches_the_partial_second_page() {
        let pages = [rows(0..500), rows(500..501)];
        let mut calls = Vec::new();

        let result = collect_playlist_pages(|offset| {
            calls.push(offset);
            let page = pages[calls.len() - 1].clone();
            async move { Ok(page) }
        })
        .await
        .unwrap();

        assert_eq!(calls, vec![0, 500]);
        assert_eq!(result.len(), 501);
        assert_eq!(result[500].id, 500);
    }

    #[tokio::test]
    async fn cloud_paging_follows_has_more_and_preserves_order() {
        let pages = [(rows(0..500), Some(true)), (rows(500..501), Some(false))];
        let mut calls = Vec::new();

        let result = collect_cloud_pages(|offset| {
            calls.push(offset);
            let page = pages[calls.len() - 1].clone();
            async move { Ok(page) }
        })
        .await
        .unwrap();

        assert_eq!(calls, vec![0, 500]);
        assert_eq!(result.len(), 501);
        assert_eq!(result[500].id, 500);
    }

    #[tokio::test]
    async fn cloud_paging_falls_back_to_page_length_without_has_more() {
        let pages = [(rows(0..500), None), (rows(500..501), None)];
        let mut calls = Vec::new();

        let result = collect_cloud_pages(|offset| {
            calls.push(offset);
            let page = pages[calls.len() - 1].clone();
            async move { Ok(page) }
        })
        .await
        .unwrap();

        assert_eq!(calls, vec![0, 500]);
        assert_eq!(result.len(), 501);
    }

    #[tokio::test]
    async fn empty_cloud_page_stops_even_when_has_more_is_true() {
        let mut calls = Vec::new();

        let result = collect_cloud_pages(|offset| {
            calls.push(offset);
            async { Ok((Vec::new(), Some(true))) }
        })
        .await
        .unwrap();

        assert_eq!(calls, vec![0]);
        assert!(result.is_empty());
    }

    #[test]
    fn library_payloads_distinguish_an_explicit_empty_list_from_missing_data() {
        let empty_payloads: [(Value, &[&str]); 5] = [
            (serde_json::json!({ "code": 200, "songs": [] }), &["songs"]),
            (serde_json::json!({ "code": 200, "ids": [] }), &["ids"]),
            (
                serde_json::json!({ "code": 200, "data": { "dailySongs": [] } }),
                &["data", "dailySongs"],
            ),
            (serde_json::json!({ "code": 200, "data": [] }), &["data"]),
            (serde_json::json!({ "data": [] }), &["data"]),
        ];
        for (body, path) in &empty_payloads {
            assert!(response_array(body, path).unwrap().is_empty());
        }

        let missing_payloads: [(Value, &[&str]); 5] = [
            (serde_json::json!({ "code": 200 }), &["songs"]),
            (serde_json::json!({ "code": 200 }), &["ids"]),
            (
                serde_json::json!({ "code": 200, "data": {} }),
                &["data", "dailySongs"],
            ),
            (serde_json::json!({ "code": 200 }), &["data"]),
            (serde_json::json!({ "code": 301, "data": [] }), &["data"]),
        ];
        for (body, path) in &missing_payloads {
            assert!(response_array(body, path).is_err());
        }
    }

    #[test]
    fn song_rows_preserve_album_names_from_both_payload_shapes() {
        let standard = parse_song_row(&serde_json::json!({
            "id": 1,
            "name": "Track",
            "ar": [{ "name": "Artist" }],
            "al": { "name": "Standard Album", "picUrl": null },
            "dt": 180_000
        }))
        .unwrap();
        let flexible = song_row_flex(&serde_json::json!({
            "id": 2,
            "name": "Cloud Track",
            "artists": [{ "name": "Cloud Artist" }],
            "album": { "name": "Flexible Album", "picUrl": null },
            "duration": 210_000
        }));

        assert_eq!(standard.album, "Standard Album");
        assert_eq!(flexible.album, "Flexible Album");
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
        let ncm = ncm(AudioQuality::High320);
        let requested_quality = ncm.quality();
        ncm.set_quality(AudioQuality::HiRes);
        let track = ncm.resolved_track(
            SongRow {
                id: 42,
                title: "Track".into(),
                artist: "Artist".into(),
                album: "Album".into(),
                duration_ms: 180_000,
                pic_url: None,
            },
            requested_quality,
            PlaybackSource {
                url: "https://example.test/audio.mp3".into(),
                codec: AudioCodec::Mp3,
                actual_bitrate: 320_000,
                expected_bytes: Some(7_654_321),
                expected_md5: Some([0x11; 16]),
            },
        );

        assert_eq!(track.cache_key, CacheKey::new(42, AudioQuality::High320));
        assert_eq!(track.codec, AudioCodec::Mp3);
        assert_eq!(track.kind, "mp3");
        assert_eq!(track.actual_bitrate, 320_000);
        assert_eq!(track.album, "Album");
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
    fn unm_url_codec_uses_the_path_extension_and_defaults_to_mp3() {
        assert_eq!(
            codec_from_url("https://audio.example/track.FLAC?token=secret"),
            AudioCodec::Flac
        );
        assert_eq!(
            codec_from_url("https://audio.example/signed?token=secret"),
            AudioCodec::Mp3
        );
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

    #[tokio::test]
    async fn http_requests_give_up_instead_of_hanging_on_a_silent_server() {
        // Accepts the connection, then never answers — the failure mode a
        // bare `reqwest::get` waits out forever.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let stalled = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            std::future::pending::<()>().await;
            drop(stream);
        });

        let client = build_http_client(Duration::from_millis(200), Duration::from_millis(200));
        let started = std::time::Instant::now();
        let error = client
            .get(format!("http://{address}/cover"))
            .send()
            .await
            .unwrap_err();

        assert!(error.is_timeout(), "expected a timeout, got {error:?}");
        assert!(started.elapsed() < Duration::from_secs(5));
        stalled.abort();
    }

    #[test]
    fn shared_http_client_carries_the_timeouts_covers_rely_on() {
        assert_eq!(HTTP_CONNECT_TIMEOUT, Duration::from_secs(10));
        assert_eq!(HTTP_REQUEST_TIMEOUT, Duration::from_secs(30));
        // Same instance every call: the pool has to survive across covers.
        assert!(std::ptr::eq(http_client(), http_client()));
    }

    #[test]
    fn song_url_separates_refusals_from_tracks_without_rights() {
        let refused = classify_song_url(&serde_json::json!({
            "code": 301,
            "data": [{ "url": Value::Null }]
        }));
        assert!(matches!(refused, Err(SongUrlFailure::Rejected(Some(301)))));

        let risk_control = classify_song_url(&serde_json::json!({ "code": -462 }));
        assert!(matches!(
            risk_control,
            Err(SongUrlFailure::Rejected(Some(-462)))
        ));

        let codeless = classify_song_url(&serde_json::json!({ "data": [] }));
        assert!(matches!(codeless, Err(SongUrlFailure::Rejected(None))));

        let unavailable = classify_song_url(&serde_json::json!({
            "code": 200,
            "data": [{ "url": Value::Null }]
        }));
        assert!(matches!(unavailable, Err(SongUrlFailure::Unavailable)));

        let playable = classify_song_url(&serde_json::json!({
            "code": 200,
            "data": [{
                "url": "https://audio.example/track.mp3",
                "type": "mp3",
                "br": 320_000,
                "size": 8_000_000,
                "md5": Value::Null
            }]
        }))
        .unwrap();
        assert_eq!(playable.url, "https://audio.example/track.mp3");
        assert_eq!(playable.actual_bitrate, 320_000);
    }

    #[tokio::test]
    async fn refused_song_url_reports_sign_in_instead_of_burning_a_unm_call() {
        let calls = Arc::new(AtomicUsize::new(0));
        let ncm = ncm_with_unm(
            true,
            FakeUnmOutcome::Found(UnmResolution {
                provider: "kugou".into(),
                url: "https://audio.example/recovered.mp3".into(),
            }),
            Duration::ZERO,
            Duration::from_secs(1),
            calls.clone(),
        );

        let error = ncm
            .resolve_after_native(
                &unavailable_row(),
                AudioQuality::High320,
                Err(SongUrlFailure::Rejected(Some(-462))),
            )
            .await
            .unwrap_err();

        assert_eq!(calls.load(Ordering::Relaxed), 0);
        assert!(!error.is::<TrackUnavailable>());
        assert!(error.to_string().contains("-462"));
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
