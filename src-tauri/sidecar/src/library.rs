//! Typed library endpoints (liked ids, like toggle, FM trash) backed by
//! `core::ncm`, shared with the TUI.

use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use axum::{
    body::Body,
    extract::{Query as UrlQuery, State},
    http::{HeaderMap, Response, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use ncm_api_rs::{api::Query, ApiClient};
use serde::Deserialize;
use serde_json::json;
use yesplaymusic_core::ncm::{
    capture_rotated_cookies, fm_trash_with, liked_ids_with, set_like_with, NcmClientError,
};

use crate::native::{respond, with_no_store};

const LIBRARY_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Clone)]
pub struct LibraryState {
    resolver: Arc<dyn LibraryResolver>,
}

impl LibraryState {
    pub fn production(client: Arc<ApiClient>) -> Self {
        Self { resolver: client }
    }

    #[cfg(test)]
    fn testing(resolver: Arc<dyn LibraryResolver>) -> Self {
        Self { resolver }
    }
}

#[async_trait]
trait LibraryResolver: Send + Sync {
    async fn liked_ids(&self, query: Query, uid: i64) -> Result<Vec<i64>, NcmClientError>;
    async fn set_like(&self, query: Query, id: i64, like: bool) -> Result<(), NcmClientError>;
    async fn fm_trash(&self, query: Query, id: i64) -> Result<(), NcmClientError>;
}

#[async_trait]
impl LibraryResolver for ApiClient {
    async fn liked_ids(&self, query: Query, uid: i64) -> Result<Vec<i64>, NcmClientError> {
        liked_ids_with(self, query, uid).await
    }

    async fn set_like(&self, query: Query, id: i64, like: bool) -> Result<(), NcmClientError> {
        set_like_with(self, query, id, like).await
    }

    async fn fm_trash(&self, query: Query, id: i64) -> Result<(), NcmClientError> {
        fm_trash_with(self, query, id).await
    }
}

#[derive(Deserialize)]
struct LikedIdsQuery {
    uid: i64,
    #[serde(rename = "realIP")]
    real_ip: Option<String>,
    proxy: Option<String>,
}

#[derive(Deserialize)]
struct LikeQuery {
    id: i64,
    like: bool,
    #[serde(rename = "realIP")]
    real_ip: Option<String>,
    proxy: Option<String>,
}

#[derive(Deserialize)]
struct TrashQuery {
    id: i64,
    #[serde(rename = "realIP")]
    real_ip: Option<String>,
    proxy: Option<String>,
}

pub fn router(state: LibraryState) -> Router {
    Router::new()
        .route("/native/library/liked-ids", get(liked_ids_handler))
        .route("/native/library/like", post(like_handler))
        .route("/native/library/fm-trash", post(fm_trash_handler))
        .with_state(state)
}

async fn liked_ids_handler(
    State(state): State<LibraryState>,
    UrlQuery(query): UrlQuery<LikedIdsQuery>,
    headers: HeaderMap,
) -> Response<Body> {
    let request = crate::playback::ncm_query(&headers, query.real_ip, query.proxy);
    respond("liked songs", async move {
        let ids = state.resolver.liked_ids(request, query.uid).await?;
        Ok(json!({ "ids": ids }))
    })
    .await
}

async fn like_handler(
    State(state): State<LibraryState>,
    UrlQuery(query): UrlQuery<LikeQuery>,
    headers: HeaderMap,
) -> Response<Body> {
    mutation_response(
        "like",
        tokio::time::timeout(
            LIBRARY_TIMEOUT,
            capture_rotated_cookies(state.resolver.set_like(
                crate::playback::ncm_query(&headers, query.real_ip, query.proxy),
                query.id,
                query.like,
            )),
        )
        .await,
    )
}

async fn fm_trash_handler(
    State(state): State<LibraryState>,
    UrlQuery(query): UrlQuery<TrashQuery>,
    headers: HeaderMap,
) -> Response<Body> {
    mutation_response(
        "fm-trash",
        tokio::time::timeout(
            LIBRARY_TIMEOUT,
            capture_rotated_cookies(state.resolver.fm_trash(
                crate::playback::ncm_query(&headers, query.real_ip, query.proxy),
                query.id,
            )),
        )
        .await,
    )
}

type CapturedMutation =
    Result<(Result<(), NcmClientError>, Vec<String>), tokio::time::error::Elapsed>;

fn mutation_response(operation: &'static str, resolved: CapturedMutation) -> Response<Body> {
    let (response, cookies) = match resolved {
        Ok((Ok(()), cookies)) => (StatusCode::NO_CONTENT.into_response(), cookies),
        Ok((Err(error), cookies)) if crate::native::is_session_expired(&error) => {
            tracing::warn!(operation, "library mutation refused: session expired");
            (
                (
                    StatusCode::UNAUTHORIZED,
                    Json(crate::native::session_expired_body()),
                )
                    .into_response(),
                cookies,
            )
        }
        Ok((Err(NcmClientError::Rejected(code)), cookies)) => {
            tracing::warn!(operation, ?code, "library mutation rejected");
            (
                (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    Json(json!({ "status": "rejected", "code": code })),
                )
                    .into_response(),
                cookies,
            )
        }
        Ok((Err(error), cookies)) => {
            tracing::warn!(operation, %error, "library mutation failed");
            (
                (
                    StatusCode::BAD_GATEWAY,
                    Json(json!({ "status": "error", "message": "library operation failed" })),
                )
                    .into_response(),
                cookies,
            )
        }
        Err(_) => (
            (
                StatusCode::GATEWAY_TIMEOUT,
                Json(json!({ "status": "error", "message": "library operation timed out" })),
            )
                .into_response(),
            Vec::new(),
        ),
    };
    let mut response = with_no_store(response);
    crate::native::append_set_cookies(&mut response, &cookies);
    response
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use axum::{body::to_bytes, http::header};
    use tower::ServiceExt;

    use super::*;

    #[derive(Default)]
    struct FakeResolver {
        liked: Mutex<Option<Result<Vec<i64>, NcmClientError>>>,
        like_outcome: Mutex<Option<Result<(), NcmClientError>>>,
        seen_query: Mutex<Option<Query>>,
        seen_like: Mutex<Option<(i64, bool)>>,
    }

    #[async_trait]
    impl LibraryResolver for FakeResolver {
        async fn liked_ids(&self, query: Query, _uid: i64) -> Result<Vec<i64>, NcmClientError> {
            *self.seen_query.lock().unwrap() = Some(query);
            self.liked.lock().unwrap().take().expect("single call")
        }

        async fn set_like(&self, query: Query, id: i64, like: bool) -> Result<(), NcmClientError> {
            *self.seen_query.lock().unwrap() = Some(query);
            *self.seen_like.lock().unwrap() = Some((id, like));
            self.like_outcome
                .lock()
                .unwrap()
                .take()
                .expect("single call")
        }

        async fn fm_trash(&self, query: Query, id: i64) -> Result<(), NcmClientError> {
            *self.seen_query.lock().unwrap() = Some(query);
            *self.seen_like.lock().unwrap() = Some((id, false));
            self.like_outcome
                .lock()
                .unwrap()
                .take()
                .expect("single call")
        }
    }

    async fn request(
        resolver: Arc<FakeResolver>,
        method: axum::http::Method,
        uri: &str,
    ) -> (StatusCode, Vec<u8>) {
        let app = router(LibraryState::testing(resolver));
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method(method)
                    .uri(uri)
                    .header(header::COOKIE, "MUSIC_U=token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let body = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
        (status, body.to_vec())
    }

    #[tokio::test]
    async fn liked_ids_answer_in_upstream_order_with_the_cookie_forwarded() {
        let resolver = Arc::new(FakeResolver {
            liked: Mutex::new(Some(Ok(vec![3, 1, 2]))),
            ..FakeResolver::default()
        });
        let (status, body) = request(
            resolver.clone(),
            axum::http::Method::GET,
            "/native/library/liked-ids?uid=42&realIP=1.2.3.4",
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
        // Most-recently-liked-first order is user-visible in the GUI.
        assert_eq!(body["ids"], serde_json::json!([3, 1, 2]));
        let query = resolver.seen_query.lock().unwrap().take().unwrap();
        assert_eq!(query.cookie.as_deref(), Some("MUSIC_U=token"));
        assert_eq!(query.real_ip.as_deref(), Some("1.2.3.4"));
    }

    #[tokio::test]
    async fn like_toggle_is_no_content_on_success_and_422_on_refusal() {
        let resolver = Arc::new(FakeResolver {
            like_outcome: Mutex::new(Some(Ok(()))),
            ..FakeResolver::default()
        });
        let (status, _) = request(
            resolver.clone(),
            axum::http::Method::POST,
            "/native/library/like?id=7&like=true",
        )
        .await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        assert_eq!(*resolver.seen_like.lock().unwrap(), Some((7, true)));

        let refused = Arc::new(FakeResolver {
            like_outcome: Mutex::new(Some(Err(NcmClientError::Rejected(Some(-462))))),
            ..FakeResolver::default()
        });
        let (status, body) = request(
            refused,
            axum::http::Method::POST,
            "/native/library/like?id=7&like=false",
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body["status"], "rejected");
    }

    #[tokio::test]
    async fn fm_trash_shares_the_mutation_contract() {
        let resolver = Arc::new(FakeResolver {
            like_outcome: Mutex::new(Some(Ok(()))),
            ..FakeResolver::default()
        });
        let (status, _) = request(
            resolver,
            axum::http::Method::POST,
            "/native/library/fm-trash?id=99",
        )
        .await;
        assert_eq!(status, StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn an_expired_session_turns_mutations_into_401_not_422() {
        let resolver = Arc::new(FakeResolver {
            like_outcome: Mutex::new(Some(Err(NcmClientError::Api(
                ncm_api_rs::NcmError::AuthRequired("需要登录".into()),
            )))),
            ..FakeResolver::default()
        });
        let (status, body) = request(
            resolver,
            axum::http::Method::POST,
            "/native/library/like?id=7&like=true",
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body["code"], 301);
        assert_eq!(body["msg"], "需要登录");
    }
}
