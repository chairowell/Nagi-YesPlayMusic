//! NCM listening-history reporting shared by desktop clients.

use async_trait::async_trait;
use ncm_api_rs::{ApiClient, ApiResponse, CryptoType, NcmError, Query, RequestOption};
use serde_json::{json, Value};

const WEBLOG_PATH: &str = "/api/feedback/weblog";
const CLIENTLOG_DOMAIN: &str = "https://clientlog.music.163.com";
const OSX_USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) \
AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";
const CLIENT_IDENTITY: [(&str, &str); 4] = [
    ("os", "osx"),
    ("appver", "3.1.10.5100"),
    ("osver", "15.5"),
    ("channel", "netease"),
];

#[async_trait]
trait Transport: Send + Sync {
    async fn send(
        &self,
        path: &'static str,
        data: Value,
        options: RequestOption,
    ) -> Result<ApiResponse, NcmError>;
}

#[async_trait]
impl Transport for ApiClient {
    async fn send(
        &self,
        path: &'static str,
        data: Value,
        options: RequestOption,
    ) -> Result<ApiResponse, NcmError> {
        self.request(path, data, options).await
    }
}

/// Report one listening session. `startplay` must succeed before `play` is sent.
pub async fn scrobble(client: &ApiClient, query: &Query) -> Result<ApiResponse, NcmError> {
    scrobble_with(client, query).await
}

async fn scrobble_with(transport: &dyn Transport, query: &Query) -> Result<ApiResponse, NcmError> {
    let song_id = query.get_or("id", "0");
    let source_id = query.get_or("sourceid", "");
    let played_seconds = query.get_i64("time", 0).max(0);
    let options = request_options(query);

    let startplay = transport
        .send(
            WEBLOG_PATH,
            startplay_payload(&song_id, &source_id)?,
            options.clone(),
        )
        .await?;
    let play = transport
        .send(
            WEBLOG_PATH,
            play_payload(&song_id, &source_id, played_seconds)?,
            options,
        )
        .await?;

    Ok(ApiResponse {
        status: 200,
        body: json!({
            "code": 200,
            "data": "success",
            "details": {
                "startplay": startplay.body,
                "play": play.body,
            },
        }),
        cookie: Vec::new(),
    })
}

fn request_options(query: &Query) -> RequestOption {
    RequestOption {
        crypto: CryptoType::Eapi,
        cookie: Some(osx_cookie(query.cookie.as_deref())),
        ua: query.ua.clone().or_else(|| Some(OSX_USER_AGENT.to_owned())),
        proxy: query.proxy.clone(),
        real_ip: query.real_ip.clone(),
        random_cn_ip: query.random_cn_ip,
        e_r: query.e_r,
        domain: Some(CLIENTLOG_DOMAIN.to_owned()),
        check_token: false,
    }
}

fn startplay_payload(song_id: &str, source_id: &str) -> Result<Value, NcmError> {
    weblog_payload(json!({
        "action": "startplay",
        "json": {
            "id": song_id,
            "type": "song",
            "mainsite": "1",
            "mainsiteWeb": "1",
            "content": format!("id={source_id}"),
        },
    }))
}

fn play_payload(song_id: &str, source_id: &str, played_seconds: i64) -> Result<Value, NcmError> {
    weblog_payload(json!({
        "action": "play",
        "json": {
            "download": 0,
            "end": "playend",
            "id": song_id,
            "sourceId": source_id,
            "time": played_seconds,
            "type": "song",
            "wifi": 0,
            "source": "list",
            "mainsite": "1",
            "mainsiteWeb": "1",
            "content": format!("id={source_id}"),
        },
    }))
}

fn weblog_payload(log: Value) -> Result<Value, NcmError> {
    Ok(json!({ "logs": serde_json::to_string(&[log])? }))
}

fn osx_cookie(cookie: Option<&str>) -> String {
    let mut fields = cookie
        .unwrap_or_default()
        .split(';')
        .filter_map(|field| {
            let (name, value) = field.trim().split_once('=')?;
            (!name.is_empty()
                && !CLIENT_IDENTITY
                    .iter()
                    .any(|(identity, _)| name == *identity))
            .then(|| (name.to_owned(), value.to_owned()))
        })
        .collect::<Vec<_>>();
    fields.extend(
        CLIENT_IDENTITY
            .into_iter()
            .map(|(name, value)| (name.to_owned(), value.to_owned())),
    );
    fields
        .into_iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect::<Vec<_>>()
        .join("; ")
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, VecDeque};
    use std::sync::Mutex;

    use super::*;

    #[derive(Debug)]
    struct Call {
        path: &'static str,
        data: Value,
        options: RequestOption,
    }

    struct RecordingTransport {
        calls: Mutex<Vec<Call>>,
        responses: Mutex<VecDeque<Result<ApiResponse, NcmError>>>,
    }

    impl RecordingTransport {
        fn succeeding() -> Self {
            Self::with_responses([
                Ok(response(json!({ "phase": "startplay" }))),
                Ok(response(json!({ "phase": "play" }))),
            ])
        }

        fn with_responses(
            responses: impl IntoIterator<Item = Result<ApiResponse, NcmError>>,
        ) -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
                responses: Mutex::new(responses.into_iter().collect()),
            }
        }
    }

    #[async_trait]
    impl Transport for RecordingTransport {
        async fn send(
            &self,
            path: &'static str,
            data: Value,
            options: RequestOption,
        ) -> Result<ApiResponse, NcmError> {
            self.calls.lock().unwrap().push(Call {
                path,
                data,
                options,
            });
            self.responses.lock().unwrap().pop_front().unwrap()
        }
    }

    #[tokio::test]
    async fn sends_startplay_then_play_with_current_clientlog_contract() {
        let transport = RecordingTransport::succeeding();
        let query = Query::new()
            .param("id", "347230")
            .param("sourceid", "778899")
            .param("time", "83")
            .cookie("MUSIC_U=session; __csrf=token; os=pc; appver=old");

        let response = scrobble_with(&transport, &query).await.unwrap();
        let calls = transport.calls.lock().unwrap();

        assert_eq!(response.status, 200);
        assert_eq!(response.body["code"], 200);
        assert_eq!(response.body["data"], "success");
        assert_eq!(calls.len(), 2);
        for call in calls.iter() {
            assert_eq!(call.path, WEBLOG_PATH);
            assert_eq!(call.options.crypto, CryptoType::Eapi);
            assert_eq!(call.options.domain.as_deref(), Some(CLIENTLOG_DOMAIN));
            let cookie = call.options.cookie.as_deref().unwrap();
            let fields = cookie_fields(cookie);
            assert_eq!(fields.get("MUSIC_U").map(String::as_str), Some("session"));
            assert_eq!(fields.get("__csrf").map(String::as_str), Some("token"));
            for (name, value) in CLIENT_IDENTITY {
                assert_eq!(fields.get(name).map(String::as_str), Some(value));
            }
        }

        let startplay = decoded_log(&calls[0].data);
        assert_eq!(startplay["action"], "startplay");
        assert_eq!(startplay["json"]["id"], "347230");
        assert_eq!(startplay["json"]["type"], "song");
        assert_eq!(startplay["json"]["mainsite"], "1");
        assert_eq!(startplay["json"]["mainsiteWeb"], "1");
        assert_eq!(startplay["json"]["content"], "id=778899");

        let play = decoded_log(&calls[1].data);
        assert_eq!(play["action"], "play");
        assert_eq!(play["json"]["id"], "347230");
        assert_eq!(play["json"]["sourceId"], "778899");
        assert_eq!(play["json"]["time"], 83);
        assert_eq!(play["json"]["type"], "song");
        assert_eq!(play["json"]["end"], "playend");
        assert_eq!(play["json"]["source"], "list");
        assert_eq!(play["json"]["mainsite"], "1");
        assert_eq!(play["json"]["mainsiteWeb"], "1");
        assert_eq!(play["json"]["content"], "id=778899");
    }

    #[tokio::test]
    async fn first_failure_stops_before_play_and_second_failure_is_returned() {
        let first_failure = RecordingTransport::with_responses([Err(NcmError::Unknown(
            "startplay failed".to_owned(),
        ))]);
        let error = scrobble_with(&first_failure, &Query::new())
            .await
            .unwrap_err();
        assert_eq!(error.to_string(), "startplay failed");
        assert_eq!(first_failure.calls.lock().unwrap().len(), 1);

        let second_failure = RecordingTransport::with_responses([
            Ok(response(json!({ "code": 200 }))),
            Err(NcmError::Unknown("play failed".to_owned())),
        ]);
        let error = scrobble_with(&second_failure, &Query::new())
            .await
            .unwrap_err();
        assert_eq!(error.to_string(), "play failed");
        assert_eq!(second_failure.calls.lock().unwrap().len(), 2);
    }

    #[test]
    fn desktop_identity_replaces_duplicates_and_preserves_login_cookie() {
        let cookie = osx_cookie(Some(
            "MUSIC_U=secret==; os=pc; __csrf=token; os=android; \
             appver=old; osver=old; channel=old",
        ));
        let fields = cookie_fields(&cookie);

        assert_eq!(fields.get("MUSIC_U").map(String::as_str), Some("secret=="));
        assert_eq!(fields.get("__csrf").map(String::as_str), Some("token"));
        for (name, value) in CLIENT_IDENTITY {
            assert_eq!(fields.get(name).map(String::as_str), Some(value));
            assert_eq!(
                cookie
                    .split(';')
                    .filter(|field| field.trim().starts_with(&format!("{name}=")))
                    .count(),
                1
            );
        }
    }

    #[test]
    fn desktop_identity_is_added_when_cookie_has_none() {
        let cookie = osx_cookie(Some("MUSIC_U=secret; __csrf=token"));
        let fields = cookie_fields(&cookie);

        assert_eq!(fields.get("MUSIC_U").map(String::as_str), Some("secret"));
        assert_eq!(fields.get("__csrf").map(String::as_str), Some("token"));
        for (name, value) in CLIENT_IDENTITY {
            assert_eq!(fields.get(name).map(String::as_str), Some(value));
        }
    }

    fn response(body: Value) -> ApiResponse {
        ApiResponse {
            status: 200,
            body,
            cookie: Vec::new(),
        }
    }

    fn decoded_log(payload: &Value) -> Value {
        serde_json::from_str::<Value>(payload["logs"].as_str().unwrap()).unwrap()[0].clone()
    }

    fn cookie_fields(cookie: &str) -> HashMap<String, String> {
        cookie
            .split(';')
            .filter_map(|field| {
                let (name, value) = field.trim().split_once('=')?;
                Some((name.to_owned(), value.to_owned()))
            })
            .collect()
    }
}
