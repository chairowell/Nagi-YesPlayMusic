import { defineStore } from 'pinia';
import pkg from '../../package.json';
import defaultStorageState from './defaults';
import updateApp from '@/utils/updateApp';
import defaultShortcuts from '@/utils/shortcuts';
import { isAccountLoggedIn, isLooseLoggedIn } from '@/utils/auth';
import { likeATrack as requestLikeTrack, getTrackDetail } from '@/api/track';
import { getPlaylistDetail } from '@/api/playlist';
import {
  cloudDisk,
  likedAlbums,
  likedArtists,
  likedMVs,
  userAccount,
  userLikedSongsIDs,
  userPlayHistory,
  userPlaylist,
} from '@/api/user';
import type {
  AppState,
  LikedState,
  ModalUpdate,
  ModalsState,
  ToastState,
} from '@/types/store';
import type { LastfmState, Track } from '@/types/domain';
import type { DataState, SettingsState, Shortcut } from '@/types/persistence';
import Player from '@/utils/Player';
import {
  decodeDataState,
  decodeLastfmState,
  decodeSettingsState,
  readStoredJson,
} from '@/utils/persistedState';
import { fetchLikedSongIdsForUser } from './fetchLikedSongs';
import { resolveScrollingState } from './stateTransitions';

let modalScrollLockTimer: ReturnType<typeof setTimeout> | null = null;

function hasVisibleModal(modals: ModalsState): boolean {
  return modals.addTrackToPlaylistModal.show || modals.newPlaylistModal.show;
}

function createEmptyLikedState(): LikedState {
  return {
    songs: [],
    songsWithDetails: [],
    playlists: [],
    albums: [],
    artists: [],
    mvs: [],
    cloudDisk: [],
    playHistory: {
      weekData: [],
      allData: [],
    },
  };
}

function createInitialState(): AppState {
  if (localStorage.getItem('appVersion') === null) {
    localStorage.setItem(
      'settings',
      JSON.stringify(defaultStorageState.settings)
    );
    localStorage.setItem('data', JSON.stringify(defaultStorageState.data));
    localStorage.setItem('appVersion', pkg.version);
  }

  updateApp();

  return {
    sessionEpoch: 0,
    showLyrics: false,
    enableScrolling: true,
    title: 'YesPlayMusic',
    liked: createEmptyLikedState(),
    contextMenu: {
      clickObjectID: 0,
      showMenu: false,
    },
    toast: {
      show: false,
      text: '',
      timer: null,
    },
    modals: {
      addTrackToPlaylistModal: {
        show: false,
        selectedTrackID: 0,
      },
      newPlaylistModal: {
        show: false,
        afterCreateAddTrackID: 0,
      },
    },
    dailyTracks: [],
    lastfm: decodeLastfmState(readStoredJson(localStorage, 'lastfm')),
    // Persistence and timers are attached before components mount.
    player: new Player(),
    settings: decodeSettingsState(
      readStoredJson(localStorage, 'settings'),
      defaultStorageState.settings
    ),
    data: decodeDataState(
      readStoredJson(localStorage, 'data'),
      defaultStorageState.data
    ),
  };
}

export const useAppStore = defineStore('app', {
  state: createInitialState,
  actions: {
    updateLikedXXX<K extends keyof LikedState>({
      name,
      data,
    }: {
      name: K;
      data: LikedState[K];
    }) {
      this.liked[name] = data;
    },
    changeLang(lang: string) {
      this.settings.lang = lang;
    },
    changeMusicQuality(value: SettingsState['musicQuality']) {
      this.settings.musicQuality = value;
    },
    changeLyricFontSize(value: number) {
      this.settings.lyricFontSize = value;
    },
    changeOutputDevice(deviceId: string) {
      this.settings.outputDevice = deviceId;
    },
    updateSettings<K extends keyof SettingsState>({
      key,
      value,
    }: {
      key: K;
      value: SettingsState[K];
    }) {
      this.settings[key] = value;
    },
    updateData<K extends keyof DataState>({
      key,
      value,
    }: {
      key: K;
      value: DataState[K];
    }) {
      this.data[key] = value;
    },
    startUserSession({
      mode,
      user = {},
    }: {
      mode: Exclude<DataState['loginMode'], null>;
      user?: DataState['user'];
    }) {
      this.sessionEpoch += 1;
      this.liked = createEmptyLikedState();
      this.data.user = user;
      this.data.loginMode = mode;
      this.data.likedSongPlaylistID = undefined;
    },
    clearUserSession() {
      this.sessionEpoch += 1;
      this.liked = createEmptyLikedState();
      this.data.user = {};
      this.data.loginMode = null;
      this.data.likedSongPlaylistID = undefined;
    },
    togglePlaylistCategory(name: string) {
      const index = this.settings.enabledPlaylistCategories.indexOf(name);
      if (index !== -1) {
        this.settings.enabledPlaylistCategories =
          this.settings.enabledPlaylistCategories.filter(
            category => category !== name
          );
      } else {
        this.settings.enabledPlaylistCategories.push(name);
      }
    },
    updateToast(toast: ToastState) {
      this.toast = toast;
    },
    updateModal(payload: ModalUpdate) {
      const { modalName, key, value } = payload;
      if (modalName === 'addTrackToPlaylistModal') {
        if (key === 'show') {
          this.modals.addTrackToPlaylistModal.show = value;
        } else {
          this.modals.addTrackToPlaylistModal.selectedTrackID = value;
        }
      } else if (key === 'show') {
        this.modals.newPlaylistModal.show = value;
      } else {
        this.modals.newPlaylistModal.afterCreateAddTrackID = value;
      }
      if (key === 'show') {
        if (modalScrollLockTimer !== null) {
          clearTimeout(modalScrollLockTimer);
          modalScrollLockTimer = null;
        }
        // Wait for menu blur so its closing click cannot reach the modal.
        if (hasVisibleModal(this.modals)) {
          modalScrollLockTimer = setTimeout(() => {
            modalScrollLockTimer = null;
            if (hasVisibleModal(this.modals)) this.enableScrolling = false;
          }, 100);
        } else {
          this.enableScrolling = true;
        }
      }
    },
    toggleLyrics() {
      this.showLyrics = !this.showLyrics;
    },
    updateDailyTracks(dailyTracks: Track[]) {
      this.dailyTracks = dailyTracks;
    },
    updateLastfm(session: LastfmState) {
      this.lastfm = session;
    },
    updateShortcut({
      id,
      type,
      shortcut,
    }: {
      id: string;
      type: 'shortcut' | 'globalShortcut';
      shortcut: string;
    }) {
      const current = this.settings.shortcuts.find(item => item.id === id);
      if (!current) return;
      const updated = { ...current, [type]: shortcut };
      this.settings.shortcuts = this.settings.shortcuts.map(item =>
        item.id === id ? updated : item
      );
    },
    restoreDefaultShortcuts() {
      this.settings.shortcuts = defaultShortcuts.map(
        shortcut => ({ ...shortcut } as Shortcut)
      );
    },
    enableScrollingWith(status: boolean | null = null) {
      this.enableScrolling = resolveScrollingState(
        this.enableScrolling,
        status
      );
    },
    updateTitle(title: string) {
      this.title = title;
    },
    showToast(text: string) {
      if (this.toast.timer !== null) {
        clearTimeout(this.toast.timer);
      }
      this.toast = {
        show: true,
        text,
        timer: setTimeout(() => {
          this.toast = { show: false, text: this.toast.text, timer: null };
        }, 3200),
      };
    },
    async likeATrack(id: number) {
      if (!isAccountLoggedIn()) {
        this.showToast('此操作需要登录网易云账号');
        return;
      }
      const sessionEpoch = this.sessionEpoch;
      const like = !this.liked.songs.includes(id);
      try {
        await requestLikeTrack({ id, like });
        if (sessionEpoch !== this.sessionEpoch || !isAccountLoggedIn()) return;
        this.updateLikedXXX({
          name: 'songs',
          data: like
            ? [...this.liked.songs, id]
            : this.liked.songs.filter(songId => songId !== id),
        });
        await this.fetchLikedSongsWithDetails();
      } catch {
        this.showToast('操作失败，专辑下架或版权锁定');
      }
    },
    async fetchLikedSongs() {
      const sessionEpoch = this.sessionEpoch;
      const userId = this.data.user.userId;
      const ids = await fetchLikedSongIdsForUser(userId, {
        isLooseLoggedIn,
        isAccountLoggedIn,
        fetchLikedSongIds: userLikedSongsIDs,
      });
      if (
        ids &&
        sessionEpoch === this.sessionEpoch &&
        this.data.user.userId === userId
      ) {
        this.updateLikedXXX({ name: 'songs', data: ids });
      }
    },
    async fetchLikedSongsWithDetails() {
      const sessionEpoch = this.sessionEpoch;
      const playlistId = this.data.likedSongPlaylistID;
      if (playlistId === undefined) return;
      const result = await getPlaylistDetail(playlistId, true);
      if (
        sessionEpoch !== this.sessionEpoch ||
        this.data.likedSongPlaylistID !== playlistId
      ) {
        return;
      }
      const trackIds = result.playlist?.trackIds;
      if (!trackIds?.length) return;
      const details = await getTrackDetail(
        trackIds
          .slice(0, 12)
          .map((track: { id: number }) => track.id)
          .join(',')
      );
      if (
        sessionEpoch !== this.sessionEpoch ||
        this.data.likedSongPlaylistID !== playlistId
      ) {
        return;
      }
      this.updateLikedXXX({
        name: 'songsWithDetails',
        data: details.songs ?? [],
      });
    },
    async fetchLikedPlaylist() {
      if (!isLooseLoggedIn() || !isAccountLoggedIn()) return;
      const userId = this.data.user.userId;
      if (userId === undefined) return;
      const sessionEpoch = this.sessionEpoch;
      const result = await userPlaylist({
        uid: userId,
        limit: 2000,
        timestamp: Date.now(),
      });
      if (
        sessionEpoch !== this.sessionEpoch ||
        this.data.user.userId !== userId
      ) {
        return;
      }
      if (!result.playlist) return;
      this.updateLikedXXX({ name: 'playlists', data: result.playlist });
      if (result.playlist[0]) {
        this.updateData({
          key: 'likedSongPlaylistID',
          value: result.playlist[0].id,
        });
      }
    },
    async fetchLikedAlbums() {
      if (!isAccountLoggedIn()) return;
      const sessionEpoch = this.sessionEpoch;
      const result = await likedAlbums({
        limit: 2000,
      });
      if (sessionEpoch === this.sessionEpoch && result.data)
        this.updateLikedXXX({ name: 'albums', data: result.data });
    },
    async fetchLikedArtists() {
      if (!isAccountLoggedIn()) return;
      const sessionEpoch = this.sessionEpoch;
      const result = await likedArtists({
        limit: 2000,
      });
      if (sessionEpoch === this.sessionEpoch && result.data)
        this.updateLikedXXX({ name: 'artists', data: result.data });
    },
    async fetchLikedMVs() {
      if (!isAccountLoggedIn()) return;
      const sessionEpoch = this.sessionEpoch;
      const result = await likedMVs({
        limit: 1000,
      });
      if (sessionEpoch === this.sessionEpoch && result.data) {
        this.updateLikedXXX({ name: 'mvs', data: result.data });
      }
    },
    async fetchCloudDisk() {
      if (!isAccountLoggedIn()) return;
      const sessionEpoch = this.sessionEpoch;
      const result = await cloudDisk({
        limit: 1000,
      });
      if (sessionEpoch === this.sessionEpoch && result.data) {
        this.updateLikedXXX({ name: 'cloudDisk', data: result.data });
      }
    },
    async fetchPlayHistory() {
      if (!isAccountLoggedIn()) return;
      const userId = this.data.user.userId;
      if (userId === undefined) return;
      const sessionEpoch = this.sessionEpoch;
      const [allResult, weekResult] = await Promise.all([
        userPlayHistory({ uid: userId, type: 0 }),
        userPlayHistory({ uid: userId, type: 1 }),
      ]);
      if (
        sessionEpoch !== this.sessionEpoch ||
        this.data.user.userId !== userId
      ) {
        return;
      }
      const mapHistory = (items: typeof allResult.allData = []) =>
        items.map(item => ({ ...item.song, playCount: item.playCount }));
      this.updateLikedXXX({
        name: 'playHistory',
        data: {
          allData: mapHistory(allResult.allData),
          weekData: mapHistory(weekResult.weekData),
        },
      });
    },
    async fetchUserProfile() {
      if (!isAccountLoggedIn()) return false;
      const sessionEpoch = this.sessionEpoch;
      const result = await userAccount();
      if (sessionEpoch === this.sessionEpoch && result.code === 200) {
        this.updateData({ key: 'user', value: result.profile });
        return true;
      }
      return false;
    },
  },
});

export type AppStore = ReturnType<typeof useAppStore>;
