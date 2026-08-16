//! Typed playback-source endpoint backed by `core::ncm`.
//!
//! The renderer used to fetch `/song/url` and classify the answer itself;
//! this endpoint moves that business logic server-side so the GUI and the
//! TUI share one implementation (candidate matching, free-trial refusal,
//! rejected-vs-unavailable split).

use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use axum::{
    body::Body,
    extract::{Path, Query as UrlQuery, State},
    http::{header, HeaderMap, HeaderValue, Response, StatusCode},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use ncm_api_rs::{api::Query, ApiClient};
use serde::Deserialize;
use serde_json::json;
use yesplaymusic_core::ncm::{song_url_with, PlaybackSource, SongUrlError};

const RESOLVE_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Clone)]
pub struct PlaybackState {
    resolver: Arc<dyn PlaybackResolver>,
}

impl PlaybackState {
    pub fn production(client: Arc<ApiClient>) -> Self {
        Self {
            resolver: Arc::new(ProductionResolver { client }),
        }
    }

    #[cfg(test)]
    fn testing(resolver: Arc<dyn PlaybackResolver>) -> Self {
        Self { resolver }
    }
}

#[async_trait]
trait PlaybackResolver: Send + Sync {
    async fn song_url(
        &self,
        cookie: Option<String>,
        track_id: i64,
        bitrate: u32,
    ) -> Result<PlaybackSource, SongUrlError>;
}

struct ProductionResolver {
    client: Arc<ApiClient>,
}

#[async_trait]
impl PlaybackResolver for ProductionResolver {
    async fn song_url(
        &self,
        cookie: Option<String>,
        track_id: i64,
        bitrate: u32,
    ) -> Result<PlaybackSource, SongUrlError> {
        let mut query = Query::new();
        query.cookie = cookie;
        song_url_with(&self.client, query, track_id, bitrate).await
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SourceQuery {
    bitrate: u32,
}

pub fn router(state: PlaybackState) -> Router {
    Router::new()
        .route("/native/playback/source/{track_id}", get(source_handler))
        .with_state(state)
}

async fn source_handler(
    State(state): State<PlaybackState>,
    Path(track_id): Path<i64>,
    UrlQuery(query): UrlQuery<SourceQuery>,
    headers: HeaderMap,
) -> Response<Body> {
    let cookie = headers
        .get(header::COOKIE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let resolved = tokio::time::timeout(
        RESOLVE_TIMEOUT,
        state.resolver.song_url(cookie, track_id, query.bitrate),
    )
    .await;
    let body = match resolved {
        Ok(Ok(source)) => (StatusCode::OK, source_body(&source)),
        Ok(Err(SongUrlError::Unavailable)) => (StatusCode::OK, json!({ "status": "unavailable" })),
        Ok(Err(SongUrlError::Rejected(code))) => (
            StatusCode::OK,
            json!({ "status": "rejected", "code": code }),
        ),
        Ok(Err(SongUrlError::Other(error))) => {
            tracing::warn!(track_id, %error, "playback source resolution failed");
            (
                StatusCode::BAD_GATEWAY,
                json!({ "status": "error", "message": "could not resolve the playback source" }),
            )
        }
        Err(_) => {
            tracing::warn!(track_id, "playback source resolution timed out");
            (
                StatusCode::GATEWAY_TIMEOUT,
                json!({ "status": "error", "message": "playback source resolution timed out" }),
            )
        }
    };
    let mut response = (body.0, Json(body.1)).into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

fn source_body(source: &PlaybackSource) -> serde_json::Value {
    json!({
        "status": "ok",
        "url": source.url,
        "codec": source.codec.extension(),
        "actualBitrate": source.actual_bitrate,
        "expectedBytes": source.expected_bytes,
        "expectedMd5": source.expected_md5.map(encode_hex),
    })
}

fn encode_hex(bytes: [u8; 16]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use axum::body::to_bytes;
    use tower::ServiceExt;
    use yesplaymusic_core::media::AudioCodec;
    use yesplaymusic_core::ncm::NcmClientError;

    use super::*;

    struct FakeResolver {
        outcome: Mutex<Option<Result<PlaybackSource, SongUrlError>>>,
        seen_cookie: Mutex<Option<Option<String>>>,
        seen_request: Mutex<Option<(i64, u32)>>,
    }

    impl FakeResolver {
        fn new(outcome: Result<PlaybackSource, SongUrlError>) -> Arc<Self> {
            Arc::new(Self {
                outcome: Mutex::new(Some(outcome)),
                seen_cookie: Mutex::new(None),
                seen_request: Mutex::new(None),
            })
        }
    }

    #[async_trait]
    impl PlaybackResolver for FakeResolver {
        async fn song_url(
            &self,
            cookie: Option<String>,
            track_id: i64,
            bitrate: u32,
        ) -> Result<PlaybackSource, SongUrlError> {
            *self.seen_cookie.lock().unwrap() = Some(cookie);
            *self.seen_request.lock().unwrap() = Some((track_id, bitrate));
            self.outcome.lock().unwrap().take().expect("single call")
        }
    }

    async fn request(
        resolver: Arc<FakeResolver>,
        uri: &str,
        cookie: Option<&str>,
    ) -> (StatusCode, serde_json::Value) {
        let app = router(PlaybackState::testing(resolver));
        let mut builder = axum::http::Request::builder().uri(uri);
        if let Some(cookie) = cookie {
            builder = builder.header(header::COOKIE, cookie);
        }
        let response = app
            .oneshot(builder.body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let body = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
        (status, serde_json::from_slice(&body).unwrap())
    }

    #[tokio::test]
    async fn resolved_source_answers_typed_metadata_and_forwards_the_cookie() {
        let resolver = FakeResolver::new(Ok(PlaybackSource {
            url: "https://audio.example/track.flac".into(),
            codec: AudioCodec::Flac,
            actual_bitrate: 850_321,
            expected_bytes: Some(12_345_678),
            expected_md5: Some([0xab; 16]),
        }));
        let (status, body) = request(
            resolver.clone(),
            "/native/playback/source/186016?bitrate=350000",
            Some("MUSIC_U=token"),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["status"], "ok");
        assert_eq!(body["url"], "https://audio.example/track.flac");
        assert_eq!(body["codec"], "flac");
        assert_eq!(body["actualBitrate"], 850_321);
        assert_eq!(body["expectedBytes"], 12_345_678);
        assert_eq!(body["expectedMd5"], "ab".repeat(16));
        assert_eq!(
            *resolver.seen_cookie.lock().unwrap(),
            Some(Some("MUSIC_U=token".to_owned()))
        );
        assert_eq!(
            *resolver.seen_request.lock().unwrap(),
            Some((186_016, 350_000))
        );
    }

    #[tokio::test]
    async fn unavailable_and_rejected_are_data_not_transport_errors() {
        let (status, body) = request(
            FakeResolver::new(Err(SongUrlError::Unavailable)),
            "/native/playback/source/42?bitrate=320000",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["status"], "unavailable");

        let (status, body) = request(
            FakeResolver::new(Err(SongUrlError::Rejected(Some(301)))),
            "/native/playback/source/42?bitrate=320000",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["status"], "rejected");
        assert_eq!(body["code"], 301);
    }

    #[tokio::test]
    async fn transport_failures_surface_as_bad_gateway() {
        let (status, body) = request(
            FakeResolver::new(Err(SongUrlError::Other(NcmClientError::MalformedPayload(
                "playback response is missing its audio codec",
            )))),
            "/native/playback/source/42?bitrate=320000",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_GATEWAY);
        assert_eq!(body["status"], "error");
    }
}
