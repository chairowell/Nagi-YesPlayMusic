use crate::action::{Action, View, SEEK_STEP};
use crate::api::Source;
use crate::event;
use crate::player::PlayerCommand;

use super::{
    desired_idle_cells, render_idle_art, scale_cells, spawn_render_cover, AppState, Effects,
};

impl AppState {
    pub(super) fn update(&mut self, action: Action, fx: &Effects) {
        // The quit-confirm dialog is modal: y/Enter/q confirm, n/Esc cancel.
        if self.confirm_quit {
            let action = match action {
                Action::RawKey(key) => match event::key_action(key) {
                    Some(action) => action,
                    None => return,
                },
                action => action,
            };
            match action {
                Action::ConfirmYes | Action::Quit | Action::Activate => self.should_quit = true,
                Action::Back | Action::NextTrack => self.confirm_quit = false,
                Action::Player(event) => self.apply_player_event(event),
                _ => {}
            }
            return;
        }
        // The help overlay is modal: any key dismisses it.
        if self.show_help && matches!(action, Action::RawKey(_) | Action::Mouse(_)) {
            self.show_help = false;
            return;
        }
        // Text-input mode: the search box owns the keyboard.
        if let Action::RawKey(key) = &action {
            if self.view == View::Search && self.search.input && !self.confirm_quit {
                self.handle_search_key(fx, *key);
                return;
            }
            let Some(mapped) = event::key_action(*key) else {
                return;
            };
            self.update(mapped, fx);
            return;
        }
        if let Action::Paste(text) = &action {
            if self.view == View::Search && self.search.input {
                self.search.paste(text);
            }
            return;
        }
        // vim gg: a second bare `g` right after the first jumps to the top.
        let was_pending_g = self.pending_g;
        self.pending_g = false;
        match action {
            Action::GKey => {
                if was_pending_g {
                    self.selected = 0;
                } else {
                    self.pending_g = true;
                }
            }
            Action::JumpBottom => {
                let len = match self.view {
                    View::Library => self.library.len(),
                    View::Queue => self.queue.len(),
                    _ => 0,
                };
                self.selected = len.saturating_sub(1);
            }
            Action::ConfirmYes => {}
            Action::Quit => self.confirm_quit = true,
            Action::SwitchView(view) => {
                self.view = view;
                if view == View::Search {
                    self.search.input = true;
                }
            }
            Action::Back => {
                if self.view == View::Search && !self.search.input {
                    self.search.input = true;
                } else if self.view == View::Library && !self.sidebar_focus {
                    self.sidebar_focus = true;
                    self.sidebar_selected = self.source_index();
                } else {
                    self.sidebar_focus = false;
                    self.view = View::NowPlaying;
                }
            }
            Action::ToggleZen => {
                self.zen = !self.zen;
                if self.zen {
                    self.view = View::NowPlaying;
                }
            }
            Action::TogglePlay => fx.player.send(PlayerCommand::TogglePause),
            Action::SeekBy(sign) => {
                let target = if sign >= 0 {
                    self.position.saturating_add(SEEK_STEP)
                } else {
                    self.position.saturating_sub(SEEK_STEP)
                };
                fx.player.send(PlayerCommand::SeekTo(target));
            }
            Action::VolumeBy(delta) => {
                self.volume = (self.volume + delta).clamp(0.0, 1.5);
                fx.player.send(PlayerCommand::SetVolume(self.volume));
            }
            Action::MoveSelection(delta) => {
                if self.view == View::Library && self.sidebar_focus {
                    let next = (self.sidebar_selected as i32 + delta.signum()).clamp(0, 3);
                    self.sidebar_selected = next as usize;
                    return;
                }
                let len = match self.view {
                    View::Library => self.library.len(),
                    View::Queue => self.queue.len(),
                    View::Search => self.search.results.len(),
                    _ => 0,
                };
                if len > 0 {
                    let last = len as i32 - 1;
                    let next = (self.selected as i32 + delta).clamp(0, last);
                    self.selected = next as usize;
                }
            }
            Action::Activate => match self.view {
                View::Library if self.sidebar_focus => {
                    self.open_source(fx, self.sidebar_selected);
                }
                View::Library => {
                    if let Some(row) = self.library.get(self.selected).cloned() {
                        if self.enter_replaces_queue {
                            // Desktop/NCM semantics: the list becomes the
                            // listening context from this song onward.
                            self.queue = self.library.clone();
                            self.queue_pos = Some(self.selected);
                        } else {
                            self.queue = vec![row.clone()];
                            self.queue_pos = Some(0);
                        }
                        self.queue_source = self.library_source;
                        self.play_row(fx, row);
                        self.view = View::NowPlaying;
                    }
                }
                View::Queue => {
                    if let Some(row) = self.queue.get(self.selected).cloned() {
                        self.queue_pos = Some(self.selected);
                        self.play_row(fx, row);
                        self.view = View::NowPlaying;
                    }
                }
                View::Search if !self.search.input => {
                    if let Some(row) = self.search.results.get(self.selected).cloned() {
                        self.queue = self.search.results.clone();
                        self.queue_pos = Some(self.selected);
                        self.queue_source = Source::Search;
                        self.play_row(fx, row);
                        self.view = View::NowPlaying;
                    }
                }
                _ => {}
            },
            Action::NextTrack => self.step_queue(fx, 1, false),
            Action::PrevTrack => self.step_queue(fx, -1, false),
            Action::ToggleHelp => self.show_help = true,
            Action::CycleMode => {
                self.play_mode = self.play_mode.next();
            }
            Action::SetVolumeTo(ratio) => {
                self.volume = ratio.clamp(0.0, 1.0);
                fx.player.send(PlayerCommand::SetVolume(self.volume));
            }
            Action::ToggleLike => self.toggle_like(fx),
            Action::OpenSource(index) => self.open_source(fx, index),
            Action::LikedIds { session, ids } => self.apply_liked_ids(session, ids),
            Action::FmMore { session, rows } => self.apply_fm_more(fx, session, rows),
            Action::SelectIndex(index) => {
                let len = match self.view {
                    View::Library => self.library.len(),
                    View::Queue => self.queue.len(),
                    View::Search => self.search.results.len(),
                    _ => 0,
                };
                if index < len {
                    self.selected = index;
                    if self.view == View::Search {
                        self.search.input = false;
                    }
                }
            }
            Action::Mouse(_) => {} // resolved against Hits in the event loop
            Action::RawKey(_) | Action::Paste(_) => {} // handled before this match
            Action::StartLogin => self.start_login(fx),
            Action::LoginQrReady { attempt, art } => self.apply_login_qr(attempt, art),
            Action::LoginProgress { attempt, message } => {
                self.apply_login_progress(attempt, message);
            }
            Action::LoginFailed { attempt, message } => {
                self.apply_login_failed(attempt, message);
            }
            Action::LoginSucceeded {
                attempt,
                session,
                uid,
                nickname,
            } => self.apply_login_succeeded(fx, attempt, session, uid, nickname),
            Action::SessionRestored {
                epoch,
                uid,
                nickname,
            } => self.apply_session_restored(fx, epoch, uid, nickname),
            Action::SessionRestoreFailed { epoch, message } => {
                self.apply_session_restore_failed(epoch, message);
            }
            Action::LibraryLoaded {
                session,
                source,
                rows,
            } => self.apply_library_loaded(fx, session, source, rows),
            Action::SearchResults { seq, query, rows } => {
                if self.search.accept(seq, &query, rows) && !self.search.results.is_empty() {
                    self.selected = 0;
                }
            }
            Action::PersonalNotice { session, message } => {
                self.apply_personal_notice(session, message);
            }
            Action::LyricsLoaded { generation, lines } => {
                if generation == self.generation {
                    self.lyrics = lines;
                }
            }
            Action::TrackResolved { generation, track } => {
                if generation == self.generation {
                    self.apply_resolved(fx, generation, track);
                }
            }
            Action::PrefetchReady { index, track } => {
                // Guard against a rebuilt queue: only keep it if the row
                // at that index is still the same song.
                if self.queue.get(index).is_some_and(|row| row.id == track.id) {
                    self.prefetched = Some((index, track));
                }
            }
            Action::ResolveFailed {
                generation,
                message,
            } => {
                if generation == self.generation {
                    self.status = Some(message);
                }
            }
            Action::CoverBytes { generation, bytes } => {
                if generation == self.generation {
                    self.cover_bytes = Some(bytes.clone());
                    spawn_render_cover(
                        fx,
                        generation,
                        bytes,
                        self.theme.palette,
                        self.desired_cover_cells(),
                    );
                }
            }
            Action::CoverLoaded { generation, cover } => {
                if generation == self.generation {
                    self.cover = Some(cover);
                }
            }
            Action::Player(event) => {
                self.apply_player_event(event);
                if self.pending_auto_next {
                    self.pending_auto_next = false;
                    self.step_queue(fx, 1, true);
                }
            }
            Action::Resize => {
                // Layout-dependent resolution: re-render cover and idle art
                // from their kept source bytes when the desired grid changed.
                if let (Some(bytes), Some(cover)) = (&self.cover_bytes, &self.cover) {
                    let desired = self.desired_cover_cells();
                    if (cover.width, cover.height) != desired {
                        spawn_render_cover(
                            fx,
                            self.generation,
                            bytes.clone(),
                            self.theme.palette,
                            desired,
                        );
                    }
                }
                let desired = scale_cells(desired_idle_cells(), self.pixel_scale);
                if (self.idle_art.width, self.idle_art.height) != desired {
                    self.idle_art =
                        render_idle_art(self.idle_bytes.as_deref(), self.theme.palette, desired);
                }
                if self.now.is_some() {
                    self.ensure_placeholder();
                }
            }
        }
    }
}
