use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use std::io::Write;
use tempfile::TempDir;
use yesplaymusic_core::cache::{AudioCodec, AudioQuality, CacheKey};

use super::*;

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
        covers: None,
        config_path: directory.path().join("config.toml"),
    }
}

fn raw_key(code: KeyCode) -> Action {
    Action::RawKey(KeyEvent::new(code, KeyModifiers::NONE))
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

fn named_row(id: i64, title: &str, artist: &str) -> SongRow {
    SongRow {
        id,
        title: title.into(),
        artist: artist.into(),
        duration_ms: 180_000,
        pic_url: None,
    }
}

fn covered_row(id: i64) -> SongRow {
    SongRow {
        pic_url: Some(format!("https://example.test/{id}.jpg")),
        ..row(id)
    }
}

#[tokio::test]
async fn paused_ui_ticks_advance_the_marquee_without_consuming_the_gg_prefix() {
    let directory = tempfile::tempdir().unwrap();
    let fx = effects(&directory);
    let mut state = AppState::new(&Config::default());
    state.view = View::Queue;
    state.paused = true;
    state.queue = vec![named_row(1, "A title long enough to scroll", "Artist")];

    state.update(Action::GKey, &fx);
    state.update(Action::UiTick, &fx);
    assert_eq!(state.marquee_frame, 0);
    assert!(state.pending_g);

    state.update(Action::UiTick, &fx);
    assert_eq!(state.marquee_frame, 1);
    assert!(state.pending_g);
}

#[tokio::test]
async fn returning_to_a_long_row_restarts_the_marquee_pause() {
    let directory = tempfile::tempdir().unwrap();
    let fx = effects(&directory);
    let hits = ui::Hits::default();
    let mut state = AppState::new(&Config::default());
    state.view = View::Queue;
    state.queue = vec![
        named_row(1, "A title long enough to scroll", "Artist"),
        named_row(2, "Short", "Artist"),
    ];

    apply(&mut state, Action::UiTick, &fx, &hits);
    apply(&mut state, Action::UiTick, &fx, &hits);
    assert_eq!(state.marquee_frame, 1);

    apply(&mut state, Action::MoveSelection(1), &fx, &hits);
    assert!(state.marquee_target.is_none());
    apply(&mut state, Action::MoveSelection(-1), &fx, &hits);
    assert_eq!(state.marquee_frame, 0);
    assert_eq!(
        state.marquee_target.as_ref().map(|target| target.id),
        Some(1)
    );
}

#[tokio::test]
async fn selected_cover_waits_for_debounce_and_schedules_three_neighbors_each_side() {
    let directory = tempfile::tempdir().unwrap();
    let (player, _events) = player::spawn(tokio::runtime::Handle::current());
    let (actions, mut receiver) = mpsc::unbounded_channel();
    let fx = Effects {
        player,
        ncm: Arc::new(Ncm::new(
            directory.path().join("session.json"),
            AudioQuality::High320,
        )),
        store: Arc::new(crate::store::LibraryStore::new(
            directory.path().join("library"),
        )),
        actions,
        cache_root: None,
        covers: None,
        config_path: directory.path().join("config.toml"),
    };
    let mut state = AppState::new(&Config::default());
    state.view = View::Library;
    state.terminal_size = (96, 15);
    state.library = (0..9).map(covered_row).collect();
    state.selected = 4;

    state.reconcile_selected_cover(&fx);
    assert!(
        tokio::time::timeout(Duration::from_millis(100), receiver.recv())
            .await
            .is_err()
    );

    let due = tokio::time::timeout(Duration::from_millis(250), receiver.recv())
        .await
        .expect("debounced cover request")
        .expect("cover action channel");
    let Action::SelectionCoverDue {
        generation,
        row,
        neighbors,
    } = due
    else {
        panic!("unexpected action");
    };
    assert_eq!(generation, state.selected_cover.generation);
    assert_eq!(row.id, 4);
    assert_eq!(
        neighbors,
        [1, 2, 3, 5, 6, 7].map(|id| format!("https://example.test/{id}.jpg"))
    );
}

#[tokio::test]
async fn a_selection_without_art_still_prefetches_neighbor_originals() {
    let directory = tempfile::tempdir().unwrap();
    let (player, _events) = player::spawn(tokio::runtime::Handle::current());
    let (actions, mut receiver) = mpsc::unbounded_channel();
    let fx = Effects {
        player,
        ncm: Arc::new(Ncm::new(
            directory.path().join("session.json"),
            AudioQuality::High320,
        )),
        store: Arc::new(crate::store::LibraryStore::new(
            directory.path().join("library"),
        )),
        actions,
        cache_root: None,
        covers: None,
        config_path: directory.path().join("config.toml"),
    };
    let mut state = AppState::new(&Config::default());
    state.view = View::Library;
    state.terminal_size = (96, 15);
    state.library = vec![covered_row(1), row(2), covered_row(3)];
    state.selected = 1;

    state.reconcile_selected_cover(&fx);
    let due = tokio::time::timeout(Duration::from_millis(250), receiver.recv())
        .await
        .expect("neighbor prefetch request")
        .expect("cover action channel");
    let Action::SelectionCoverDue { row, neighbors, .. } = due else {
        panic!("unexpected action");
    };
    assert_eq!(row.id, 2);
    assert_eq!(
        neighbors,
        [1, 3].map(|id| format!("https://example.test/{id}.jpg"))
    );
}

#[tokio::test]
async fn mute_restores_the_volume_that_was_active_before_muting() {
    let directory = tempfile::tempdir().unwrap();
    let fx = effects(&directory);
    let mut state = AppState::new(&Config::default());
    state.volume = 0.65;

    state.update(Action::ToggleMute, &fx);
    assert_eq!(state.volume, 0.0);
    assert_eq!(state.volume_before_mute, Some(0.65));

    state.update(Action::ToggleMute, &fx);
    assert_eq!(state.volume, 0.65);
    assert_eq!(state.volume_before_mute, None);
}

#[tokio::test]
async fn shuffle_and_repeat_change_independently() {
    let directory = tempfile::tempdir().unwrap();
    let fx = effects(&directory);
    let mut state = AppState::new(&Config::default());

    state.update(Action::ToggleShuffle, &fx);
    assert!(state.shuffle);
    assert_eq!(state.play_mode, PlayMode::Off);

    state.update(Action::CycleRepeat, &fx);
    assert!(state.shuffle);
    assert_eq!(state.play_mode, PlayMode::List);
    state.update(Action::CycleRepeat, &fx);
    assert_eq!(state.play_mode, PlayMode::One);
    state.update(Action::CycleRepeat, &fx);
    assert_eq!(state.play_mode, PlayMode::Off);
}

#[tokio::test]
async fn list_repeat_wraps_while_single_repeat_replays_only_on_track_end() {
    let directory = tempfile::tempdir().unwrap();
    let mut fx = effects(&directory);
    fx.cache_root = Some(directory.path().join("audio"));
    let mut state = AppState::new(&Config::default());
    state.queue = vec![row(1), row(2)];
    state.queue_pos = Some(1);
    state.play_mode = PlayMode::List;

    state.update(Action::NextTrack, &fx);
    assert_eq!(state.queue_pos, Some(0));

    state.queue_pos = Some(1);
    state.play_mode = PlayMode::One;
    let generation = state.generation;
    state.update(Action::Player(PlayerEvent::Ended { generation }), &fx);
    assert_eq!(state.queue_pos, Some(1));
    assert_eq!(
        state.now.as_ref().map(|now| now.title.as_str()),
        Some("Track 2")
    );
}

#[tokio::test]
async fn local_filter_maps_visible_rows_to_the_correct_song_and_escape_restores_the_list() {
    let directory = tempfile::tempdir().unwrap();
    let mut fx = effects(&directory);
    fx.cache_root = Some(directory.path().join("audio"));
    let mut state = AppState::new(&Config::default());
    state.view = View::Library;
    state.library = vec![
        named_row(1, "Alpha", "One"),
        named_row(2, "Beta", "Two"),
        named_row(3, "Gamma", "Three"),
    ];

    state.update(Action::StartFilter, &fx);
    state.update(raw_key(KeyCode::Char('g')), &fx);
    state.update(raw_key(KeyCode::Char('m')), &fx);
    assert_eq!(state.visible_len(), 1);
    state.update(raw_key(KeyCode::Enter), &fx);
    state.update(Action::Activate, &fx);

    assert_eq!(
        state.now.as_ref().map(|now| now.title.as_str()),
        Some("Gamma")
    );
    assert_eq!(
        state.queue.iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![3]
    );

    state.view = View::Library;
    state.update(Action::StartFilter, &fx);
    state.update(raw_key(KeyCode::Char('z')), &fx);
    assert_eq!(state.visible_len(), 0);
    state.update(raw_key(KeyCode::Esc), &fx);
    assert_eq!(state.visible_len(), 3);
    assert!(state.filter.query.is_empty());
    assert_eq!(state.view, View::Library);
}

#[tokio::test]
async fn starting_a_library_filter_moves_focus_from_the_sidebar_to_results() {
    let directory = tempfile::tempdir().unwrap();
    let mut fx = effects(&directory);
    fx.cache_root = Some(directory.path().join("audio"));
    let mut state = AppState::new(&Config::default());
    state.view = View::Library;
    state.sidebar_focus = true;
    state.library = vec![named_row(1, "Alpha", "One"), named_row(2, "Beta", "Two")];

    state.update(Action::StartFilter, &fx);
    state.update(raw_key(KeyCode::Char('b')), &fx);
    state.update(raw_key(KeyCode::Enter), &fx);

    assert!(!state.sidebar_focus);
    assert_eq!(state.visible_len(), 1);
    state.update(raw_key(KeyCode::Enter), &fx);
    assert_eq!(state.current_track_id, Some(2));
}

#[tokio::test]
async fn adding_a_filtered_selection_appends_without_starting_playback() {
    let directory = tempfile::tempdir().unwrap();
    let fx = effects(&directory);
    let mut state = AppState::new(&Config::default());
    state.view = View::Library;
    state.library = vec![named_row(1, "Alpha", "One"), named_row(2, "Beta", "Two")];
    state.filter.query = "bt".into();

    state.update(Action::AddSelectedToQueue, &fx);

    assert_eq!(
        state.queue.iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![2]
    );
    assert_eq!(state.generation, 0);
    assert!(state.now.is_none());
    assert_eq!(state.view, View::Library);
}

#[tokio::test]
async fn filtered_mouse_rows_keep_their_visible_to_underlying_mapping() {
    let directory = tempfile::tempdir().unwrap();
    let mut fx = effects(&directory);
    fx.cache_root = Some(directory.path().join("audio"));
    let mut state = AppState::new(&Config::default());
    state.view = View::Library;
    state.library = vec![
        named_row(1, "Alpha", "One"),
        named_row(2, "Gamma", "Two"),
        named_row(3, "Gamut", "Three"),
    ];
    state.filter.query = "ga".into();
    let mut hits = ui::Hits::default();
    hits.rows.push((Rect::new(4, 5, 20, 1), 1));
    let click = || {
        Action::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 5,
            row: 5,
            modifiers: KeyModifiers::NONE,
        })
    };

    apply(&mut state, click(), &fx, &hits);
    assert_eq!(state.selected, 1);
    assert!(state.now.is_none());

    apply(&mut state, click(), &fx, &hits);
    assert_eq!(state.current_track_id, Some(3));
}

#[tokio::test]
async fn page_home_end_and_tab_follow_the_visible_library() {
    let directory = tempfile::tempdir().unwrap();
    let fx = effects(&directory);
    let mut state = AppState::new(&Config::default());
    state.view = View::Library;
    state.library = (0..30).map(row).collect();
    state.update(Action::Resize { cols: 80, rows: 10 }, &fx);

    state.update(Action::MovePage(1), &fx);
    assert_eq!(state.selected, 7);
    state.update(Action::JumpBottom, &fx);
    assert_eq!(state.selected, 29);
    state.update(Action::JumpTop, &fx);
    assert_eq!(state.selected, 0);

    state.update(Action::ToggleLibraryFocus, &fx);
    assert!(state.sidebar_focus);
    state.update(Action::ToggleLibraryFocus, &fx);
    assert!(!state.sidebar_focus);
    state.update(Action::Resize { cols: 40, rows: 10 }, &fx);
    state.update(Action::ToggleLibraryFocus, &fx);
    assert!(!state.sidebar_focus);
}

#[tokio::test]
async fn restored_playback_stays_paused_until_space_then_seeks_after_start() {
    let directory = tempfile::tempdir().unwrap();
    let mut fx = effects(&directory);
    fx.cache_root = Some(directory.path().join("audio"));
    let mut state = AppState::new(&Config::default());
    state.restore_playback(crate::store::StoredPlayback {
        queue: vec![crate::store::StoredSong::from(&row(7))],
        current: Some(crate::store::StoredSong::from(&row(7))),
        queue_pos: Some(0),
        position_ms: 42_000,
        volume: 0.0,
        volume_before_mute: Some(0.8),
        play_mode: PlayMode::List,
        shuffle: true,
        queue_source: Source::Fm,
    });

    assert!(state.paused);
    assert_eq!(state.generation, 0);
    assert_eq!(state.position, Duration::from_secs(42));
    assert_eq!(state.current_track_id, Some(7));
    assert!(state.status.is_none());

    state.update(Action::TogglePlay, &fx);
    assert_eq!(state.generation, 1);
    assert_eq!(state.position, Duration::from_secs(42));
    assert_eq!(state.status.as_deref(), Some(i18n::t(Key::Resolving)));
    assert_eq!(state.queue_source, Source::Fm);

    state.update(
        Action::Player(PlayerEvent::Started {
            generation: 1,
            total: Some(Duration::from_secs(180)),
        }),
        &fx,
    );
    assert_eq!(state.position, Duration::from_secs(42));
    assert!(state.seek_after_start.is_none());
    assert!(state.resume_on_play.is_none());
}

#[tokio::test]
async fn a_failed_restored_track_can_be_retried_with_space() {
    let directory = tempfile::tempdir().unwrap();
    let mut fx = effects(&directory);
    fx.cache_root = Some(directory.path().join("audio"));
    let mut state = AppState::new(&Config::default());
    state.restore_playback(crate::store::StoredPlayback {
        queue: vec![crate::store::StoredSong::from(&row(7))],
        current: Some(crate::store::StoredSong::from(&row(7))),
        queue_pos: Some(0),
        position_ms: 42_000,
        volume: 0.7,
        volume_before_mute: None,
        play_mode: PlayMode::Off,
        shuffle: false,
        queue_source: Source::Liked,
    });

    state.update(Action::TogglePlay, &fx);
    state.update(
        Action::ResolveFailed {
            generation: 1,
            message: "offline".into(),
        },
        &fx,
    );
    assert_eq!(state.resume_on_play, Some(Duration::from_secs(42)));

    state.update(Action::TogglePlay, &fx);
    assert_eq!(state.generation, 2);
    assert_eq!(state.position, Duration::from_secs(42));
}

#[tokio::test]
async fn quit_dialog_handles_raw_confirm_and_cancel_keys() {
    let directory = tempfile::tempdir().unwrap();
    let fx = effects(&directory);

    for key in [KeyCode::Char('y'), KeyCode::Enter, KeyCode::Char('q')] {
        let mut state = AppState::new(&Config::default());
        state.update(Action::Quit, &fx);
        state.update(raw_key(key), &fx);
        assert!(state.should_quit, "{key:?} should confirm quitting");
    }

    for key in [KeyCode::Char('n'), KeyCode::Esc] {
        let mut state = AppState::new(&Config::default());
        state.update(Action::Quit, &fx);
        state.update(raw_key(key), &fx);
        assert!(!state.confirm_quit, "{key:?} should cancel quitting");
        assert!(!state.should_quit);
    }

    let mut state = AppState::new(&Config::default());
    state.update(Action::Quit, &fx);
    state.update(
        Action::RawKey(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
        &fx,
    );
    assert!(state.should_quit);
}

#[tokio::test]
async fn settings_preview_can_be_cancelled_without_touching_disk() {
    let directory = tempfile::tempdir().unwrap();
    let fx = effects(&directory);
    let mut state = AppState::new(&Config::default());
    let original_theme = state.theme;

    state.update(raw_key(KeyCode::Char(',')), &fx);
    assert_eq!(state.view, View::Settings);
    state.update(raw_key(KeyCode::Right), &fx);

    assert_eq!(state.config.theme, "pico8");
    assert_ne!(state.theme, original_theme);
    assert!(!fx.config_path.exists());

    state.update(raw_key(KeyCode::Esc), &fx);

    assert_eq!(state.view, View::NowPlaying);
    assert_eq!(state.config.theme, "db16");
    assert_eq!(state.theme, original_theme);
    assert!(!fx.config_path.exists());
}

#[tokio::test]
async fn settings_save_persists_the_preview_and_updates_playback_quality() {
    let directory = tempfile::tempdir().unwrap();
    let fx = effects(&directory);
    let mut state = AppState::new(&Config::default());
    state.view = View::Library;

    state.update(Action::SwitchView(View::Settings), &fx);
    state.update(Action::AdjustSetting(1), &fx);
    state.update(Action::MoveSelection(1), &fx);
    state.update(Action::MoveSelection(1), &fx);
    state.update(Action::AdjustSetting(1), &fx);
    assert_eq!(state.config.quality, AudioQuality::Lossless);
    assert_eq!(fx.ncm.quality(), AudioQuality::Lossless);

    state.update(Action::SaveSettings, &fx);

    assert_eq!(state.view, View::Library);
    let reloaded: Config =
        toml::from_str(&std::fs::read_to_string(&fx.config_path).unwrap()).unwrap();
    assert_eq!(reloaded.theme, "pico8");
    assert_eq!(reloaded.quality, AudioQuality::Lossless);
}

#[tokio::test]
async fn quality_preview_rejects_a_prefetch_from_the_previous_setting() {
    let directory = tempfile::tempdir().unwrap();
    let fx = effects(&directory);
    let mut state = AppState::new(&Config::default());
    state.queue = vec![row(42)];
    state.update(Action::SwitchView(View::Settings), &fx);
    state.update(Action::MoveSelection(1), &fx);
    state.update(Action::MoveSelection(1), &fx);
    state.update(Action::AdjustSetting(1), &fx);

    state.update(
        Action::PrefetchReady {
            index: 0,
            track: api::ResolvedTrack {
                id: 42,
                title: "Track 42".into(),
                artist: "Artist".into(),
                url: "https://example.test/audio.mp3".into(),
                kind: "mp3".into(),
                cache_key: CacheKey::new(42, AudioQuality::High320),
                codec: AudioCodec::Mp3,
                actual_bitrate: 320_000,
                expected_bytes: None,
                expected_md5: None,
                duration_ms: 180_000,
                pic_url: None,
            },
        },
        &fx,
    );

    assert!(state.prefetched.is_none());
}

#[tokio::test]
async fn a_settings_save_failure_keeps_the_editor_and_preview_open() {
    let directory = tempfile::tempdir().unwrap();
    let mut fx = effects(&directory);
    fx.config_path = directory.path().to_path_buf();
    let mut state = AppState::new(&Config::default());
    state.update(Action::SwitchView(View::Settings), &fx);
    state.update(Action::AdjustSetting(1), &fx);

    state.update(Action::SaveSettings, &fx);

    assert_eq!(state.view, View::Settings);
    assert_eq!(state.config.theme, "pico8");
    assert!(state.status.is_some());
    assert!(directory.path().is_dir());
}

#[tokio::test]
async fn quit_dialog_keeps_processing_async_state_updates() {
    let directory = tempfile::tempdir().unwrap();
    let fx = effects(&directory);
    let mut state = AppState::new(&Config::default());
    state.update(Action::Quit, &fx);

    state.update(
        Action::LyricsLoaded {
            generation: 0,
            lines: vec![crate::lyrics::LyricLine {
                time: Duration::from_secs(1),
                text: "new lyric".into(),
                translation: None,
            }],
        },
        &fx,
    );

    assert!(state.confirm_quit);
    assert_eq!(state.lyrics[0].text, "new lyric");
}

#[tokio::test]
async fn track_end_advances_the_queue_while_quit_dialog_is_open() {
    let directory = tempfile::tempdir().unwrap();
    let fx = effects(&directory);
    let mut state = AppState::new(&Config::default());
    state.queue = vec![row(1), row(2)];
    state.queue_pos = Some(0);
    state.update(Action::Quit, &fx);

    state.update(Action::Player(PlayerEvent::Ended { generation: 0 }), &fx);

    assert!(state.confirm_quit);
    assert_eq!(state.queue_pos, Some(1));
    assert_eq!(
        state.now.as_ref().map(|now| now.title.as_str()),
        Some("Track 2")
    );
}

#[tokio::test]
async fn editing_search_rejects_results_and_failures_for_the_previous_query() {
    let directory = tempfile::tempdir().unwrap();
    let fx = effects(&directory);
    let edits = [
        raw_key(KeyCode::Char('x')),
        Action::Paste("x".into()),
        raw_key(KeyCode::Esc),
    ];

    for edit in edits {
        let mut state = AppState::new(&Config::default());
        state.view = View::Search;
        state.search.query = "old".into();
        let request = state.search.submit().unwrap();
        let stale_seq = request.seq;
        let stale_query = request.query.clone();

        state.update(edit, &fx);
        state.update(
            Action::SearchResults {
                seq: request.seq,
                query: request.query,
                rows: vec![row(1)],
            },
            &fx,
        );
        state.update(
            Action::SearchFailed {
                seq: stale_seq,
                query: stale_query,
                message: "old failure".into(),
            },
            &fx,
        );

        assert!(state.search.results.is_empty());
        assert!(state.search.error.is_none());
        assert!(!state.search.searching);
        assert!(state.search.input);
    }
}

#[tokio::test]
async fn search_row_click_selects_first_then_activates() {
    let directory = tempfile::tempdir().unwrap();
    let fx = effects(&directory);
    let mut state = AppState::new(&Config::default());
    state.view = View::Search;
    state.search.query = "query".into();
    let request = state.search.submit().unwrap();
    assert!(state
        .search
        .accept(request.seq, &request.query, vec![row(1), row(2)]));
    assert!(state.search.input);
    state.selected = 0;

    let mut hits = ui::Hits::default();
    hits.rows.push((Rect::new(4, 5, 20, 1), 0));
    let click = Action::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 5,
        row: 5,
        modifiers: KeyModifiers::NONE,
    });

    apply(&mut state, click, &fx, &hits);
    assert_eq!(state.view, View::Search);
    assert_eq!(state.selected, 0);
    assert!(!state.search.input);

    apply(
        &mut state,
        Action::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 5,
            row: 5,
            modifiers: KeyModifiers::NONE,
        }),
        &fx,
        &hits,
    );
    assert_eq!(state.view, View::NowPlaying);
}

#[tokio::test]
async fn selecting_a_different_search_row_focuses_the_result_list() {
    let directory = tempfile::tempdir().unwrap();
    let fx = effects(&directory);
    let mut state = AppState::new(&Config::default());
    state.view = View::Search;
    state.search.input = true;
    state.search.results = vec![row(1), row(2)];

    state.update(Action::SelectIndex(1), &fx);

    assert_eq!(state.selected, 1);
    assert!(!state.search.input);
}

#[tokio::test]
async fn jump_bottom_reaches_the_last_search_result() {
    let directory = tempfile::tempdir().unwrap();
    let fx = effects(&directory);
    let mut state = AppState::new(&Config::default());
    state.view = View::Search;
    state.search.input = false;
    state.search.results = vec![row(1), row(2), row(3)];

    state.update(Action::JumpBottom, &fx);

    assert_eq!(state.selected, 2);
}

#[tokio::test]
async fn selecting_a_library_row_moves_focus_from_the_sidebar_before_activation() {
    let directory = tempfile::tempdir().unwrap();
    let fx = effects(&directory);
    let mut state = AppState::new(&Config::default());
    state.view = View::Library;
    state.library = vec![row(1), row(2)];
    state.sidebar_focus = true;

    state.update(Action::SelectIndex(1), &fx);
    state.update(Action::Activate, &fx);

    assert_eq!(state.view, View::NowPlaying);
    assert_eq!(state.queue_pos, Some(1));
    assert_eq!(
        state.now.as_ref().map(|now| now.title.as_str()),
        Some("Track 2")
    );
}

#[tokio::test]
async fn first_click_on_the_selected_library_row_only_moves_focus_from_the_sidebar() {
    let directory = tempfile::tempdir().unwrap();
    let fx = effects(&directory);
    let mut state = AppState::new(&Config::default());
    state.view = View::Library;
    state.library = vec![row(1), row(2)];
    state.selected = 0;
    state.sidebar_focus = true;
    let mut hits = ui::Hits::default();
    hits.rows.push((Rect::new(4, 5, 20, 1), 0));
    let click = || {
        Action::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 5,
            row: 5,
            modifiers: KeyModifiers::NONE,
        })
    };

    apply(&mut state, click(), &fx, &hits);

    assert_eq!(state.view, View::Library);
    assert!(!state.sidebar_focus);
    assert!(state.queue.is_empty());

    apply(&mut state, click(), &fx, &hits);

    assert_eq!(state.view, View::NowPlaying);
    assert_eq!(state.queue_pos, Some(0));
}

#[tokio::test]
async fn mouse_click_over_a_help_overlay_only_dismisses_the_overlay() {
    let directory = tempfile::tempdir().unwrap();
    let fx = effects(&directory);
    let mut state = AppState::new(&Config::default());
    state.view = View::Library;
    state.show_help = true;
    let mut hits = ui::Hits::default();
    hits.tabs.push((Rect::new(4, 1, 20, 1), View::Search));

    apply(
        &mut state,
        Action::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 5,
            row: 1,
            modifiers: KeyModifiers::NONE,
        }),
        &fx,
        &hits,
    );

    assert!(!state.show_help);
    assert_eq!(state.view, View::Library);
    assert!(state.queue.is_empty());
}

#[tokio::test]
async fn narrowing_the_terminal_clears_hidden_sidebar_focus() {
    let directory = tempfile::tempdir().unwrap();
    let fx = effects(&directory);
    let mut state = AppState::new(&Config::default());
    state.view = View::Library;
    state.update(Action::Resize { cols: 80, rows: 24 }, &fx);
    state.update(Action::Back, &fx);
    assert!(state.sidebar_focus);

    state.update(Action::Resize { cols: 40, rows: 24 }, &fx);

    assert!(!state.sidebar_focus);
    assert_eq!(state.view, View::Library);
}

#[tokio::test]
async fn back_leaves_library_when_the_sidebar_is_hidden() {
    let directory = tempfile::tempdir().unwrap();
    let fx = effects(&directory);
    let mut state = AppState::new(&Config::default());
    state.view = View::Library;
    state.update(Action::Resize { cols: 40, rows: 24 }, &fx);

    state.update(Action::Back, &fx);

    assert_eq!(state.view, View::NowPlaying);
    assert!(!state.sidebar_focus);
}

#[test]
fn starting_a_cover_load_clears_the_previous_song_art() {
    let mut state = AppState::new(&Config::default());
    state.cover = Some(pixel::vinyl(state.theme.palette, state.theme.bg, 4, 2));

    state.clear_cover();

    assert!(state.cover.is_none());
}

#[test]
fn cover_result_for_an_old_size_cannot_replace_the_current_cover() {
    let theme = Theme::db16();
    let current = pixel::vinyl(theme.palette, theme.bg, 4, 2);
    let replacement = pixel::vinyl(theme.palette, Color::Rgb(1, 2, 3), 4, 2);
    let stale = pixel::vinyl(theme.palette, theme.bg, 8, 4);
    let mut slot = Some(current.clone());

    apply_pixel_cover(
        &mut slot,
        7,
        (4, 2),
        3,
        CoverRenderRequest {
            surface: CoverSurface::Playing,
            generation: 7,
            cells: (4, 2),
            style_revision: 3,
            song_id: 1,
            source_key: "source".into(),
        },
        replacement.clone(),
    );
    assert_eq!(slot, Some(replacement.clone()));

    apply_pixel_cover(
        &mut slot,
        7,
        (4, 2),
        3,
        CoverRenderRequest {
            surface: CoverSurface::Playing,
            generation: 6,
            cells: (4, 2),
            style_revision: 3,
            song_id: 1,
            source_key: "source".into(),
        },
        current,
    );
    apply_pixel_cover(
        &mut slot,
        7,
        (4, 2),
        3,
        CoverRenderRequest {
            surface: CoverSurface::Playing,
            generation: 7,
            cells: (8, 4),
            style_revision: 3,
            song_id: 1,
            source_key: "source".into(),
        },
        stale,
    );

    let previous_theme = pixel::vinyl(Theme::db16().palette, Color::Rgb(9, 9, 9), 4, 2);
    apply_pixel_cover(
        &mut slot,
        7,
        (4, 2),
        4,
        CoverRenderRequest {
            surface: CoverSurface::Playing,
            generation: 7,
            cells: (4, 2),
            style_revision: 3,
            song_id: 1,
            source_key: "source".into(),
        },
        previous_theme,
    );

    assert_eq!(slot, Some(replacement));
}

#[test]
fn original_mode_only_accepts_a_real_graphics_protocol() {
    assert!(select_graphics_picker(CoverMode::Original, None).is_none());
    assert!(select_graphics_picker(CoverMode::Original, Some(Picker::halfblocks())).is_none());

    let mut kitty = Picker::halfblocks();
    kitty.set_protocol_type(ProtocolType::Kitty);
    let selected = select_graphics_picker(CoverMode::Original, Some(kitty)).unwrap();
    assert_eq!(selected.protocol_type(), ProtocolType::Kitty);

    let mut sixel = Picker::halfblocks();
    sixel.set_protocol_type(ProtocolType::Sixel);
    assert!(select_graphics_picker(CoverMode::Pixel, Some(sixel)).is_none());
}

#[tokio::test]
async fn a_known_row_replaces_the_old_track_identity_before_resolution() {
    let directory = tempfile::tempdir().unwrap();
    let fx = effects(&directory);
    let mut state = AppState::new(&Config::default());
    state.current_track_id = Some(99);
    state.paused = true;

    state.play_row(&fx, row(1));

    assert_eq!(state.current_track_id, Some(1));
    assert!(!state.paused);
    assert_eq!(
        state.now.as_ref().map(|now| now.title.as_str()),
        Some("Track 1")
    );
}

#[tokio::test]
async fn an_unresolved_demo_row_does_not_keep_the_previous_track_identity() {
    let directory = tempfile::tempdir().unwrap();
    let fx = effects(&directory);
    let mut state = AppState::new(&Config::default());
    state.current_track_id = Some(99);
    let mut unresolved = row(0);
    unresolved.title = "Search me".into();

    state.play_row(&fx, unresolved);

    assert_eq!(state.current_track_id, None);
}

#[tokio::test]
async fn a_known_track_uses_the_shared_cache_before_resolving_a_url() {
    let directory = tempfile::tempdir().unwrap();
    let cache_root = directory.path().join("audio");
    let key = CacheKey::new(42, yesplaymusic_core::cache::AudioQuality::High320);
    let cache = TrackCache::open(&cache_root).unwrap();
    let mut writer = cache
        .begin_write(CacheWriteRequest::new(
            key,
            yesplaymusic_core::cache::AudioCodec::Mp3,
            320_000,
        ))
        .unwrap();
    writer.write_all(b"cached audio").unwrap();
    writer.finish().unwrap();
    drop(cache);

    let (player, _events) = player::spawn(tokio::runtime::Handle::current());
    let (actions, mut receiver) = mpsc::unbounded_channel();
    let fx = Effects {
        player,
        ncm: Arc::new(Ncm::new(
            directory.path().join("session.json"),
            yesplaymusic_core::cache::AudioQuality::High320,
        )),
        store: Arc::new(crate::store::LibraryStore::new(
            directory.path().join("library"),
        )),
        actions,
        cache_root: Some(cache_root),
        covers: None,
        config_path: directory.path().join("config.toml"),
    };
    let mut state = AppState::new(&Config::default());
    state.play_row(&fx, row(42));

    let action = tokio::time::timeout(Duration::from_secs(1), receiver.recv())
        .await
        .expect("cache lookup should finish")
        .expect("cache lookup action");
    assert!(matches!(
        &action,
        Action::RowCacheReady {
            generation: 1,
            row: cached_row,
            lease: Some(_),
        } if cached_row.id == 42
    ));
    state.update(action, &fx);

    assert_eq!(state.current_track_id, Some(42));
    assert_eq!(
        state.now.as_ref().map(|now| now.title.as_str()),
        Some("Track 42")
    );
}
