use std::{future::Future, time::Duration};

use axum::{
    body::Body,
    http::{header, HeaderValue, Response, StatusCode},
    response::IntoResponse,
    Json,
};
use ncm_api_rs::NcmError;
use serde_json::{json, Value};
use yesplaymusic_core::ncm::{capture_rotated_cookies, NcmClientError, SongItem};

const NATIVE_TIMEOUT: Duration = Duration::from_secs(15);

/// The generic `/api` route's session-expiry contract; the renderer's
/// expiry interceptor matches this body, not the bare 401.
pub(crate) fn session_expired_body() -> Value {
    json!({ "code": 301, "msg": "需要登录" })
}

pub(crate) fn is_session_expired(error: &NcmClientError) -> bool {
    matches!(error, NcmClientError::Api(NcmError::AuthRequired(_)))
}

pub(crate) async fn respond<F>(kind: &'static str, fut: F) -> Response<Body>
where
    F: Future<Output = Result<Value, NcmClientError>>,
{
    let (body, cookies) = match tokio::time::timeout(NATIVE_TIMEOUT, capture_rotated_cookies(fut))
        .await
    {
        Ok((Ok(payload), cookies)) => ((StatusCode::OK, payload), cookies),
        Ok((Err(error), cookies)) if is_session_expired(&error) => {
            tracing::warn!(kind, "native endpoint refused: session expired");
            ((StatusCode::UNAUTHORIZED, session_expired_body()), cookies)
        }
        Ok((Err(error), cookies)) => {
            tracing::warn!(kind, %error, "native endpoint resolution failed");
            (
                (
                    StatusCode::BAD_GATEWAY,
                    json!({ "status": "error", "message": format!("could not resolve {kind}") }),
                ),
                cookies,
            )
        }
        Err(_) => {
            tracing::warn!(kind, "native endpoint resolution timed out");
            (
                (
                    StatusCode::GATEWAY_TIMEOUT,
                    json!({ "status": "error", "message": format!("{kind} timed out") }),
                ),
                Vec::new(),
            )
        }
    };
    let mut response = with_no_store((body.0, Json(body.1)).into_response());
    append_set_cookies(&mut response, &cookies);
    response
}

/// Forward upstream Set-Cookie rotation verbatim; the renderer proxy hardens
/// the auth cookies on the way to the browser, same as the generic route.
pub(crate) fn append_set_cookies(response: &mut Response<Body>, cookies: &[String]) {
    for cookie in cookies {
        match HeaderValue::from_str(cookie) {
            Ok(value) => {
                response.headers_mut().append(header::SET_COOKIE, value);
            }
            Err(error) => {
                tracing::warn!(%error, "dropping an unencodable upstream Set-Cookie");
            }
        }
    }
}

pub(crate) fn with_no_store(mut response: Response<Body>) -> Response<Body> {
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

pub(crate) fn song_item_body(song: &SongItem) -> Value {
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
        "cd": song.cd,
    })
}
