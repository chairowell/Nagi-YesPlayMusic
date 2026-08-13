use crate::action::{Action, CoverRenderRequest, View, SEEK_STEP};
use crate::api::Source;
use crate::event;
use crate::player::PlayerCommand;

use super::{
    apply_pixel_cover, song_row_from_resolved, spawn_decode_cover, spawn_render_cover,
    spawn_render_idle, spawn_resolve, AppState, Effects,
};

impl AppState {
    pub(super) fn update(&mut self, action: Action, fx: &Effects) {
        // The quit dialog owns keyboard input, while background results
        // keep flowing through the normal reducer.
        let action = if self.confirm_quit {
            match action {
                Action::RawKey(key) => {
                    match event::key_action(key) {
                        Some(Action::ConfirmYes | Action::Quit | Action::Activate) => {
                            self.should_quit = true;
                        }
                        Some(Action::Back | Action::NextTrack) => self.confirm_quit = false,
                        _ => {}
                    }
                    return;
                }
                Action::ConfirmYes | Action::Quit | Action::Activate => {
                    self.should_quit = true;
                    return;
                }
                Action::Back | Action::NextTrack => {
                    self.confirm_quit = false;
                    return;
                }
                action => action,
            }
        } else {
            action
        };
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
                    View::Search => self.search.results.len(),
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
                } else if self.view == View::Library
                    && !self.sidebar_focus
                    && self.sidebar_visible()
                {
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
                        if self.queue_source != Source::Fm {
                            self.pending_fm_next = false;
                            self.fm_request_pending = false;
                        }
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
                        self.pending_fm_next = false;
                        self.fm_request_pending = false;
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
            Action::FmLoadFailed { session, message } => {
                self.apply_fm_load_failed(session, message);
            }
            Action::SelectIndex(index) => {
                let len = match self.view {
                    View::Library => self.library.len(),
                    View::Queue => self.queue.len(),
                    View::Search => self.search.results.len(),
                    _ => 0,
                };
                if index < len {
                    self.selected = index;
                    match self.view {
                        View::Library => self.sidebar_focus = false,
                        View::Search => self.search.input = false,
                        _ => {}
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
                request,
                source,
                rows,
            } => self.apply_library_loaded(fx, session, request, source, rows),
            Action::LibraryFailed {
                session,
                request,
                message,
            } => self.apply_library_failed(session, request, message),
            Action::SearchResults { seq, query, rows } => {
                if self.search.accept(seq, &query, rows) && !self.search.results.is_empty() {
                    self.selected = 0;
                }
            }
            Action::SearchFailed {
                seq,
                query,
                message,
            } => {
                self.search.fail(seq, &query, message);
            }
            Action::LikeFinished {
                session,
                id,
                mutation,
                attempted_like,
                error,
            } => self.apply_like_finished(fx, session, id, mutation, attempted_like, error),
            Action::LyricsLoaded { generation, lines } => {
                if generation == self.generation {
                    self.lyrics = lines;
                }
            }
            Action::TrackResolved { generation, track } => {
                if generation == self.generation {
                    self.prepare_resolved(fx, generation, track);
                }
            }
            Action::RowCacheReady {
                generation,
                row,
                lease,
            } => {
                if generation == self.generation {
                    if let Some(lease) = lease {
                        self.apply_cached(fx, generation, row, lease);
                    } else {
                        spawn_resolve(fx, generation, row);
                    }
                }
            }
            Action::ResolvedCacheReady {
                generation,
                track,
                lease,
            } => {
                if generation == self.generation {
                    if let Some(lease) = lease {
                        let row = song_row_from_resolved(&track);
                        self.apply_cached(fx, generation, row, lease);
                    } else {
                        self.apply_resolved(fx, generation, track);
                    }
                }
            }
            Action::CacheFallbackResolved { generation, track } => {
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
                    let original_bytes = self.original_cover.is_some().then(|| bytes.clone());
                    let request = CoverRenderRequest {
                        generation,
                        cells: self.desired_cover_cells(),
                    };
                    spawn_render_cover(
                        fx,
                        request,
                        bytes,
                        self.theme.palette,
                        self.theme.bg,
                        self.pixel_detail_scale,
                    );
                    if let Some(bytes) = original_bytes {
                        spawn_decode_cover(fx, generation, bytes);
                    }
                }
            }
            Action::CoverLoaded { request, cover } => {
                let desired = self.desired_cover_cells();
                apply_pixel_cover(&mut self.cover, self.generation, desired, request, cover);
            }
            Action::CoverDecoded { generation, image } => {
                if generation == self.generation {
                    if let Some(original) = &mut self.original_cover {
                        original.replace(generation, image);
                    }
                }
            }
            Action::IdleArtBytes { bytes } => {
                self.idle_bytes = Some(bytes.clone());
                spawn_render_idle(
                    fx,
                    bytes,
                    self.theme.palette,
                    self.theme.bg,
                    self.desired_idle_cells(),
                    self.pixel_detail_scale,
                );
            }
            Action::IdleArtLoaded { cells, cover } => {
                if cells == self.desired_idle_cells() {
                    self.idle_art = cover;
                }
            }
            Action::Player(event) => {
                self.apply_player_event(fx, event);
                if self.pending_auto_next {
                    self.pending_auto_next = false;
                    self.step_queue(fx, 1, true);
                }
            }
            Action::Resize { cols, rows } => {
                self.terminal_size = (cols, rows);
                if !self.sidebar_visible() {
                    self.sidebar_focus = false;
                }
                // Layout-dependent resolution: re-render cover and idle art
                // from their kept source bytes when the desired grid changed.
                if let Some(bytes) = &self.cover_bytes {
                    let desired = self.desired_cover_cells();
                    let current = self.cover.as_ref().map(|cover| (cover.width, cover.height));
                    if current != Some(desired) {
                        spawn_render_cover(
                            fx,
                            CoverRenderRequest {
                                generation: self.generation,
                                cells: desired,
                            },
                            bytes.clone(),
                            self.theme.palette,
                            self.theme.bg,
                            self.pixel_detail_scale,
                        );
                    }
                }
                let desired = self.desired_idle_cells();
                if (self.idle_art.width, self.idle_art.height) != desired {
                    if let Some(bytes) = self.idle_bytes.clone() {
                        spawn_render_idle(
                            fx,
                            bytes,
                            self.theme.palette,
                            self.theme.bg,
                            desired,
                            self.pixel_detail_scale,
                        );
                    }
                }
                if self.now.is_some() {
                    self.ensure_placeholder();
                }
            }
        }
    }
}
