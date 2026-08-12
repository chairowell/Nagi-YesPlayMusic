//! Persistent NCM login session for clients without a browser cookie jar.
//! The WebView keeps its own cookies; the ypm TUI stores the two auth
//! cookies here and injects them into every `ncm-api-rs` Query.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    pub music_u: String,
    #[serde(default)]
    pub csrf: String,
}

impl Session {
    /// Collect the auth cookies out of NCM `Set-Cookie` values
    /// (e.g. the `login/qr/check` response). Returns None until the
    /// login cookie is actually present.
    pub fn from_set_cookies<S: AsRef<str>>(cookies: impl IntoIterator<Item = S>) -> Option<Self> {
        let mut music_u = None;
        let mut csrf = None;
        for cookie in cookies {
            let Some((name, value)) = cookie
                .as_ref()
                .split(';')
                .next()
                .and_then(|pair| pair.split_once('='))
            else {
                continue;
            };
            match name.trim() {
                "MUSIC_U" if !value.is_empty() => music_u = Some(value.to_owned()),
                "__csrf" if !value.is_empty() => csrf = Some(value.to_owned()),
                _ => {}
            }
        }
        Some(Session {
            music_u: music_u?,
            csrf: csrf.unwrap_or_default(),
        })
    }

    /// The `Cookie` header / `Query.cookie` value for API requests.
    pub fn cookie_header(&self) -> String {
        if self.csrf.is_empty() {
            format!("MUSIC_U={}", self.music_u)
        } else {
            format!("MUSIC_U={}; __csrf={}", self.music_u, self.csrf)
        }
    }
}

pub struct SessionStore {
    path: PathBuf,
}

impl SessionStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// A corrupt or missing file is simply "not logged in".
    pub fn load(&self) -> Option<Session> {
        let bytes = fs::read(&self.path).ok()?;
        serde_json::from_slice(&bytes).ok()
    }

    /// Atomic write (tmp + rename); 0600 on Unix before any secret lands on disk.
    pub fn save(&self, session: &Session) -> io::Result<()> {
        let parent = self
            .path
            .parent()
            .ok_or_else(|| io::Error::other("session path has no parent directory"))?;
        fs::create_dir_all(parent)?;
        let tmp = self.path.with_extension("tmp");
        {
            let mut options = fs::OpenOptions::new();
            options.write(true).create(true).truncate(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options.open(&tmp)?;
            use std::io::Write;
            file.write_all(&serde_json::to_vec_pretty(session)?)?;
            file.sync_all()?;
        }
        fs::rename(&tmp, &self.path)
    }

    pub fn clear(&self) -> io::Result<()> {
        match fs::remove_file(&self.path) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (tempfile::TempDir, SessionStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path().join("ypm/session.json"));
        (dir, store)
    }

    #[test]
    fn roundtrips_and_clears_atomically() {
        let (_dir, store) = store();
        assert!(store.load().is_none());

        let session = Session {
            music_u: "u-token".into(),
            csrf: "c-token".into(),
        };
        store.save(&session).unwrap();
        assert_eq!(store.load(), Some(session.clone()));
        assert_eq!(session.cookie_header(), "MUSIC_U=u-token; __csrf=c-token");

        store.clear().unwrap();
        store.clear().unwrap();
        assert!(store.load().is_none());
    }

    #[cfg(unix)]
    #[test]
    fn session_file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let (_dir, store) = store();
        store
            .save(&Session {
                music_u: "secret".into(),
                csrf: String::new(),
            })
            .unwrap();
        let mode = std::fs::metadata(store.path()).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[test]
    fn parses_ncm_set_cookies_and_tolerates_noise() {
        let session = Session::from_set_cookies([
            "NMTID=noise; Path=/",
            "MUSIC_U=login-token; Path=/; HttpOnly",
            "__csrf=csrf-token; Path=/",
            "malformed-cookie",
        ])
        .unwrap();
        assert_eq!(session.music_u, "login-token");
        assert_eq!(session.csrf, "csrf-token");

        assert!(Session::from_set_cookies(["NMTID=only-noise"]).is_none());
        let no_csrf = Session::from_set_cookies(["MUSIC_U=token"]).unwrap();
        assert_eq!(no_csrf.cookie_header(), "MUSIC_U=token");
    }

    #[test]
    fn corrupt_session_file_reads_as_logged_out() {
        let (_dir, store) = store();
        std::fs::create_dir_all(store.path().parent().unwrap()).unwrap();
        std::fs::write(store.path(), b"{not json").unwrap();
        assert!(store.load().is_none());
    }
}
