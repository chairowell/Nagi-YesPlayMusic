import { getAlbum } from '@/api/album';
import { nativeFetch } from '@/utils/nativeFetch';
import { getArtist } from '@/api/artist';
import { trackScrobble, trackUpdateNowPlaying } from '@/api/lastfm';
import { fetchPersonalFM } from '@/services/fmSource';
import { trashFM } from '@/services/librarySource';
import { getPlaylistDetail, intelligencePlaylist } from '@/api/playlist';
import { getLyric, getTrackDetail, scrobble } from '@/api/track';
import { getAppStore } from '@/stores/accessor';
import { isAccountLoggedIn } from '@/utils/auth';
import {
  cacheTrackSource,
  deleteTrackSource,
  getTrackDetailFromCache,
  getTrackSource,
  hasTrackSource,
} from '@/utils/db';
import { revokeBlobURLs } from '@/utils/cacheStats';
import { Howl, Howler } from 'howler';
import { markRaw } from 'vue';
import shuffle from 'lodash/shuffle';
import { decode as base642Buffer } from '@/utils/base64';
import {
  consumeQueuedTrack,
  getAdjacentTrack,
  getActiveTrackIndex,
  getUpcomingTrackIDs,
  pickRandomTrackID,
} from '@/utils/playerQueue';
import { sendDesktop } from '@/services/desktopTransport';
import { isDesktopRuntime } from '@/utils/runtime';
import { requestUnblockedSong } from '@/services/unblockMusicTransport';
import { reportNeteaseScrobble } from '@/utils/scrobbleReport';
import {
  createBlobAudioSource,
  createRemoteAudioSource,
  discardFailedCache,
  isCacheCorruptionError,
  resolveAudioSource,
  toHowlSourceOptions,
} from '@/utils/audioSource';
import { resolveNeteasePlaybackSource } from '@/services/playbackSource';
import {
  createTrackSwitchGuard,
  runLatestTrackSwitch,
} from '@/utils/trackSwitch';
import {
  createNextTrackPrefetcher,
  warmTrackArtwork,
} from '@/utils/trackPrefetch';
import { UPCOMING_ARTWORK_COUNT, buildArtworkURL } from '@/utils/artwork';
import { resolvePlaybackDuration } from '@/utils/playbackDuration';
import { startHowlerSeek } from '@/utils/playbackSeek';
import { getHowlerMediaNode } from '@/utils/howlerMedia';
import {
  decodeFlacToWavBlob,
  discardPreciseWav,
  parseFlacStreamInfo,
  requestPreciseWavURL,
} from '@/utils/pcmSeekSource';
import { createPreciseSeekUpgrader } from '@/utils/preciseSeekUpgrade';
import { sendConfiguredDiscordPresence } from '@/services/desktopSettings';
import {
  createSharedAudioProxy,
  deleteSharedCachedAudio,
  findSharedCachedAudio,
  importTrackIntoSharedCache,
  isSharedAudioProxyURL,
  prefetchSharedAudio,
  reportSharedCacheFailure,
  shouldUseSharedAudioProxy,
} from '@/services/sharedCache';
import type { Track } from '@/types/domain';
import type { AudioSource, AudioSourceOrigin } from '@/utils/audioSource';
import { isAudioSourceOrigin } from '@/utils/audioSource';
import type { TrackSwitchGuard } from '@/utils/trackSwitch';
import type { PreciseSeekUpgrader } from '@/utils/preciseSeekUpgrade';

const PLAY_PAUSE_FADE_DURATION = 200;

const INDEX_IN_PLAY_NEXT = -1;

/**
 * @readonly
 * @enum {string}
 */
const UNPLAYABLE_CONDITION = {
  PLAY_NEXT_TRACK: 'playNextTrack',
  PLAY_PREV_TRACK: 'playPrevTrack',
} as const;

type RepeatMode = 'off' | 'on' | 'one';
type UnplayableCondition =
  (typeof UNPLAYABLE_CONDITION)[keyof typeof UNPLAYABLE_CONDITION];

interface PlaylistSource {
  type: string;
  id: number | string;
}

interface CurrentSourceMeta {
  origin: string | null;
  format: string | null;
  url: string;
}

interface RetryAudioSourceOptions {
  failedHowler: Howl;
  failedSource: AudioSource;
  autoplay: boolean;
  ifUnplayableThen: UnplayableCondition;
  errCode: unknown;
}

interface PersistedPlayerState {
  _progress: number;
  _enabled: boolean;
  _repeatMode: RepeatMode;
  _shuffle: boolean;
  _reversed: boolean;
  _volume: number;
  _volumeBeforeMuted: number;
  _list: number[];
  _current: number;
  _shuffledList: number[];
  _shuffledCurrent: number;
  _playlistSource: PlaylistSource;
  _currentTrack: Track;
  _playNextList: number[];
  _isPersonalFM: boolean;
  _personalFMTrack: Track;
  _personalFMNextTrack?: Track;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

function isTrack(value: unknown): value is Track {
  return isRecord(value) && typeof value['id'] === 'number';
}

function isNumberArray(value: unknown): value is number[] {
  return Array.isArray(value) && value.every(item => typeof item === 'number');
}

function isRepeatMode(value: unknown): value is RepeatMode {
  return value === 'off' || value === 'on' || value === 'one';
}

function isPlaylistSource(value: unknown): value is PlaylistSource {
  return (
    isRecord(value) &&
    typeof value['type'] === 'string' &&
    (typeof value['id'] === 'number' || typeof value['id'] === 'string')
  );
}

const delay = (ms: number): Promise<void> =>
  new Promise(resolve => {
    setTimeout(() => {
      resolve();
    }, ms);
  });

function setTitle(track: Track | null): void {
  const artist = track?.ar?.[0]?.name ?? '';
  document.title = track
    ? `${track.name ?? ''} · ${artist} - YesPlayMusic`
    : 'YesPlayMusic';
  if (isDesktopRuntime) {
    void sendDesktop('updateTrayTooltip', document.title);
  }
  getAppStore().updateTitle(document.title);
}

export default class Player {
  _playing: boolean;
  _progress: number;
  _audioDuration: number;
  _seeking: boolean;
  _enabled: boolean;
  _repeatMode: RepeatMode;
  _shuffle: boolean;
  _reversed: boolean;
  _volume: number;
  _volumeBeforeMuted: number;
  _personalFMLoading: boolean;
  _personalFMNextLoading: boolean;
  _list: number[];
  _current: number;
  _shuffledList: number[];
  _shuffledCurrent: number;
  _playlistSource: PlaylistSource;
  _currentTrack: Track;
  _playNextList: number[];
  _isPersonalFM: boolean;
  _personalFMTrack: Track;
  _personalFMNextTrack: Track | undefined;
  createdBlobRecords: string[];
  _howler: Howl | null;
  declare _initialized: boolean;
  declare readonly _trackSwitchGuard: Readonly<TrackSwitchGuard>;
  declare _pendingSeekCancel: (() => void) | null;
  declare _currentSourceMeta: CurrentSourceMeta | null;
  declare _seekToken: number;
  declare _pausePending: boolean;
  declare _preciseSeekUpgrader: PreciseSeekUpgrader | null;
  declare readonly _nextTrackPrefetcher: ReturnType<
    typeof createNextTrackPrefetcher<Track>
  >;

  constructor() {
    this._playing = false;
    this._progress = 0;
    this._audioDuration = 0;
    this._seeking = false;
    this._enabled = false;
    this._repeatMode = 'off'; // off | on | one
    this._shuffle = false; // true | false
    this._reversed = false;
    this._volume = 1; // 0 to 1
    this._volumeBeforeMuted = 1;
    this._personalFMLoading = false;
    this._personalFMNextLoading = false;

    this._list = [];
    this._current = 0;
    this._shuffledList = [];
    this._shuffledCurrent = 0;
    this._playlistSource = { type: 'album', id: 123 };
    this._currentTrack = { id: 86827685 };
    this._playNextList = [];
    this._isPersonalFM = false;
    this._personalFMTrack = { id: 0 };
    this._personalFMNextTrack = {
      id: 0,
    };

    /**
     * The blob records for cleanup.
     *
     * @private
     * @type {string[]}
     */
    this.createdBlobRecords = [];

    // howler (https://github.com/goldfire/howler.js)
    this._howler = null;
    Object.defineProperty(this, '_howler', {
      enumerable: false,
    });

    Object.defineProperty(this, '_initialized', {
      enumerable: false,
      value: false,
      writable: true,
    });

    Object.defineProperty(this, '_trackSwitchGuard', {
      enumerable: false,
      value: createTrackSwitchGuard(),
    });

    Object.defineProperty(this, '_pendingSeekCancel', {
      enumerable: false,
      value: null,
      writable: true,
    });

    // Source format decides whether FLAC seeks need a precise WAV upgrade.
    Object.defineProperty(this, '_currentSourceMeta', {
      enumerable: false,
      value: null,
      writable: true,
    });

    // A generation counter prevents stale upgrades from restoring old positions.
    Object.defineProperty(this, '_seekToken', {
      enumerable: false,
      value: 0,
      writable: true,
    });

    // Preserve pending pause intent across precise-source replacement.
    Object.defineProperty(this, '_pausePending', {
      enumerable: false,
      value: false,
      writable: true,
    });

    Object.defineProperty(this, '_preciseSeekUpgrader', {
      enumerable: false,
      value: null,
      writable: true,
    });

    // Vue proxies violate invariants on readonly properties unless this is raw.
    Object.defineProperty(this, '_nextTrackPrefetcher', {
      enumerable: false,
      value: markRaw(
        createNextTrackPrefetcher({
          loadTrack: id =>
            getTrackDetail(id).then(
              data =>
                data.songs.find(track => Number(track.id) === Number(id)) ??
                null
            ),
          loadLyric: id => getLyric(id),
          warmArtwork: track => warmTrackArtwork(track),
          cacheAudio: (track, isCurrent) =>
            this._cachePrefetchedAudio(track, isCurrent),
        })
      ),
    });
  }

  get repeatMode() {
    return this._repeatMode;
  }
  set repeatMode(mode: RepeatMode) {
    if (this._isPersonalFM) return;
    if (!['off', 'on', 'one'].includes(mode)) {
      console.warn("repeatMode: invalid args, must be 'on' | 'off' | 'one'");
      return;
    }
    this._repeatMode = mode;
    this._prefetchNextTrack();
  }
  get shuffle() {
    return this._shuffle;
  }
  set shuffle(shuffle: boolean) {
    if (this._isPersonalFM) return;
    if (shuffle !== true && shuffle !== false) {
      console.warn('shuffle: invalid args, must be Boolean');
      return;
    }
    this._shuffle = shuffle;
    if (shuffle) {
      this._shuffleTheList();
    }
    // Keep the active queue index aligned with the selected track.
    this.current = this.list.indexOf(this.currentTrackID);
    this._prefetchNextTrack();
  }
  get reversed() {
    return this._reversed;
  }
  set reversed(reversed: boolean) {
    if (this._isPersonalFM) return;
    if (reversed !== true && reversed !== false) {
      console.warn('reversed: invalid args, must be Boolean');
      return;
    }
    console.log('changing reversed to:', reversed);
    this._reversed = reversed;
    this._prefetchNextTrack();
  }
  get volume() {
    return this._volume;
  }
  set volume(volume: number) {
    this._volume = volume;
    this._howler?.volume(volume);
  }
  get list() {
    return this.shuffle ? this._shuffledList : this._list;
  }
  set list(list: number[]) {
    this._list = list;
  }
  get current() {
    return this.shuffle ? this._shuffledCurrent : this._current;
  }
  set current(current: number) {
    if (this.shuffle) {
      this._shuffledCurrent = current;
    } else {
      this._current = current;
    }
  }
  get enabled() {
    return this._enabled;
  }
  get playing() {
    return this._playing;
  }
  get seeking() {
    return this._seeking;
  }
  get currentTrack() {
    return this._currentTrack;
  }
  get currentTrackID() {
    return this._currentTrack?.id ?? 0;
  }
  get playlistSource() {
    return this._playlistSource;
  }
  get playNextList() {
    return this._playNextList;
  }
  get isPersonalFM() {
    return this._isPersonalFM;
  }
  get personalFMTrack() {
    return this._personalFMTrack;
  }
  get currentTrackDuration() {
    return resolvePlaybackDuration(this._currentTrack.dt, this._audioDuration);
  }
  get currentAudioSourceUrl() {
    return this._currentSourceMeta?.url ?? '';
  }
  get progress() {
    return this._progress;
  }
  set progress(value: number) {
    this.seek(value);
  }
  get isCurrentTrackLiked() {
    return getAppStore().liked.songs.includes(this.currentTrack.id);
  }

  initialize() {
    if (this._initialized) return;
    this._initialized = true;
    this._init();
  }

  _init() {
    this._loadSelfFromLocalStorage();
    this._howler?.volume(this.volume);

    if (this._enabled) {
      // Restore the current track.
      const savedTrackTime = Number(
        localStorage.getItem('playerCurrentTrackTime') ?? 0
      );
      this._replaceCurrentTrack(this.currentTrackID, false).then(replaced => {
        // A stale initialization must not seek a newer track.
        if (!replaced || !Number.isFinite(savedTrackTime)) return;
        this.seek(savedTrackTime);
      }); // update audio source and init howler
      this._initMediaSession();
    }

    this._setIntervals();

    // Initialize personal FM.
    if (
      this._personalFMTrack.id === 0 ||
      this._personalFMNextTrack?.id === 0 ||
      this._personalFMTrack.id === this._personalFMNextTrack?.id
    ) {
      fetchPersonalFM().then(result => {
        const currentTrack = result.data[0];
        if (currentTrack) this._personalFMTrack = currentTrack;
        this._personalFMNextTrack = result.data[1];
        return this._personalFMTrack;
      });
    }
  }
  _setPlaying(isPlaying: boolean): void {
    this._playing = isPlaying;
  }
  _setIntervals() {
    // TODO: Avoid overwriting progress changed outside this timer.
    setInterval(() => {
      if (this._howler !== null && !this._seeking) {
        this._progress = this._howler.seek();
        localStorage.setItem('playerCurrentTrackTime', String(this._progress));
      }
      if (isDesktopRuntime) {
        void nativeFetch('/api/native/player-info', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            currentTrack: this._isPersonalFM
              ? this._personalFMTrack
              : this._currentTrack,
            progress: this._progress,
            playing: this._playing,
          }),
        }).catch(() => undefined);
      }
    }, 1000);
  }
  _getNextTrack(): [number | undefined, number] {
    if (this._playNextList.length > 0) {
      const trackID = this._playNextList[0];
      return [trackID, INDEX_IN_PLAY_NEXT];
    }

    const direction = this._reversed ? -1 : 1;
    return getAdjacentTrack(
      this.list,
      this.current,
      direction,
      this.repeatMode === 'on'
    );
  }
  _getPrevTrack(): [number | undefined, number] {
    const direction = this._reversed ? 1 : -1;
    return getAdjacentTrack(
      this.list,
      this.current,
      direction,
      this.repeatMode === 'on'
    );
  }
  async _shuffleTheList(
    firstTrackID: number | 'first' = this.currentTrackID
  ): Promise<void> {
    let list = this._list.filter(tid => tid !== firstTrackID);
    if (firstTrackID === 'first') list = this._list;
    this._shuffledList = shuffle(list);
    if (firstTrackID !== 'first') this._shuffledList.unshift(firstTrackID);
  }
  async _scrobble(
    track: Track,
    time: number | undefined,
    completed = false
  ): Promise<void> {
    const artist = track.ar?.[0]?.name ?? '';
    console.debug(
      `[debug][Player.ts] scrobble track 👉 ${
        track.name ?? ''
      } by ${artist} 👉 time:${time ?? 0} completed: ${completed}`
    );
    const trackDuration = Math.trunc((track.dt ?? 0) / 1000);
    const listenedTime = completed ? trackDuration : Math.trunc(time ?? 0);
    reportNeteaseScrobble(
      {
        id: track.id,
        sourceid: this.playlistSource.id,
        time: listenedTime,
      },
      scrobble
    );
    if (
      getAppStore().lastfm['key'] !== undefined &&
      (listenedTime >= trackDuration / 2 || listenedTime >= 240)
    ) {
      const timestamp = Math.trunc(Date.now() / 1000) - listenedTime;
      trackScrobble({
        artist,
        track: track.name ?? '',
        timestamp,
        album: track.al?.name ?? '',
        trackNumber: typeof track['no'] === 'number' ? track['no'] : undefined,
        duration: trackDuration,
      });
    }
  }
  _playAudioSource(
    source: AudioSource,
    autoplay = true,
    ifUnplayableThen: UnplayableCondition = UNPLAYABLE_CONDITION.PLAY_NEXT_TRACK
  ): void {
    // Cancel pending seeks before every Howler replacement to unfreeze timing.
    this._pendingSeekCancel?.();
    this._pendingSeekCancel = null;
    this._seeking = false;
    // Replacement invalidates the previous instance's fade callbacks.
    this._pausePending = false;
    // Release temporary WAV storage when leaving the precise source.
    if (
      this._currentSourceMeta?.origin === 'precise-wav' &&
      source.origin !== 'precise-wav'
    ) {
      void discardPreciseWav();
    }
    Howler.unload();
    let handlingLoadError = false;
    const howlerOptions = toHowlSourceOptions(source);
    let howler: Howl | null = null;
    const handleLoadError = (_soundID: number, errCode: unknown): void => {
      if (handlingLoadError) return;
      handlingLoadError = true;
      // Defer synchronous constructor errors until the instance is assigned.
      queueMicrotask(() => {
        if (!howler) return;
        void this._retryAudioSourceAfterFailure({
          failedHowler: howler,
          failedSource: source,
          autoplay,
          ifUnplayableThen,
          errCode,
        });
      });
    };
    howler = new Howl({
      ...howlerOptions,
      html5: true,
      preload: true,
      onload: () => {
        const loadedHowler = howler;
        if (loadedHowler && this._howler === loadedHowler) {
          this._audioDuration = loadedHowler.duration();
        }
        Promise.resolve(source.cacheAfterLoad?.()).catch(error => {
          console.warn('[Player] 音频播放成功，但写入缓存失败', error);
        });
      },
      onend: () => {
        if (this._howler !== howler) return;
        this._nextTrackCallback();
      },
      // Register before construction because Howler may fail synchronously.
      onloaderror: handleLoadError,
    });
    this._howler = howler;
    this._currentSourceMeta = {
      origin: source.origin ?? null,
      format: source.format ?? null,
      url: source.url,
    };
    if (autoplay) {
      this.play();
      if (this._currentTrack.name) {
        setTitle(this._currentTrack);
      }
    }
    this.setOutputDevice();
  }

  async _retryAudioSourceAfterFailure({
    failedHowler,
    failedSource,
    autoplay,
    ifUnplayableThen,
    errCode,
  }: RetryAudioSourceOptions): Promise<void> {
    if (this._howler !== failedHowler) return;
    const failedTrack = this.currentTrack;
    // A dead proxy URL must not be re-resolved into the same dead proxy URL,
    // otherwise every fallback fails too and the whole playlist skips forever.
    if (isSharedAudioProxyURL(failedSource.url)) {
      reportSharedCacheFailure();
    }
    if (failedSource.origin === 'cache' && isCacheCorruptionError(errCode)) {
      await discardFailedCache(
        async trackID => {
          const tasks: Promise<unknown>[] = [deleteTrackSource(trackID)];
          if (getAppStore().settings.shareCacheWithYpm) {
            tasks.push(
              deleteSharedCachedAudio(
                Number(trackID),
                getAppStore().settings.musicQuality
              )
            );
          }
          await Promise.all(tasks);
          return true;
        },
        failedTrack.id,
        error =>
          console.warn('[Player] 删除损坏缓存失败，继续尝试备用源', error)
      );
    }
    if (failedSource.url.startsWith('blob:')) {
      URL.revokeObjectURL(failedSource.url);
      this.createdBlobRecords = this.createdBlobRecords.filter(
        url => url !== failedSource.url
      );
    }

    let fallback: AudioSource | null | undefined;
    if (failedSource.origin === 'unm' && failedSource.provider) {
      const excludedProviders = new Set(failedSource.excludedProviders || []);
      if (!excludedProviders.has(failedSource.provider)) {
        excludedProviders.add(failedSource.provider);
        fallback = await resolveAudioSource(
          {
            unm: () =>
              this._getAudioSourceFromUnblockMusic(failedTrack, [
                ...excludedProviders,
              ]),
          },
          'netease',
          (_origin, error) =>
            console.warn('[Player] 备用 provider 请求失败', error)
        );
      }
    } else {
      // Re-resolve an expired temporary WAV; the cached FLAC remains valid.
      fallback = await this._getAudioSource(failedTrack, {
        afterOrigin:
          failedSource.origin === 'precise-wav' ||
          !isAudioSourceOrigin(failedSource.origin)
            ? null
            : failedSource.origin,
      });
    }
    if (
      this.currentTrackID !== failedTrack.id ||
      this._howler !== failedHowler
    ) {
      return;
    }
    if (fallback) {
      console.warn(
        `[Player] ${failedSource.origin} 音源加载失败 (${errCode})，改用 ${fallback.origin}`
      );
      // Retry precise sources from current playback state to avoid silent playback.
      this._playAudioSource(
        fallback,
        autoplay || this._playing,
        ifUnplayableThen
      );
      return;
    }

    getAppStore().showToast(
      `无法播放 ${failedTrack.name}: 网易云及可用备用源均不可用`
    );
    switch (ifUnplayableThen) {
      case UNPLAYABLE_CONDITION.PLAY_PREV_TRACK:
        this.playPrevTrack();
        break;
      case UNPLAYABLE_CONDITION.PLAY_NEXT_TRACK:
      default:
        this._playNextTrack(this._isPersonalFM);
        break;
    }
  }

  _getAudioSourceBlobURL(data: BlobPart, origin = 'cache'): AudioSource {
    const source = createBlobAudioSource(
      data,
      blob => URL.createObjectURL(blob),
      origin
    );

    // Clean up the previous object URLs since we've created a new one.
    // Revoke object URLs can release the memory taken by a Blob,
    // which occupied a large proportion of memory.
    revokeBlobURLs(this.createdBlobRecords, url => URL.revokeObjectURL(url));

    // Then, we replace the createBlobRecords with new one with
    // our newly created object URL.
    this.createdBlobRecords = [source.url];

    return source;
  }
  async _getAudioSourceFromCache(id: string): Promise<AudioSource | null> {
    const settings = getAppStore().settings;
    if (settings.shareCacheWithYpm) {
      const shared = await findSharedCachedAudio(
        Number(id),
        settings.musicQuality
      );
      if (shared) return shared;
    }
    const track = await getTrackSource(id);
    return track ? this._getAudioSourceBlobURL(track.source, 'cache') : null;
  }
  async _getAudioSourceFromNetease(track: Track): Promise<AudioSource | null> {
    if (isAccountLoggedIn()) {
      // Candidate matching, free-trial refusal and rejected-vs-unavailable
      // classification live server-side (core::ncm), shared with the TUI.
      const audio = await resolveNeteasePlaybackSource(track.id);
      if (!audio) return null;
      const source = audio.url.replace(/^http:/, 'https:');
      const settings = getAppStore().settings;
      if (await shouldUseSharedAudioProxy(settings.shareCacheWithYpm)) {
        return createSharedAudioProxy({
          track,
          quality: settings.musicQuality,
          source,
          format: audio.codec,
          actualBitrate: audio.actualBitrate,
          cache: settings.automaticallyCacheSongs,
          origin: 'netease',
        });
      }
      return createRemoteAudioSource(source, {
        origin: 'netease',
        format: audio.codec,
        cacheAfterLoad: settings.automaticallyCacheSongs
          ? () => cacheTrackSource(track, source, audio.actualBitrate)
          : null,
      });
    } else {
      const source = `https://music.163.com/song/media/outer/url?id=${track.id}`;
      const settings = getAppStore().settings;
      if (await shouldUseSharedAudioProxy(settings.shareCacheWithYpm)) {
        return createSharedAudioProxy({
          track,
          quality: settings.musicQuality,
          source,
          format: 'mp3',
          actualBitrate: 128000,
          cache: settings.automaticallyCacheSongs,
          origin: 'netease',
        });
      }
      return createRemoteAudioSource(source, {
        origin: 'netease',
        format: 'mp3',
      });
    }
  }
  async _getAudioSourceFromUnblockMusic(
    track: Track,
    excludedProviders: string[] = []
  ): Promise<AudioSource | null> {
    console.debug(`[debug][Player.ts] _getAudioSourceFromUnblockMusic`);

    if (
      !isDesktopRuntime ||
      getAppStore().settings.enableUnblockNeteaseMusic === false
    ) {
      return null;
    }

    /**
     *
     * @param {string=} searchMode
     * @returns {import("@unblockneteasemusic/rust-napi").SearchMode}
     */
    const determineSearchMode = (searchMode?: string): 0 | 1 => {
      /**
       * FastFirst = 0
       * OrderFirst = 1
       */
      switch (searchMode) {
        case 'fast-first':
          return 0;
        case 'order-first':
          return 1;
        default:
          return 0;
      }
    };

    const retrieveSongInfo = await requestUnblockedSong(
      getAppStore().settings.unmSource,
      track,
      {
        enableFlac: getAppStore().settings.unmEnableFlac || null,
        proxyUri: getAppStore().settings.unmProxyUri || null,
        searchMode: determineSearchMode(getAppStore().settings.unmSearchMode),
        excludedSources: excludedProviders,
        config: {
          'joox:cookie': getAppStore().settings.unmJooxCookie || null,
          'qq:cookie': getAppStore().settings.unmQQCookie || null,
          'ytdl:exe': getAppStore().settings.unmYtDlExe || null,
        },
      }
    );

    if (!retrieveSongInfo) {
      return null;
    }

    if (retrieveSongInfo.source !== 'bilibili') {
      const settings = getAppStore().settings;
      if (await shouldUseSharedAudioProxy(settings.shareCacheWithYpm)) {
        return createSharedAudioProxy({
          track,
          quality: settings.musicQuality,
          source: retrieveSongInfo.url,
          format: 'mp3',
          actualBitrate: 128000,
          cache: settings.automaticallyCacheSongs,
          origin: 'unm',
          provider: retrieveSongInfo.source,
          excludedProviders,
        });
      }
      return createRemoteAudioSource(retrieveSongInfo.url, {
        origin: 'unm',
        provider: retrieveSongInfo.source,
        excludedProviders,
        // A format hint lets Howler accept signed URLs without extensions.
        fallbackFormat: 'mp3',
        cacheAfterLoad: getAppStore().settings.automaticallyCacheSongs
          ? () =>
              cacheTrackSource(
                track,
                retrieveSongInfo.url,
                128000,
                `unm:${retrieveSongInfo.source}`
              )
          : null,
      });
    }

    const buffer = base642Buffer(retrieveSongInfo.url);
    const source = this._getAudioSourceBlobURL(buffer, 'unm');
    source.provider = retrieveSongInfo.source;
    source.excludedProviders = excludedProviders;
    const settings = getAppStore().settings;
    const useSharedCache = await shouldUseSharedAudioProxy(
      settings.shareCacheWithYpm
    );
    if (settings.automaticallyCacheSongs) {
      source.cacheAfterLoad = useSharedCache
        ? () =>
            importTrackIntoSharedCache(
              track,
              buffer,
              128000,
              settings.musicQuality,
              source.format
            )
        : () =>
            cacheTrackSource(
              track,
              `data:${source.mimeType};base64,${retrieveSongInfo.url}`,
              128000,
              'unm:bilibili'
            );
    }
    return source;
  }
  async _getAudioSource(
    track: Track,
    { afterOrigin = null }: { afterOrigin?: AudioSourceOrigin | null } = {}
  ): Promise<AudioSource | null> {
    const resolvers = {
      cache: () => this._getAudioSourceFromCache(String(track.id)),
      netease: () => this._getAudioSourceFromNetease(track),
      unm: () => this._getAudioSourceFromUnblockMusic(track),
    };
    return resolveAudioSource(resolvers, afterOrigin, (origin, error) => {
      console.warn(`[Player] ${origin} 音源请求失败，继续尝试下一层`, error);
    });
  }
  _replaceCurrentTrack(
    id: number,
    autoplay = true,
    ifUnplayableThen: UnplayableCondition = UNPLAYABLE_CONDITION.PLAY_NEXT_TRACK
  ): Promise<boolean> {
    if (autoplay && this._currentTrack.name) {
      this._scrobble(this.currentTrack, this._howler?.seek());
    }
    return runLatestTrackSwitch(this._trackSwitchGuard, {
      onBegin: () => {
        // Stop old audio before network work so it cannot outlive the old artwork.
        const previousHowler = this._howler;
        this._pendingSeekCancel?.();
        this._pendingSeekCancel = null;
        this._seeking = false;
        this._howler = null;
        previousHowler?.stop();
        Howler.unload();
        this._progress = 0;
        this._audioDuration = 0;
        localStorage.setItem('playerCurrentTrackTime', '0');
        if (this._playing) this._setPlaying(false);
      },
      loadTrack: () =>
        getTrackDetail(id).then(data => {
          const track = data.songs[0];
          if (!track) throw new Error(`歌曲详情响应为空：${id}`);
          return track;
        }),
      commitTrack: track => {
        this._currentTrack = track;
        this._updateMediaSessionMetaData(track);
        // Start artwork prefetch before source resolution completes.
        this._warmUpcomingArtwork();
      },
      loadSource: track => this._getAudioSource(track),
      commitSource: source => {
        this._playAudioSource(source, autoplay, ifUnplayableThen);
        this._prefetchNextTrack();
      },
      onMissingSource: track => {
        getAppStore().showToast(`无法播放 ${track.name}`);
        switch (ifUnplayableThen) {
          case UNPLAYABLE_CONDITION.PLAY_NEXT_TRACK:
            this._playNextTrack(this.isPersonalFM);
            break;
          case UNPLAYABLE_CONDITION.PLAY_PREV_TRACK:
            this.playPrevTrack();
            break;
          default:
            getAppStore().showToast(
              `undefined Unplayable condition: ${ifUnplayableThen}`
            );
            break;
        }
      },
    });
  }
  async _cachePrefetchedAudio(
    track: Track,
    isCurrent: () => boolean
  ): Promise<void> {
    if (!isCurrent() || (await hasTrackSource(track.id))) return;
    if (!isCurrent()) return;

    // Recheck before audio download so stale queues cannot consume bandwidth.
    const source = await this._getAudioSourceFromNetease(track);
    if (!isCurrent()) return;
    if (source && isSharedAudioProxyURL(source.url)) {
      await prefetchSharedAudio(source.url);
    } else {
      await source?.cacheAfterLoad?.();
    }
  }
  /**
   * Warm upcoming artwork from locally cached track details. Remote detail requests
   * would compete with audio and arrive too late to help.
   */
  _warmUpcomingArtwork() {
    const ids = this._isPersonalFM
      ? [this._personalFMNextTrack?.id].filter(Boolean)
      : getUpcomingTrackIDs(
          this.list,
          this.current,
          this._reversed ? -1 : 1,
          this.repeatMode === 'on',
          UPCOMING_ARTWORK_COUNT
        );
    if (!ids.length) return;

    for (const id of ids) {
      void getTrackDetailFromCache([String(id)])
        .then(result => {
          const track = result?.songs?.[0];
          if (track) warmTrackArtwork(track);
        })
        .catch(() => {});
    }
  }
  _prefetchNextTrack() {
    const nextTrackID = this._isPersonalFM
      ? this._personalFMNextTrack?.id ?? 0
      : this._getNextTrack()[0];
    if (!nextTrackID || this._personalFMTrack.id == nextTrackID) {
      this._nextTrackPrefetcher.clear();
      return Promise.resolve(null);
    }

    return this._nextTrackPrefetcher.prefetch(nextTrackID, {
      includeAudio: getAppStore().settings.automaticallyCacheSongs === true,
    });
  }
  _loadSelfFromLocalStorage() {
    const rawPlayer = localStorage.getItem('player');
    if (!rawPlayer) return;

    let player: unknown;
    try {
      player = JSON.parse(rawPlayer);
    } catch {
      console.warn('[Player] 忽略无法解析的本地播放器状态');
      return;
    }
    if (!isRecord(player)) return;

    const finiteNumber = (key: keyof PersistedPlayerState): number | null => {
      const value = player[key];
      return typeof value === 'number' && Number.isFinite(value) ? value : null;
    };
    const progress = finiteNumber('_progress');
    const volume = finiteNumber('_volume');
    const volumeBeforeMuted = finiteNumber('_volumeBeforeMuted');
    const current = finiteNumber('_current');
    const shuffledCurrent = finiteNumber('_shuffledCurrent');
    if (progress !== null) this._progress = progress;
    if (volume !== null) this._volume = volume;
    if (volumeBeforeMuted !== null) this._volumeBeforeMuted = volumeBeforeMuted;
    if (current !== null) this._current = current;
    if (shuffledCurrent !== null) this._shuffledCurrent = shuffledCurrent;

    if (typeof player['_enabled'] === 'boolean')
      this._enabled = player['_enabled'];
    if (typeof player['_shuffle'] === 'boolean')
      this._shuffle = player['_shuffle'];
    if (typeof player['_reversed'] === 'boolean')
      this._reversed = player['_reversed'];
    if (typeof player['_isPersonalFM'] === 'boolean')
      this._isPersonalFM = player['_isPersonalFM'];
    if (isRepeatMode(player['_repeatMode']))
      this._repeatMode = player['_repeatMode'];
    if (isNumberArray(player['_list'])) this._list = player['_list'];
    if (isNumberArray(player['_shuffledList']))
      this._shuffledList = player['_shuffledList'];
    if (isNumberArray(player['_playNextList']))
      this._playNextList = player['_playNextList'];
    if (isPlaylistSource(player['_playlistSource']))
      this._playlistSource = player['_playlistSource'];
    if (isTrack(player['_currentTrack']))
      this._currentTrack = player['_currentTrack'];
    if (isTrack(player['_personalFMTrack']))
      this._personalFMTrack = player['_personalFMTrack'];
    if (isTrack(player['_personalFMNextTrack']))
      this._personalFMNextTrack = player['_personalFMNextTrack'];
  }
  _initMediaSession() {
    if ('mediaSession' in navigator) {
      navigator.mediaSession.setActionHandler('play', () => {
        this.play();
      });
      navigator.mediaSession.setActionHandler('pause', () => {
        this.pause();
      });
      navigator.mediaSession.setActionHandler('previoustrack', () => {
        this.playPrevTrack();
      });
      navigator.mediaSession.setActionHandler('nexttrack', () => {
        this._playNextTrack(this.isPersonalFM);
      });
      navigator.mediaSession.setActionHandler('stop', () => {
        this.pause();
      });
      navigator.mediaSession.setActionHandler('seekto', event => {
        this.seek(event.seekTime);
        this._updateMediaSessionPositionState();
      });
      navigator.mediaSession.setActionHandler('seekbackward', event => {
        this.seek(this.seek() - (event.seekOffset || 10));
        this._updateMediaSessionPositionState();
      });
      navigator.mediaSession.setActionHandler('seekforward', event => {
        this.seek(this.seek() + (event.seekOffset || 10));
        this._updateMediaSessionPositionState();
      });
    }
  }
  _updateMediaSessionMetaData(track: Track): void {
    const artists = track.ar?.map(artist => artist.name ?? '') ?? [];
    const album = track.al;
    void this._syncDesktopMediaMetadata(track, artists);
    if ('mediaSession' in navigator === false) return;

    const metadata: MediaMetadataInit = {
      title: track.name ?? '',
      artist: artists.join(','),
      album: album?.name ?? '',
      artwork: [
        {
          src: buildArtworkURL(album?.picUrl, 224),
          type: 'image/jpg',
          sizes: '224x224',
        },
        {
          src: buildArtworkURL(album?.picUrl, 512),
          type: 'image/jpg',
          sizes: '512x512',
        },
      ],
    };

    navigator.mediaSession.metadata = new window.MediaMetadata(metadata);
  }
  async _syncDesktopMediaMetadata(
    track: Track,
    artists: string[]
  ): Promise<void> {
    if (!isDesktopRuntime) return;

    let lyrics: { title: string; artists: string[]; content: string } | null =
      null;
    if (getAppStore().settings.enableOsdlyricsSupport) {
      try {
        const result = await getLyric(track.id);
        const content = result.lrc?.lyric?.trim();
        if (content) lyrics = { title: track.name ?? '', artists, content };
      } catch (error) {
        console.warn('[Player] OSDLyrics fetch failed', error);
      }
    }
    if (this.currentTrackID !== track.id) return;

    await sendDesktop('mediaMetadata', {
      trackId: String(track.id),
      title: track.name ?? '',
      album: track.al?.name ?? '',
      artists,
      artworkUrl: track.al?.picUrl
        ? buildArtworkURL(track.al.picUrl, 512)
        : null,
      mediaUrl: `/trackid/${track.id}`,
      lengthSeconds: Math.max(0, (track.dt ?? 0) / 1000),
      lyrics,
    });
  }
  syncDesktopMediaMetadata(): void {
    if (!this.currentTrack.name) return;
    const artists =
      this.currentTrack.ar?.map(artist => artist.name ?? '') ?? [];
    void this._syncDesktopMediaMetadata(this.currentTrack, artists);
  }
  _updateMediaSessionPositionState() {
    if ('mediaSession' in navigator === false) {
      return;
    }
    if ('setPositionState' in navigator.mediaSession) {
      navigator.mediaSession.setPositionState({
        duration: Math.max(1, Math.trunc((this.currentTrack.dt ?? 0) / 1000)),
        playbackRate: 1.0,
        position: this.seek(),
      });
    }
  }
  _nextTrackCallback() {
    this._scrobble(this._currentTrack, 0, true);
    if (!this.isPersonalFM && this.repeatMode === 'one') {
      this._replaceCurrentTrack(this.currentTrackID);
    } else {
      this._playNextTrack(this.isPersonalFM);
    }
  }
  async _loadPersonalFMNextTrack(): Promise<
    readonly [boolean, Track | undefined]
  > {
    if (this._personalFMNextLoading) {
      return [false, undefined] as const;
    }
    this._personalFMNextLoading = true;
    try {
      const result = await fetchPersonalFM();
      this._personalFMNextTrack = result.data[0];
      if (this._personalFMNextTrack) this._prefetchNextTrack();
      return [true, this._personalFMNextTrack] as const;
    } catch {
      this._personalFMNextTrack = undefined;
      return [false, undefined] as const;
    } finally {
      this._personalFMNextLoading = false;
    }
  }
  _playNextTrack(isPersonal: boolean): void {
    if (isPersonal) {
      this.playNextFMTrack();
    } else {
      this.playNextTrack();
    }
  }

  appendTrack(trackID: number): void {
    this.list.push(trackID);
  }
  playNextTrack() {
    // TODO: Expose a loading state while switching tracks.
    const [trackID, index] = this._getNextTrack();
    if (trackID === undefined) {
      this._howler?.stop();
      this._setPlaying(false);
      return false;
    }
    let next = index;
    if (index === INDEX_IN_PLAY_NEXT) {
      this._playNextList.shift();
      next = this.current;
    }
    this.current = next;
    this._replaceCurrentTrack(trackID);
    return true;
  }
  async playNextFMTrack() {
    if (this._personalFMLoading) {
      return false;
    }

    this._isPersonalFM = true;
    if (!this._personalFMNextTrack) {
      this._personalFMLoading = true;
      let result = null;
      let retryCount = 5;
      for (; retryCount >= 0; retryCount--) {
        result = await fetchPersonalFM().catch(() => null);
        if (!result) {
          this._personalFMLoading = false;
          getAppStore().showToast('personal fm timeout');
          return false;
        }
        if (result.data?.length > 0) {
          break;
        } else if (retryCount > 0) {
          await delay(1000);
        }
      }
      this._personalFMLoading = false;

      if (retryCount < 0) {
        let content = '获取私人FM数据时重试次数过多，请手动切换下一首';
        getAppStore().showToast(content);
        console.log(content);
        return false;
      }
      // This endpoint returns one track at a time.
      const nextTrack = result?.data[0];
      if (!nextTrack) return false;
      this._personalFMTrack = nextTrack;
    } else {
      if (this._personalFMNextTrack.id === this._personalFMTrack.id) {
        return false;
      }
      this._personalFMTrack = this._personalFMNextTrack;
    }
    if (this._isPersonalFM) {
      this._replaceCurrentTrack(this._personalFMTrack.id);
    }
    this._loadPersonalFMNextTrack();
    return true;
  }
  playPrevTrack() {
    const [trackID, index] = this._getPrevTrack();
    if (trackID === undefined) return false;
    this.current = index;
    this._replaceCurrentTrack(
      trackID,
      true,
      UNPLAYABLE_CONDITION.PLAY_PREV_TRACK
    );
    return true;
  }
  saveSelfToLocalStorage(): void {
    const player: PersistedPlayerState = {
      _progress: this._progress,
      _enabled: this._enabled,
      _repeatMode: this._repeatMode,
      _shuffle: this._shuffle,
      _reversed: this._reversed,
      _volume: this._volume,
      _volumeBeforeMuted: this._volumeBeforeMuted,
      _list: this._list,
      _current: this._current,
      _shuffledList: this._shuffledList,
      _shuffledCurrent: this._shuffledCurrent,
      _playlistSource: this._playlistSource,
      _currentTrack: this._currentTrack,
      _playNextList: this._playNextList,
      _isPersonalFM: this._isPersonalFM,
      _personalFMTrack: this._personalFMTrack,
      ...(this._personalFMNextTrack === undefined
        ? {}
        : { _personalFMNextTrack: this._personalFMNextTrack }),
    };
    localStorage.setItem('player', JSON.stringify(player));
  }

  pause() {
    const howler = this._howler;
    if (!howler) return;
    // Record pause intent because _playing stays true until fade completes.
    this._pausePending = true;
    howler.fade(this.volume, 0, PLAY_PAUSE_FADE_DURATION);

    howler.once('fade', () => {
      if (this._howler !== howler) return;
      this._pausePending = false;
      howler.pause();
      this._setPlaying(false);
      setTitle(null);
      this.syncDiscordPresence();
    });
  }
  play() {
    // Explicit playback supersedes a pending pause gesture.
    this._pausePending = false;
    const howler = this._howler;
    if (!howler || howler.playing()) return;

    howler.play();

    howler.once('play', () => {
      if (this._howler !== howler) return;
      howler.fade(0, this.volume, PLAY_PAUSE_FADE_DURATION);

      // Playback always enables the player UI.
      this._enabled = true;
      this._setPlaying(true);
      if (this._currentTrack.name) {
        setTitle(this._currentTrack);
      }
      this.syncDiscordPresence();
      if (getAppStore().lastfm['key'] !== undefined) {
        trackUpdateNowPlaying({
          artist: this.currentTrack.ar?.[0]?.name ?? '',
          track: this.currentTrack.name ?? '',
          album: this.currentTrack.al?.name ?? '',
          trackNumber:
            typeof this.currentTrack['no'] === 'number'
              ? this.currentTrack['no']
              : undefined,
          duration: Math.trunc((this.currentTrack.dt ?? 0) / 1000),
        });
      }
    });
  }
  playOrPause() {
    if (this._howler?.playing()) {
      this.pause();
    } else {
      this.play();
    }
  }
  seek(time: number | null = null): number {
    if (time !== null) {
      // Each explicit seek invalidates older upgrade targets.
      this._seekToken += 1;
      if (this._canUpgradeSeekPrecision(time)) {
        this._getPreciseSeekUpgrader().request(time);
        return Math.max(0, Number(time) || 0);
      }
      return this._startSeekTransaction(time);
    }
    return this._howler === null ? 0 : this._howler.seek();
  }
  _startSeekTransaction(time: number): number {
    const howler = this._howler;
    this._pendingSeekCancel?.();
    this._pendingSeekCancel = null;
    const transaction = startHowlerSeek(howler, time, actualPosition => {
      if (howler !== this._howler) return;
      // Native seeked confirms WebKit has reached a stable decoded position.
      this._progress = actualPosition;
      this._seeking = false;
      this._pendingSeekCancel = null;
      if (isDesktopRuntime) {
        void sendDesktop('mediaSeeked', actualPosition);
      }
      if (this._playing) this.syncDiscordPresence();
    });
    if (transaction === null) return 0;
    this._progress = transaction.position;
    this._seeking = transaction.pending;
    this._pendingSeekCancel = transaction.pending ? transaction.cancel : null;
    return transaction.position;
  }
  _canUpgradeSeekPrecision(time: number): boolean {
    // AVPlayer misaligns FLAC seeks; a constant-rate WAV keeps time and audio exact.
    return (
      this._howler !== null &&
      Number(time) > 0 &&
      this._currentSourceMeta?.format === 'flac'
    );
  }
  _getPreciseSeekUpgrader(): PreciseSeekUpgrader {
    if (this._preciseSeekUpgrader) return this._preciseSeekUpgrader;
    // Build lazily so closures capture Vue's reactive proxy, not the raw instance.
    const player = this;
    this._preciseSeekUpgrader = markRaw(
      createPreciseSeekUpgrader({
        getSnapshot: () => ({
          howler: player._howler,
          trackId: player.currentTrackID,
          playing: player._playing,
          pausePending: player._pausePending,
          seekToken: player._seekToken,
        }),
        readCachedFlac: async trackId => {
          const cached = await getTrackSource(String(trackId));
          if (!cached?.source) return null;
          const bitsPerSample = parseFlacStreamInfo(
            cached.source
          )?.bitsPerSample;
          return {
            bytes: cached.source,
            ...(bitsPerSample === undefined ? {} : { bitsPerSample }),
          };
        },
        // Prefer the sidecar conversion to keep renderer memory bounded.
        convertViaSidecar: (trackId, bytes, bits) =>
          requestPreciseWavURL(trackId, bytes, bits),
        convertInRenderer: async bytes => {
          const wavBlob = await decodeFlacToWavBlob(bytes);
          if (!wavBlob) return null;
          const url = URL.createObjectURL(wavBlob);
          revokeBlobURLs(player.createdBlobRecords, u =>
            URL.revokeObjectURL(u)
          );
          player.createdBlobRecords = [url];
          return url;
        },
        // Freeze lyric timing while showing the requested position immediately.
        freezeAt: time => {
          player._seeking = true;
          player._progress = time;
        },
        seekStream: time => {
          player._seeking = false;
          player._startSeekTransaction(time);
        },
        applyPreciseSource: (url, time, resume) => {
          // Distinguish precise WAV failures from valid cached FLAC corruption.
          player._playAudioSource(
            {
              url,
              origin: 'precise-wav',
              format: 'wav',
              mimeType: 'audio/wav',
            },
            false
          );
          player._startSeekTransaction(time);
          if (resume) player.play();
        },
        onError: error =>
          console.warn('[Player] FLAC 转 WAV 失败，退回流式 seek', error),
      })
    );
    return this._preciseSeekUpgrader;
  }
  mute() {
    if (this.volume === 0) {
      this.volume = this._volumeBeforeMuted;
    } else {
      this._volumeBeforeMuted = this.volume;
      this.volume = 0;
    }
  }
  setOutputDevice() {
    const mediaNode = getHowlerMediaNode(this._howler);
    if (!mediaNode) return;
    void mediaNode
      .setSinkId?.(getAppStore().settings.outputDevice)
      .catch(error => console.warn('[Player] 切换输出设备失败', error));
  }

  replacePlaylist(
    trackIDs: number[],
    playlistSourceID: number | string,
    playlistSourceType: string,
    autoPlayTrackID: number | 'first' = 'first'
  ): void {
    this._isPersonalFM = false;
    this.list = trackIDs;
    this.current = 0;
    this._playlistSource = {
      type: playlistSourceType,
      id: playlistSourceID,
    };
    if (this.shuffle) this._shuffleTheList(autoPlayTrackID);
    if (autoPlayTrackID === 'first') {
      const firstTrackID = this.list[0];
      if (firstTrackID !== undefined) this._replaceCurrentTrack(firstTrackID);
    } else {
      this.current = this.list.indexOf(autoPlayTrackID);
      this._replaceCurrentTrack(autoPlayTrackID);
    }
  }
  playAlbumByID(id: number, trackID: number | 'first' = 'first'): void {
    getAlbum(id).then(data => {
      let trackIDs = data.songs.map(t => t.id);
      this.replacePlaylist(trackIDs, id, 'album', trackID);
    });
  }
  playPlaylistByID(
    id: number,
    trackID: number | 'first' = 'first',
    noCache = false
  ): void {
    console.debug(
      `[debug][Player.js] playPlaylistByID 👉 id:${id} trackID:${trackID} noCache:${noCache}`
    );
    getPlaylistDetail(id, noCache).then(data => {
      const trackIDs = data.playlist?.trackIds?.map(track => track.id);
      if (!trackIDs) {
        getAppStore().showToast('歌单详情缺少歌曲列表');
        return;
      }
      this.replacePlaylist(trackIDs, id, 'playlist', trackID);
    });
  }
  playArtistByID(id: number, trackID: number | 'first' = 'first'): void {
    getArtist(id).then(data => {
      let trackIDs = data.hotSongs.map(t => t.id);
      this.replacePlaylist(trackIDs, id, 'artist', trackID);
    });
  }
  playTrackOnListByID(
    id: number,
    listName: 'default' | 'playNext' = 'default'
  ): void {
    if (listName === 'default') {
      this.current = getActiveTrackIndex(
        {
          shuffle: this.shuffle,
          list: this._list,
          shuffledList: this._shuffledList,
        },
        id
      );
    } else if (listName === 'playNext') {
      consumeQueuedTrack(this._playNextList, id);
    }
    this._replaceCurrentTrack(id);
  }
  playIntelligenceListById(
    id: number,
    trackID: number | 'first' = 'first',
    noCache = false
  ): void {
    getPlaylistDetail(id, noCache).then(data => {
      const songId = pickRandomTrackID(data.playlist?.trackIds ?? []);
      if (songId === undefined) {
        getAppStore().showToast('歌单里没有可用于心动模式的歌曲');
        return;
      }
      intelligencePlaylist({ id: songId, pid: id }).then(result => {
        let trackIDs = result.data.map(t => t.id);
        this.replacePlaylist(trackIDs, id, 'playlist', trackID);
      });
    });
  }
  addTrackToPlayNext(trackID: number, playNow = false): void {
    this._playNextList.push(trackID);
    if (playNow) {
      this.playNextTrack();
    } else {
      this._prefetchNextTrack();
    }
  }
  playPersonalFM() {
    this._isPersonalFM = true;
    if (this.currentTrackID !== this._personalFMTrack.id) {
      this._replaceCurrentTrack(this._personalFMTrack.id, true);
    } else {
      this.playOrPause();
    }
  }
  async moveToFMTrash() {
    this._isPersonalFM = true;
    let id = this._personalFMTrack.id;
    if (await this.playNextFMTrack()) {
      void trashFM(id).catch(() => {});
    }
  }

  syncDiscordPresence(): void {
    if (
      !isDesktopRuntime ||
      !getAppStore().settings.enableDiscordRichPresence ||
      !this.currentTrack.name
    ) {
      return;
    }
    void sendConfiguredDiscordPresence({
      title: this.currentTrack.name,
      artists: this.currentTrack.ar?.map(artist => artist.name ?? '') ?? [],
      album: this.currentTrack.al?.name ?? '',
      coverUrl: this.currentTrack.al?.picUrl ?? '',
      durationMs: this.currentTrack.dt ?? 0,
      positionSeconds: this.seek(),
      playing: this.playing,
    }).catch(error => {
      console.warn('[discord] presence update failed', error);
    });
  }

  switchRepeatMode() {
    if (this._repeatMode === 'on') {
      this.repeatMode = 'one';
    } else if (this._repeatMode === 'one') {
      this.repeatMode = 'off';
    } else {
      this.repeatMode = 'on';
    }
  }
  switchShuffle() {
    this.shuffle = !this.shuffle;
  }
  switchReversed() {
    this.reversed = !this.reversed;
  }

  clearPlayNextList() {
    this._playNextList = [];
    this._prefetchNextTrack();
  }
  removeTrackFromQueue(index: number): void {
    this._playNextList.splice(index, 1);
    this._prefetchNextTrack();
  }
}
