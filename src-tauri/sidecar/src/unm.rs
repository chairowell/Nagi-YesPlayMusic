//! HTTP adapter for the UNM resolver; the logic itself lives in
//! yesplaymusic-core so the TUI can call it without axum.

use axum::{
    extract::{rejection::JsonRejection, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use serde_json::{json, Value};

pub use yesplaymusic_core::unm::{
    run_hermetic_executor_smoke, BilibiliClient, BilibiliDownloadRequest, InvalidTrack,
    UnmDependencyResult, UnmExecutorBackend, UnmState, BILIBILI_REFERER, BILIBILI_USER_AGENT,
    DEFAULT_SOURCES,
};

pub fn router(state: UnmState) -> Router {
    Router::new()
        .route("/native/unblock-music", post(unblock_music_handler))
        .with_state(state)
}

pub async fn unblock_music_handler(
    State(state): State<UnmState>,
    payload: Result<Json<Value>, JsonRejection>,
) -> Response {
    let Json(payload) = match payload {
        Ok(payload) => payload,
        Err(_) => return invalid_track_response(),
    };
    match state.resolve(&payload).await {
        Ok(result) => Json(result).into_response(),
        Err(_) => invalid_track_response(),
    }
}

fn invalid_track_response() -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(json!({ "message": "缺少歌曲信息" })),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{header, Request, StatusCode};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    use super::*;

    #[tokio::test]
    async fn malformed_payloads_answer_with_the_legacy_error_body() {
        let app = router(UnmState::new());
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/native/unblock-music")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from("not json"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(
            serde_json::from_slice::<Value>(&body).unwrap(),
            json!({ "message": "缺少歌曲信息" })
        );
    }
}
