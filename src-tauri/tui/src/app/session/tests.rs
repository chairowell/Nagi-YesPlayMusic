use std::collections::HashSet;
use std::sync::Arc;

use tempfile::TempDir;
use tokio::sync::mpsc;
use yesplaymusic_core::auth::{Session, SessionStore};

use super::*;
use crate::config::Config;
use crate::player;

fn effects(directory: &TempDir) -> Effects {
    let (player, _events) = player::spawn(tokio::runtime::Handle::current());
    let (actions, _receiver) = mpsc::unbounded_channel();
    Effects {
        player,
        ncm: Arc::new(Ncm::new(
            directory.path().join("session.json"),
            yesplaymusic_core::cache::AudioQuality::High320,
        )),
        store: Arc::new(crate::store::LibraryStore::new(
            directory.path().join("library"),
        )),
        actions,
        cache_root: None,
    }
}

fn candidate(name: &str) -> Session {
    Session {
        music_u: format!("{name}-music-u"),
        csrf: format!("{name}-csrf"),
    }
}

fn row(id: i64) -> SongRow {
    SongRow {
        id,
        title: format!("Track {id}"),
        artist: "Artist".into(),
        duration_ms: 180_000,
        pic_url: None,
    }
}

#[tokio::test]
async fn only_the_current_login_attempt_can_commit_its_candidate() {
    let directory = tempfile::tempdir().unwrap();
    let session_path = directory.path().join("session.json");
    let fx = effects(&directory);
    let mut state = AppState::new(&Config::default());
    state.view = View::Login;
    let stale_attempt = state.session.begin_login();
    let current_attempt = state.session.begin_login();

    state.update(
        Action::LoginQrReady {
            attempt: stale_attempt,
            art: "stale QR".into(),
        },
        &fx,
    );
    assert!(state.session.login_qr.is_none());
    state.update(
        Action::LoginQrReady {
            attempt: current_attempt,
            art: "current QR".into(),
        },
        &fx,
    );
    assert_eq!(state.session.login_qr.as_deref(), Some("current QR"));

    state.update(
        Action::LoginSucceeded {
            attempt: stale_attempt,
            session: candidate("stale"),
            uid: 11,
            nickname: "stale".into(),
        },
        &fx,
    );

    assert!(fx.ncm.session_snapshot().is_none());
    assert!(SessionStore::new(&session_path).load().is_none());
    assert!(state.session.nickname.is_none());

    let current = candidate("current");
    state.update(
        Action::LoginSucceeded {
            attempt: current_attempt,
            session: current.clone(),
            uid: 22,
            nickname: "current".into(),
        },
        &fx,
    );

    assert_eq!(fx.ncm.session_snapshot(), Some(current.clone()));
    assert_eq!(SessionStore::new(session_path).load(), Some(current));
    assert_eq!(state.session.nickname.as_deref(), Some("current"));
    assert_eq!(
        state.session.current_stamp(),
        Some(SessionStamp { epoch: 1, uid: 22 })
    );
}

#[tokio::test]
async fn a_login_attempt_supersedes_an_in_flight_session_restore() {
    let directory = tempfile::tempdir().unwrap();
    let fx = effects(&directory);
    let mut state = AppState::new(&Config::default());
    let restore_epoch = state.session.begin_restore();
    state.session.begin_login();

    state.update(
        Action::SessionRestored {
            epoch: restore_epoch,
            uid: 11,
            nickname: "restored account".into(),
        },
        &fx,
    );

    assert!(state.session.nickname.is_none());
    assert!(state.session.current_stamp().is_none());
}

#[tokio::test]
async fn personal_results_from_the_previous_account_are_ignored_and_not_saved_for_the_next() {
    let directory = tempfile::tempdir().unwrap();
    let fx = effects(&directory);
    let mut state = AppState::new(&Config::default());
    let first_attempt = state.session.begin_login();
    let first = state
        .session
        .accept_login(first_attempt, 11, "first".into())
        .unwrap();
    let second_attempt = state.session.begin_login();
    assert!(state.session.current_stamp().is_none());
    state.update(
        Action::LibraryLoaded {
            session: first,
            source: Source::Liked,
            rows: vec![row(11)],
        },
        &fx,
    );
    assert_ne!(state.library.first().map(|row| row.id), Some(11));
    let second = state
        .session
        .accept_login(second_attempt, 22, "second".into())
        .unwrap();
    state.library = vec![row(99)];
    state.liked = HashSet::from([99]);

    state.update(
        Action::LibraryLoaded {
            session: first,
            source: Source::Liked,
            rows: vec![row(11)],
        },
        &fx,
    );
    state.update(
        Action::LikedIds {
            session: first,
            ids: HashSet::from([11]),
        },
        &fx,
    );

    assert_eq!(state.library[0].id, 99);
    assert_eq!(state.liked, HashSet::from([99]));
    assert!(fx.store.load(second.uid, "liked").is_none());

    state.update(
        Action::LibraryLoaded {
            session: second,
            source: Source::Liked,
            rows: vec![row(22)],
        },
        &fx,
    );
    state.update(
        Action::LikedIds {
            session: second,
            ids: HashSet::from([22]),
        },
        &fx,
    );

    assert_eq!(state.library[0].id, 22);
    assert_eq!(state.liked, HashSet::from([22]));
    for _ in 0..50 {
        if fx.store.load(second.uid, "liked").is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(
        fx.store.load(second.uid, "liked").unwrap()[0].id,
        second.uid
    );
}
