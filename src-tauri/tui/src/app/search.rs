use crate::action::{Action, View};
use crate::api::SongRow;

use super::{AppState, Effects};

impl AppState {
    pub(super) fn handle_search_key(&mut self, fx: &Effects, key: crossterm::event::KeyEvent) {
        use crossterm::event::{KeyCode, KeyModifiers};
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            if key.code == KeyCode::Char('c') {
                self.confirm_quit = true;
            }
            return;
        }
        match key.code {
            KeyCode::Char(c) => self.search.push(c),
            KeyCode::Backspace => {
                self.search.pop();
            }
            KeyCode::Enter => {
                if let Some(request) = self.search.submit() {
                    spawn_search(fx, request);
                }
            }
            KeyCode::Esc => {
                if self.search.query.is_empty() {
                    self.search.invalidate();
                    self.view = View::NowPlaying;
                } else {
                    self.search.clear();
                }
            }
            KeyCode::Down | KeyCode::Tab if !self.search.results.is_empty() => {
                self.search.input = false;
                self.selected = 0;
            }
            _ => {}
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SearchRequest {
    pub seq: u64,
    pub query: String,
}

#[derive(Default)]
pub struct SearchState {
    pub query: String,
    pub results: Vec<SongRow>,
    pub input: bool,
    pub searching: bool,
    seq: u64,
    active: Option<SearchRequest>,
}

impl SearchState {
    pub(super) fn new() -> Self {
        Self {
            input: true,
            ..Self::default()
        }
    }

    pub(super) fn push(&mut self, character: char) {
        self.query.push(character);
        self.invalidate();
    }

    pub(super) fn pop(&mut self) {
        self.query.pop();
        self.invalidate();
    }

    pub(super) fn paste(&mut self, text: &str) {
        self.query.push_str(&text.replace(['\n', '\r'], " "));
        self.invalidate();
    }

    pub(super) fn clear(&mut self) {
        self.query.clear();
        self.invalidate();
    }

    pub(super) fn invalidate(&mut self) {
        self.seq += 1;
        self.active = None;
        self.searching = false;
        self.results.clear();
    }

    pub(super) fn submit(&mut self) -> Option<SearchRequest> {
        let query = self.query.trim().to_owned();
        if query.is_empty() {
            return None;
        }
        self.seq += 1;
        let request = SearchRequest {
            seq: self.seq,
            query,
        };
        self.searching = true;
        self.active = Some(request.clone());
        Some(request)
    }

    pub(super) fn accept(&mut self, seq: u64, query: &str, rows: Vec<SongRow>) -> bool {
        let matches = self
            .active
            .as_ref()
            .is_some_and(|request| request.seq == seq && request.query == query);
        if !matches {
            return false;
        }
        self.active = None;
        self.searching = false;
        self.results = rows;
        true
    }
}

pub(super) fn spawn_search(fx: &Effects, request: SearchRequest) {
    let ncm = fx.ncm.clone();
    let actions = fx.actions.clone();
    tokio::spawn(async move {
        let rows = ncm
            .search_rows(&request.query, 30)
            .await
            .unwrap_or_default();
        let _ = actions.send(Action::SearchResults {
            seq: request.seq,
            query: request.query,
            rows,
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: i64) -> SongRow {
        SongRow {
            id,
            title: format!("Track {id}"),
            artist: "Artist".into(),
            duration_ms: 180_000,
            pic_url: None,
        }
    }

    #[test]
    fn results_require_the_active_sequence_and_query() {
        let mut state = SearchState::new();
        state.query = "first".into();
        let first = state.submit().unwrap();
        state.push('!');
        state.query = "second".into();
        let second = state.submit().unwrap();

        assert!(!state.accept(first.seq, &first.query, vec![row(1)]));
        assert!(state.results.is_empty());
        assert!(state.searching);

        assert!(!state.accept(second.seq, "different", vec![row(2)]));
        assert!(state.accept(second.seq, &second.query, vec![row(3)]));
        assert_eq!(state.results[0].id, 3);
        assert!(!state.searching);
        assert!(state.input);
    }
}
