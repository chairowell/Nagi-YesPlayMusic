import { expect, test } from 'bun:test';
import { createDesktopEventHandlers } from '../src/services/desktopEventHandlers';

test('Tauri 与 Electron 的播放控制事件共用同一组动作', () => {
  const calls = [];
  const player = {
    isPersonalFM: false,
    volume: 0.95,
    currentTrack: { id: 42 },
    playOrPause: () => calls.push('play'),
    playNextTrack: () => calls.push('next'),
    playPrevTrack: () => calls.push('previous'),
    switchRepeatMode: () => calls.push('repeat'),
    switchShuffle: () => calls.push('shuffle'),
    _howler: { seek: position => calls.push(['seek', position]) },
  };
  const self = {
    $router: { push: path => calls.push(['route', path]) },
    $refs: {
      navbar: { go: where => calls.push(['go', where]) },
      player: { goToNextTracksPage: () => calls.push('nextUp') },
    },
  };
  const store = {
    state: { showLyrics: false },
    dispatch: () => {},
    commit: () => {},
  };
  const handlers = createDesktopEventHandlers(self, store, player);

  handlers.play();
  handlers.next();
  handlers.previous();
  handlers.repeat();
  handlers.shuffle();
  handlers.setPosition(12);
  handlers.increaseVolume();

  expect(calls).toEqual([
    'play',
    'next',
    'previous',
    'repeat',
    'shuffle',
    ['seek', 12],
  ]);
  expect(player.volume).toBe(1);
});
