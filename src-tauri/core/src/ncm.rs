//! Typed NCM endpoint client shared by every Rust frontend.
//!
//! Every call injects the persisted session cookie; anonymous calls degrade
//! the same way the desktop client does (standard quality, no personal data).
//! Errors are typed so each frontend attaches its own user-facing wording
//! (the TUI translates them, a future CLI can print them raw).

use std::path::PathBuf;
use std::sync::RwLock;

use ncm_api_rs::{api::Query, ApiClient, NcmError};
use serde_json::Value;

use crate::auth::{Session, SessionStore};
use crate::media::AudioCodec;

const PLAYLIST_PAGE_SIZE: usize = 500;

#[derive(Debug, thiserror::Error)]
pub enum NcmClientError {
    #[error(transparent)]
    Api(#[from] NcmError),
    #[error("NCM response is missing {0}")]
    MissingPayload(&'static str),
    #[error("invalid NCM response at {0}")]
    InvalidPayload(String),
    #[error("{0}")]
    MalformedPayload(&'static str),
    /// The transport answered, but the body's own code refused the request.
    #[error("NCM rejected the request (code {0:?})")]
    Rejected(Option<i64>),
    #[error("QR login answered an unknown status {0}")]
    UnknownQrStatus(i64),
    #[error("QR login succeeded without a session cookie")]
    LoginCookieMissing,
    #[error("could not persist the session: {0}")]
    PersistSession(#[from] std::io::Error),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QrStatus {
    Waiting,
    Scanned,
    Expired,
    Success(Session),
}

/// Why `account()` failed — the split decides whether a frontend treats the
/// stored session as dead or merely unverifiable.
#[derive(Debug)]
pub enum AccountError {
    /// NCM never answered (offline, DNS, timeout, rate limit, 5xx). The
    /// stored session may still be valid; don't log the user out over it.
    Unreachable(AccountReason),
    /// NCM answered and rejected or omitted the account: the session is dead.
    Expired(AccountReason),
}

#[derive(Debug)]
pub enum AccountReason {
    Api(NcmError),
    /// The body carried no usable account payload.
    InvalidPayload,
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
    /// Romanized lines — only the GUI renders these today.
    pub romalrc: Option<String>,
    pub yrc: Option<String>,
}

/// The cloudsearch channels both frontends can request. Tab order and
/// cycling are view concerns and live with each frontend.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum SearchChannel {
    #[default]
    Songs,
    Artists,
    Albums,
    Playlists,
    MusicVideos,
    Users,
}

impl SearchChannel {
    pub const fn api_type(self) -> &'static str {
        match self {
            Self::Songs => "1",
            Self::Artists => "100",
            Self::Albums => "10",
            Self::Playlists => "1000",
            Self::MusicVideos => "1004",
            Self::Users => "1002",
        }
    }

    pub fn from_api_type(code: &str) -> Option<Self> {
        match code {
            "1" => Some(Self::Songs),
            "100" => Some(Self::Artists),
            "10" => Some(Self::Albums),
            "1000" => Some(Self::Playlists),
            "1004" => Some(Self::MusicVideos),
            "1002" => Some(Self::Users),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchPage<T> {
    pub items: Vec<T>,
    pub total: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtistRef {
    pub id: i64,
    pub name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AlbumRef {
    pub id: i64,
    pub name: String,
    pub pic_url: Option<String>,
}

/// Per-song play permission bits, carried verbatim so frontends can apply
/// their account-aware playability policy (VIP tier lives client-side).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SongPrivilege {
    pub pl: i64,
    pub cs: bool,
    pub fee: i64,
    pub st: i64,
}

/// One song search result with everything the richest frontend renders:
/// linkable artist/album ids, subtitle aliases, the explicit-content mark
/// and raw permission fields.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SongHit {
    pub id: i64,
    pub name: String,
    pub artists: Vec<ArtistRef>,
    pub album: AlbumRef,
    pub duration_ms: i64,
    pub alias: Vec<String>,
    pub trans_names: Vec<String>,
    pub mark: i64,
    pub fee: Option<i64>,
    pub no_copyright_rcmd: bool,
    pub privilege: Option<SongPrivilege>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtistHit {
    pub id: i64,
    pub name: String,
    pub pic_url: Option<String>,
    /// The square avatar variant the GUI prefers over `pic_url`.
    pub img1v1_url: Option<String>,
    pub album_count: usize,
    pub song_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AlbumHit {
    pub id: i64,
    pub name: String,
    pub artist: ArtistRef,
    pub pic_url: Option<String>,
    pub song_count: usize,
    pub mark: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlaylistHit {
    pub id: i64,
    pub name: String,
    pub creator: String,
    pub cover_url: Option<String>,
    pub track_count: usize,
    /// `10` means a private playlist (the GUI shows a lock badge).
    pub privacy: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MvHit {
    pub id: i64,
    pub name: String,
    pub cover_url: Option<String>,
    pub artist_id: i64,
    pub artist_name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserHit {
    pub user_id: i64,
    pub nickname: String,
    pub avatar_url: Option<String>,
    /// `0` means no VIP; the GUI's settings page badges on it.
    pub vip_type: i64,
    pub signature: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SearchPayload {
    Songs(SearchPage<SongHit>),
    Artists(SearchPage<ArtistHit>),
    Albums(SearchPage<AlbumHit>),
    Playlists(SearchPage<PlaylistHit>),
    MusicVideos(SearchPage<MvHit>),
    Users(SearchPage<UserHit>),
}

#[derive(Debug, Eq, PartialEq)]
pub struct PlaybackSource {
    pub url: String,
    pub codec: AudioCodec,
    pub actual_bitrate: u32,
    pub expected_bytes: Option<u64>,
    pub expected_md5: Option<[u8; 16]>,
}

#[derive(Debug)]
pub enum SongUrlError {
    /// NCM answered `code: 200` with no playable URL — really no rights.
    Unavailable,
    /// NCM refused the request itself (expired cookie, rate limit, risk
    /// control). Distinct from `Unavailable` so fallback providers don't
    /// hide a fixable sign-in problem behind "no copyright".
    Rejected(Option<i64>),
    Other(NcmClientError),
}

pub struct NcmClient {
    client: ApiClient,
    store: SessionStore,
    session: RwLock<Option<Session>>,
}

impl NcmClient {
    pub fn new(session_path: impl Into<PathBuf>) -> Self {
        let store = SessionStore::new(session_path);
        let session = RwLock::new(store.load());
        Self {
            client: ApiClient::new(None),
            store,
            session,
        }
    }

    pub fn session_snapshot(&self) -> Option<Session> {
        self.session.read().ok().and_then(|session| session.clone())
    }

    pub fn commit_session(&self, session: &Session) -> Result<(), NcmClientError> {
        self.store.save(session)?;
        *self.session.write().expect("session lock") = Some(session.clone());
        Ok(())
    }

    fn query(&self) -> Query {
        let session = self.session_snapshot();
        Self::query_with_session(session.as_ref())
    }

    fn query_with_session(session: Option<&Session>) -> Query {
        match session.map(Session::cookie_header) {
            Some(cookie) => Query::new().cookie(&cookie),
            None => Query::new(),
        }
    }

    // ── login ────────────────────────────────────────────────────────

    pub async fn qr_key(&self) -> Result<String, NcmClientError> {
        let response = self.client.login_qr_key(&self.query()).await?;
        let body = &response.body;
        body["unikey"]
            .as_str()
            .or_else(|| body["data"]["unikey"].as_str())
            .map(str::to_owned)
            .ok_or(NcmClientError::MissingPayload("unikey"))
    }

    pub fn qr_login_url(key: &str) -> String {
        format!("https://music.163.com/login?codekey={key}")
    }

    pub async fn qr_check(&self, key: &str) -> Result<QrStatus, NcmClientError> {
        let query = self.query().param("key", key);
        let response = self.client.login_qr_check(&query).await?;
        parse_qr_status(&response.body, &response.cookie)
    }

    // ── account ──────────────────────────────────────────────────────

    pub async fn account(&self, session: Option<&Session>) -> Result<(i64, String), AccountError> {
        let response = self
            .client
            .user_account(&Self::query_with_session(session))
            .await
            .map_err(classify_account_error)?;
        account_from_body(&response.body)
    }

    // ── library ──────────────────────────────────────────────────────

    /// The user's "我喜欢的音乐" — by NCM convention the first playlist.
    pub async fn liked_playlist_id(
        &self,
        uid: i64,
        session: Option<&Session>,
    ) -> Result<i64, NcmClientError> {
        let query = Self::query_with_session(session)
            .param("uid", &uid.to_string())
            .param("limit", "1");
        let response = self.client.user_playlist(&query).await?;
        response.body["playlist"][0]["id"]
            .as_i64()
            .ok_or(NcmClientError::MissingPayload("the liked playlist"))
    }

    pub async fn playlist_songs(
        &self,
        playlist_id: i64,
        session: Option<&Session>,
    ) -> Result<Vec<SongRow>, NcmClientError> {
        collect_playlist_pages(|offset| self.playlist_songs_page(playlist_id, session, offset))
            .await
    }

    async fn playlist_songs_page(
        &self,
        playlist_id: i64,
        session: Option<&Session>,
        offset: usize,
    ) -> Result<Vec<SongRow>, NcmClientError> {
        let query = Self::query_with_session(session)
            .param("id", &playlist_id.to_string())
            .param("limit", &PLAYLIST_PAGE_SIZE.to_string())
            .param("offset", &offset.to_string());
        let response = self.client.playlist_track_all(&query).await?;
        parse_song_collection(&response.body, &["songs"])
    }

    pub async fn set_like(
        &self,
        id: i64,
        like: bool,
        session: Option<&Session>,
    ) -> Result<(), NcmClientError> {
        set_like_with(&self.client, Self::query_with_session(session), id, like).await
    }

    /// Ordered as NCM answers (most recently liked first) — the GUI renders
    /// the head of this list, so it must stay a sequence, not a set.
    pub async fn liked_ids(
        &self,
        uid: i64,
        session: Option<&Session>,
    ) -> Result<Vec<i64>, NcmClientError> {
        liked_ids_with(&self.client, Self::query_with_session(session), uid).await
    }

    pub async fn daily_songs(
        &self,
        session: Option<&Session>,
    ) -> Result<Vec<SongRow>, NcmClientError> {
        let response = self
            .client
            .recommend_songs(&Self::query_with_session(session))
            .await?;
        let songs = response_array(&response.body, &["data", "dailySongs"])?;
        Ok(songs.iter().map(song_row_flex).collect())
    }

    pub async fn personal_fm(
        &self,
        session: Option<&Session>,
    ) -> Result<Vec<SongRow>, NcmClientError> {
        let response = self
            .client
            .personal_fm(&Self::query_with_session(session))
            .await?;
        let songs = response_array(&response.body, &["data"])?;
        Ok(songs.iter().map(song_row_flex).collect())
    }

    /// FM trash ("never play this again").
    pub async fn fm_trash(&self, id: i64, session: Option<&Session>) -> Result<(), NcmClientError> {
        fm_trash_with(&self.client, Self::query_with_session(session), id).await
    }

    pub async fn cloud_songs(
        &self,
        session: Option<&Session>,
    ) -> Result<Vec<SongRow>, NcmClientError> {
        collect_cloud_pages(|offset| self.cloud_songs_page(session, offset)).await
    }

    async fn cloud_songs_page(
        &self,
        session: Option<&Session>,
        offset: usize,
    ) -> Result<(Vec<SongRow>, Option<bool>), NcmClientError> {
        let query = Self::query_with_session(session)
            .param("limit", &PLAYLIST_PAGE_SIZE.to_string())
            .param("offset", &offset.to_string());
        let response = self.client.user_cloud(&query).await?;
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

    // ── playback & metadata ──────────────────────────────────────────

    pub async fn song_url(&self, id: i64, bitrate: u32) -> Result<PlaybackSource, SongUrlError> {
        song_url_with(&self.client, self.query(), id, bitrate).await
    }

    /// Raw line-, translation-, and word-synchronised lyrics for a song.
    pub async fn lyrics(&self, id: i64) -> Result<LyricsPayload, NcmClientError> {
        lyrics_with(&self.client, self.query(), id).await
    }

    pub async fn search_channel(
        &self,
        keywords: &str,
        channel: SearchChannel,
        limit: u32,
        offset: u32,
    ) -> Result<SearchPayload, NcmClientError> {
        search_with(&self.client, self.query(), keywords, channel, limit, offset).await
    }

    pub async fn artist_top_songs(&self, artist_id: i64) -> Result<Vec<SongRow>, NcmClientError> {
        let query = self.query().param("id", &artist_id.to_string());
        let response = self.client.artist_top_song(&query).await?;
        parse_song_collection(&response.body, &["songs"])
    }

    pub async fn album_songs(&self, album_id: i64) -> Result<Vec<SongRow>, NcmClientError> {
        let query = self.query().param("id", &album_id.to_string());
        let response = self.client.album(&query).await?;
        parse_song_collection(&response.body, &["songs"])
    }

    pub async fn playlist_detail_songs(
        &self,
        playlist_id: i64,
    ) -> Result<Vec<SongRow>, NcmClientError> {
        let query = self.query().param("id", &playlist_id.to_string());
        let response = self.client.playlist_detail(&query).await?;
        let (embedded, total) = parse_playlist_detail(&response.body)?;
        let session = self.session_snapshot();
        complete_playlist_detail(embedded, total, || {
            self.playlist_songs(playlist_id, session.as_ref())
        })
        .await
    }

    pub async fn search_songs(
        &self,
        keywords: &str,
        limit: u32,
    ) -> Result<Vec<Value>, NcmClientError> {
        let query = self
            .query()
            .param("keywords", keywords)
            .param("type", SearchChannel::Songs.api_type())
            .param("limit", &limit.to_string());
        let response = self.client.cloudsearch(&query).await?;
        require_success(&response.body)?;
        Ok(response_array(&response.body, &["result", "songs"])?.to_vec())
    }
}

fn parse_qr_status(body: &Value, cookies: &[String]) -> Result<QrStatus, NcmClientError> {
    match body["code"].as_i64().unwrap_or(0) {
        800 => Ok(QrStatus::Expired),
        801 => Ok(QrStatus::Waiting),
        802 => Ok(QrStatus::Scanned),
        803 => Session::from_set_cookies(cookies)
            .map(QrStatus::Success)
            .ok_or(NcmClientError::LoginCookieMissing),
        other => Err(NcmClientError::UnknownQrStatus(other)),
    }
}

/// Only an explicit auth rejection proves the session expired; every other
/// failure mode (transport, throttling, server trouble) leaves it unknown.
fn classify_account_error(error: NcmError) -> AccountError {
    match error {
        NcmError::AuthRequired(_) => AccountError::Expired(AccountReason::Api(error)),
        _ => AccountError::Unreachable(AccountReason::Api(error)),
    }
}

/// NCM's logged-out answer is a well-formed code-200 body with no account.
/// Anything else missing the account — captive-portal HTML passed through as
/// a string body, an EAPI decrypt that fell back to null — never proves the
/// session dead.
fn account_from_body(body: &Value) -> Result<(i64, String), AccountError> {
    match parse_account(body) {
        Some(account) => Ok(account),
        None if body["code"].as_i64() == Some(200) => {
            Err(AccountError::Expired(AccountReason::InvalidPayload))
        }
        None => Err(AccountError::Unreachable(AccountReason::InvalidPayload)),
    }
}

fn parse_account(body: &Value) -> Option<(i64, String)> {
    let uid = body["account"]["id"].as_i64()?;
    let nickname = body["profile"]["nickname"]
        .as_str()
        .unwrap_or("")
        .to_owned();
    Some((uid, nickname))
}

async fn collect_playlist_pages<F, Fut>(mut fetch: F) -> Result<Vec<SongRow>, NcmClientError>
where
    F: FnMut(usize) -> Fut,
    Fut: std::future::Future<Output = Result<Vec<SongRow>, NcmClientError>>,
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

async fn collect_cloud_pages<F, Fut>(mut fetch: F) -> Result<Vec<SongRow>, NcmClientError>
where
    F: FnMut(usize) -> Fut,
    Fut: std::future::Future<Output = Result<(Vec<SongRow>, Option<bool>), NcmClientError>>,
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

/// Library payloads answer with a code-200 body whose arrays may legally be
/// empty; anything else (bad code, missing path, non-array) is one condition
/// for the frontends: the library payload is missing.
fn response_array<'a>(body: &'a Value, path: &[&str]) -> Result<&'a [Value], NcmClientError> {
    if body
        .get("code")
        .is_some_and(|code| code.as_i64() != Some(200))
    {
        return Err(NcmClientError::MissingPayload("the library payload"));
    }
    let mut value = body;
    for segment in path {
        value = value
            .get(*segment)
            .ok_or(NcmClientError::MissingPayload("the library payload"))?;
    }
    value
        .as_array()
        .map(Vec::as_slice)
        .ok_or(NcmClientError::MissingPayload("the library payload"))
}

fn parse_search_payload(
    body: &Value,
    channel: SearchChannel,
) -> Result<SearchPayload, NcmClientError> {
    require_success(body)?;
    match channel {
        SearchChannel::Songs => {
            let total = required_usize(body, &["result", "songCount"])?;
            Ok(SearchPayload::Songs(SearchPage {
                items: search_result_array(body, &["result", "songs"], total)?
                    .iter()
                    .map(parse_song_hit)
                    .collect::<Result<_, _>>()?,
                total,
            }))
        }
        SearchChannel::Artists => {
            let total = required_usize(body, &["result", "artistCount"])?;
            Ok(SearchPayload::Artists(SearchPage {
                items: search_result_array(body, &["result", "artists"], total)?
                    .iter()
                    .map(parse_artist_hit)
                    .collect::<Result<_, _>>()?,
                total,
            }))
        }
        SearchChannel::Albums => {
            let total = required_usize(body, &["result", "albumCount"])?;
            Ok(SearchPayload::Albums(SearchPage {
                items: search_result_array(body, &["result", "albums"], total)?
                    .iter()
                    .map(parse_album_hit)
                    .collect::<Result<_, _>>()?,
                total,
            }))
        }
        SearchChannel::Playlists => {
            let total = required_usize(body, &["result", "playlistCount"])?;
            Ok(SearchPayload::Playlists(SearchPage {
                items: search_result_array(body, &["result", "playlists"], total)?
                    .iter()
                    .map(parse_playlist_hit)
                    .collect::<Result<_, _>>()?,
                total,
            }))
        }
        SearchChannel::MusicVideos => {
            let total = required_usize(body, &["result", "mvCount"])?;
            Ok(SearchPayload::MusicVideos(SearchPage {
                items: search_result_array(body, &["result", "mvs"], total)?
                    .iter()
                    .map(parse_mv_hit)
                    .collect::<Result<_, _>>()?,
                total,
            }))
        }
        SearchChannel::Users => {
            let total = required_usize(body, &["result", "userprofileCount"])?;
            Ok(SearchPayload::Users(SearchPage {
                items: search_result_array(body, &["result", "userprofiles"], total)?
                    .iter()
                    .map(parse_user_hit)
                    .collect::<Result<_, _>>()?,
                total,
            }))
        }
    }
}

fn search_result_array<'a>(
    value: &'a Value,
    path: &[&str],
    total: usize,
) -> Result<&'a [Value], NcmClientError> {
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

fn parse_song_collection(body: &Value, path: &[&str]) -> Result<Vec<SongRow>, NcmClientError> {
    require_success(body)?;
    required_array(body, path)?
        .iter()
        .map(parse_song_row)
        .collect()
}

fn parse_playlist_detail(body: &Value) -> Result<(Vec<SongRow>, usize), NcmClientError> {
    let rows = parse_song_collection(body, &["playlist", "tracks"])?;
    let total = required_usize(body, &["playlist", "trackCount"])?;
    Ok((rows, total))
}

async fn complete_playlist_detail<F, Fut>(
    embedded: Vec<SongRow>,
    total: usize,
    fetch_all: F,
) -> Result<Vec<SongRow>, NcmClientError>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<Vec<SongRow>, NcmClientError>>,
{
    if embedded.len() >= total {
        Ok(embedded)
    } else {
        fetch_all().await
    }
}

fn parse_song_row(song: &Value) -> Result<SongRow, NcmClientError> {
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

fn parse_song_hit(song: &Value) -> Result<SongHit, NcmClientError> {
    let row = parse_song_row(song)?;
    // The row parser already guaranteed a named first artist; the rest of
    // the credits list tolerates id-0 or unnamed entries instead of turning
    // one malformed guest credit into a failed page.
    let artists = required_array(song, &["ar"])?
        .iter()
        .map(|artist| ArtistRef {
            id: artist["id"].as_i64().unwrap_or(0),
            name: artist["name"].as_str().unwrap_or("").to_owned(),
        })
        .collect();
    Ok(SongHit {
        id: row.id,
        name: row.title,
        artists,
        album: AlbumRef {
            id: song["al"]["id"].as_i64().unwrap_or(0),
            name: row.album,
            pic_url: row.pic_url,
        },
        duration_ms: row.duration_ms,
        alias: string_list(song, "alia"),
        trans_names: string_list(song, "tns"),
        mark: song["mark"].as_i64().unwrap_or(0),
        fee: song["fee"].as_i64(),
        no_copyright_rcmd: !song["noCopyrightRcmd"].is_null(),
        privilege: parse_song_privilege(&song["privilege"]),
    })
}

fn parse_song_privilege(privilege: &Value) -> Option<SongPrivilege> {
    privilege.as_object().map(|fields| SongPrivilege {
        pl: fields.get("pl").and_then(Value::as_i64).unwrap_or(0),
        cs: fields.get("cs").and_then(Value::as_bool).unwrap_or(false),
        fee: fields.get("fee").and_then(Value::as_i64).unwrap_or(0),
        st: fields.get("st").and_then(Value::as_i64).unwrap_or(0),
    })
}

/// Display-only counters: a missing or malformed count degrades to zero
/// instead of failing the whole page (the GUI never renders these).
fn usize_or_zero(value: &Value, field: &str) -> usize {
    value[field]
        .as_u64()
        .and_then(|number| usize::try_from(number).ok())
        .unwrap_or(0)
}

fn string_list(value: &Value, field: &str) -> Vec<String> {
    value[field]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn parse_artist_hit(artist: &Value) -> Result<ArtistHit, NcmClientError> {
    let id = required_i64(artist, &["id"])?;
    if id <= 0 {
        return Err(invalid_payload("$.id"));
    }
    let pic_url = optional_string(artist, "picUrl")?;
    let img1v1_url = optional_string(artist, "img1v1Url")?;
    Ok(ArtistHit {
        id,
        name: required_string(artist, &["name"])?,
        pic_url: pic_url.clone().or_else(|| img1v1_url.clone()),
        img1v1_url,
        album_count: usize_or_zero(artist, "albumSize"),
        song_count: usize_or_zero(artist, "musicSize"),
    })
}

fn parse_album_hit(album: &Value) -> Result<AlbumHit, NcmClientError> {
    let id = required_i64(album, &["id"])?;
    if id <= 0 {
        return Err(invalid_payload("$.id"));
    }
    Ok(AlbumHit {
        id,
        name: required_string(album, &["name"])?,
        artist: ArtistRef {
            id: album["artist"]["id"].as_i64().unwrap_or(0),
            name: required_string(album, &["artist", "name"])?,
        },
        pic_url: optional_string(album, "picUrl")?,
        song_count: usize_or_zero(album, "size"),
        mark: album["mark"].as_i64().unwrap_or(0),
    })
}

fn parse_playlist_hit(playlist: &Value) -> Result<PlaylistHit, NcmClientError> {
    let id = required_i64(playlist, &["id"])?;
    if id <= 0 {
        return Err(invalid_payload("$.id"));
    }
    Ok(PlaylistHit {
        id,
        name: required_string(playlist, &["name"])?,
        creator: playlist["creator"]["nickname"]
            .as_str()
            .unwrap_or("")
            .to_owned(),
        cover_url: optional_string(playlist, "coverImgUrl")?,
        track_count: usize_or_zero(playlist, "trackCount"),
        privacy: playlist["privacy"].as_i64().unwrap_or(0),
    })
}

fn parse_mv_hit(mv: &Value) -> Result<MvHit, NcmClientError> {
    let id = mv["id"].as_i64().or_else(|| mv["vid"].as_i64());
    let Some(id) = id.filter(|id| *id > 0) else {
        return Err(invalid_payload("$.id"));
    };
    let name = mv["name"]
        .as_str()
        .or_else(|| mv["title"].as_str())
        .ok_or_else(|| invalid_payload("$.name"))?
        .to_owned();
    // Same fallback chain the GUI has always used for MV covers/credits.
    let cover_url = mv["imgurl16v9"]
        .as_str()
        .or_else(|| mv["cover"].as_str())
        .or_else(|| mv["coverUrl"].as_str())
        .filter(|url| !url.is_empty())
        .map(str::to_owned);
    let (artist_id, artist_name) = match mv["artistName"].as_str() {
        Some(artist_name) => (mv["artistId"].as_i64().unwrap_or(0), artist_name.to_owned()),
        None => (
            mv["creator"][0]["userId"].as_i64().unwrap_or(0),
            mv["creator"][0]["userName"]
                .as_str()
                .unwrap_or("")
                .to_owned(),
        ),
    };
    Ok(MvHit {
        id,
        name,
        cover_url,
        artist_id,
        artist_name,
    })
}

fn parse_user_hit(user: &Value) -> Result<UserHit, NcmClientError> {
    let user_id = required_i64(user, &["userId"])?;
    if user_id <= 0 {
        return Err(invalid_payload("$.userId"));
    }
    Ok(UserHit {
        user_id,
        nickname: required_string(user, &["nickname"])?,
        avatar_url: optional_string(user, "avatarUrl")?,
        vip_type: user["vipType"].as_i64().unwrap_or(0),
        signature: optional_string(user, "signature")?,
    })
}

fn require_success(body: &Value) -> Result<(), NcmClientError> {
    match body.get("code").and_then(Value::as_i64) {
        Some(200) => Ok(()),
        _ => Err(invalid_payload("$.code")),
    }
}

fn required_value<'a>(value: &'a Value, path: &[&str]) -> Result<&'a Value, NcmClientError> {
    let mut current = value;
    for segment in path {
        current = current
            .get(*segment)
            .ok_or_else(|| invalid_payload_path(path))?;
    }
    Ok(current)
}

fn required_array<'a>(value: &'a Value, path: &[&str]) -> Result<&'a [Value], NcmClientError> {
    required_value(value, path)?
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| invalid_payload_path(path))
}

fn required_string(value: &Value, path: &[&str]) -> Result<String, NcmClientError> {
    required_value(value, path)?
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| invalid_payload_path(path))
}

fn optional_string(value: &Value, field: &str) -> Result<Option<String>, NcmClientError> {
    match value.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => Ok((!text.is_empty()).then_some(text.clone())),
        Some(_) => Err(invalid_payload(&format!("$.{field}"))),
    }
}

fn required_i64(value: &Value, path: &[&str]) -> Result<i64, NcmClientError> {
    required_value(value, path)?
        .as_i64()
        .ok_or_else(|| invalid_payload_path(path))
}

fn required_usize(value: &Value, path: &[&str]) -> Result<usize, NcmClientError> {
    required_value(value, path)?
        .as_u64()
        .and_then(|number| usize::try_from(number).ok())
        .ok_or_else(|| invalid_payload_path(path))
}

fn invalid_payload_path(path: &[&str]) -> NcmClientError {
    invalid_payload(&format!("$.{}", path.join(".")))
}

fn invalid_payload(path: &str) -> NcmClientError {
    NcmClientError::InvalidPayload(path.to_owned())
}

/// Playback resolution for frontends that carry their own cookie transport:
/// the GUI sidecar forwards the browser cookie on every request instead of
/// using [`NcmClient`]'s persisted session.
pub async fn song_url_with(
    client: &ApiClient,
    query: Query,
    id: i64,
    bitrate: u32,
) -> Result<PlaybackSource, SongUrlError> {
    let query = query
        .param("id", &id.to_string())
        .param("br", &bitrate.to_string());
    let response = client
        .song_url(&query)
        .await
        .map_err(|error| SongUrlError::Other(error.into()))?;
    classify_song_url(&response.body, id)
}

/// Separate "the account may not ask" from "the track has no rights".
/// NCM answers `code: 200` with a null url for the second case and a
/// non-200 code (301 signed out, -462 risk control, 400 rate limited…)
/// for the first — reading only `data[0].url` conflates them.
fn classify_song_url(body: &Value, track_id: i64) -> Result<PlaybackSource, SongUrlError> {
    let code = body.get("code").and_then(Value::as_i64);
    if code != Some(200) {
        return Err(SongUrlError::Rejected(code));
    }
    // The answer array can carry a different track (an upstream cross-talk
    // bug the GUI has long guarded against): only an entry with the
    // requested id may be used.
    let data = body["data"]
        .as_array()
        .and_then(|entries| {
            entries
                .iter()
                .find(|entry| entry["id"].as_i64() == Some(track_id))
        })
        .unwrap_or(&Value::Null);
    if data["url"].as_str().is_none_or(str::is_empty) {
        return Err(SongUrlError::Unavailable);
    }
    // A free-trial clip is not the track: playing or caching a 30-second
    // preview as the full song is worse than letting fallbacks look for
    // the real audio.
    if !data["freeTrialInfo"].is_null() {
        return Err(SongUrlError::Unavailable);
    }
    parse_playback_source(data).map_err(SongUrlError::Other)
}

fn parse_playback_source(data: &Value) -> Result<PlaybackSource, NcmClientError> {
    let url = data["url"].as_str().filter(|url| !url.is_empty()).ok_or(
        NcmClientError::MalformedPayload("playback response is missing its URL"),
    )?;
    let codec = data["type"]
        .as_str()
        .ok_or(NcmClientError::MalformedPayload(
            "playback response is missing its audio codec",
        ))?
        .parse::<AudioCodec>()
        .map_err(|_| NcmClientError::MalformedPayload("unsupported audio codec"))?;
    let actual_bitrate = data["br"]
        .as_u64()
        .and_then(|bitrate| u32::try_from(bitrate).ok())
        .ok_or(NcmClientError::MalformedPayload(
            "playback response is missing its actual bitrate",
        ))?;
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

fn parse_md5(value: Option<&str>) -> Result<Option<[u8; 16]>, NcmClientError> {
    let Some(value) = value.filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    if value.len() != 32 || !value.is_ascii() {
        return Err(NcmClientError::MalformedPayload(
            "playback response contains an invalid MD5",
        ));
    }

    let mut digest = [0_u8; 16];
    for (index, byte) in digest.iter_mut().enumerate() {
        let offset = index * 2;
        *byte = u8::from_str_radix(&value[offset..offset + 2], 16).map_err(|_| {
            NcmClientError::MalformedPayload("playback response contains an invalid MD5")
        })?;
    }
    Ok(Some(digest))
}

/// Library operations for frontends that carry their own cookie transport
/// (the GUI sidecar forwards the browser cookie on every request).
pub async fn liked_ids_with(
    client: &ApiClient,
    query: Query,
    uid: i64,
) -> Result<Vec<i64>, NcmClientError> {
    let query = query.param("uid", &uid.to_string());
    let response = client.likelist(&query).await?;
    Ok(response_array(&response.body, &["ids"])?
        .iter()
        .filter_map(Value::as_i64)
        .collect())
}

pub async fn set_like_with(
    client: &ApiClient,
    query: Query,
    id: i64,
    like: bool,
) -> Result<(), NcmClientError> {
    let query = query
        .param("id", &id.to_string())
        .param("like", if like { "true" } else { "false" });
    let response = client.like(&query).await?;
    match response.body["code"].as_i64() {
        Some(200) => Ok(()),
        code => Err(NcmClientError::Rejected(code)),
    }
}

/// The transport layer rewrites some refusals into HTTP 200, so the body's
/// own code is the real verdict.
pub async fn fm_trash_with(
    client: &ApiClient,
    query: Query,
    id: i64,
) -> Result<(), NcmClientError> {
    let query = query.param("id", &id.to_string());
    let response = client.fm_trash(&query).await?;
    match response.body.get("code").and_then(Value::as_i64) {
        Some(200) => Ok(()),
        code => Err(NcmClientError::Rejected(code)),
    }
}

/// Search for frontends that carry their own cookie transport (the GUI
/// sidecar forwards the browser cookie on every request).
pub async fn search_with(
    client: &ApiClient,
    query: Query,
    keywords: &str,
    channel: SearchChannel,
    limit: u32,
    offset: u32,
) -> Result<SearchPayload, NcmClientError> {
    let query = query
        .param("keywords", keywords)
        .param("type", channel.api_type())
        .param("limit", &limit.to_string())
        .param("offset", &offset.to_string());
    let response = client.cloudsearch(&query).await?;
    parse_search_payload(&response.body, channel)
}

/// Lyrics for frontends that carry their own cookie transport (the GUI
/// sidecar forwards the browser cookie on every request).
pub async fn lyrics_with(
    client: &ApiClient,
    query: Query,
    id: i64,
) -> Result<LyricsPayload, NcmClientError> {
    let query = query.param("id", &id.to_string());
    // `lyric_new` sends yv/ytv/yrv, the API's YRC request flags.
    let response = client.lyric_new(&query).await?;
    parse_lyrics_payload(&response.body)
}

fn parse_lyrics_payload(body: &Value) -> Result<LyricsPayload, NcmClientError> {
    require_success(body)?;
    Ok(LyricsPayload {
        lrc: lyric_text(body, "lrc")?.unwrap_or_default(),
        tlyric: lyric_text(body, "tlyric")?,
        romalrc: lyric_text(body, "romalrc")?,
        yrc: lyric_text(body, "yrc")?,
    })
}

fn lyric_text(body: &Value, field: &str) -> Result<Option<String>, NcmClientError> {
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

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    #[test]
    fn a_captive_portal_body_does_not_expire_the_session() {
        // Real logged-out answer: code-200 JSON without an account.
        assert!(matches!(
            account_from_body(&serde_json::json!({ "code": 200, "account": null })),
            Err(AccountError::Expired(AccountReason::InvalidPayload))
        ));
        // Portal HTML arrives as a string body; decrypt failures as null.
        for garbage in [
            Value::String("<html>login to hotel wifi</html>".into()),
            Value::Null,
        ] {
            assert!(matches!(
                account_from_body(&garbage),
                Err(AccountError::Unreachable(AccountReason::InvalidPayload))
            ));
        }
        let account = account_from_body(
            &serde_json::json!({ "code": 200, "account": { "id": 7 }, "profile": { "nickname": "n" } }),
        )
        .unwrap();
        assert_eq!(account, (7, "n".to_owned()));
    }

    #[test]
    fn song_url_separates_refusals_from_tracks_without_rights() {
        let refused = classify_song_url(
            &serde_json::json!({
                "code": 301,
                "data": [{ "id": 42, "url": Value::Null }]
            }),
            42,
        );
        assert!(matches!(refused, Err(SongUrlError::Rejected(Some(301)))));

        let risk_control = classify_song_url(&serde_json::json!({ "code": -462 }), 42);
        assert!(matches!(
            risk_control,
            Err(SongUrlError::Rejected(Some(-462)))
        ));

        let codeless = classify_song_url(&serde_json::json!({ "data": [] }), 42);
        assert!(matches!(codeless, Err(SongUrlError::Rejected(None))));

        let unavailable = classify_song_url(
            &serde_json::json!({
                "code": 200,
                "data": [{ "id": 42, "url": Value::Null }]
            }),
            42,
        );
        assert!(matches!(unavailable, Err(SongUrlError::Unavailable)));

        let playable = classify_song_url(
            &serde_json::json!({
                "code": 200,
                "data": [{
                    "id": 42,
                    "url": "https://audio.example/track.mp3",
                    "type": "mp3",
                    "br": 320_000,
                    "size": 8_000_000,
                    "md5": Value::Null
                }]
            }),
            42,
        )
        .unwrap();
        assert_eq!(playable.url, "https://audio.example/track.mp3");
        assert_eq!(playable.actual_bitrate, 320_000);
    }

    #[test]
    fn song_url_rejects_cross_talk_and_free_trial_answers() {
        // Upstream sometimes answers with a different track's entry; using
        // it would play (and cache) the wrong song.
        let cross_talk = classify_song_url(
            &serde_json::json!({
                "code": 200,
                "data": [{
                    "id": 7,
                    "url": "https://audio.example/other.mp3",
                    "type": "mp3",
                    "br": 320_000
                }]
            }),
            42,
        );
        assert!(matches!(cross_talk, Err(SongUrlError::Unavailable)));

        let trial = classify_song_url(
            &serde_json::json!({
                "code": 200,
                "data": [{
                    "id": 42,
                    "url": "https://audio.example/trial.mp3",
                    "type": "mp3",
                    "br": 320_000,
                    "freeTrialInfo": { "start": 45, "end": 75 }
                }]
            }),
            42,
        );
        assert!(matches!(trial, Err(SongUrlError::Unavailable)));

        // A second entry with the right id must still be found.
        let second_entry = classify_song_url(
            &serde_json::json!({
                "code": 200,
                "data": [
                    { "id": 7, "url": "https://audio.example/other.mp3" },
                    {
                        "id": 42,
                        "url": "https://audio.example/track.flac",
                        "type": "flac",
                        "br": 850_000,
                        "freeTrialInfo": Value::Null
                    }
                ]
            }),
            42,
        )
        .unwrap();
        assert_eq!(second_entry.url, "https://audio.example/track.flac");
    }

    #[test]
    fn missing_session_means_logged_out_and_cookieless_queries() {
        let directory = tempfile::tempdir().unwrap();
        let client = NcmClient::new(directory.path().join("session.json"));
        assert!(client.session_snapshot().is_none());
        assert!(client.query().cookie.is_none());
    }

    #[test]
    fn invalid_account_response_is_an_error_instead_of_uid_zero() {
        assert!(account_from_body(&serde_json::json!({
            "account": {},
            "profile": { "nickname": "unknown" }
        }))
        .is_err());
    }

    #[test]
    fn only_an_auth_rejection_counts_as_an_expired_session() {
        assert!(matches!(
            classify_account_error(NcmError::AuthRequired("需要登录".into())),
            AccountError::Expired(_)
        ));
        for unproven in [
            NcmError::Timeout("connect".into()),
            NcmError::RateLimited("503".into()),
            NcmError::Api {
                code: 502,
                msg: "bad gateway".into(),
            },
            NcmError::Unknown("connection reset".into()),
        ] {
            assert!(matches!(
                classify_account_error(unproven),
                AccountError::Unreachable(_)
            ));
        }
    }

    #[test]
    fn qr_status_codes_map_to_the_login_state_machine() {
        assert_eq!(
            parse_qr_status(&serde_json::json!({ "code": 801 }), &[]).unwrap(),
            QrStatus::Waiting
        );
        assert_eq!(
            parse_qr_status(&serde_json::json!({ "code": 802 }), &[]).unwrap(),
            QrStatus::Scanned
        );
        assert_eq!(
            parse_qr_status(&serde_json::json!({ "code": 800 }), &[]).unwrap(),
            QrStatus::Expired
        );
        assert!(matches!(
            parse_qr_status(&serde_json::json!({ "code": 803 }), &[]),
            Err(NcmClientError::LoginCookieMissing)
        ));
        assert!(matches!(
            parse_qr_status(&serde_json::json!({ "code": 418 }), &[]),
            Err(NcmClientError::UnknownQrStatus(418))
        ));

        let cookies = [
            "MUSIC_U=candidate-token; Path=/; HttpOnly".to_owned(),
            "__csrf=candidate-csrf; Path=/".to_owned(),
        ];
        assert!(matches!(
            parse_qr_status(&serde_json::json!({ "code": 803 }), &cookies),
            Ok(QrStatus::Success(_))
        ));
    }

    #[test]
    fn lyric_payload_keeps_all_supported_timeline_kinds() {
        let payload = parse_lyrics_payload(&serde_json::json!({
            "code": 200,
            "lrc": { "lyric": "[00:01]line" },
            "tlyric": { "lyric": "[00:01]翻译" },
            "romalrc": { "lyric": "[00:01]ro-ma-ji" },
            "yrc": { "lyric": "[1000,500](1000,500,0)line" }
        }))
        .unwrap();

        assert_eq!(payload.lrc, "[00:01]line");
        assert_eq!(payload.tlyric.as_deref(), Some("[00:01]翻译"));
        assert_eq!(payload.romalrc.as_deref(), Some("[00:01]ro-ma-ji"));
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
        let channels = [
            (SearchChannel::Songs, "1"),
            (SearchChannel::Artists, "100"),
            (SearchChannel::Albums, "10"),
            (SearchChannel::Playlists, "1000"),
            (SearchChannel::MusicVideos, "1004"),
            (SearchChannel::Users, "1002"),
        ];
        for (channel, code) in channels {
            assert_eq!(channel.api_type(), code);
            assert_eq!(SearchChannel::from_api_type(code), Some(channel));
        }
        assert_eq!(SearchChannel::from_api_type("1006"), None);
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
        assert_eq!(songs.items[0].name, "晴天");
        assert_eq!(songs.items[0].album.name, "叶惠美");
        assert_eq!(songs.items[0].album.id, 18_905);
        assert_eq!(
            songs.items[0].artists,
            vec![ArtistRef {
                id: 6_452,
                name: "周杰伦".into()
            }]
        );

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
        assert_eq!(albums.items[0].artist.name, "周杰伦");
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
        assert_eq!(playlists.items[0].privacy, 0);

        let mvs = parse_search_payload(
            &serde_json::json!({
                "code": 200,
                "result": {
                    "mvCount": 4,
                    "mvs": [{
                        "id": 10_902_601,
                        "name": "晴天 (MV)",
                        "cover": "https://example.test/mv.jpg",
                        "artistName": "周杰伦",
                        "artistId": 6_452
                    }]
                }
            }),
            SearchChannel::MusicVideos,
        )
        .unwrap();
        let SearchPayload::MusicVideos(mvs) = mvs else {
            panic!("MV search returned the wrong variant");
        };
        assert_eq!(mvs.total, 4);
        assert_eq!(
            mvs.items[0].cover_url.as_deref(),
            Some("https://example.test/mv.jpg")
        );
        assert_eq!(mvs.items[0].artist_id, 6_452);

        let users = parse_search_payload(
            &serde_json::json!({
                "code": 200,
                "result": {
                    "userprofileCount": 2,
                    "userprofiles": [{
                        "userId": 32_953_014,
                        "nickname": "圈圈",
                        "avatarUrl": "https://example.test/avatar.jpg"
                    }]
                }
            }),
            SearchChannel::Users,
        )
        .unwrap();
        let SearchPayload::Users(users) = users else {
            panic!("user search returned the wrong variant");
        };
        assert_eq!(users.total, 2);
        assert_eq!(users.items[0].nickname, "圈圈");
    }

    #[test]
    fn song_hits_carry_link_ids_marks_and_raw_permission_fields() {
        let hit = parse_song_hit(&serde_json::json!({
            "id": 186_016,
            "name": "晴天",
            "ar": [
                { "id": 6_452, "name": "周杰伦" },
                { "id": 0, "name": "客串" }
            ],
            "al": { "id": 18_905, "name": "叶惠美", "picUrl": "https://example.test/cover.jpg" },
            "dt": 269_000,
            "alia": ["别名"],
            "tns": ["Sunny Day"],
            "mark": 1_048_576,
            "fee": 1,
            "noCopyrightRcmd": Value::Null,
            "privilege": { "pl": 128_000, "fee": 1, "st": 0 }
        }))
        .unwrap();

        assert_eq!(hit.artists.len(), 2);
        assert_eq!(hit.album.id, 18_905);
        assert_eq!(hit.alias, vec!["别名"]);
        assert_eq!(hit.trans_names, vec!["Sunny Day"]);
        assert_eq!(hit.mark, 1_048_576);
        assert_eq!(hit.fee, Some(1));
        assert!(!hit.no_copyright_rcmd);
        assert_eq!(
            hit.privilege,
            Some(SongPrivilege {
                pl: 128_000,
                cs: false,
                fee: 1,
                st: 0
            })
        );
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
                SearchPayload::MusicVideos(page) => (page.items.len(), page.total),
                SearchPayload::Users(page) => (page.items.len(), page.total),
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
                SearchPayload::MusicVideos(page) => (page.items.len(), page.total),
                SearchPayload::Users(page) => (page.items.len(), page.total),
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
}
