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

#[tokio::test]
async fn current_like_failure_rolls_back_only_the_requested_song() {
    let directory = tempfile::tempdir().unwrap();
    let fx = effects(&directory);
    let mut state = AppState::new(&Config::default());
    let attempt = state.session.begin_login();
    let session = state
        .session
        .accept_login(attempt, 42, "listener".into())
        .unwrap();
    state.liked = HashSet::from([7, 9]);
    let like_mutation = state.begin_like_mutation(7, true);
    state.begin_like_request_for_test(7, like_mutation);

    state.update(
        Action::LikeFinished {
            session,
            id: 7,
            mutation: like_mutation,
            attempted_like: true,
            error: Some("like failed".into()),
        },
        &fx,
    );

    assert_eq!(state.liked, HashSet::from([9]));
    assert_eq!(state.status.as_deref(), Some("like failed"));

    let unlike_mutation = state.begin_like_mutation(7, false);
    state.begin_like_request_for_test(7, unlike_mutation);
    state.update(
        Action::LikeFinished {
            session,
            id: 7,
            mutation: unlike_mutation,
            attempted_like: false,
            error: Some("unlike failed".into()),
        },
        &fx,
    );

    assert_eq!(state.liked, HashSet::from([7, 9]));
    assert_eq!(state.status.as_deref(), Some("unlike failed"));
}

#[tokio::test]
async fn like_failure_cannot_undo_a_newer_choice_or_another_session() {
    let directory = tempfile::tempdir().unwrap();
    let fx = effects(&directory);
    let mut state = AppState::new(&Config::default());
    let first_attempt = state.session.begin_login();
    let old_session = state
        .session
        .accept_login(first_attempt, 11, "first".into())
        .unwrap();
    let current_attempt = state.session.begin_login();
    let current_session = state
        .session
        .accept_login(current_attempt, 22, "second".into())
        .unwrap();
    state.liked.insert(7);
    state.status = Some("current state".into());
    let old_mutation = state.begin_like_mutation(7, true);
    state.begin_like_request_for_test(7, old_mutation);

    state.update(
        Action::LikeFinished {
            session: old_session,
            id: 7,
            mutation: old_mutation,
            attempted_like: true,
            error: Some("old account failure".into()),
        },
        &fx,
    );
    assert!(state.liked.contains(&7));

    let superseding_mutation = state.begin_like_mutation(7, false);
    state.update(
        Action::LikeFinished {
            session: current_session,
            id: 7,
            mutation: old_mutation,
            attempted_like: true,
            error: Some("superseded failure".into()),
        },
        &fx,
    );

    assert!(!state.liked.contains(&7));
    assert_eq!(state.status.as_deref(), Some("current state"));
    assert_ne!(superseding_mutation, old_mutation);
}

#[tokio::test]
async fn only_the_latest_of_three_interleaved_like_mutations_can_roll_back() {
    let directory = tempfile::tempdir().unwrap();
    let fx = effects(&directory);
    let mut state = AppState::new(&Config::default());
    let attempt = state.session.begin_login();
    let session = state
        .session
        .accept_login(attempt, 42, "listener".into())
        .unwrap();
    state.status = Some("latest choice".into());

    let first_like = state.begin_like_mutation(7, true);
    let unlike = state.begin_like_mutation(7, false);
    let latest_like = state.begin_like_mutation(7, true);
    state.begin_like_request_for_test(7, first_like);
    assert!(state.liked.contains(&7));

    state.update(
        Action::LikeFinished {
            session,
            id: 7,
            mutation: unlike,
            attempted_like: false,
            error: Some("out-of-order failure".into()),
        },
        &fx,
    );
    assert_eq!(state.like_in_flight.get(&7), Some(&first_like));

    fx.ncm.commit_session(&candidate("listener")).unwrap();
    state.update(
        Action::LikeFinished {
            session,
            id: 7,
            mutation: first_like,
            attempted_like: true,
            error: None,
        },
        &fx,
    );

    assert!(state.liked.contains(&7));
    assert_eq!(state.status.as_deref(), Some("latest choice"));
    assert_eq!(state.like_in_flight.get(&7), Some(&latest_like));

    state.update(
        Action::LikeFinished {
            session,
            id: 7,
            mutation: latest_like,
            attempted_like: true,
            error: Some("latest failure".into()),
        },
        &fx,
    );

    assert!(!state.liked.contains(&7));
    assert_eq!(state.status.as_deref(), Some("latest failure"));
    assert!(!state.like_in_flight.contains_key(&7));
}

#[tokio::test]
async fn late_liked_snapshot_preserves_locally_mutated_songs() {
    let directory = tempfile::tempdir().unwrap();
    let fx = effects(&directory);
    let mut state = AppState::new(&Config::default());
    let attempt = state.session.begin_login();
    let session = state
        .session
        .accept_login(attempt, 42, "listener".into())
        .unwrap();
    state.begin_like_mutation(7, true);

    state.update(
        Action::LikedIds {
            session,
            ids: HashSet::from([9]),
        },
        &fx,
    );

    assert_eq!(state.liked, HashSet::from([7, 9]));
}

#[tokio::test]
async fn a_new_session_does_not_inherit_like_mutations() {
    let directory = tempfile::tempdir().unwrap();
    let fx = effects(&directory);
    let mut state = AppState::new(&Config::default());
    let first_attempt = state.session.begin_login();
    state
        .session
        .accept_login(first_attempt, 11, "first".into())
        .unwrap();
    state.begin_like_mutation(7, true);

    let second_attempt = state.session.begin_login();
    let second = state
        .session
        .accept_login(second_attempt, 22, "second".into())
        .unwrap();
    state.finish_account(&fx, second, "second".into(), candidate("second"));
    state.update(
        Action::LikedIds {
            session: second,
            ids: HashSet::new(),
        },
        &fx,
    );

    assert!(!state.liked.contains(&7));
    assert!(state.like_mutations.is_empty());
}

#[tokio::test]
async fn empty_or_failed_fm_page_clears_pending_request_and_allows_retry() {
    let directory = tempfile::tempdir().unwrap();
    let fx = effects(&directory);
    let mut state = AppState::new(&Config::default());
    let attempt = state.session.begin_login();
    let session = state
        .session
        .accept_login(attempt, 42, "listener".into())
        .unwrap();
    state.queue_source = Source::Fm;
    state.queue = vec![row(1)];
    state.queue_pos = Some(0);
    state.pending_fm_next = true;
    state.fm_request_pending = true;

    state.update(
        Action::FmMore {
            session,
            rows: Vec::new(),
        },
        &fx,
    );

    assert!(!state.pending_fm_next);
    assert!(!state.fm_request_pending);
    assert_eq!(state.queue.len(), 1);

    state.pending_fm_next = true;
    state.fm_request_pending = true;
    state.update(
        Action::FmLoadFailed {
            session,
            message: "fm failed".into(),
        },
        &fx,
    );

    assert!(!state.pending_fm_next);
    assert!(!state.fm_request_pending);
    assert_eq!(state.status.as_deref(), Some("fm failed"));
}

#[tokio::test]
async fn repeated_fm_next_while_loading_keeps_one_request_pending() {
    let directory = tempfile::tempdir().unwrap();
    let fx = effects(&directory);
    let mut state = AppState::new(&Config::default());
    let attempt = state.session.begin_login();
    state
        .session
        .accept_login(attempt, 42, "listener".into())
        .unwrap();
    fx.ncm.commit_session(&candidate("listener")).unwrap();
    state.queue_source = Source::Fm;
    state.queue = vec![row(1)];
    state.queue_pos = Some(0);

    state.update(Action::NextTrack, &fx);
    state.update(Action::NextTrack, &fx);

    assert!(state.pending_fm_next);
    assert!(state.fm_request_pending);
}

#[tokio::test]
async fn browsing_another_library_does_not_cancel_pending_fm_playback() {
    let directory = tempfile::tempdir().unwrap();
    let fx = effects(&directory);
    let mut state = AppState::new(&Config::default());
    let attempt = state.session.begin_login();
    let session = state
        .session
        .accept_login(attempt, 42, "listener".into())
        .unwrap();
    state.queue_source = Source::Fm;
    state.queue = vec![row(1)];
    state.queue_pos = Some(0);
    state.pending_fm_next = true;
    state.fm_request_pending = true;

    state.open_source(&fx, 0);
    state.update(
        Action::FmMore {
            session,
            rows: vec![row(2)],
        },
        &fx,
    );

    assert_eq!(state.queue_pos, Some(1));
    assert_eq!(state.queue[1].id, 2);
    assert_eq!(
        state.now.as_ref().map(|now| now.title.as_str()),
        Some("Track 2")
    );
    assert!(!state.pending_fm_next);
    assert!(!state.fm_request_pending);
}

#[tokio::test]
async fn late_fm_page_cannot_advance_a_replaced_queue() {
    let directory = tempfile::tempdir().unwrap();
    let fx = effects(&directory);
    let mut state = AppState::new(&Config::default());
    let attempt = state.session.begin_login();
    let session = state
        .session
        .accept_login(attempt, 42, "listener".into())
        .unwrap();
    state.queue_source = Source::Fm;
    state.queue = vec![row(1)];
    state.queue_pos = Some(0);
    state.pending_fm_next = true;
    state.fm_request_pending = true;
    state.view = View::Search;
    state.search.input = false;
    state.search.results = vec![row(9)];

    state.update(Action::Activate, &fx);
    let generation = state.generation;
    state.update(
        Action::FmMore {
            session,
            rows: vec![row(2)],
        },
        &fx,
    );

    assert_eq!(state.queue_pos, Some(0));
    assert_eq!(state.queue[0].id, 9);
    assert_eq!(state.generation, generation);
    assert_eq!(
        state.now.as_ref().map(|now| now.title.as_str()),
        Some("Track 9")
    );
    assert!(!state.pending_fm_next);
    assert!(!state.fm_request_pending);
}
