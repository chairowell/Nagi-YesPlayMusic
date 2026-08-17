//! Typed detail-page endpoints (playlist / album / artist) backed by
//! `core::ncm`, shared with the TUI. Container metadata passes through
//! verbatim; the song lists are the parsed business payload.

use std::sync::Arc;

use async_trait::async_trait;
use axum::{
    body::Body,
    extract::{Query as UrlQuery, State},
    http::{HeaderMap, Response},
    routing::get,
    Router,
};
use ncm_api_rs::{api::Query, ApiClient};
use serde::Deserialize;
use serde_json::{json, Value};
use yesplaymusic_core::ncm::{
    album_with, artist_with, playlist_detail_with, song_detail_with, AlbumDetailPayload,
    ArtistDetailPayload, NcmClientError, PlaylistDetailPayload, SongItem, TrackDetailPayload,
};

use crate::native::{respond, song_item_body};
use crate::playback::ncm_query;

#[derive(Clone)]
pub struct DetailState {
    resolver: Arc<dyn DetailResolver>,
}

impl DetailState {
    pub fn production(client: Arc<ApiClient>) -> Self {
        Self { resolver: client }
    }

    #[cfg(test)]
    fn testing(resolver: Arc<dyn DetailResolver>) -> Self {
        Self { resolver }
    }
}

#[async_trait]
trait DetailResolver: Send + Sync {
    async fn playlist(
        &self,
        query: Query,
        id: i64,
    ) -> Result<PlaylistDetailPayload, NcmClientError>;
    async fn album(&self, query: Query, id: i64) -> Result<AlbumDetailPayload, NcmClientError>;
    async fn artist(&self, query: Query, id: i64) -> Result<ArtistDetailPayload, NcmClientError>;
    async fn songs(&self, query: Query, ids: &str) -> Result<TrackDetailPayload, NcmClientError>;
}

#[async_trait]
impl DetailResolver for ApiClient {
    async fn playlist(
        &self,
        query: Query,
        id: i64,
    ) -> Result<PlaylistDetailPayload, NcmClientError> {
        playlist_detail_with(self, query, id).await
    }

    async fn album(&self, query: Query, id: i64) -> Result<AlbumDetailPayload, NcmClientError> {
        album_with(self, query, id).await
    }

    async fn artist(&self, query: Query, id: i64) -> Result<ArtistDetailPayload, NcmClientError> {
        artist_with(self, query, id).await
    }

    async fn songs(&self, query: Query, ids: &str) -> Result<TrackDetailPayload, NcmClientError> {
        song_detail_with(self, query, ids).await
    }
}

#[derive(Deserialize)]
struct DetailQuery {
    id: i64,
    #[serde(rename = "realIP")]
    real_ip: Option<String>,
    proxy: Option<String>,
}

#[derive(Deserialize)]
struct SongsQuery {
    /// Comma-separated track ids, as the NCM API expects.
    ids: String,
    #[serde(rename = "realIP")]
    real_ip: Option<String>,
    proxy: Option<String>,
}

pub fn router(state: DetailState) -> Router {
    Router::new()
        .route("/native/playlist/detail", get(playlist_handler))
        .route("/native/album/detail", get(album_handler))
        .route("/native/artist/detail", get(artist_handler))
        .route("/native/song/detail", get(songs_handler))
        .with_state(state)
}

async fn playlist_handler(
    State(state): State<DetailState>,
    UrlQuery(query): UrlQuery<DetailQuery>,
    headers: HeaderMap,
) -> Response<Body> {
    let request = ncm_query(&headers, query.real_ip, query.proxy);
    respond("playlist detail", async move {
        let payload = state.resolver.playlist(request, query.id).await?;
        Ok(json!({
            "playlist": payload.playlist,
            "songs": song_bodies(&payload.songs),
            "embeddedCount": payload.embedded_count,
        }))
    })
    .await
}

async fn album_handler(
    State(state): State<DetailState>,
    UrlQuery(query): UrlQuery<DetailQuery>,
    headers: HeaderMap,
) -> Response<Body> {
    let request = ncm_query(&headers, query.real_ip, query.proxy);
    respond("album detail", async move {
        let payload = state.resolver.album(request, query.id).await?;
        Ok(json!({
            "album": payload.album,
            "songs": song_bodies(&payload.songs),
        }))
    })
    .await
}

async fn artist_handler(
    State(state): State<DetailState>,
    UrlQuery(query): UrlQuery<DetailQuery>,
    headers: HeaderMap,
) -> Response<Body> {
    let request = ncm_query(&headers, query.real_ip, query.proxy);
    respond("artist detail", async move {
        let payload = state.resolver.artist(request, query.id).await?;
        Ok(json!({
            "artist": payload.artist,
            "hotSongs": song_bodies(&payload.hot_songs),
        }))
    })
    .await
}

async fn songs_handler(
    State(state): State<DetailState>,
    UrlQuery(query): UrlQuery<SongsQuery>,
    headers: HeaderMap,
) -> Response<Body> {
    let request = ncm_query(&headers, query.real_ip, query.proxy);
    respond("song detail", async move {
        let payload = state.resolver.songs(request, &query.ids).await?;
        // Verbatim rows: the renderer caches them and computes playability
        // itself at read time.
        Ok(json!({
            "songs": payload.songs,
            "privileges": payload.privileges,
        }))
    })
    .await
}

fn song_bodies(songs: &[SongItem]) -> Vec<Value> {
    songs.iter().map(song_item_body).collect()
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use axum::{
        body::to_bytes,
        http::{header, StatusCode},
    };
    use tower::ServiceExt;
    use yesplaymusic_core::ncm::{AlbumRef, ArtistRef};

    use super::*;

    fn hit(id: i64) -> SongItem {
        SongItem {
            id,
            name: format!("Song {id}"),
            artists: vec![ArtistRef {
                id: 7,
                name: "Artist".into(),
            }],
            album: AlbumRef {
                id: 9,
                name: "Album".into(),
                pic_url: None,
            },
            duration_ms: 1_000,
            alias: vec![],
            trans_names: vec![],
            mark: 0,
            fee: None,
            no_copyright_rcmd: false,
            privilege: None,
            cd: None,
        }
    }

    #[derive(Default)]
    struct FakeResolver {
        playlist: Mutex<Option<Result<PlaylistDetailPayload, NcmClientError>>>,
        album: Mutex<Option<Result<AlbumDetailPayload, NcmClientError>>>,
        artist: Mutex<Option<Result<ArtistDetailPayload, NcmClientError>>>,
        songs: Mutex<Option<Result<TrackDetailPayload, NcmClientError>>>,
        seen_query: Mutex<Option<Query>>,
        seen_ids: Mutex<Option<String>>,
    }

    #[async_trait]
    impl DetailResolver for FakeResolver {
        async fn playlist(
            &self,
            query: Query,
            _id: i64,
        ) -> Result<PlaylistDetailPayload, NcmClientError> {
            *self.seen_query.lock().unwrap() = Some(query);
            self.playlist.lock().unwrap().take().expect("single call")
        }

        async fn album(
            &self,
            query: Query,
            _id: i64,
        ) -> Result<AlbumDetailPayload, NcmClientError> {
            *self.seen_query.lock().unwrap() = Some(query);
            self.album.lock().unwrap().take().expect("single call")
        }

        async fn artist(
            &self,
            query: Query,
            _id: i64,
        ) -> Result<ArtistDetailPayload, NcmClientError> {
            *self.seen_query.lock().unwrap() = Some(query);
            self.artist.lock().unwrap().take().expect("single call")
        }

        async fn songs(
            &self,
            query: Query,
            ids: &str,
        ) -> Result<TrackDetailPayload, NcmClientError> {
            *self.seen_query.lock().unwrap() = Some(query);
            *self.seen_ids.lock().unwrap() = Some(ids.to_owned());
            self.songs.lock().unwrap().take().expect("single call")
        }
    }

    async fn request(resolver: Arc<FakeResolver>, uri: &str) -> (StatusCode, serde_json::Value) {
        let app = router(DetailState::testing(resolver));
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri(uri)
                    .header(header::COOKIE, "MUSIC_U=token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let body = to_bytes(response.into_body(), 256 * 1024).await.unwrap();
        (status, serde_json::from_slice(&body).unwrap())
    }

    #[tokio::test]
    async fn playlist_detail_passes_metadata_through_verbatim() {
        let resolver = Arc::new(FakeResolver {
            playlist: Mutex::new(Some(Ok(PlaylistDetailPayload {
                playlist: json!({
                    "id": 3,
                    "name": "歌单",
                    "trackCount": 2,
                    "trackIds": [{ "id": 1 }, { "id": 2 }],
                    "creator": { "userId": 9, "nickname": "n" },
                    "unknownFutureField": true
                }),
                songs: vec![hit(1)],
                embedded_count: 2,
            }))),
            ..FakeResolver::default()
        });
        let (status, body) = request(
            resolver.clone(),
            "/native/playlist/detail?id=3&realIP=1.2.3.4",
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        // View metadata survives untouched, unknown fields included.
        assert_eq!(body["playlist"]["unknownFutureField"], true);
        assert_eq!(body["playlist"]["trackIds"][1]["id"], 2);
        assert_eq!(body["songs"][0]["id"], 1);
        // Pre-drop row count: the GUI's paging cursor indexes trackIds.
        assert_eq!(body["embeddedCount"], 2);
        let query = resolver.seen_query.lock().unwrap().take().unwrap();
        assert_eq!(query.cookie.as_deref(), Some("MUSIC_U=token"));
        assert_eq!(query.real_ip.as_deref(), Some("1.2.3.4"));
    }

    #[tokio::test]
    async fn album_and_artist_answer_their_page_shapes() {
        let resolver = Arc::new(FakeResolver {
            album: Mutex::new(Some(Ok(AlbumDetailPayload {
                album: json!({ "id": 9, "name": "Album" }),
                songs: vec![hit(1), hit(2)],
            }))),
            ..FakeResolver::default()
        });
        let (status, body) = request(resolver, "/native/album/detail?id=9").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["album"]["name"], "Album");
        assert_eq!(body["songs"].as_array().unwrap().len(), 2);

        let resolver = Arc::new(FakeResolver {
            artist: Mutex::new(Some(Ok(ArtistDetailPayload {
                artist: json!({ "id": 7, "name": "Artist" }),
                hot_songs: vec![hit(1)],
            }))),
            ..FakeResolver::default()
        });
        let (status, body) = request(resolver, "/native/artist/detail?id=7").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["artist"]["name"], "Artist");
        assert_eq!(body["hotSongs"][0]["id"], 1);
    }

    #[tokio::test]
    async fn song_detail_passes_rows_and_privileges_through_verbatim() {
        let resolver = Arc::new(FakeResolver {
            songs: Mutex::new(Some(Ok(TrackDetailPayload {
                songs: vec![json!({ "id": 1, "name": "Song", "rawOnlyField": 7 })],
                privileges: vec![json!({ "id": 1, "pl": 320_000 })],
            }))),
            ..FakeResolver::default()
        });
        // The renderer percent-encodes the comma (ids=1%2C2); the decoded
        // list must reach the resolver intact.
        let (status, body) = request(resolver.clone(), "/native/song/detail?ids=1%2C2").await;

        assert_eq!(status, StatusCode::OK);
        // Verbatim: unknown raw fields must survive for the renderer cache.
        assert_eq!(body["songs"][0]["rawOnlyField"], 7);
        assert_eq!(body["privileges"][0]["pl"], 320_000);
        assert_eq!(
            resolver.seen_ids.lock().unwrap().take().as_deref(),
            Some("1,2")
        );
    }

    #[tokio::test]
    async fn upstream_failure_is_a_bad_gateway() {
        let resolver = Arc::new(FakeResolver {
            playlist: Mutex::new(Some(Err(NcmClientError::MissingPayload("the playlist")))),
            ..FakeResolver::default()
        });
        let (status, body) = request(resolver, "/native/playlist/detail?id=3").await;
        assert_eq!(status, StatusCode::BAD_GATEWAY);
        assert_eq!(body["status"], "error");
    }
}
