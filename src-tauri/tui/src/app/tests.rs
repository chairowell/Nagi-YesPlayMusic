use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use tempfile::TempDir;

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
async fn editing_search_rejects_the_result_for_the_previous_query() {
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

        state.update(edit, &fx);
        state.update(
            Action::SearchResults {
                seq: request.seq,
                query: request.query,
                rows: vec![row(1)],
            },
            &fx,
        );

        assert!(state.search.results.is_empty());
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

#[test]
fn starting_a_cover_load_clears_the_previous_song_art() {
    let mut state = AppState::new(&Config::default());
    state.cover = Some(pixel::vinyl(state.theme.palette, state.theme.bg, 4, 2));
    state.cover_bytes = Some(vec![1, 2, 3]);

    state.clear_cover();

    assert!(state.cover.is_none());
    assert!(state.cover_bytes.is_none());
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
        CoverRenderRequest {
            generation: 7,
            cells: (4, 2),
        },
        replacement.clone(),
    );
    assert_eq!(slot, Some(replacement.clone()));

    apply_pixel_cover(
        &mut slot,
        7,
        (4, 2),
        CoverRenderRequest {
            generation: 6,
            cells: (4, 2),
        },
        current,
    );
    apply_pixel_cover(
        &mut slot,
        7,
        (4, 2),
        CoverRenderRequest {
            generation: 7,
            cells: (8, 4),
        },
        stale,
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
async fn starting_a_new_track_clears_the_old_track_identity_immediately() {
    let directory = tempfile::tempdir().unwrap();
    let fx = effects(&directory);
    let mut state = AppState::new(&Config::default());
    state.current_track_id = Some(99);
    state.paused = true;

    state.play_row(&fx, row(1));

    assert_eq!(state.current_track_id, None);
    assert!(!state.paused);
    assert_eq!(
        state.now.as_ref().map(|now| now.title.as_str()),
        Some("Track 1")
    );
}
