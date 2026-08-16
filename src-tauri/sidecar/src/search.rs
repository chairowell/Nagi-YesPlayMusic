//! Typed search endpoint backed by `core::ncm`'s cloudsearch parsing.
//!
//! The renderer used to call the generic `/search` proxy and narrow the
//! answer itself; this endpoint serves all six channels from the same
//! implementation the TUI uses.

use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use axum::{
    body::Body,
    extract::{Query as UrlQuery, State},
    http::{header, HeaderMap, HeaderValue, Response, StatusCode},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use ncm_api_rs::{api::Query, ApiClient};
use serde::Deserialize;
use serde_json::{json, Value};
use yesplaymusic_core::ncm::{
    search_with, NcmClientError, SearchChannel, SearchPage, SearchPayload,
};

use crate::playback::ncm_query;

const SEARCH_TIMEOUT: Duration = Duration::from_secs(15);
/// The Node adapter's defaults, kept so results page identically.
const DEFAULT_LIMIT: u32 = 30;

#[derive(Clone)]
pub struct SearchState {
    resolver: Arc<dyn SearchResolver>,
}

impl SearchState {
    pub fn production(client: Arc<ApiClient>) -> Self {
        Self {
            resolver: Arc::new(ProductionResolver { client }),
        }
    }

    #[cfg(test)]
    fn testing(resolver: Arc<dyn SearchResolver>) -> Self {
        Self { resolver }
    }
}

#[async_trait]
trait SearchResolver: Send + Sync {
    async fn search(
        &self,
        query: Query,
        keywords: &str,
        channel: SearchChannel,
        limit: u32,
        offset: u32,
    ) -> Result<SearchPayload, NcmClientError>;
}

struct ProductionResolver {
    client: Arc<ApiClient>,
}

#[async_trait]
impl SearchResolver for ProductionResolver {
    async fn search(
        &self,
        query: Query,
        keywords: &str,
        channel: SearchChannel,
        limit: u32,
        offset: u32,
    ) -> Result<SearchPayload, NcmClientError> {
        search_with(&self.client, query, keywords, channel, limit, offset).await
    }
}

#[derive(Deserialize)]
struct SearchQuery {
    keywords: String,
    /// The numeric NCM type code the old `/search` route used.
    #[serde(rename = "type")]
    channel: String,
    limit: Option<u32>,
    offset: Option<u32>,
    #[serde(rename = "realIP")]
    real_ip: Option<String>,
    proxy: Option<String>,
}

pub fn router(state: SearchState) -> Router {
    Router::new()
        .route("/native/search", get(search_handler))
        .with_state(state)
}

async fn search_handler(
    State(state): State<SearchState>,
    UrlQuery(query): UrlQuery<SearchQuery>,
    headers: HeaderMap,
) -> Response<Body> {
    let Some(channel) = SearchChannel::from_api_type(&query.channel) else {
        return with_no_store(
            (
                StatusCode::BAD_REQUEST,
                Json(json!({ "status": "error", "message": "unsupported search type" })),
            )
                .into_response(),
        );
    };
    let resolved = tokio::time::timeout(
        SEARCH_TIMEOUT,
        state.resolver.search(
            ncm_query(&headers, query.real_ip, query.proxy),
            &query.keywords,
            channel,
            query.limit.unwrap_or(DEFAULT_LIMIT),
            query.offset.unwrap_or(0),
        ),
    )
    .await;
    let body = match resolved {
        Ok(Ok(payload)) => (StatusCode::OK, payload_body(payload)),
        Ok(Err(error)) => {
            tracing::warn!(%error, "search resolution failed");
            (
                StatusCode::BAD_GATEWAY,
                json!({ "status": "error", "message": "could not resolve the search" }),
            )
        }
        Err(_) => {
            tracing::warn!("search resolution timed out");
            (
                StatusCode::GATEWAY_TIMEOUT,
                json!({ "status": "error", "message": "search timed out" }),
            )
        }
    };
    with_no_store((body.0, Json(body.1)).into_response())
}

fn with_no_store(mut response: Response<Body>) -> Response<Body> {
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

fn payload_body(payload: SearchPayload) -> Value {
    match payload {
        SearchPayload::Songs(page) => page_body("songs", page, |song| {
            json!({
                "id": song.id,
                "name": song.name,
                "artists": song
                    .artists
                    .iter()
                    .map(|artist| json!({ "id": artist.id, "name": artist.name }))
                    .collect::<Vec<_>>(),
                "album": {
                    "id": song.album.id,
                    "name": song.album.name,
                    "picUrl": song.album.pic_url,
                },
                "durationMs": song.duration_ms,
                "alias": song.alias,
                "transNames": song.trans_names,
                "mark": song.mark,
                "fee": song.fee,
                "noCopyrightRcmd": song.no_copyright_rcmd,
                "privilege": song.privilege.map(|privilege| json!({
                    "pl": privilege.pl,
                    "cs": privilege.cs,
                    "fee": privilege.fee,
                    "st": privilege.st,
                })),
            })
        }),
        SearchPayload::Artists(page) => page_body("artists", page, |artist| {
            json!({
                "id": artist.id,
                "name": artist.name,
                "picUrl": artist.pic_url,
                "img1v1Url": artist.img1v1_url,
                "albumCount": artist.album_count,
                "songCount": artist.song_count,
            })
        }),
        SearchPayload::Albums(page) => page_body("albums", page, |album| {
            json!({
                "id": album.id,
                "name": album.name,
                "artist": { "id": album.artist.id, "name": album.artist.name },
                "picUrl": album.pic_url,
                "songCount": album.song_count,
                "mark": album.mark,
            })
        }),
        SearchPayload::Playlists(page) => page_body("playlists", page, |playlist| {
            json!({
                "id": playlist.id,
                "name": playlist.name,
                "creator": playlist.creator,
                "coverUrl": playlist.cover_url,
                "trackCount": playlist.track_count,
                "privacy": playlist.privacy,
            })
        }),
        SearchPayload::MusicVideos(page) => page_body("musicVideos", page, |mv| {
            json!({
                "id": mv.id,
                "name": mv.name,
                "coverUrl": mv.cover_url,
                "artistId": mv.artist_id,
                "artistName": mv.artist_name,
            })
        }),
        SearchPayload::Users(page) => page_body("users", page, |user| {
            json!({
                "userId": user.user_id,
                "nickname": user.nickname,
                "avatarUrl": user.avatar_url,
                "vipType": user.vip_type,
                "signature": user.signature,
            })
        }),
    }
}

fn page_body<T>(channel: &str, page: SearchPage<T>, item: impl Fn(&T) -> Value) -> Value {
    json!({
        "channel": channel,
        "total": page.total,
        "items": page.items.iter().map(item).collect::<Vec<_>>(),
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use axum::body::to_bytes;
    use tower::ServiceExt;
    use yesplaymusic_core::ncm::{ArtistRef, SongHit, SongPrivilege};

    use super::*;

    type SeenSearch = (Query, String, SearchChannel, u32, u32);

    struct FakeResolver {
        outcome: Mutex<Option<Result<SearchPayload, NcmClientError>>>,
        seen: Mutex<Option<SeenSearch>>,
    }

    impl FakeResolver {
        fn new(outcome: Result<SearchPayload, NcmClientError>) -> Arc<Self> {
            Arc::new(Self {
                outcome: Mutex::new(Some(outcome)),
                seen: Mutex::new(None),
            })
        }
    }

    #[async_trait]
    impl SearchResolver for FakeResolver {
        async fn search(
            &self,
            query: Query,
            keywords: &str,
            channel: SearchChannel,
            limit: u32,
            offset: u32,
        ) -> Result<SearchPayload, NcmClientError> {
            *self.seen.lock().unwrap() = Some((query, keywords.to_owned(), channel, limit, offset));
            self.outcome.lock().unwrap().take().expect("single call")
        }
    }

    async fn request(resolver: Arc<FakeResolver>, uri: &str) -> (StatusCode, serde_json::Value) {
        let app = router(SearchState::testing(resolver));
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
    async fn song_search_answers_rich_rows_and_forwards_paging() {
        let resolver = FakeResolver::new(Ok(SearchPayload::Songs(SearchPage {
            items: vec![SongHit {
                id: 186_016,
                name: "晴天".into(),
                artists: vec![ArtistRef {
                    id: 6_452,
                    name: "周杰伦".into(),
                }],
                album: yesplaymusic_core::ncm::AlbumRef {
                    id: 18_905,
                    name: "叶惠美".into(),
                    pic_url: Some("https://example.test/cover.jpg".into()),
                },
                duration_ms: 269_000,
                alias: vec![],
                trans_names: vec![],
                mark: 1_048_576,
                fee: Some(1),
                no_copyright_rcmd: false,
                privilege: Some(SongPrivilege {
                    pl: 128_000,
                    cs: false,
                    fee: 1,
                    st: 0,
                }),
            }],
            total: 240,
        })));
        let (status, body) = request(
            resolver.clone(),
            "/native/search?keywords=%E6%99%B4%E5%A4%A9&type=1&limit=16&offset=32&realIP=1.2.3.4",
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["channel"], "songs");
        assert_eq!(body["total"], 240);
        assert_eq!(body["items"][0]["artists"][0]["id"], 6_452);
        assert_eq!(
            body["items"][0]["album"]["picUrl"],
            "https://example.test/cover.jpg"
        );
        assert_eq!(body["items"][0]["mark"], 1_048_576);
        assert_eq!(body["items"][0]["privilege"]["fee"], 1);
        let (query, keywords, channel, limit, offset) =
            resolver.seen.lock().unwrap().take().unwrap();
        assert_eq!(query.cookie.as_deref(), Some("MUSIC_U=token"));
        assert_eq!(query.real_ip.as_deref(), Some("1.2.3.4"));
        assert_eq!(keywords, "晴天");
        assert_eq!(channel, SearchChannel::Songs);
        assert_eq!((limit, offset), (16, 32));
    }

    #[tokio::test]
    async fn unknown_type_codes_are_bad_requests_not_upstream_calls() {
        let resolver = FakeResolver::new(Ok(SearchPayload::Songs(SearchPage {
            items: vec![],
            total: 0,
        })));
        let (status, body) = request(resolver.clone(), "/native/search?keywords=x&type=1006").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["status"], "error");
        assert!(resolver.seen.lock().unwrap().is_none());
    }

    #[tokio::test]
    async fn upstream_failures_surface_as_bad_gateway() {
        let resolver = FakeResolver::new(Err(NcmClientError::InvalidPayload("$.code".into())));
        let (status, body) = request(resolver.clone(), "/native/search?keywords=x&type=1000").await;
        assert_eq!(status, StatusCode::BAD_GATEWAY);
        assert_eq!(body["status"], "error");
        // Omitted limit/offset fall back to the Node adapter's paging.
        let (_, _, _, limit, offset) = resolver.seen.lock().unwrap().take().unwrap();
        assert_eq!((limit, offset), (30, 0));
    }
}
