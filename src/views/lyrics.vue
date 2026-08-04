<template>
  <transition name="slide-up">
    <div
      data-tauri-drag-region
      class="lyrics-page"
      :class="{ 'no-lyric': noLyric }"
      :data-theme="theme"
    >
      <div
        v-if="
          (settings.lyricsBackground === 'blur') |
            (settings.lyricsBackground === 'dynamic')
        "
        data-tauri-drag-region
        class="lyrics-background"
        :class="{
          'dynamic-background': settings.lyricsBackground === 'dynamic',
        }"
      >
        <div
          data-tauri-drag-region
          class="top-right"
          :style="{ backgroundImage: `url(${bgImageUrl})` }"
        />
        <div
          data-tauri-drag-region
          class="bottom-left"
          :style="{ backgroundImage: `url(${bgImageUrl})` }"
        />
      </div>
      <div
        v-if="settings.lyricsBackground === true"
        data-tauri-drag-region
        class="gradient-background"
        :style="{ background }"
      ></div>

      <!-- 迷你模式：小封面 + 一行歌词。窗口拖窄时自动切换 -->
      <div
        v-if="isMini"
        class="mini-player"
        @mouseenter="setWindowButtons(true)"
        @mouseleave="handleMiniMouseLeave"
        @mousedown="handleMiniMouseDown"
        @dblclick="handleMiniDoubleClick"
      >
        <img class="mini-cover" :src="imageUrl" loading="lazy" />
        <!--
          只有真正压在字上才可选中复制，所以 .mini-copyable 只包住文字本身，
          不包外面的容器：容器的空白仍然是"按住挪窗口"。
        -->
        <div class="mini-info">
          <div class="mini-title" :title="currentTrack.name">
            <span class="mini-copyable">{{ currentTrack.name }}</span>
          </div>
          <div class="mini-artist" :title="artist.name">
            <span class="mini-copyable">{{ artist.name }}</span>
          </div>
        </div>
        <div class="mini-lyric" :title="displayLyric">
          <div class="mini-lyric-origin" :style="lyricFontSize">
            <span class="mini-copyable">{{ displayLyric }}</span>
          </div>
          <div v-if="showMiniTranslation" class="mini-lyric-translation">
            <span class="mini-copyable">{{ currentLyricTranslation }}</span>
          </div>
        </div>
        <div class="mini-controls">
          <button-icon
            class="mini-pin"
            :class="{
              active: isAlwaysOnTop,
              'pin-dismissed': pinDismissed,
            }"
            :title="isAlwaysOnTop ? '取消置顶' : '窗口置顶'"
            @click="toggleAlwaysOnTop"
          >
            <svg-icon icon-class="pin" />
          </button-icon>
          <button-icon @click="playPrevTrack">
            <svg-icon icon-class="previous" />
          </button-icon>
          <button-icon class="mini-play" @click="playOrPause">
            <svg-icon :icon-class="player.playing ? 'pause' : 'play'" />
          </button-icon>
          <button-icon @click="playNextTrack">
            <svg-icon icon-class="next" />
          </button-icon>
        </div>
        <div
          class="mini-progress-track"
          :class="{ dragging: miniSeekDragging }"
          role="slider"
          tabindex="0"
          :aria-label="`播放进度 ${formatTrackTime(
            miniSeekPreview ?? player.progress
          )}`"
          :aria-valuemin="0"
          :aria-valuemax="player.currentTrackDuration"
          :aria-valuenow="Math.round(miniSeekPreview ?? player.progress)"
          @pointerdown="startMiniSeek"
          @pointermove="moveMiniSeek"
          @pointerup="finishMiniSeek"
          @pointercancel="commitMiniSeek"
          @keydown.left.prevent="nudgeMiniSeek(-5)"
          @keydown.right.prevent="nudgeMiniSeek(5)"
        >
          <div
            class="mini-progress"
            :class="{ anon: settings.anonStyle }"
            :style="{ width: miniProgressPercent + '%' }"
          ></div>
          <span
            v-if="settings.anonStyle"
            class="mini-progress-rider"
            :style="miniProgressRiderStyle"
          ></span>
        </div>
      </div>

      <div v-if="!isMini" class="left-side">
        <div>
          <div v-if="settings.showLyricsTime" class="date">
            {{ date }}
          </div>
          <div class="cover">
            <div class="cover-container">
              <img :src="imageUrl" loading="lazy" />
              <div
                class="shadow"
                :style="{ backgroundImage: `url(${imageUrl})` }"
              ></div>
            </div>
          </div>
          <div class="controls">
            <div class="top-part">
              <div class="track-info">
                <div class="title" :title="currentTrack.name">
                  <router-link
                    v-if="hasList()"
                    :to="`${getListPath()}`"
                    @click="toggleLyrics"
                    >{{ currentTrack.name }}
                  </router-link>
                  <span v-else>
                    {{ currentTrack.name }}
                  </span>
                </div>
                <div class="subtitle">
                  <router-link
                    v-if="artist.id !== 0"
                    :to="`/artist/${artist.id}`"
                    @click="toggleLyrics"
                    >{{ artist.name }}
                  </router-link>
                  <span v-else>
                    {{ artist.name }}
                  </span>
                  <span v-if="album.id !== 0">
                    -
                    <router-link
                      :to="`/album/${album.id}`"
                      :title="album.name"
                      @click="toggleLyrics"
                      >{{ album.name }}
                    </router-link>
                  </span>
                </div>
              </div>
              <div class="top-right">
                <div class="volume-control">
                  <button-icon :title="$t('player.mute')" @click="mute">
                    <svg-icon v-show="volume > 0.5" icon-class="volume" />
                    <svg-icon v-show="volume === 0" icon-class="volume-mute" />
                    <svg-icon
                      v-show="volume <= 0.5 && volume !== 0"
                      icon-class="volume-half"
                    />
                  </button-icon>
                  <div class="volume-bar">
                    <vue-slider
                      v-model="volume"
                      :min="0"
                      :max="1"
                      :interval="0.01"
                      :drag-on-click="true"
                      :duration="0"
                      tooltip="none"
                      :dot-size="12"
                    ></vue-slider>
                  </div>
                </div>
                <div class="buttons">
                  <button-icon
                    :title="$t('player.like')"
                    @click="likeATrack(player.currentTrack.id)"
                  >
                    <svg-icon
                      :icon-class="
                        player.isCurrentTrackLiked ? 'heart-solid' : 'heart'
                      "
                    />
                  </button-icon>
                  <button-icon
                    :title="$t('contextMenu.addToPlaylist')"
                    @click="addToPlaylist"
                  >
                    <svg-icon icon-class="plus" />
                  </button-icon>
                  <!-- <button-icon @click="openMenu" title="Menu"
                    ><svg-icon icon-class="more"
                  /></button-icon> -->
                </div>
              </div>
            </div>
            <div class="progress-bar">
              <span>{{ formatTrackTime(player.progress) || '0:00' }}</span>
              <div class="slider">
                <player-progress-slider
                  :key="player.currentTrackID"
                  v-model="player.progress"
                  :min="0"
                  :max="player.currentTrackDuration"
                  :drag-on-click="true"
                  :duration="0"
                  :dot-size="12"
                  :height="2"
                  :tooltip-formatter="formatTrackTime"
                  :silent="true"
                ></player-progress-slider>
              </div>
              <span>{{ formatTrackTime(player.currentTrackDuration) }}</span>
            </div>
            <div class="media-controls">
              <button-icon
                v-show="!player.isPersonalFM"
                :title="
                  player.repeatMode === 'one'
                    ? $t('player.repeatTrack')
                    : $t('player.repeat')
                "
                :class="{ active: player.repeatMode !== 'off' }"
                @click="switchRepeatMode"
              >
                <svg-icon
                  v-show="player.repeatMode !== 'one'"
                  icon-class="repeat"
                />
                <svg-icon
                  v-show="player.repeatMode === 'one'"
                  icon-class="repeat-1"
                />
              </button-icon>
              <div class="middle">
                <button-icon
                  v-show="!player.isPersonalFM"
                  :title="$t('player.previous')"
                  @click="playPrevTrack"
                >
                  <svg-icon icon-class="previous" />
                </button-icon>
                <button-icon
                  v-show="player.isPersonalFM"
                  title="不喜欢"
                  @click="moveToFMTrash"
                >
                  <svg-icon icon-class="thumbs-down" />
                </button-icon>
                <button-icon
                  id="play"
                  :title="$t(player.playing ? 'player.pause' : 'player.play')"
                  @click="playOrPause"
                >
                  <svg-icon :icon-class="player.playing ? 'pause' : 'play'" />
                </button-icon>
                <button-icon
                  :title="$t('player.next')"
                  @click="playNextTrack"
                >
                  <svg-icon icon-class="next" />
                </button-icon>
              </div>
              <button-icon
                v-show="!player.isPersonalFM"
                :title="$t('player.shuffle')"
                :class="{ active: player.shuffle }"
                @click="switchShuffle"
              >
                <svg-icon icon-class="shuffle" />
              </button-icon>
              <button-icon
                v-show="
                  isShowLyricTypeSwitch &&
                  $store.state.settings.showLyricsTranslation &&
                  lyricType === 'translation'
                "
                :title="$t('player.translationLyric')"
                @click="switchLyricType"
              >
                <span class="lyric-switch-icon">译</span>
              </button-icon>
              <button-icon
                v-show="
                  isShowLyricTypeSwitch &&
                  $store.state.settings.showLyricsTranslation &&
                  lyricType === 'romaPronunciation'
                "
                :title="$t('player.PronunciationLyric')"
                @click="switchLyricType"
              >
                <span class="lyric-switch-icon">音</span>
              </button-icon>
            </div>
          </div>
        </div>
      </div>
      <div v-if="!isMini" class="right-side">
        <transition name="slide-fade">
          <div
            v-show="!noLyric"
            ref="lyricsContainer"
            class="lyrics-container"
            :style="lyricFontSize"
          >
            <div id="line-1" class="line"></div>
            <div
              v-for="(line, index) in lyricToShow"
              :id="`line${index}`"
              :key="index"
              class="line"
              :class="{
                highlight: highlightLyricIndex === index,
              }"
              @click="clickLyricLine(line.time)"
              @dblclick="clickLyricLine(line.time, true)"
            >
              <div class="content">
                <span
                  v-if="line.contents[0]"
                  @click.right="openLyricMenu($event, line, 0)"
                  >{{ line.contents[0] }}</span
                >
                <br />
                <span
                  v-if="
                    line.contents[1] &&
                    $store.state.settings.showLyricsTranslation
                  "
                  class="translation"
                  @click.right="openLyricMenu($event, line, 1)"
                  >{{ line.contents[1] }}</span
                >
              </div>
            </div>
            <ContextMenu v-if="!noLyric" ref="lyricMenu">
              <div class="item" @click="copyLyric(false)">{{
                $t('contextMenu.copyLyric')
              }}</div>
              <div
                v-if="
                  rightClickLyric &&
                  rightClickLyric.contents[1] &&
                  $store.state.settings.showLyricsTranslation
                "
                class="item"
                @click="copyLyric(true)"
                >{{ $t('contextMenu.copyLyricWithTranslation') }}</div
              >
            </ContextMenu>
          </div>
        </transition>
      </div>
      <div v-if="!isMini" class="close-button" @click="toggleLyrics">
        <button>
          <svg-icon icon-class="arrow-down" />
        </button>
      </div>
      <div
        v-if="!isMini"
        class="close-button"
        style="left: 24px"
        @click="fullscreen"
      >
        <button>
          <svg-icon v-if="isFullscreen" icon-class="fullscreen-exit" />
          <svg-icon v-else icon-class="fullscreen" />
        </button>
      </div>
    </div>
  </transition>
</template>

<script>
// The lyrics page of Apple Music is so gorgeous, so I copy the design.
// Some of the codes are from https://github.com/sl1673495/vue-netease-music

import { mapState, mapMutations, mapActions } from 'vuex';
import VueSlider from 'vue-slider-component';
import ContextMenu from '@/components/ContextMenu.vue';
import PlayerProgressSlider from '@/components/PlayerProgressSlider.vue';
import { formatTrackTime } from '@/utils/common';
import { getLyric, getCloudLyric } from '@/api/track';
import {
  copyLyric,
  findActiveLyricIndex,
  hasNoLyric,
  lyricClockInterval,
  lyricParser,
  parseLyric,
  resolveLyricDisplay,
  shouldRunLyricClock,
} from '@/utils/lyrics';
import ButtonIcon from '@/components/ButtonIcon.vue';
import * as Vibrant from 'node-vibrant/dist/vibrant.worker.min.js';
import Color from 'color';
import { isAccountLoggedIn } from '@/utils/auth';
import { hasListSource, getListSourcePath } from '@/utils/playList';
import locale from '@/locale';
import {
  disposeListeners,
  listen,
  startVisibilityAwareInterval,
} from '@/utils/mediaLifecycle';
import {
  invokeDesktop,
  sendDesktop,
  startDesktopWindowDragging,
} from '@/services/desktopTransport';
import { isDesktopRuntime, isTauriRuntime } from '@/utils/runtime';
import {
  calculateMiniSeekTime,
  getMiniProgressRiderStyle,
} from '@/utils/miniPlayer';
import {
  beginMiniWindowDragGesture,
  hasCrossedMiniWindowDragThreshold,
  shouldToggleMiniWindow,
} from '@/utils/miniWindow';
import { buildArtworkURL } from '@/utils/artwork';

export default {
  name: 'Lyrics',
  emits: ['expand-compact-window'],
  components: {
    VueSlider,
    PlayerProgressSlider,
    ButtonIcon,
    ContextMenu,
  },
  data() {
    return {
      lyricsIntervalCleanup: null,
      timer: null,
      listenerCleanups: [],
      lyric: [],
      tlyric: [],
      romalyric: [],
      lyricLoading: false,
      lyricRequestVersion: 0,
      lyricType: 'translation', // or 'romaPronunciation'
      highlightLyricIndex: -1,
      minimize: true,
      background: '',
      date: this.formatTime(new Date()),
      isFullscreen: !!document.fullscreenElement,
      rightClickLyric: null,
      isMini: false,
      miniTall: false,
      isAlwaysOnTop: false,
      pinDismissed: false,
      miniSeekDragging: false,
      miniSeekPreview: null,
      miniWindowDragStart: null,
    };
  },
  computed: {
    ...mapState(['player', 'settings', 'showLyrics']),
    activeLyricIndex() {
      if (this.miniSeekDragging && Number.isFinite(this.miniSeekPreview)) {
        return findActiveLyricIndex(
          this.lyricToShow,
          this.miniSeekPreview
        );
      }
      return this.highlightLyricIndex;
    },
    // 迷你模式下只显示当前这一行歌词
    currentLyric() {
      const line = this.lyricToShow[this.activeLyricIndex];
      if (!line) return '';
      return Array.isArray(line.contents) ? line.contents[0] : line.contents;
    },
    // 纯音乐没有歌词，用网易云那套标准引导词占位
    displayLyric() {
      return resolveLyricDisplay(
        this.currentLyric,
        this.lyric.length,
        this.lyricLoading
      );
    },
    currentLyricTranslation() {
      const line = this.lyricToShow[this.activeLyricIndex];
      return Array.isArray(line?.contents) ? line.contents[1] : '';
    },
    // 窗口够高才塞得下两行，太扁就只留原文
    showMiniTranslation() {
      return (
        this.settings.showLyricsTranslation &&
        this.miniTall &&
        !!this.currentLyricTranslation
      );
    },
    miniProgressPercent() {
      const duration = this.player.currentTrackDuration;
      if (!duration) return 0;
      const progress = this.miniSeekPreview ?? this.player.progress;
      return Math.min(100, (progress / duration) * 100);
    },
    miniProgressRiderStyle() {
      return getMiniProgressRiderStyle(this.miniProgressPercent);
    },
    currentTrack() {
      return this.player.currentTrack;
    },
    volume: {
      get() {
        return this.player.volume;
      },
      set(value) {
        this.player.volume = value;
      },
    },
    imageUrl() {
      return buildArtworkURL(this.player.currentTrack?.al?.picUrl, 1024);
    },
    bgImageUrl() {
      return buildArtworkURL(this.player.currentTrack?.al?.picUrl, 512);
    },
    isShowLyricTypeSwitch() {
      return this.romalyric.length > 0 && this.tlyric.length > 0;
    },
    lyricToShow() {
      return this.lyricType === 'translation'
        ? this.lyricWithTranslation
        : this.lyricWithRomaPronunciation;
    },
    lyricWithTranslation() {
      let ret = [];
      // 空内容的去除
      const lyricFiltered = this.lyric.filter(({ content }) =>
        Boolean(content)
      );
      // content统一转换数组形式
      if (lyricFiltered.length) {
        lyricFiltered.forEach(l => {
          const { rawTime, time, content } = l;
          const lyricItem = { time, content, contents: [content] };
          const sameTimeTLyric = this.tlyric.find(
            ({ rawTime: tLyricRawTime }) => tLyricRawTime === rawTime
          );
          if (sameTimeTLyric) {
            const { content: tLyricContent } = sameTimeTLyric;
            if (content) {
              lyricItem.contents.push(tLyricContent);
            }
          }
          ret.push(lyricItem);
        });
      } else {
        ret = lyricFiltered.map(({ time, content }) => ({
          time,
          content,
          contents: [content],
        }));
      }
      return ret;
    },
    lyricWithRomaPronunciation() {
      let ret = [];
      // 空内容的去除
      const lyricFiltered = this.lyric.filter(({ content }) =>
        Boolean(content)
      );
      // content统一转换数组形式
      if (lyricFiltered.length) {
        lyricFiltered.forEach(l => {
          const { rawTime, time, content } = l;
          const lyricItem = { time, content, contents: [content] };
          const sameTimeRomaLyric = this.romalyric.find(
            ({ rawTime: tLyricRawTime }) => tLyricRawTime === rawTime
          );
          if (sameTimeRomaLyric) {
            const { content: romaLyricContent } = sameTimeRomaLyric;
            if (content) {
              lyricItem.contents.push(romaLyricContent);
            }
          }
          ret.push(lyricItem);
        });
      } else {
        ret = lyricFiltered.map(({ time, content }) => ({
          time,
          content,
          contents: [content],
        }));
      }
      return ret;
    },
    lyricFontSize() {
      return {
        fontSize: `${this.$store.state.settings.lyricFontSize || 28}px`,
      };
    },
    noLyric() {
      return hasNoLyric(this.lyric.length, this.lyricLoading);
    },
    artist() {
      return this.currentTrack?.ar
        ? this.currentTrack.ar[0]
        : { id: 0, name: 'unknown' };
    },
    album() {
      return this.currentTrack?.al || { id: 0, name: 'unknown' };
    },
    theme() {
      return this.settings.lyricsBackground === true ? 'dark' : 'auto';
    },
  },
  watch: {
    currentTrack() {
      // 新歌歌词尚未返回时不能继续拿上一首的时间轴更新菜单栏。
      this.highlightLyricIndex = -1;
      this.lyric = [];
      this.tlyric = [];
      this.romalyric = [];
      this.getLyric();
      this.getCoverColor();
      this.pushToTray();
    },
    // 歌词逐行推给菜单栏
    displayLyric() {
      this.pushToTray();
    },
    showLyrics(show) {
      if (shouldRunLyricClock(show, isDesktopRuntime)) {
        this.setLyricsInterval(show);
      } else {
        this.stopLyricsInterval();
      }
      this.$store.commit('enableScrolling', !show);
    },
  },
  created() {
    this.getLyric();
    this.getCoverColor();
    this.initDate();
    this.listenerCleanups.push(
      listen(document, 'keydown', this.handleLyricsKeydown),
      listen(document, 'fullscreenchange', this.handleFullscreenChange),
      listen(window, 'resize', this.checkMini)
    );
    // watcher 不会在组件首次创建时触发；桌面端即使收起歌词页，
    // 菜单栏仍需用低频时钟跟随当前歌词。
    if (shouldRunLyricClock(this.showLyrics, isDesktopRuntime)) {
      this.setLyricsInterval(this.showLyrics);
    }
    this.checkMini();
    if (isDesktopRuntime) {
      invokeDesktop('isAlwaysOnTop').then(v => {
        this.isAlwaysOnTop = v;
      });
    }
  },
  beforeUnmount: function () {
    if (this.timer) {
      clearInterval(this.timer);
    }
    this.stopLyricsInterval();
    this.cancelMiniWindowDrag();
    disposeListeners(this.listenerCleanups);
    this.setWindowButtons(true); // 离开歌词页别把红绿灯留在隐藏状态
  },
  unmounted() {
    this.stopLyricsInterval();
  },
  methods: {
    ...mapMutations(['toggleLyrics', 'updateModal']),
    ...mapActions(['likeATrack']),
    handleLyricsKeydown(event) {
      if (event.key !== 'F11') return;
      event.preventDefault();
      this.fullscreen();
    },
    handleFullscreenChange() {
      this.isFullscreen = !!document.fullscreenElement;
    },
    // 窗口拖到这个尺寸以下就切成迷你播放器
    checkMini() {
      const next = window.innerWidth < 620 || window.innerHeight < 340;
      // 28px 原文 + 14px 译文 + 间距 ≈ 53px，留点余量到 64 就够放两行
      this.miniTall = window.innerHeight >= 64;
      if (next === this.isMini) return;
      this.isMini = next;
      this.pinDismissed = false;
      // 进迷你模式就把红绿灯收起来，退出时恢复
      this.setWindowButtons(!next);
      // 切换迷你模式会改变菜单栏该不该显示歌词
      this.pushToTray();
    },
    pushToTray() {
      if (!isDesktopRuntime) return;
      void sendDesktop('updateTrayNowPlaying', {
        // 要不要在菜单栏显示文字由主进程定：
        // 它还要考虑窗口是不是被隐藏了，这边看不到
        title: this.displayLyric || this.currentTrack.name,
        isMini: this.isMini,
        // 菜单栏只有 18px，拉小图省流量
        coverUrl: buildArtworkURL(this.currentTrack?.al?.picUrl, 64),
      });
    },
    setWindowButtons(visible) {
      if (!isDesktopRuntime) return;
      void sendDesktop('setWindowButtonVisibility', visible);
    },
    async toggleAlwaysOnTop() {
      if (!isDesktopRuntime) return;
      this.isAlwaysOnTop = await invokeDesktop('toggleAlwaysOnTop');
      // 点击后的鼠标仍压在按钮上，单靠 :hover 不会自动淡出。
      this.pinDismissed = true;
    },
    handleMiniMouseLeave() {
      this.pinDismissed = false;
      this.setWindowButtons(false);
    },
    handleMiniMouseDown(event) {
      // 按在空白处就当场掐掉选中起点（见 beginMiniWindowDragGesture）；
      // Electron 靠原生 app-region 拖窗，这里到此为止。
      if (
        !beginMiniWindowDragGesture(event, window.getSelection()) ||
        !isTauriRuntime
      ) {
        return;
      }
      // 等鼠标真的移动后再交给原生窗口，否则第一次按下会吞掉双击事件。
      this.cancelMiniWindowDrag();
      this.miniWindowDragStart = {
        clientX: event.clientX,
        clientY: event.clientY,
      };
      document.addEventListener('mousemove', this.handleMiniDragMove);
      document.addEventListener('mouseup', this.cancelMiniWindowDrag);
    },
    handleMiniDragMove(event) {
      if (
        !hasCrossedMiniWindowDragThreshold(this.miniWindowDragStart, event)
      ) {
        return;
      }
      event.preventDefault();
      this.cancelMiniWindowDrag();
      void startDesktopWindowDragging();
    },
    cancelMiniWindowDrag() {
      this.miniWindowDragStart = null;
      document.removeEventListener('mousemove', this.handleMiniDragMove);
      document.removeEventListener('mouseup', this.cancelMiniWindowDrag);
    },
    handleMiniDoubleClick(event) {
      if (!shouldToggleMiniWindow(event)) return;
      event.preventDefault();
      this.$emit('expand-compact-window');
    },
    updateMiniSeekPreview(event) {
      const bounds = event.currentTarget.getBoundingClientRect();
      this.miniSeekPreview = calculateMiniSeekTime(
        event.clientX,
        bounds.left,
        bounds.width,
        this.player.currentTrackDuration
      );
    },
    startMiniSeek(event) {
      // 同 handleMiniMouseDown：preventDefault 会连"按下清除选中"一起拦掉
      window.getSelection()?.removeAllRanges();
      event.preventDefault();
      event.stopPropagation();
      this.miniSeekDragging = true;
      event.currentTarget.setPointerCapture?.(event.pointerId);
      this.updateMiniSeekPreview(event);
    },
    moveMiniSeek(event) {
      if (!this.miniSeekDragging) return;
      this.updateMiniSeekPreview(event);
    },
    finishMiniSeek(event) {
      if (!this.miniSeekDragging) return;
      this.updateMiniSeekPreview(event);
      this.commitMiniSeek(event);
    },
    commitMiniSeek(event) {
      if (!this.miniSeekDragging) return;
      const seekTime = this.miniSeekPreview;
      if (Number.isFinite(seekTime)) {
        this.player.progress = seekTime;
        const actualSeekTime = this.player.progress;
        // WKWebView 可能修正流媒体 seek 的落点；歌词必须跟随读回的实际
        // 播放位置，不能继续使用指针请求值。
        this.highlightLyricIndex = findActiveLyricIndex(
          this.lyricToShow,
          actualSeekTime
        );
      }
      this.miniSeekDragging = false;
      this.miniSeekPreview = null;
      const target = event?.currentTarget;
      if (target?.hasPointerCapture?.(event.pointerId)) {
        target.releasePointerCapture(event.pointerId);
      }
    },
    nudgeMiniSeek(offset) {
      const duration = this.player.currentTrackDuration;
      this.player.progress = Math.min(
        duration,
        Math.max(0, this.player.progress + offset)
      );
    },
    initDate() {
      var _this = this;
      clearInterval(this.timer);
      this.timer = setInterval(function () {
        _this.date = _this.formatTime(new Date());
      }, 1000);
    },
    formatTime(value) {
      let hour = value.getHours().toString();
      let minute = value.getMinutes().toString();
      let second = value.getSeconds().toString();
      return (
        hour.padStart(2, '0') +
        ':' +
        minute.padStart(2, '0') +
        ':' +
        second.padStart(2, '0')
      );
    },
    fullscreen() {
      if (document.fullscreenElement) {
        document.exitFullscreen();
      } else {
        document.documentElement.requestFullscreen();
      }
    },
    addToPlaylist() {
      if (!isAccountLoggedIn()) {
        this.showToast(locale.t('toast.needToLogin'));
        return;
      }
      this.$store.dispatch('fetchLikedPlaylist');
      this.updateModal({
        modalName: 'addTrackToPlaylistModal',
        key: 'show',
        value: true,
      });
      this.updateModal({
        modalName: 'addTrackToPlaylistModal',
        key: 'selectedTrackID',
        value: this.currentTrack?.id,
      });
    },
    playPrevTrack() {
      this.player.playPrevTrack();
    },
    playOrPause() {
      this.player.playOrPause();
    },
    playNextTrack() {
      if (this.player.isPersonalFM) {
        this.player.playNextFMTrack();
      } else {
        this.player.playNextTrack();
      }
    },
    getLyric() {
      if (!this.currentTrack.id) {
        this.lyricLoading = false;
        return;
      }
      // 记下这次请求对应的歌曲。网络慢的时候前一首的响应可能后到，
      // 不加这个判断就会把已经切过去的新歌的歌词覆盖掉。
      const requestedId = this.currentTrack.id;
      const requestVersion = ++this.lyricRequestVersion;
      this.lyricLoading = true;
      const isStale = () =>
        this.currentTrack.id !== requestedId ||
        this.lyricRequestVersion !== requestVersion;
      const finishLyricRequest = () => {
        if (!isStale()) this.lyricLoading = false;
      };
      if (
        this.currentTrack.pc !== null &&
        this.currentTrack.cd === null &&
        this.$store.state.data.user?.userId
      ) {
        //云盘未设置关联的歌曲获取其内置歌词
        return getCloudLyric(
          requestedId,
          this.$store.state.data.user?.userId
        )
          .then(data => {
            if (isStale()) return false;
            this.tlyric = [];
            this.romalyric = [];
            this.lyric = data?.lrc?.length > 0 ? parseLyric(data.lrc) : [];
            this.lyricType = 'translation';
            return true;
          })
          .finally(finishLyricRequest);
      }
      return getLyric(requestedId).then(data => {
        if (isStale()) return false;
        if (!data?.lrc?.lyric) {
          this.lyric = [];
          this.tlyric = [];
          this.romalyric = [];
          return false;
        } else {
          let { lyric, tlyric, romalyric } = lyricParser(data);
          lyric = lyric.filter(
            l => !/^作(词|曲)\s*(:|：)\s*无$/.exec(l.content)
          );
          let includeAM =
            lyric.length <= 10 &&
            lyric.map(l => l.content).includes('纯音乐，请欣赏');
          if (includeAM) {
            let reg = /^作(词|曲)\s*(:|：)\s*/;
            let author = this.currentTrack?.ar[0]?.name;
            lyric = lyric.filter(l => {
              let regExpArr = l.content.match(reg);
              return (
                !regExpArr || l.content.replace(regExpArr[0], '') !== author
              );
            });
          }
          if (lyric.length === 1 && includeAM) {
            this.lyric = [];
            this.tlyric = [];
            this.romalyric = [];
            return false;
          } else {
            this.lyric = lyric;
            this.tlyric = tlyric;
            this.romalyric = romalyric;
            if (tlyric.length * romalyric.length > 0) {
              this.lyricType = 'translation';
            } else {
              this.lyricType =
                lyric.length > 0 ? 'translation' : 'romaPronunciation';
            }
            return true;
          }
        }
      }).finally(finishLyricRequest);
    },
    switchLyricType() {
      this.lyricType =
        this.lyricType === 'translation' ? 'romaPronunciation' : 'translation';
    },
    formatTrackTime(value) {
      return formatTrackTime(value);
    },
    clickLyricLine(value, startPlay = false) {
      // TODO: 双击选择还会选中文字，考虑搞个右键菜单复制歌词
      let jumpFlag = false;
      this.lyric.filter(function (item) {
        if (item.content == '纯音乐，请欣赏') {
          jumpFlag = true;
        }
      });
      if (window.getSelection().toString().length === 0 && !jumpFlag) {
        this.player.seek(value);
      }
      if (startPlay === true) {
        this.player.play();
      }
    },
    openLyricMenu(e, lyric, idx) {
      this.rightClickLyric = { ...lyric, idx };
      this.$refs.lyricMenu.openMenu(e);
      e.preventDefault();
    },
    copyLyric(withTranslation) {
      if (this.rightClickLyric) {
        const idx = this.rightClickLyric.idx;
        if (!withTranslation) {
          copyLyric(this.rightClickLyric.contents[idx]);
        } else {
          copyLyric(this.rightClickLyric.contents.join(' '));
        }
      }
    },
    setLyricsInterval(showLyrics = this.showLyrics) {
      this.stopLyricsInterval();
      this.lyricsIntervalCleanup = startVisibilityAwareInterval(
        document,
        () => {
          if (this.player.seeking) return;
          const progress = this.player.seek(null, false) ?? 0;
          let oldHighlightLyricIndex = this.highlightLyricIndex;
          this.highlightLyricIndex = findActiveLyricIndex(
            this.lyricToShow,
            progress
          );
          if (
            showLyrics &&
            oldHighlightLyricIndex !== this.highlightLyricIndex
          ) {
            const el = document.getElementById(
              `line${this.highlightLyricIndex}`
            );
            if (el)
              el.scrollIntoView({
                behavior: 'smooth',
                block: 'center',
              });
          }
        },
        {
          foregroundMs: lyricClockInterval(showLyrics),
          backgroundMs: 250,
        }
      );
    },
    stopLyricsInterval() {
      this.lyricsIntervalCleanup?.();
      this.lyricsIntervalCleanup = null;
    },
    moveToFMTrash() {
      this.player.moveToFMTrash();
    },
    switchRepeatMode() {
      this.player.switchRepeatMode();
    },
    switchShuffle() {
      this.player.switchShuffle();
    },
    getCoverColor() {
      if (this.settings.lyricsBackground !== true) return;
      const cover = buildArtworkURL(this.currentTrack.al?.picUrl, 256);
      Vibrant.from(cover, { colorCount: 1 })
        .getPalette()
        .then(palette => {
          const originColor = Color.rgb(palette.DarkMuted._rgb);
          const color = originColor.darken(0.1).rgb().string();
          const color2 = originColor.lighten(0.28).rotate(-30).rgb().string();
          this.background = `linear-gradient(to top left, ${color}, ${color2})`;
        });
    },
    hasList() {
      return hasListSource();
    },
    getListPath() {
      return getListSourcePath();
    },
    mute() {
      this.player.mute();
    },
  },
};
</script>

<style lang="scss" scoped>
// ===== 迷你播放器：小封面 + 一行歌词 =====
.mini-player {
  position: absolute;
  top: 0;
  right: 0;
  bottom: 0;
  left: 0;
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 0 16px 0 76px; // 左边留出红绿灯的位置
  -webkit-app-region: drag;
  user-select: none;
  overflow: hidden;

  // 角色 GIF 就是进度条的抓手，用户是冲着它去点的。轨道的命中高度必须
  // 完整盖住角色，否则点在角色身上会漏到下面的窗口拖拽（表现为"想点进度
  // 结果窗口跑了"）。macOS 还会吃掉贴边几像素做缩放热区，底部 10px 的旧
  // 命中区在左下角几乎点不中，所以这里按角色尺寸取值而不是按视觉条粗细。
  --mini-rider-size: 22px;
  --mini-rider-bottom: 1px;
  --mini-progress-hit-height: 24px;

  // 只有文字本身可选中复制。容器空白不带这个类，那里按住就是挪窗口。
  .mini-copyable {
    -webkit-app-region: no-drag;
    user-select: text;
    cursor: text;
    // 命中区变高后会盖住窄窗口里歌名/歌词的下半截，抬一层保住选中和复制
    position: relative;
    z-index: 2;
  }

  // 跟着窗口高度缩，压到最扁也不会溢出
  .mini-cover {
    height: min(58px, calc(100% - 8px));
    width: auto;
    aspect-ratio: 1;
    border-radius: 6px;
    object-fit: cover;
    flex-shrink: 0;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.24);
    -webkit-user-drag: none;
  }

  // 固定宽度，不然长歌名会一路挤占歌词的空间
  .mini-info {
    flex: 0 0 168px;
    min-width: 0;
  }

  .mini-title {
    font-size: 14px;
    font-weight: 600;
    color: var(--color-text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .mini-artist {
    margin-top: 2px;
    font-size: 12px;
    color: var(--color-text);
    opacity: 0.58;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  // 歌词放右边，字号沿用设置里的 lyricFontSize（和大视图一致）
  .mini-lyric {
    flex: 1;
    min-width: 0;
    text-align: center;
    color: var(--color-text);

    .mini-lyric-origin {
      font-weight: 600;
      line-height: 1.2;
      white-space: nowrap;
      overflow: hidden;
      text-overflow: ellipsis;
    }

    .mini-lyric-translation {
      margin-top: 2px;
      font-size: 14px;
      font-weight: 500;
      line-height: 1.2;
      opacity: 0.62;
      white-space: nowrap;
      overflow: hidden;
      text-overflow: ellipsis;
    }
  }

  .mini-controls {
    display: flex;
    align-items: center;
    gap: 2px;
    flex-shrink: 0;
    -webkit-app-region: no-drag;
    // 进度命中区是绝对定位又排在后面，48px 高的窗口里会压住按钮下沿；
    // 抬一层保证上一首/播放/下一首在任何窗口高度下都点得着
    position: relative;
    z-index: 2;

    .svg-icon {
      width: 16px;
      height: 16px;
      color: var(--color-text);
    }

    .mini-play .svg-icon {
      width: 20px;
      height: 20px;
    }

    // 任何时候都藏着，只有鼠标移到播放条上才浮现（置顶开着也一样）
    .mini-pin {
      margin-right: 6px;
      opacity: 0;
      transition: opacity 0.18s;

      .svg-icon {
        width: 14px;
        height: 14px;
      }

      // 只在浮现出来的时候用颜色区分开关状态
      &.active {
        color: var(--color-primary);
      }

      &.pin-dismissed {
        opacity: 0 !important;
        pointer-events: none;
      }
    }
  }

  &:hover .mini-controls .mini-pin {
    opacity: 0.5;

    &:hover,
    &.active {
      opacity: 1;
    }
  }

  .mini-progress-track {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    height: var(--mini-progress-hit-height);
    cursor: pointer;
    touch-action: none;
    -webkit-app-region: no-drag;

    &:focus-visible {
      outline: 2px solid var(--color-primary);
      outline-offset: -2px;
    }

    // 只画已播放的那一段。试过给未播放部分补一条贯穿全宽的底色轨，
    // 顶满窗口下沿反而像给窗口描了道边，用户否掉了。
    .mini-progress {
      position: absolute;
      left: 0;
      bottom: 0;
      height: 2px;
      background-color: var(--color-primary);
      transition: width 0.4s linear;

      &.anon {
        height: 3px;
        background: linear-gradient(
          90deg,
          #ffc2d4 0%,
          #ff8fb1 60%,
          #f76d99 100%
        );
      }
    }

    // 角色在轨道内部完成整段行程；它的右边缘只在 100% 时碰到终点。
    .mini-progress-rider {
      position: absolute;
      bottom: var(--mini-rider-bottom);
      width: var(--mini-rider-size);
      height: var(--mini-rider-size);
      background: url('/img/logos/anon.gif') center / var(--mini-rider-size)
        no-repeat;
      image-rendering: pixelated;
      pointer-events: none;
      transition: left 0.4s linear, transform 0.4s linear;
    }

    &.dragging .mini-progress,
    &.dragging .mini-progress-rider {
      transition: none;
    }
  }
}

.lyrics-page {
  position: fixed;
  top: 0;
  right: 0;
  left: 0;
  bottom: 0;
  z-index: 200;
  background: var(--color-body-bg);
  display: flex;
  clip: rect(auto, auto, auto, auto);
}

.lyrics-background {
  --contrast-lyrics-background: 75%;
  --brightness-lyrics-background: 150%;
}

[data-theme='dark'] .lyrics-background {
  --contrast-lyrics-background: 125%;
  --brightness-lyrics-background: 50%;
}

.lyrics-background {
  filter: blur(50px) contrast(var(--contrast-lyrics-background))
    brightness(var(--brightness-lyrics-background));
  position: absolute;
  height: 100vh;
  width: 100vw;

  .top-right,
  .bottom-left {
    z-index: 0;
    width: 140vw;
    height: 140vw;
    opacity: 0.6;
    position: absolute;
    background-size: cover;
  }

  .top-right {
    right: 0;
    top: 0;
    mix-blend-mode: luminosity;
  }

  .bottom-left {
    left: 0;
    bottom: 0;
    animation-direction: reverse;
    animation-delay: 10s;
  }
}

.dynamic-background > div {
  animation: rotate 150s linear infinite;
}

@keyframes rotate {
  0% {
    transform: rotate(0deg);
  }

  100% {
    transform: rotate(360deg);
  }
}

.gradient-background {
  position: absolute;
  height: 100vh;
  width: 100vw;
}

.left-side {
  flex: 1;
  display: flex;
  justify-content: flex-end;
  margin-right: 32px;
  margin-top: 24px;
  align-items: center;
  transition: all 0.5s;

  z-index: 1;

  .date {
    max-width: 54vh;
    margin: 24px 0;
    color: var(--color-text);
    text-align: center;
    font-size: 4rem;
    font-weight: 600;
    opacity: 0.88;
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 1;
    overflow: hidden;
  }

  .controls {
    max-width: 54vh;
    margin-top: 24px;
    color: var(--color-text);

    .title {
      margin-top: 8px;
      font-size: 1.4rem;
      font-weight: 600;
      opacity: 0.88;
      display: -webkit-box;
      -webkit-box-orient: vertical;
      -webkit-line-clamp: 1;
      overflow: hidden;
    }

    .subtitle {
      margin-top: 4px;
      font-size: 1rem;
      opacity: 0.58;
      display: -webkit-box;
      -webkit-box-orient: vertical;
      -webkit-line-clamp: 1;
      overflow: hidden;
    }

    .top-part {
      display: flex;
      justify-content: space-between;

      .top-right {
        display: flex;
        justify-content: space-between;

        .volume-control {
          margin: 0 10px;
          display: flex;
          align-items: center;

          .volume-bar {
            width: 84px;
          }
        }

        .buttons {
          display: flex;
          align-items: center;

          button {
            margin: 0 0 0 4px;
          }

          .svg-icon {
            height: 18px;
            width: 18px;
          }
        }
      }
    }

    .progress-bar {
      margin-top: 22px;
      display: flex;
      align-items: center;
      justify-content: space-between;

      .slider {
        width: 100%;
        flex-grow: grow;
        padding: 0 10px;
      }

      span {
        font-size: 15px;
        opacity: 0.58;
        min-width: 28px;
      }
    }

    .media-controls {
      display: flex;
      justify-content: center;
      margin-top: 18px;
      align-items: center;

      button {
        margin: 0;
      }

      .svg-icon {
        opacity: 0.38;
        height: 14px;
        width: 14px;
      }

      .active .svg-icon {
        opacity: 0.88;
      }

      .middle {
        padding: 0 16px;
        display: flex;
        align-items: center;

        button {
          margin: 0 8px;
        }

        button#play .svg-icon {
          height: 28px;
          width: 28px;
          padding: 2px;
        }

        .svg-icon {
          opacity: 0.88;
          height: 22px;
          width: 22px;
        }
      }

      .lyric-switch-icon {
        color: var(--color-text);
        font-size: 14px;
        line-height: 14px;
        opacity: 0.88;
      }
    }
  }
}

.cover {
  position: relative;

  .cover-container {
    position: relative;
  }

  img {
    border-radius: 0.75em;
    width: 54vh;
    height: 54vh;
    user-select: none;
    object-fit: cover;
  }

  .shadow {
    position: absolute;
    top: 12px;
    height: 54vh;
    width: 54vh;
    filter: blur(16px) opacity(0.6);
    transform: scale(0.92, 0.96);
    z-index: -1;
    background-size: cover;
    border-radius: 0.75em;
  }
}

.right-side {
  flex: 1;
  font-weight: 600;
  color: var(--color-text);
  margin-right: 24px;
  z-index: 0;

  .lyrics-container {
    height: 100%;
    display: flex;
    flex-direction: column;
    padding-left: 78px;
    max-width: 460px;
    overflow-y: auto;
    transition: 0.5s;
    scrollbar-width: none; // firefox

    .line {
      margin: 2px 0;
      padding: 12px 18px;
      transition: 0.5s;
      border-radius: 12px;

      &:hover {
        background: var(--color-secondary-bg-for-transparent);
      }

      .content {
        transform-origin: center left;
        transform: scale(0.95);
        transition: all 0.35s cubic-bezier(0.25, 0.46, 0.45, 0.94);
        user-select: none;

        span {
          opacity: 0.28;
          cursor: default;
          font-size: 1em;
          transition: all 0.35s cubic-bezier(0.25, 0.46, 0.45, 0.94);
        }

        span.translation {
          opacity: 0.2;
          font-size: 0.925em;
        }
      }
    }

    .line#line-1:hover {
      background: unset;
    }

    .translation {
      margin-top: 0.1em;
    }

    .highlight div.content {
      transform: scale(1);

      span {
        opacity: 0.98;
        display: inline-block;
      }

      span.translation {
        opacity: 0.65;
      }
    }
  }

  ::-webkit-scrollbar {
    display: none;
  }

  .lyrics-container .line:first-child {
    margin-top: 50vh;
  }

  .lyrics-container .line:last-child {
    margin-bottom: calc(50vh - 128px);
  }
}

.close-button {
  position: fixed;
  top: 24px;
  right: 24px;
  z-index: 300;
  border-radius: 0.75rem;
  height: 44px;
  width: 44px;
  display: flex;
  justify-content: center;
  align-items: center;
  opacity: 0.28;
  transition: 0.2s;
  -webkit-app-region: no-drag;

  .svg-icon {
    color: var(--color-text);
    padding-top: 5px;
    height: 22px;
    width: 22px;
  }

  &:hover {
    background: var(--color-secondary-bg-for-transparent);
    opacity: 0.88;
  }
}

.lyrics-page.no-lyric {
  .left-side {
    transition: all 0.5s;
    transform: translateX(27vh);
    margin-right: 0;
  }
}

@media (max-aspect-ratio: 10/9) {
  .left-side {
    display: none;
  }

  .right-side .lyrics-container {
    max-width: 100%;
  }
}

@media screen and (min-width: 1200px) {
  .right-side .lyrics-container {
    max-width: 600px;
  }
}

.slide-up-enter-active,
.slide-up-leave-active {
  transition: all 0.4s;
}

.slide-up-enter-from,
.slide-up-leave-to

/* .fade-leave-active below version 2.1.8 */ {
  transform: translateY(100%);
}

.slide-fade-enter-active {
  transition: all 0.5s ease;
}

.slide-fade-leave-active {
  transition: all 0.5s cubic-bezier(0.2, 0.2, 0, 1);
}

.slide-fade-enter-from,
.slide-fade-leave-to {
  transform: translateX(27vh);
  opacity: 0;
}
</style>
