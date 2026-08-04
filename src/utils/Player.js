import { getAlbum } from '@/api/album';
import { getArtist } from '@/api/artist';
import { trackScrobble, trackUpdateNowPlaying } from '@/api/lastfm';
import { fmTrash, personalFM } from '@/api/others';
import { getPlaylistDetail, intelligencePlaylist } from '@/api/playlist';
import { getLyric, getMP3, getTrackDetail, scrobble } from '@/api/track';
import store from '@/store';
import { isAccountLoggedIn } from '@/utils/auth';
import {
  cacheTrackSource,
  deleteTrackSource,
  getTrackSource,
  hasTrackSource,
} from '@/utils/db';
import { revokeBlobURLs } from '@/utils/cacheStats';
import { isCreateMpris, isCreateTray } from '@/utils/platform';
import { Howl, Howler } from 'howler';
import shuffle from 'lodash/shuffle';
import { decode as base642Buffer } from '@/utils/base64';
import {
  consumeQueuedTrack,
  getAdjacentTrack,
  getActiveTrackIndex,
  pickRandomTrackID,
} from '@/utils/playerQueue';
import { sendDesktop } from '@/services/desktopTransport';
import { isDesktopRuntime } from '@/utils/runtime';
import { requestUnblockedSong } from '@/services/unblockMusicTransport';
import {
  createBlobAudioSource,
  createRemoteAudioSource,
  discardFailedCache,
  resolveAudioSource,
  toHowlSourceOptions,
} from '@/utils/audioSource';
import { findMatchingAudioResponse } from '@/utils/audioCacheIntegrity';
import {
  createTrackSwitchGuard,
  runLatestTrackSwitch,
} from '@/utils/trackSwitch';
import {
  createNextTrackPrefetcher,
  warmTrackArtwork,
} from '@/utils/trackPrefetch';
import { buildArtworkURL } from '@/utils/artwork';
import { resolvePlaybackDuration } from '@/utils/playbackDuration';
import { startHowlerSeek } from '@/utils/playbackSeek';
import { getHowlerMediaNode } from '@/utils/howlerMedia';

const PLAY_PAUSE_FADE_DURATION = 200;

const INDEX_IN_PLAY_NEXT = -1;

/**
 * @readonly
 * @enum {string}
 */
const UNPLAYABLE_CONDITION = {
  PLAY_NEXT_TRACK: 'playNextTrack',
  PLAY_PREV_TRACK: 'playPrevTrack',
};

const electron =
  process.env.IS_ELECTRON === true ? window.require('electron') : null;
const ipcRenderer =
  process.env.IS_ELECTRON === true ? electron.ipcRenderer : null;
const delay = ms =>
  new Promise(resolve => {
    setTimeout(() => {
      resolve('');
    }, ms);
  });
const excludeSaveKeys = [
  '_playing',
  '_audioDuration',
  '_seeking',
  '_personalFMLoading',
  '_personalFMNextLoading',
];

function setTitle(track) {
  document.title = track
    ? `${track.name} · ${track.ar[0].name} - YesPlayMusic`
    : 'YesPlayMusic';
  if (isCreateTray) {
    void sendDesktop('updateTrayTooltip', document.title);
  }
  store.commit('updateTitle', document.title);
}

function setTrayLikeState(isLiked) {
  if (isCreateTray) {
    void sendDesktop('updateTrayLikeState', isLiked);
  }
}

export default class {
  constructor() {
    // 播放器状态
    this._playing = false; // 是否正在播放中
    this._progress = 0; // 当前播放歌曲的进度
    this._audioDuration = 0; // 浏览器实际解码出的音频长度
    this._seeking = false; // WebKit 是否仍在寻找拖拽后的可解码帧
    this._enabled = false; // 是否启用Player
    this._repeatMode = 'off'; // off | on | one
    this._shuffle = false; // true | false
    this._reversed = false;
    this._volume = 1; // 0 to 1
    this._volumeBeforeMuted = 1; // 用于保存静音前的音量
    this._personalFMLoading = false; // 是否正在私人FM中加载新的track
    this._personalFMNextLoading = false; // 是否正在缓存私人FM的下一首歌曲

    // 播放信息
    this._list = []; // 播放列表
    this._current = 0; // 当前播放歌曲在播放列表里的index
    this._shuffledList = []; // 被随机打乱的播放列表，随机播放模式下会使用此播放列表
    this._shuffledCurrent = 0; // 当前播放歌曲在随机列表里面的index
    this._playlistSource = { type: 'album', id: 123 }; // 当前播放列表的信息
    this._currentTrack = { id: 86827685 }; // 当前播放歌曲的详细信息
    this._playNextList = []; // 当这个list不为空时，会优先播放这个list的歌
    this._isPersonalFM = false; // 是否是私人FM模式
    this._personalFMTrack = { id: 0 }; // 私人FM当前歌曲
    this._personalFMNextTrack = {
      id: 0,
    }; // 私人FM下一首歌曲信息（为了快速加载下一首）

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

    Object.defineProperty(this, '_nextTrackPrefetcher', {
      enumerable: false,
      value: createNextTrackPrefetcher({
        loadTrack: id =>
          getTrackDetail(id).then(data =>
            data?.songs?.find(track => Number(track.id) === Number(id))
          ),
        loadLyric: id => getLyric(id),
        warmArtwork: track => warmTrackArtwork(track),
        cacheAudio: (track, isCurrent) =>
          this._cachePrefetchedAudio(track, isCurrent),
      }),
    });
  }

  get repeatMode() {
    return this._repeatMode;
  }
  set repeatMode(mode) {
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
  set shuffle(shuffle) {
    if (this._isPersonalFM) return;
    if (shuffle !== true && shuffle !== false) {
      console.warn('shuffle: invalid args, must be Boolean');
      return;
    }
    this._shuffle = shuffle;
    if (shuffle) {
      this._shuffleTheList();
    }
    // 同步当前歌曲在列表中的下标
    this.current = this.list.indexOf(this.currentTrackID);
    this._prefetchNextTrack();
  }
  get reversed() {
    return this._reversed;
  }
  set reversed(reversed) {
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
  set volume(volume) {
    this._volume = volume;
    this._howler?.volume(volume);
  }
  get list() {
    return this.shuffle ? this._shuffledList : this._list;
  }
  set list(list) {
    this._list = list;
  }
  get current() {
    return this.shuffle ? this._shuffledCurrent : this._current;
  }
  set current(current) {
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
    return resolvePlaybackDuration(
      this._currentTrack.dt,
      this._audioDuration
    );
  }
  get progress() {
    return this._progress;
  }
  set progress(value) {
    this.seek(value);
  }
  get isCurrentTrackLiked() {
    return store.state.liked.songs.includes(this.currentTrack.id);
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
      // 恢复当前播放歌曲
      const savedTrackTime = Number(
        localStorage.getItem('playerCurrentTrackTime') ?? 0
      );
      this._replaceCurrentTrack(this.currentTrackID, false).then(replaced => {
        // 初始化请求若已被用户切歌淘汰，不能把旧进度写进后来那首歌。
        if (!replaced || !Number.isFinite(savedTrackTime)) return;
        this.seek(savedTrackTime, false);
      }); // update audio source and init howler
      this._initMediaSession();
    }

    this._setIntervals();

    // 初始化私人FM
    if (
      this._personalFMTrack.id === 0 ||
      this._personalFMNextTrack.id === 0 ||
      this._personalFMTrack.id === this._personalFMNextTrack.id
    ) {
      personalFM().then(result => {
        this._personalFMTrack = result.data[0];
        this._personalFMNextTrack = result.data[1];
        return this._personalFMTrack;
      });
    }
  }
  _setPlaying(isPlaying) {
    this._playing = isPlaying;
    if (isCreateTray) {
      void sendDesktop('updateTrayPlayState', this._playing);
    }
  }
  _setIntervals() {
    // 同步播放进度
    // TODO: 如果 _progress 在别的地方被改变了，
    // 这个定时器会覆盖之前改变的值，是bug
    setInterval(() => {
      if (this._howler === null || this._seeking) return;
      this._progress = this._howler.seek();
      localStorage.setItem('playerCurrentTrackTime', this._progress);
      if (isCreateMpris) {
        void sendDesktop('playerCurrentTrackTime', this._progress);
      }
    }, 1000);
  }
  _getNextTrack() {
    if (this._playNextList.length > 0) {
      let trackID = this._playNextList[0];
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
  _getPrevTrack() {
    const direction = this._reversed ? 1 : -1;
    return getAdjacentTrack(
      this.list,
      this.current,
      direction,
      this.repeatMode === 'on'
    );
  }
  async _shuffleTheList(firstTrackID = this.currentTrackID) {
    let list = this._list.filter(tid => tid !== firstTrackID);
    if (firstTrackID === 'first') list = this._list;
    this._shuffledList = shuffle(list);
    if (firstTrackID !== 'first') this._shuffledList.unshift(firstTrackID);
  }
  async _scrobble(track, time, completed = false) {
    console.debug(
      `[debug][Player.js] scrobble track 👉 ${track.name} by ${track.ar[0].name} 👉 time:${time} completed: ${completed}`
    );
    const trackDuration = ~~(track.dt / 1000);
    time = completed ? trackDuration : ~~time;
    scrobble({
      id: track.id,
      sourceid: this.playlistSource.id,
      time,
    });
    if (
      store.state.lastfm.key !== undefined &&
      (time >= trackDuration / 2 || time >= 240)
    ) {
      const timestamp = ~~(new Date().getTime() / 1000) - time;
      trackScrobble({
        artist: track.ar[0].name,
        track: track.name,
        timestamp,
        album: track.al.name,
        trackNumber: track.no,
        duration: trackDuration,
      });
    }
  }
  _playAudioSource(
    source,
    autoplay = true,
    ifUnplayableThen = UNPLAYABLE_CONDITION.PLAY_NEXT_TRACK
  ) {
    // 换源重试会绕过 _replaceCurrentTrack.onBegin 直接替换实例；挂起的 seek
    // 事务若不在此取消，永不 settle 的事务会让 _seeking 卡死，歌词时钟
    // 和进度心跳一起冻结。任何替换 Howler 实例的路径都必须先走这三行。
    this._pendingSeekCancel?.();
    this._pendingSeekCancel = null;
    this._seeking = false;
    Howler.unload();
    let handlingLoadError = false;
    const howlerOptions = toHowlSourceOptions(source);
    let howler;
    const handleLoadError = (_, errCode) => {
      if (handlingLoadError) return;
      handlingLoadError = true;
      void this._retryAudioSourceAfterFailure({
        failedHowler: howler,
        failedSource: source,
        autoplay,
        ifUnplayableThen,
        errCode,
      });
    };
    howler = new Howl({
      html5: true,
      // 缓存源用 Web Audio（html5:false）拿到采样级精确 seek，见 toHowlSourceOptions
      ...howlerOptions,
      preload: true,
      onload: () => {
        if (this._howler === howler) {
          this._audioDuration = howler.duration();
        }
        Promise.resolve(source.cacheAfterLoad?.()).catch(error => {
          console.warn('[Player] 音频播放成功，但写入缓存失败', error);
        });
      },
      onend: () => {
        if (this._howler !== howler) return;
        this._nextTrackCallback();
      },
      // Howler 会在构造函数内立即 load；必须同时注册，否则无扩展名等同步错误会漏掉。
      onloaderror: handleLoadError,
    });
    this._howler = howler;
    if (autoplay) {
      this.play();
      if (this._currentTrack.name) {
        setTitle(this._currentTrack);
      }
      setTrayLikeState(store.state.liked.songs.includes(this.currentTrack.id));
    }
    this.setOutputDevice();
  }

  async _retryAudioSourceAfterFailure({
    failedHowler,
    failedSource,
    autoplay,
    ifUnplayableThen,
    errCode,
  }) {
    if (this._howler !== failedHowler) return;
    const failedTrack = this.currentTrack;
    if (failedSource.origin === 'cache') {
      await discardFailedCache(deleteTrackSource, failedTrack.id, error =>
        console.warn('[Player] 删除损坏缓存失败，继续尝试备用源', error)
      );
    }
    if (failedSource.url.startsWith('blob:')) {
      URL.revokeObjectURL(failedSource.url);
      this.createdBlobRecords = this.createdBlobRecords.filter(
        url => url !== failedSource.url
      );
    }

    let fallback;
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
      fallback = await this._getAudioSource(failedTrack, {
        afterOrigin: failedSource.origin,
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
      this._playAudioSource(fallback, autoplay, ifUnplayableThen);
      return;
    }

    store.dispatch(
      'showToast',
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

  _getAudioSourceBlobURL(data, origin = 'cache') {
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
  _getAudioSourceFromCache(id) {
    return getTrackSource(id).then(t => {
      if (!t) return null;
      return this._getAudioSourceBlobURL(t.source, 'cache');
    });
  }
  _getAudioSourceFromNetease(track) {
    if (isAccountLoggedIn()) {
      return getMP3(track.id).then(result => {
        const audio = findMatchingAudioResponse(result.data, track.id);
        if (!audio) {
          console.warn(
            `[Player] 网易云音源响应没有当前歌曲 ID，拒绝使用可能串台的结果：${track.id}`
          );
          return null;
        }
        if (!audio.url) return null;
        if (audio.freeTrialInfo !== null) return null; // 跳过只能试听的歌曲
        const source = audio.url.replace(/^http:/, 'https:');
        return createRemoteAudioSource(source, {
          origin: 'netease',
          format: audio.type,
          cacheAfterLoad: store.state.settings.automaticallyCacheSongs
            ? () => cacheTrackSource(track, source, audio.br)
            : null,
        });
      });
    } else {
      return new Promise(resolve => {
        resolve(
          createRemoteAudioSource(
            `https://music.163.com/song/media/outer/url?id=${track.id}`,
            { origin: 'netease', format: 'mp3' }
          )
        );
      });
    }
  }
  async _getAudioSourceFromUnblockMusic(track, excludedProviders = []) {
    console.debug(`[debug][Player.js] _getAudioSourceFromUnblockMusic`);

    if (
      !isDesktopRuntime ||
      store.state.settings.enableUnblockNeteaseMusic === false
    ) {
      return null;
    }

    /**
     *
     * @param {string=} searchMode
     * @returns {import("@unblockneteasemusic/rust-napi").SearchMode}
     */
    const determineSearchMode = searchMode => {
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
      store.state.settings.unmSource,
      track,
      {
        enableFlac: store.state.settings.unmEnableFlac || null,
        proxyUri: store.state.settings.unmProxyUri || null,
        searchMode: determineSearchMode(store.state.settings.unmSearchMode),
        excludedSources: excludedProviders,
        config: {
          'joox:cookie': store.state.settings.unmJooxCookie || null,
          'qq:cookie': store.state.settings.unmQQCookie || null,
          'ytdl:exe': store.state.settings.unmYtDlExe || null,
        },
      }
    );

    if (!retrieveSongInfo) {
      return null;
    }

    if (retrieveSongInfo.source !== 'bilibili') {
      return createRemoteAudioSource(retrieveSongInfo.url, {
        origin: 'unm',
        provider: retrieveSongInfo.source,
        excludedProviders,
        // 一些 provider 返回无扩展名的签名 URL；这个值只帮助 Howler 通过
        // 能力预检，响应自己的 MIME 和字节仍由 WebKit 负责实际解码。
        fallbackFormat: 'mp3',
        cacheAfterLoad: store.state.settings.automaticallyCacheSongs
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
    if (store.state.settings.automaticallyCacheSongs) {
      source.cacheAfterLoad = () =>
        cacheTrackSource(
          track,
          `data:${source.mimeType};base64,${retrieveSongInfo.url}`,
          128000,
          'unm:bilibili'
        );
    }
    return source;
  }
  async _getAudioSource(track, { afterOrigin = null } = {}) {
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
    id,
    autoplay = true,
    ifUnplayableThen = UNPLAYABLE_CONDITION.PLAY_NEXT_TRACK
  ) {
    if (autoplay && this._currentTrack.name) {
      this._scrobble(this.currentTrack, this._howler?.seek());
    }
    return runLatestTrackSwitch(this._trackSwitchGuard, {
      onBegin: () => {
        // 网络请求开始前就切断旧音频，否则新封面加载后仍会短暂播放上一首。
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
      loadTrack: () => getTrackDetail(id).then(data => data.songs[0]),
      commitTrack: track => {
        this._currentTrack = track;
        this._updateMediaSessionMetaData(track);
      },
      loadSource: track => this._getAudioSource(track),
      commitSource: source => {
        this._playAudioSource(source, autoplay, ifUnplayableThen);
        this._prefetchNextTrack();
      },
      onMissingSource: track => {
        store.dispatch('showToast', `无法播放 ${track.name}`);
        switch (ifUnplayableThen) {
          case UNPLAYABLE_CONDITION.PLAY_NEXT_TRACK:
            this._playNextTrack(this.isPersonalFM);
            break;
          case UNPLAYABLE_CONDITION.PLAY_PREV_TRACK:
            this.playPrevTrack();
            break;
          default:
            store.dispatch(
              'showToast',
              `undefined Unplayable condition: ${ifUnplayableThen}`
            );
            break;
        }
      },
    });
  }
  async _cachePrefetchedAudio(track, isCurrent) {
    if (!isCurrent() || (await hasTrackSource(track.id))) return;
    if (!isCurrent()) return;

    // 这里只请求远端 URL；真正下载音频前再检查一次目标，避免旧随机队列继续占网速。
    const source = await this._getAudioSourceFromNetease(track);
    if (!isCurrent()) return;
    await source?.cacheAfterLoad?.();
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
      includeAudio: store.state.settings.automaticallyCacheSongs === true,
    });
  }
  _loadSelfFromLocalStorage() {
    const player = JSON.parse(localStorage.getItem('player'));
    if (!player) return;
    for (const [key, value] of Object.entries(player)) {
      this[key] = value;
    }
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
  _updateMediaSessionMetaData(track) {
    if ('mediaSession' in navigator === false) {
      return;
    }
    let artists = track.ar.map(a => a.name);
    const metadata = {
      title: track.name,
      artist: artists.join(','),
      album: track.al.name,
      artwork: [
        {
          src: buildArtworkURL(track.al.picUrl, 224),
          type: 'image/jpg',
          sizes: '224x224',
        },
        {
          src: buildArtworkURL(track.al.picUrl, 512),
          type: 'image/jpg',
          sizes: '512x512',
        },
      ],
      length: this.currentTrackDuration,
      trackId: this.current,
      url: '/trackid/' + track.id,
    };

    navigator.mediaSession.metadata = new window.MediaMetadata(metadata);
    if (isCreateMpris) {
      this._updateMprisState(track, metadata);
    }
  }
  // OSDLyrics 会检测 Mpris 状态并寻找对应歌词文件，所以要在更新 Mpris 状态之前保证歌词下载完成
  async _updateMprisState(track, metadata) {
    if (!store.state.settings.enableOsdlyricsSupport) {
      return ipcRenderer?.send('metadata', metadata);
    }

    let lyricContent = await getLyric(track.id);

    if (!lyricContent.lrc || !lyricContent.lrc.lyric) {
      return ipcRenderer?.send('metadata', metadata);
    }

    ipcRenderer.send('sendLyrics', {
      track,
      lyrics: lyricContent.lrc.lyric,
    });

    ipcRenderer.on('saveLyricFinished', () => {
      ipcRenderer?.send('metadata', metadata);
    });
  }
  _updateMediaSessionPositionState() {
    if ('mediaSession' in navigator === false) {
      return;
    }
    if ('setPositionState' in navigator.mediaSession) {
      navigator.mediaSession.setPositionState({
        duration: ~~(this.currentTrack.dt / 1000),
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
  _loadPersonalFMNextTrack() {
    if (this._personalFMNextLoading) {
      return [false, undefined];
    }
    this._personalFMNextLoading = true;
    return personalFM()
      .then(result => {
        if (!result || !result.data) {
          this._personalFMNextTrack = undefined;
        } else {
          this._personalFMNextTrack = result.data[0];
          this._prefetchNextTrack();
        }
        this._personalFMNextLoading = false;
        return [true, this._personalFMNextTrack];
      })
      .catch(() => {
        this._personalFMNextTrack = undefined;
        this._personalFMNextLoading = false;
        return [false, this._personalFMNextTrack];
      });
  }
  _playDiscordPresence(track, seekTime = 0) {
    if (
      process.env.IS_ELECTRON !== true ||
      store.state.settings.enableDiscordRichPresence === false
    ) {
      return null;
    }
    let copyTrack = { ...track };
    copyTrack.dt -= seekTime * 1000;
    ipcRenderer?.send('playDiscordPresence', copyTrack);
  }
  _pauseDiscordPresence(track) {
    if (
      process.env.IS_ELECTRON !== true ||
      store.state.settings.enableDiscordRichPresence === false
    ) {
      return null;
    }
    ipcRenderer?.send('pauseDiscordPresence', track);
  }
  _playNextTrack(isPersonal) {
    if (isPersonal) {
      this.playNextFMTrack();
    } else {
      this.playNextTrack();
    }
  }

  appendTrack(trackID) {
    this.list.append(trackID);
  }
  playNextTrack() {
    // TODO: 切换歌曲时增加加载中的状态
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
        result = await personalFM().catch(() => null);
        if (!result) {
          this._personalFMLoading = false;
          store.dispatch('showToast', 'personal fm timeout');
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
        store.dispatch('showToast', content);
        console.log(content);
        return false;
      }
      // 这里只能拿到一条数据
      this._personalFMTrack = result.data[0];
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
  saveSelfToLocalStorage() {
    let player = {};
    for (let [key, value] of Object.entries(this)) {
      if (excludeSaveKeys.includes(key)) continue;
      player[key] = value;
    }

    localStorage.setItem('player', JSON.stringify(player));
  }

  pause() {
    const howler = this._howler;
    if (!howler) return;
    howler.fade(this.volume, 0, PLAY_PAUSE_FADE_DURATION);

    howler.once('fade', () => {
      if (this._howler !== howler) return;
      howler.pause();
      this._setPlaying(false);
      setTitle(null);
      this._pauseDiscordPresence(this._currentTrack);
    });
  }
  play() {
    const howler = this._howler;
    if (!howler || howler.playing()) return;

    howler.play();

    howler.once('play', () => {
      if (this._howler !== howler) return;
      howler.fade(0, this.volume, PLAY_PAUSE_FADE_DURATION);

      // 播放时确保开启player.
      // 避免因"忘记设置"导致在播放时播放器不显示的Bug
      this._enabled = true;
      this._setPlaying(true);
      if (this._currentTrack.name) {
        setTitle(this._currentTrack);
      }
      this._playDiscordPresence(this._currentTrack, this.seek());
      if (store.state.lastfm.key !== undefined) {
        trackUpdateNowPlaying({
          artist: this.currentTrack.ar[0].name,
          track: this.currentTrack.name,
          album: this.currentTrack.al.name,
          trackNumber: this.currentTrack.no,
          duration: ~~(this.currentTrack.dt / 1000),
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
  seek(time = null, sendMpris = true) {
    if (time !== null) {
      const howler = this._howler;
      this._pendingSeekCancel?.();
      this._pendingSeekCancel = null;
      const transaction = startHowlerSeek(howler, time, actualPosition => {
        if (howler !== this._howler) return;
        // 只有原生 seeked 才代表 WebKit 的解码位置已稳定；此刻再放行歌词。
        this._progress = actualPosition;
        this._seeking = false;
        this._pendingSeekCancel = null;
        if (isCreateMpris && sendMpris) {
          void sendDesktop('seeked', actualPosition);
        }
        if (this._playing) {
          this._playDiscordPresence(this._currentTrack, actualPosition);
        }
      });
      if (transaction === null) return 0;
      this._progress = transaction.position;
      this._seeking = transaction.pending;
      this._pendingSeekCancel = transaction.pending
        ? transaction.cancel
        : null;
      return transaction.position;
    }
    return this._howler === null ? 0 : this._howler.seek();
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
    // Web Audio 模式没有媒体节点；WKWebView 的媒体节点也可能没有 setSinkId
    if (typeof mediaNode?.setSinkId !== 'function') return;
    mediaNode.setSinkId(store.state.settings.outputDevice);
  }

  replacePlaylist(
    trackIDs,
    playlistSourceID,
    playlistSourceType,
    autoPlayTrackID = 'first'
  ) {
    this._isPersonalFM = false;
    this.list = trackIDs;
    this.current = 0;
    this._playlistSource = {
      type: playlistSourceType,
      id: playlistSourceID,
    };
    if (this.shuffle) this._shuffleTheList(autoPlayTrackID);
    if (autoPlayTrackID === 'first') {
      this._replaceCurrentTrack(this.list[0]);
    } else {
      this.current = this.list.indexOf(autoPlayTrackID);
      this._replaceCurrentTrack(autoPlayTrackID);
    }
  }
  playAlbumByID(id, trackID = 'first') {
    getAlbum(id).then(data => {
      let trackIDs = data.songs.map(t => t.id);
      this.replacePlaylist(trackIDs, id, 'album', trackID);
    });
  }
  playPlaylistByID(id, trackID = 'first', noCache = false) {
    console.debug(
      `[debug][Player.js] playPlaylistByID 👉 id:${id} trackID:${trackID} noCache:${noCache}`
    );
    getPlaylistDetail(id, noCache).then(data => {
      let trackIDs = data.playlist.trackIds.map(t => t.id);
      this.replacePlaylist(trackIDs, id, 'playlist', trackID);
    });
  }
  playArtistByID(id, trackID = 'first') {
    getArtist(id).then(data => {
      let trackIDs = data.hotSongs.map(t => t.id);
      this.replacePlaylist(trackIDs, id, 'artist', trackID);
    });
  }
  playTrackOnListByID(id, listName = 'default') {
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
  playIntelligenceListById(id, trackID = 'first', noCache = false) {
    getPlaylistDetail(id, noCache).then(data => {
      const songId = pickRandomTrackID(data.playlist.trackIds);
      if (songId === undefined) {
        store.dispatch('showToast', '歌单里没有可用于心动模式的歌曲');
        return;
      }
      intelligencePlaylist({ id: songId, pid: id }).then(result => {
        let trackIDs = result.data.map(t => t.id);
        this.replacePlaylist(trackIDs, id, 'playlist', trackID);
      });
    });
  }
  addTrackToPlayNext(trackID, playNow = false) {
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
      fmTrash(id);
    }
  }

  sendSelfToIpcMain() {
    if (!isDesktopRuntime) return false;
    let liked = store.state.liked.songs.includes(this.currentTrack.id);
    void sendDesktop('player', {
      playing: this.playing,
      likedCurrentTrack: liked,
    });
    setTrayLikeState(liked);
  }

  switchRepeatMode() {
    if (this._repeatMode === 'on') {
      this.repeatMode = 'one';
    } else if (this._repeatMode === 'one') {
      this.repeatMode = 'off';
    } else {
      this.repeatMode = 'on';
    }
    if (isCreateMpris) {
      void sendDesktop('switchRepeatMode', this.repeatMode);
    }
  }
  switchShuffle() {
    this.shuffle = !this.shuffle;
    if (isCreateMpris) {
      void sendDesktop('switchShuffle', this.shuffle);
    }
  }
  switchReversed() {
    this.reversed = !this.reversed;
  }

  clearPlayNextList() {
    this._playNextList = [];
    this._prefetchNextTrack();
  }
  removeTrackFromQueue(index) {
    this._playNextList.splice(index, 1);
    this._prefetchNextTrack();
  }
}
