<template>
  <div
    id="app"
    :class="{
      'user-select-none': userSelectNone,
      'window-hidden': windowHidden,
    }"
  >
    <Scrollbar v-show="!showLyrics" ref="scrollbar" />
    <Navbar
      v-show="showNavbar"
      ref="navbar"
      :compact-window-expanded="compactWindowExpanded"
      @restore-compact-window="restoreCompactWindow"
    />
    <main
      ref="main"
      :style="{ overflow: enableScrolling ? 'auto' : 'hidden' }"
      @scroll="handleScroll"
    >
      <router-view v-slot="{ Component, route }">
        <keep-alive :max="4">
          <component
            :is="Component"
            v-if="route.meta.keepAlive"
            :key="route.fullPath"
          />
        </keep-alive>
        <component
          :is="Component"
          v-if="!route.meta.keepAlive"
          :key="route.fullPath"
        />
      </router-view>
    </main>
    <transition name="slide-up">
      <Player v-if="enablePlayer" v-show="showPlayer" ref="player" />
    </transition>
    <Toast />
    <ModalAddTrackToPlaylist v-if="isAccountLoggedIn" />
    <ModalNewPlaylist v-if="isAccountLoggedIn" />
    <transition v-if="enablePlayer" name="slide-up">
      <Lyrics
        v-show="showLyrics"
        @expand-compact-window="expandCompactWindow"
      />
    </transition>
  </div>
</template>

<script>
import ModalAddTrackToPlaylist from './components/ModalAddTrackToPlaylist.vue';
import ModalNewPlaylist from './components/ModalNewPlaylist.vue';
import Scrollbar from './components/Scrollbar.vue';
import Navbar from './components/Navbar.vue';
import Player from './components/Player.vue';
import Toast from './components/Toast.vue';
import { isAccountLoggedIn, isLooseLoggedIn } from '@/utils/auth';
import Lyrics from './views/lyrics.vue';
import { mapState } from 'vuex';
import { observeDocumentVisibility } from '@/utils/mediaLifecycle';
import { connectDesktopEvents } from '@/services/desktopBridge';
import { isDesktopRuntime } from '@/utils/runtime';
import {
  expandCompactWindow,
  restoreCompactWindow,
} from '@/services/compactWindow';

export default {
  name: 'App',
  provide() {
    return {
      appShell: {
        restoreScrollPosition: () =>
          this.$refs.scrollbar?.restorePosition(),
        scrollMainTo: (...args) => this.$refs.main?.scrollTo(...args),
      },
    };
  },
  components: {
    Navbar,
    Player,
    Toast,
    ModalAddTrackToPlaylist,
    ModalNewPlaylist,
    Lyrics,
    Scrollbar,
  },
  data() {
    return {
      isDesktop: isDesktopRuntime,
      userSelectNone: false,
      autoOpenedLyrics: false, // 迷你模式是我们自动打开的，拖回大窗口要还原
      compactWindowExpanded: false,
      windowHidden: document.hidden,
    };
  },
  computed: {
    ...mapState(['showLyrics', 'settings', 'player', 'enableScrolling']),
    isAccountLoggedIn() {
      return isAccountLoggedIn();
    },
    showPlayer() {
      return (
        [
          'mv',
          'loginUsername',
          'login',
          'loginAccount',
          'lastfmCallback',
        ].includes(this.$route.name) === false
      );
    },
    enablePlayer() {
      return this.player.enabled && this.$route.name !== 'lastfmCallback';
    },
    showNavbar() {
      return this.$route.name !== 'lastfmCallback';
    },
  },
  created() {
    if (this.isDesktop) {
      this.desktopEventsCleanup = connectDesktopEvents(this);
    }
    window.addEventListener('keydown', this.handleKeydown);
    window.addEventListener('resize', this.handleMiniResize);
    this.visibilityCleanup = observeDocumentVisibility(
      document,
      hidden => (this.windowHidden = hidden)
    );
    this.handleMiniResize();
    this.fetchData();
  },
  beforeUnmount() {
    window.removeEventListener('keydown', this.handleKeydown);
    window.removeEventListener('resize', this.handleMiniResize);
    this.visibilityCleanup?.();
    this.desktopEventsCleanup?.then(cleanup => cleanup());
  },
  methods: {
    async expandCompactWindow() {
      if (!(await expandCompactWindow())) return;
      this.compactWindowExpanded = true;
      if (this.$route.name !== 'next') {
        await this.$router.push({ name: 'next' });
      }
    },
    async restoreCompactWindow() {
      if (!(await restoreCompactWindow())) return;
      this.compactWindowExpanded = false;
    },
    // 窗口拖窄时自动切到歌词页（里面是迷你播放器布局），拖回来再收起
    handleMiniResize() {
      const isMini = window.innerWidth < 620 || window.innerHeight < 340;
      if (isMini && !this.showLyrics) {
        this.$store.commit('toggleLyrics');
        this.autoOpenedLyrics = true;
      } else if (!isMini && this.showLyrics && this.autoOpenedLyrics) {
        this.$store.commit('toggleLyrics');
        this.autoOpenedLyrics = false;
      }
    },
    handleKeydown(e) {
      if (e.code === 'Space') {
        if (e.target.tagName === 'INPUT') return false;
        if (this.$route.name === 'mv') return false;
        e.preventDefault();
        this.player.playOrPause();
      }
    },
    fetchData() {
      if (!isLooseLoggedIn()) return;
      this.$store.dispatch('fetchLikedSongs');
      this.$store.dispatch('fetchLikedSongsWithDetails');
      this.$store.dispatch('fetchLikedPlaylist');
      // 专辑、歌手、MV 和云盘可能各有上千条，只在进入“资料库”时加载。
      // 这些数据没有被全局播放器或侧栏使用，启动时预取只会常驻占内存。
    },
    handleScroll() {
      this.$refs.scrollbar.handleScroll();
    },
  },
};
</script>

<style lang="scss">
#app {
  width: 100%;
  transition: all 0.4s;
}

// Electron 和 WKWebView 都可能在隐藏窗口后继续合成动画；
// 后台只暂停视觉效果，播放器和菜单栏歌词仍照常更新。
#app.window-hidden *,
#app.window-hidden *::before,
#app.window-hidden *::after {
  animation-play-state: paused !important;
}

main {
  position: fixed;
  top: 0;
  bottom: 0;
  right: 0;
  left: 0;
  overflow: auto;
  padding: 64px 10vw 96px 10vw;
  box-sizing: border-box;
  scrollbar-width: none; // firefox
}

@media (max-width: 1336px) {
  main {
    padding: 64px 5vw 96px 5vw;
  }
}

main::-webkit-scrollbar {
  width: 0px;
}

.slide-up-enter-active,
.slide-up-leave-active {
  transition: transform 0.4s;
}
.slide-up-enter-from,
.slide-up-leave-to {
  transform: translateY(100%);
}
</style>
