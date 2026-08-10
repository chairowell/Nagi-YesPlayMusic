import { createPinia, setActivePinia } from 'pinia';
import { watch } from 'vue';
import { useAppStore } from './app';
import { registerAppStore } from './accessor';
import { changeAppearance, changeThemeColor } from '@/utils/common';
import { mountPlayerState } from '@/utils/playerState';
import { isDesktopRuntime } from '@/utils/runtime';
import { sendDesktop } from '@/services/desktopTransport';
import { isLastfmCallbackLocation } from '@/services/lastfmAuth';
import { syncDesktopSettings } from '@/services/desktopSettings';

const isLastfmCallback = isLastfmCallbackLocation(window.location);

export const pinia = createPinia();
setActivePinia(pinia);

export const appStore = useAppStore(pinia);
registerAppStore(appStore);

watch(
  () => [appStore.settings, appStore.data],
  () => {
    localStorage.setItem('settings', JSON.stringify(appStore.settings));
    localStorage.setItem('data', JSON.stringify(appStore.data));
  },
  { deep: true, flush: 'sync' }
);

if (appStore.settings.lang === null) {
  const defaultLang = 'en';
  const langMapper = new Map([
    ['zh', 'zh-CN'],
    ['zh-TW', 'zh-TW'],
    ['en', 'en'],
    ['tr', 'tr'],
  ]);
  const exactLanguage = navigator.language;
  const baseLanguage = exactLanguage.slice(0, 2);
  appStore.settings.lang =
    langMapper.get(
      langMapper.has(exactLanguage) ? exactLanguage : baseLanguage
    ) ?? defaultLang;
}

appStore.$onAction(({ name, after }) => {
  if (!isDesktopRuntime || isLastfmCallback || name !== 'updateSettings')
    return;
  after(() => {
    void syncDesktopSettings(appStore.settings);
  });
});

changeAppearance(appStore.settings.appearance);
changeThemeColor(appStore.settings.themeColor);

window
  .matchMedia('(prefers-color-scheme: dark)')
  .addEventListener('change', () => {
    if (appStore.settings.appearance === 'auto') {
      changeAppearance(appStore.settings.appearance);
      changeThemeColor(appStore.settings.themeColor);
    }
  });

if (!isLastfmCallback) {
  window.yesplaymusic ??= {};
  mountPlayerState(appStore, appStore.player, window.yesplaymusic);
}

if (isDesktopRuntime && !isLastfmCallback) {
  watch(
    () => ({
      playing: appStore.player.playing,
      likedCurrentTrack: appStore.player.isCurrentTrackLiked,
      positionSeconds: appStore.player.progress,
      repeatMode: appStore.player.repeatMode,
      shuffle: appStore.player.shuffle,
    }),
    state => {
      void sendDesktop('mediaState', state);
    },
    { immediate: true }
  );
}

export default pinia;
