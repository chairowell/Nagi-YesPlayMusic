use std::time::Duration;

use crate::action::{Action, CoverSurface, View};
use crate::api::Source;
use crate::event;
use crate::i18n::Key;
use crate::player::PlayerCommand;

use super::{
    apply_pixel_cover, song_row_from_resolved, spawn_cover_load, spawn_cover_prefetch,
    spawn_render_idle, spawn_resolve, AppState, Effects, PREVIEW_CELLS,
};

impl AppState {
    pub(super) fn update(&mut self, action: Action, fx: &Effects) {
        if matches!(action, Action::UiTick) {
            self.advance_marquee();
            return;
        }
        // The quit dialog owns keyboard input, while background results
        // keep flowing through the normal reducer.
        let action = if self.confirm_quit {
            match action {
                Action::RawKey(key) => {
                    match event::key_action(key) {
                        Some(Action::ConfirmYes | Action::Quit | Action::Activate) => {
                            self.should_quit = true;
                        }
                        Some(Action::Back | Action::Escape | Action::NextTrack) => {
                            self.confirm_quit = false;
                        }
                        _ => {}
                    }
                    return;
                }
                Action::ConfirmYes | Action::Quit | Action::Activate => {
                    self.should_quit = true;
                    return;
                }
                Action::Back | Action::Escape | Action::NextTrack => {
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
            if self.view == View::Settings {
                if let Some(mapped) = event::settings_key_action(*key) {
                    self.update(mapped, fx);
                }
                return;
            }
            if self.view == View::Search && self.search.input && !self.confirm_quit {
                self.handle_search_key(fx, *key);
                return;
            }
            if self.filter.input {
                self.handle_filter_key(*key);
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
            } else if self.filter.input {
                self.filter.paste(text);
                self.selected = 0;
            }
            return;
        }
        // vim gg: a second bare `g` right after the first jumps to the top.
        let was_pending_g = self.pending_g;
        self.pending_g = false;
        match action {
            Action::GKey => {
                if was_pending_g {
                    if self.view == View::Settings {
                        self.settings.selected = 0;
                    } else if self.view == View::Library && self.sidebar_focus {
                        self.sidebar_selected = 0;
                    } else {
                        self.selected = 0;
                    }
                } else {
                    self.pending_g = true;
                }
            }
            Action::JumpTop => {
                if self.view == View::Settings {
                    self.settings.selected = 0;
                } else if self.view == View::Library && self.sidebar_focus {
                    self.sidebar_selected = 0;
                } else {
                    self.selected = 0;
                }
            }
            Action::JumpBottom => {
                if self.view == View::Settings {
                    let len = super::settings::SettingField::ALL.len();
                    self.settings.selected = len.saturating_sub(1);
                } else if self.view == View::Library && self.sidebar_focus {
                    self.sidebar_selected = crate::ui::library::SOURCES.len() - 1;
                } else {
                    self.selected = self.visible_len().saturating_sub(1);
                }
            }
            Action::ConfirmYes => {}
            Action::Quit => self.confirm_quit = true,
            Action::SwitchView(view) => {
                self.clear_filter();
                if view == View::Settings {
                    self.open_settings();
                } else {
                    if self.view == View::Settings {
                        self.cancel_settings(fx);
                    }
                    self.view = view;
                }
                if view == View::Search {
                    self.search.input = true;
                }
            }
            Action::Back => self.navigate_back(fx),
            Action::Escape => {
                if self.filter.is_active() {
                    self.clear_filter();
                } else {
                    self.navigate_back(fx);
                }
            }
            Action::ToggleZen => {
                self.zen = !self.zen;
                if self.zen {
                    self.view = View::NowPlaying;
                }
            }
            Action::TogglePlay => self.toggle_play(fx),
            Action::SeekBy(seconds) => {
                let step = Duration::from_secs(seconds.unsigned_abs());
                let target = if seconds >= 0 {
                    self.position.saturating_add(step)
                } else {
                    self.position.saturating_sub(step)
                };
                fx.player.send(PlayerCommand::SeekTo(target));
            }
            Action::VolumeBy(delta) => self.set_volume(fx, self.volume + delta),
            Action::ToggleMute => self.toggle_mute(fx),
            Action::MoveSelection(delta) => {
                if self.view == View::Settings {
                    self.move_setting_selection(delta);
                    return;
                }
                if self.view == View::Library && self.sidebar_focus {
                    let next = (self.sidebar_selected as i32 + delta.signum()).clamp(0, 3);
                    self.sidebar_selected = next as usize;
                    return;
                }
                let len = self.visible_len();
                if len > 0 {
                    let last = len as i32 - 1;
                    let next = (self.selected as i32 + delta).clamp(0, last);
                    self.selected = next as usize;
                }
            }
            Action::MovePage(pages) => {
                if self.view == View::Library && self.sidebar_focus {
                    return;
                }
                let len = self.visible_len();
                if len > 0 {
                    let delta = pages.saturating_mul(self.list_page_size() as i32);
                    let next = (self.selected as i32 + delta).clamp(0, len as i32 - 1);
                    self.selected = next as usize;
                }
            }
            Action::Activate => match self.view {
                View::Settings => self.save_settings(fx),
                View::Library if self.sidebar_focus => {
                    self.open_source(fx, self.sidebar_selected);
                }
                View::Library => {
                    if let Some((_, row)) = self.visible_row(self.selected) {
                        if self.enter_replaces_queue {
                            self.queue = self.visible_rows_owned();
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
                        self.reset_shuffle_order();
                        self.play_row(fx, row);
                        self.clear_filter();
                        self.view = View::NowPlaying;
                    }
                }
                View::Queue => {
                    if let Some((underlying, row)) = self.visible_row(self.selected) {
                        self.queue_pos = Some(underlying);
                        self.reset_shuffle_order();
                        self.play_row(fx, row);
                        self.clear_filter();
                        self.view = View::NowPlaying;
                    }
                }
                View::Search if !self.search.input => {
                    if let Some((_, row)) = self.visible_row(self.selected) {
                        self.queue = self.visible_rows_owned();
                        self.queue_pos = Some(self.selected);
                        self.queue_source = Source::Search;
                        self.pending_fm_next = false;
                        self.fm_request_pending = false;
                        self.reset_shuffle_order();
                        self.play_row(fx, row);
                        self.clear_filter();
                        self.view = View::NowPlaying;
                    }
                }
                _ => {}
            },
            Action::AddSelectedToQueue => {
                if let Some((_, row)) = self.visible_row(self.selected) {
                    self.queue.push(row);
                    self.shuffle_order.clear();
                    self.status = Some(crate::i18n::t(Key::AddedToQueue).into());
                }
            }
            Action::NextTrack => self.step_queue(fx, 1, false),
            Action::PrevTrack => self.step_queue(fx, -1, false),
            Action::ToggleHelp => self.show_help = true,
            Action::ToggleShuffle => {
                self.shuffle = !self.shuffle;
                self.reset_shuffle_order();
            }
            Action::CycleRepeat => {
                self.play_mode = self.play_mode.next();
            }
            Action::StartFilter => {
                if matches!(self.view, View::Library | View::Queue)
                    || self.view == View::Search && !self.search.input
                {
                    self.filter.start();
                    self.selected = 0;
                    if self.view == View::Library {
                        self.sidebar_focus = false;
                    }
                }
            }
            Action::ToggleLibraryFocus => {
                if self.view == View::Library && self.sidebar_visible() {
                    self.sidebar_focus = !self.sidebar_focus;
                    if self.sidebar_focus {
                        self.sidebar_selected = self.source_index();
                    }
                }
            }
            Action::SetVolumeTo(ratio) => self.set_volume(fx, ratio.clamp(0.0, 1.0)),
            Action::ToggleLike => self.toggle_like(fx),
            Action::OpenSource(index) => self.open_source(fx, index),
            Action::SelectSetting(index) => self.select_setting(index),
            Action::AdjustSetting(delta) => self.adjust_setting(fx, delta),
            Action::SaveSettings => self.save_settings(fx),
            Action::CancelSettings => self.cancel_settings(fx),
            Action::LikedIds { session, ids } => self.apply_liked_ids(session, ids),
            Action::FmMore { session, rows } => self.apply_fm_more(fx, session, rows),
            Action::FmLoadFailed { session, message } => {
                self.apply_fm_load_failed(session, message);
            }
            Action::SelectIndex(index) => {
                let len = self.visible_len();
                if index < len {
                    self.selected = index;
                    self.filter.input = false;
                    match self.view {
                        View::Library => self.sidebar_focus = false,
                        View::Search => self.search.input = false,
                        _ => {}
                    }
                }
            }
            Action::Mouse(_) => {} // resolved against Hits in the event loop
            Action::RawKey(_) | Action::Paste(_) | Action::UiTick => {} // handled before this match
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
                // at that index is still the same song and the quality
                // request still matches the current setting.
                if self.queue.get(index).is_some_and(|row| row.id == track.id)
                    && track.cache_key.quality == fx.ncm.quality()
                {
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
            Action::CoverLoaded { request, cover } => {
                let desired_playing = self.desired_cover_cells();
                match request.surface {
                    CoverSurface::Playing => apply_pixel_cover(
                        &mut self.cover,
                        self.generation,
                        desired_playing,
                        self.style_revision,
                        request,
                        cover,
                    ),
                    CoverSurface::Selection => apply_pixel_cover(
                        &mut self.selected_cover.pixel,
                        self.selected_cover.generation,
                        PREVIEW_CELLS,
                        self.style_revision,
                        request,
                        cover,
                    ),
                }
            }
            Action::CoverDecoded {
                surface,
                generation,
                style_revision,
                image,
            } => {
                if style_revision == self.style_revision {
                    match surface {
                        CoverSurface::Playing if generation == self.generation => {
                            if let Some(original) = &mut self.original_cover {
                                original.replace(generation, image);
                            }
                        }
                        CoverSurface::Selection if generation == self.selected_cover.generation => {
                            if let Some(original) = &mut self.selected_original_cover {
                                original.replace(generation, image);
                            }
                        }
                        _ => {}
                    }
                }
            }
            Action::SelectionCoverDue {
                generation,
                row,
                neighbors,
            } => {
                let current = self.selected_cover.key.as_ref();
                if generation == self.selected_cover.generation
                    && current.is_some_and(|key| {
                        key.id == row.id && key.pic_url.as_ref() == row.pic_url.as_ref()
                    })
                {
                    if let Some(pic_url) = row.pic_url.as_deref() {
                        let request = self.cover_request(
                            CoverSurface::Selection,
                            generation,
                            row.id,
                            pic_url,
                            PREVIEW_CELLS,
                        );
                        spawn_cover_load(
                            fx,
                            request,
                            pic_url.to_owned(),
                            self.theme.palette,
                            self.theme.bg,
                            self.pixel_detail_scale,
                            self.uses_original_cover(CoverSurface::Selection),
                        );
                    }
                    spawn_cover_prefetch(fx, neighbors);
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
                    self.style_revision,
                );
            }
            Action::IdleArtLoaded {
                cells,
                style_revision,
                cover,
            } => {
                if cells == self.desired_idle_cells() && style_revision == self.style_revision {
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
                let desired = self.desired_cover_cells();
                let current = self.cover.as_ref().map(|cover| (cover.width, cover.height));
                if current != Some(desired) {
                    self.cover = None;
                    if let Some(row) = self.active_row.clone() {
                        self.load_playing_cover(fx, &row);
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
                            self.style_revision,
                        );
                    }
                }
                if self.now.is_some() {
                    self.ensure_placeholder();
                }
            }
        }
    }

    fn navigate_back(&mut self, fx: &Effects) {
        self.clear_filter();
        if self.view == View::Settings {
            self.cancel_settings(fx);
        } else if self.view == View::Search && !self.search.input {
            self.search.input = true;
        } else if self.view == View::Library && !self.sidebar_focus && self.sidebar_visible() {
            self.sidebar_focus = true;
            self.sidebar_selected = self.source_index();
        } else {
            self.sidebar_focus = false;
            self.view = View::NowPlaying;
        }
    }

    fn set_volume(&mut self, fx: &Effects, volume: f32) {
        let volume = volume.clamp(0.0, 1.5);
        if volume == 0.0 && self.volume > 0.0 {
            self.volume_before_mute = Some(self.volume);
        } else if volume > 0.0 {
            self.volume_before_mute = None;
        }
        self.volume = volume;
        fx.player.send(PlayerCommand::SetVolume(volume));
    }

    fn toggle_mute(&mut self, fx: &Effects) {
        if self.volume > 0.0 {
            let previous = self.volume;
            self.set_volume(fx, 0.0);
            self.volume_before_mute = Some(previous);
        } else {
            let volume = self.volume_before_mute.take().unwrap_or(1.0);
            self.set_volume(fx, volume);
        }
    }
}
