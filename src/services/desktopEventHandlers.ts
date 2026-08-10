export interface DesktopEventView {
  pushRoute(path: unknown): unknown;
  focusSearch(): void;
  goHistory(where: 'back' | 'forward'): void;
  goToNextTracksPage(): void;
  requestCloseChoice(): void;
}

interface DesktopEventPlayer {
  isPersonalFM: boolean;
  volume: number;
  repeatMode: 'off' | 'on' | 'one';
  shuffle: boolean;
  currentTrack: { id: number };
  playOrPause(): unknown;
  play(): unknown;
  pause(): unknown;
  playNextFMTrack(): unknown;
  playNextTrack(): unknown;
  playPrevTrack(): unknown;
  switchRepeatMode(): unknown;
  switchShuffle(): unknown;
  seek(): number;
  seek(position: number): number;
}

export interface DesktopEventStore {
  showLyrics: boolean;
  toggleLyrics(): unknown;
  likeATrack(id: number): unknown;
  updateSettings(update: {
    key: 'closeAppOption';
    value: 'ask' | 'exit' | 'minimizeToTray';
  }): unknown;
}

export type DesktopEventHandler = (payload?: unknown) => unknown;

export function createDesktopEventHandlers(
  self: DesktopEventView,
  appStore: DesktopEventStore,
  player: DesktopEventPlayer
): Record<string, DesktopEventHandler> {
  return {
    changeRouteTo: path => {
      self.pushRoute(path);
      if (appStore.showLyrics) appStore.toggleLyrics();
    },
    search: () => {
      self.focusSearch();
    },
    play: () => player.playOrPause(),
    resume: () => player.play(),
    pause: () => player.pause(),
    next: () =>
      player.isPersonalFM ? player.playNextFMTrack() : player.playNextTrack(),
    previous: () => player.playPrevTrack(),
    increaseVolume: () => {
      player.volume = Math.min(1, player.volume + 0.1);
    },
    decreaseVolume: () => {
      player.volume = Math.max(0, player.volume - 0.1);
    },
    like: () => appStore.likeATrack(player.currentTrack.id),
    repeat: () => player.switchRepeatMode(),
    shuffle: () => player.switchShuffle(),
    routerGo: where => {
      if (where === 'back' || where === 'forward') self.goHistory(where);
    },
    nextUp: () => self.goToNextTracksPage(),
    requestCloseChoice: () => self.requestCloseChoice(),
    rememberCloseAppOption: value => {
      if (value === 'ask' || value === 'exit' || value === 'minimizeToTray') {
        appStore.updateSettings({ key: 'closeAppOption', value });
      }
    },
    setPosition: position => player.seek(Number(position) || 0),
    seekBy: offset => player.seek(player.seek() + (Number(offset) || 0)),
    setRepeat: mode => {
      if (mode === 'off' || mode === 'on' || mode === 'one') {
        player.repeatMode = mode;
      }
    },
    setShuffle: enabled => {
      if (typeof enabled === 'boolean') player.shuffle = enabled;
    },
  };
}
