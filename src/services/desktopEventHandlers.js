export function createDesktopEventHandlers(self, store, player) {
  return {
    changeRouteTo: path => {
      self.$router.push(path);
      if (store.state.showLyrics) store.commit('toggleLyrics');
    },
    search: () => {
      self.$refs.navbar.$refs.searchInput.focus();
      self.$refs.navbar.inputFocus = true;
    },
    play: () => player.playOrPause(),
    next: () =>
      player.isPersonalFM
        ? player.playNextFMTrack()
        : player.playNextTrack(),
    previous: () => player.playPrevTrack(),
    increaseVolume: () => {
      player.volume = Math.min(1, player.volume + 0.1);
    },
    decreaseVolume: () => {
      player.volume = Math.max(0, player.volume - 0.1);
    },
    like: () => store.dispatch('likeATrack', player.currentTrack.id),
    repeat: () => player.switchRepeatMode(),
    shuffle: () => player.switchShuffle(),
    routerGo: where => self.$refs.navbar.go(where),
    nextUp: () => self.$refs.player.goToNextTracksPage(),
    rememberCloseAppOption: value =>
      store.commit('updateSettings', { key: 'closeAppOption', value }),
    setPosition: position => player._howler.seek(position),
  };
}
