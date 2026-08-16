//! Typed NCM endpoint client shared by every Rust frontend.
//!
//! Every call injects the persisted session cookie; anonymous calls degrade
//! the same way the desktop client does (standard quality, no personal data).
//! Errors are typed so each frontend attaches its own user-facing wording
//! (the TUI translates them, a future CLI can print them raw).

use std::path::PathBuf;
use std::sync::RwLock;

use ncm_api_rs::{api::Query, ApiClient, NcmError};
use serde_json::Value;

use crate::auth::{Session, SessionStore};

#[derive(Debug, thiserror::Error)]
pub enum NcmClientError {
    #[error(transparent)]
    Api(#[from] NcmError),
    #[error("NCM response is missing {0}")]
    MissingPayload(&'static str),
    #[error("QR login answered an unknown status {0}")]
    UnknownQrStatus(i64),
    #[error("QR login succeeded without a session cookie")]
    LoginCookieMissing,
    #[error("could not persist the session: {0}")]
    PersistSession(#[from] std::io::Error),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QrStatus {
    Waiting,
    Scanned,
    Expired,
    Success(Session),
}

/// Why `account()` failed — the split decides whether a frontend treats the
/// stored session as dead or merely unverifiable.
#[derive(Debug)]
pub enum AccountError {
    /// NCM never answered (offline, DNS, timeout, rate limit, 5xx). The
    /// stored session may still be valid; don't log the user out over it.
    Unreachable(AccountReason),
    /// NCM answered and rejected or omitted the account: the session is dead.
    Expired(AccountReason),
}

#[derive(Debug)]
pub enum AccountReason {
    Api(NcmError),
    /// The body carried no usable account payload.
    InvalidPayload,
}

pub struct NcmClient {
    client: ApiClient,
    store: SessionStore,
    session: RwLock<Option<Session>>,
}

impl NcmClient {
    pub fn new(session_path: impl Into<PathBuf>) -> Self {
        let store = SessionStore::new(session_path);
        let session = RwLock::new(store.load());
        Self {
            client: ApiClient::new(None),
            store,
            session,
        }
    }

    /// Raw transport for endpoints a frontend has not migrated to typed
    /// methods yet. Shrinks as the migration progresses.
    pub const fn api(&self) -> &ApiClient {
        &self.client
    }

    pub fn session_snapshot(&self) -> Option<Session> {
        self.session.read().ok().and_then(|session| session.clone())
    }

    pub fn commit_session(&self, session: &Session) -> Result<(), NcmClientError> {
        self.store.save(session)?;
        *self.session.write().expect("session lock") = Some(session.clone());
        Ok(())
    }

    pub fn query(&self) -> Query {
        let session = self.session_snapshot();
        Self::query_with_session(session.as_ref())
    }

    pub fn query_with_session(session: Option<&Session>) -> Query {
        match session.map(Session::cookie_header) {
            Some(cookie) => Query::new().cookie(&cookie),
            None => Query::new(),
        }
    }

    // ── login ────────────────────────────────────────────────────────

    pub async fn qr_key(&self) -> Result<String, NcmClientError> {
        let response = self.client.login_qr_key(&self.query()).await?;
        let body = &response.body;
        body["unikey"]
            .as_str()
            .or_else(|| body["data"]["unikey"].as_str())
            .map(str::to_owned)
            .ok_or(NcmClientError::MissingPayload("unikey"))
    }

    pub fn qr_login_url(key: &str) -> String {
        format!("https://music.163.com/login?codekey={key}")
    }

    pub async fn qr_check(&self, key: &str) -> Result<QrStatus, NcmClientError> {
        let query = self.query().param("key", key);
        let response = self.client.login_qr_check(&query).await?;
        parse_qr_status(&response.body, &response.cookie)
    }

    // ── account ──────────────────────────────────────────────────────

    pub async fn account(
        &self,
        session: Option<&Session>,
    ) -> Result<(i64, String), AccountError> {
        let response = self
            .client
            .user_account(&Self::query_with_session(session))
            .await
            .map_err(classify_account_error)?;
        account_from_body(&response.body)
    }
}

fn parse_qr_status(body: &Value, cookies: &[String]) -> Result<QrStatus, NcmClientError> {
    match body["code"].as_i64().unwrap_or(0) {
        800 => Ok(QrStatus::Expired),
        801 => Ok(QrStatus::Waiting),
        802 => Ok(QrStatus::Scanned),
        803 => Session::from_set_cookies(cookies)
            .map(QrStatus::Success)
            .ok_or(NcmClientError::LoginCookieMissing),
        other => Err(NcmClientError::UnknownQrStatus(other)),
    }
}

/// Only an explicit auth rejection proves the session expired; every other
/// failure mode (transport, throttling, server trouble) leaves it unknown.
fn classify_account_error(error: NcmError) -> AccountError {
    match error {
        NcmError::AuthRequired(_) => AccountError::Expired(AccountReason::Api(error)),
        _ => AccountError::Unreachable(AccountReason::Api(error)),
    }
}

/// NCM's logged-out answer is a well-formed code-200 body with no account.
/// Anything else missing the account — captive-portal HTML passed through as
/// a string body, an EAPI decrypt that fell back to null — never proves the
/// session dead.
fn account_from_body(body: &Value) -> Result<(i64, String), AccountError> {
    match parse_account(body) {
        Some(account) => Ok(account),
        None if body["code"].as_i64() == Some(200) => {
            Err(AccountError::Expired(AccountReason::InvalidPayload))
        }
        None => Err(AccountError::Unreachable(AccountReason::InvalidPayload)),
    }
}

fn parse_account(body: &Value) -> Option<(i64, String)> {
    let uid = body["account"]["id"].as_i64()?;
    let nickname = body["profile"]["nickname"]
        .as_str()
        .unwrap_or("")
        .to_owned();
    Some((uid, nickname))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_captive_portal_body_does_not_expire_the_session() {
        // Real logged-out answer: code-200 JSON without an account.
        assert!(matches!(
            account_from_body(&serde_json::json!({ "code": 200, "account": null })),
            Err(AccountError::Expired(AccountReason::InvalidPayload))
        ));
        // Portal HTML arrives as a string body; decrypt failures as null.
        for garbage in [
            Value::String("<html>login to hotel wifi</html>".into()),
            Value::Null,
        ] {
            assert!(matches!(
                account_from_body(&garbage),
                Err(AccountError::Unreachable(AccountReason::InvalidPayload))
            ));
        }
        let account = account_from_body(
            &serde_json::json!({ "code": 200, "account": { "id": 7 }, "profile": { "nickname": "n" } }),
        )
        .unwrap();
        assert_eq!(account, (7, "n".to_owned()));
    }

    #[test]
    fn invalid_account_response_is_an_error_instead_of_uid_zero() {
        assert!(account_from_body(&serde_json::json!({
            "account": {},
            "profile": { "nickname": "unknown" }
        }))
        .is_err());
    }

    #[test]
    fn only_an_auth_rejection_counts_as_an_expired_session() {
        assert!(matches!(
            classify_account_error(NcmError::AuthRequired("需要登录".into())),
            AccountError::Expired(_)
        ));
        for unproven in [
            NcmError::Timeout("connect".into()),
            NcmError::RateLimited("503".into()),
            NcmError::Api {
                code: 502,
                msg: "bad gateway".into(),
            },
            NcmError::Unknown("connection reset".into()),
        ] {
            assert!(matches!(
                classify_account_error(unproven),
                AccountError::Unreachable(_)
            ));
        }
    }

    #[test]
    fn qr_status_codes_map_to_the_login_state_machine() {
        assert_eq!(
            parse_qr_status(&serde_json::json!({ "code": 801 }), &[]).unwrap(),
            QrStatus::Waiting
        );
        assert_eq!(
            parse_qr_status(&serde_json::json!({ "code": 802 }), &[]).unwrap(),
            QrStatus::Scanned
        );
        assert_eq!(
            parse_qr_status(&serde_json::json!({ "code": 800 }), &[]).unwrap(),
            QrStatus::Expired
        );
        assert!(matches!(
            parse_qr_status(&serde_json::json!({ "code": 803 }), &[]),
            Err(NcmClientError::LoginCookieMissing)
        ));
        assert!(matches!(
            parse_qr_status(&serde_json::json!({ "code": 418 }), &[]),
            Err(NcmClientError::UnknownQrStatus(418))
        ));

        let cookies = [
            "MUSIC_U=candidate-token; Path=/; HttpOnly".to_owned(),
            "__csrf=candidate-csrf; Path=/".to_owned(),
        ];
        assert!(matches!(
            parse_qr_status(&serde_json::json!({ "code": 803 }), &cookies),
            Ok(QrStatus::Success(_))
        ));
    }
}
