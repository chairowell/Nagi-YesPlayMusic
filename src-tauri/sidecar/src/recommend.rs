//! Typed daily-recommendation endpoint backed by `core::ncm`, shared with
//! the TUI.

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
use serde_json::json;
use yesplaymusic_core::ncm::{daily_songs_with, NcmClientError, SongItem};

use crate::native::{respond, song_item_body};
use crate::playback::ncm_query;

#[derive(Clone)]
pub struct RecommendState {
    resolver: Arc<dyn RecommendResolver>,
}

impl RecommendState {
    pub fn production(client: Arc<ApiClient>) -> Self {
        Self { resolver: client }
    }

    #[cfg(test)]
    fn testing(resolver: Arc<dyn RecommendResolver>) -> Self {
        Self { resolver }
    }
}

#[async_trait]
trait RecommendResolver: Send + Sync {
    async fn daily_songs(&self, query: Query) -> Result<Vec<SongItem>, NcmClientError>;
}

#[async_trait]
impl RecommendResolver for ApiClient {
    async fn daily_songs(&self, query: Query) -> Result<Vec<SongItem>, NcmClientError> {
        daily_songs_with(self, query).await
    }
}

#[derive(Deserialize)]
struct RecommendQuery {
    #[serde(rename = "realIP")]
    real_ip: Option<String>,
    proxy: Option<String>,
}

pub fn router(state: RecommendState) -> Router {
    Router::new()
        .route("/native/recommend/daily-songs", get(daily_songs_handler))
        .with_state(state)
}

async fn daily_songs_handler(
    State(state): State<RecommendState>,
    UrlQuery(query): UrlQuery<RecommendQuery>,
    headers: HeaderMap,
) -> Response<Body> {
    let query = ncm_query(&headers, query.real_ip, query.proxy);
    respond("daily recommendations", async move {
        let items = state.resolver.daily_songs(query).await?;
        Ok(json!({
            "data": items.iter().map(song_item_body).collect::<Vec<_>>()
        }))
    })
    .await
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use axum::{
        body::to_bytes,
        http::{header, StatusCode},
    };
    use tower::ServiceExt;
    use yesplaymusic_core::ncm::{AlbumRef, ArtistRef, SongPrivilege};

    use super::*;

    struct FakeResolver {
        answer: Mutex<Option<Result<Vec<SongItem>, NcmClientError>>>,
        seen_query: Mutex<Option<Query>>,
    }

    #[async_trait]
    impl RecommendResolver for FakeResolver {
        async fn daily_songs(&self, query: Query) -> Result<Vec<SongItem>, NcmClientError> {
            *self.seen_query.lock().unwrap() = Some(query);
            self.answer.lock().unwrap().take().expect("single call")
        }
    }

    async fn request(resolver: Arc<FakeResolver>, uri: &str) -> (StatusCode, serde_json::Value) {
        let app = router(RecommendState::testing(resolver));
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
        let body = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
        (status, serde_json::from_slice(&body).unwrap())
    }

    #[tokio::test]
    async fn daily_songs_answer_the_shared_song_shape_with_privileges() {
        let resolver = Arc::new(FakeResolver {
            answer: Mutex::new(Some(Ok(vec![SongItem {
                id: 1,
                name: "Daily".into(),
                artists: vec![ArtistRef {
                    id: 7,
                    name: "Artist".into(),
                }],
                album: AlbumRef {
                    id: 9,
                    name: "Album".into(),
                    pic_url: None,
                },
                duration_ms: 200_000,
                alias: vec![],
                trans_names: vec![],
                mark: 0,
                fee: Some(8),
                no_copyright_rcmd: false,
                privilege: Some(SongPrivilege {
                    pl: 320_000,
                    cs: false,
                    fee: 8,
                    st: 0,
                }),
                cd: None,
            }]))),
            seen_query: Mutex::new(None),
        });
        let (status, body) = request(
            resolver.clone(),
            "/native/recommend/daily-songs?realIP=1.2.3.4",
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["data"][0]["id"], 1);
        // The privilege must survive: the GUI greys VIP tracks from it.
        assert_eq!(body["data"][0]["privilege"]["pl"], 320_000);
        let query = resolver.seen_query.lock().unwrap().take().unwrap();
        assert_eq!(query.cookie.as_deref(), Some("MUSIC_U=token"));
        assert_eq!(query.real_ip.as_deref(), Some("1.2.3.4"));
    }

    #[tokio::test]
    async fn daily_songs_upstream_failure_is_a_bad_gateway() {
        let resolver = Arc::new(FakeResolver {
            answer: Mutex::new(Some(Err(NcmClientError::MissingPayload("data")))),
            seen_query: Mutex::new(None),
        });
        let (status, body) = request(resolver, "/native/recommend/daily-songs").await;
        assert_eq!(status, StatusCode::BAD_GATEWAY);
        assert_eq!(body["status"], "error");
    }
}
