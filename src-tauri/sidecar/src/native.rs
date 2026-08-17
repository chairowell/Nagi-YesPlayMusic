use std::{future::Future, time::Duration};

use axum::{
    body::Body,
    http::{header, HeaderValue, Response, StatusCode},
    response::IntoResponse,
    Json,
};
use serde_json::{json, Value};
use yesplaymusic_core::ncm::{NcmClientError, SongItem};

const NATIVE_TIMEOUT: Duration = Duration::from_secs(15);

pub(crate) async fn respond<F>(kind: &'static str, fut: F) -> Response<Body>
where
    F: Future<Output = Result<Value, NcmClientError>>,
{
    let body = match tokio::time::timeout(NATIVE_TIMEOUT, fut).await {
        Ok(Ok(payload)) => (StatusCode::OK, payload),
        Ok(Err(error)) => {
            tracing::warn!(kind, %error, "native endpoint resolution failed");
            (
                StatusCode::BAD_GATEWAY,
                json!({ "status": "error", "message": format!("could not resolve {kind}") }),
            )
        }
        Err(_) => {
            tracing::warn!(kind, "native endpoint resolution timed out");
            (
                StatusCode::GATEWAY_TIMEOUT,
                json!({ "status": "error", "message": format!("{kind} timed out") }),
            )
        }
    };
    with_no_store((body.0, Json(body.1)).into_response())
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
