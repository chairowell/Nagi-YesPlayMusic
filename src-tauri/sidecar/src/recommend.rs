//! Typed daily-recommendation endpoint backed by `core::ncm`, shared with
//! the TUI.

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
use serde_json::json;
use yesplaymusic_core::ncm::{daily_songs_with, NcmClientError, SongHit};

use crate::playback::ncm_query;
use crate::search::song_item_body;

const RECOMMEND_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Clone)]
pub struct RecommendState {
    resolver: Arc<dyn RecommendResolver>,
}

impl RecommendState {
    pub fn production(client: Arc<ApiClient>) -> Self {
        Self {
            resolver: Arc::new(ProductionResolver { client }),
        }
    }

    #[cfg(test)]
    fn testing(resolver: Arc<dyn RecommendResolver>) -> Self {
        Self { resolver }
    }
}

#[async_trait]
trait RecommendResolver: Send + Sync {
    async fn daily_songs(&self, query: Query) -> Result<Vec<SongHit>, NcmClientError>;
}

struct ProductionResolver {
    client: Arc<ApiClient>,
}

#[async_trait]
impl RecommendResolver for ProductionResolver {
    async fn daily_songs(&self, query: Query) -> Result<Vec<SongHit>, NcmClientError> {
        daily_songs_with(&self.client, query).await
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
    let resolved = tokio::time::timeout(
        RECOMMEND_TIMEOUT,
        state
            .resolver
            .daily_songs(ncm_query(&headers, query.real_ip, query.proxy)),
    )
    .await;
    let body = match resolved {
        Ok(Ok(hits)) => (
            StatusCode::OK,
            json!({ "data": hits.iter().map(song_item_body).collect::<Vec<_>>() }),
        ),
        Ok(Err(error)) => {
            tracing::warn!(%error, "daily recommendations resolution failed");
            (
                StatusCode::BAD_GATEWAY,
                json!({ "status": "error", "message": "could not resolve daily recommendations" }),
            )
        }
        Err(_) => {
            tracing::warn!("daily recommendations timed out");
            (
                StatusCode::GATEWAY_TIMEOUT,
                json!({ "status": "error", "message": "daily recommendations timed out" }),
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

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use axum::body::to_bytes;
    use tower::ServiceExt;
    use yesplaymusic_core::ncm::{AlbumRef, ArtistRef, SongPrivilege};

    use super::*;

    struct FakeResolver {
        answer: Mutex<Option<Result<Vec<SongHit>, NcmClientError>>>,
        seen_query: Mutex<Option<Query>>,
    }

    #[async_trait]
    impl RecommendResolver for FakeResolver {
        async fn daily_songs(&self, query: Query) -> Result<Vec<SongHit>, NcmClientError> {
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
            answer: Mutex::new(Some(Ok(vec![SongHit {
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
