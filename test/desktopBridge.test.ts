import { expect, test } from 'bun:test';
import { createDesktopEventHandlers } from '../src/services/desktopEventHandlers';

test('桌面播放控制事件使用同一组动作', () => {
  const calls: unknown[] = [];
  const player = {
    isPersonalFM: false,
    volume: 0.95,
    repeatMode: 'off' as 'off' | 'on' | 'one',
    shuffle: false as boolean,
    currentTrack: { id: 42 },
    playOrPause: () => calls.push('play'),
    play: () => calls.push('resume'),
    pause: () => calls.push('pause'),
    playNextFMTrack: () => calls.push('nextFM'),
    playNextTrack: () => calls.push('next'),
    playPrevTrack: () => calls.push('previous'),
    switchRepeatMode: () => calls.push('repeat'),
    switchShuffle: () => calls.push('shuffle'),
    seek: (position?: number) => {
      if (position === undefined) return 12;
      calls.push(['seek', position]);
      return position;
    },
  } satisfies Parameters<typeof createDesktopEventHandlers>[2];
  const view = {
    pushRoute: (path: unknown) => calls.push(['route', path]),
    focusSearch: () => calls.push('search'),
    goHistory: (where: 'back' | 'forward') => calls.push(['go', where]),
    goToNextTracksPage: () => calls.push('nextUp'),
    requestCloseChoice: () => calls.push('closePrompt'),
  } satisfies Parameters<typeof createDesktopEventHandlers>[0];
  const appStore = Object.assign(
    Object.create(null) as Parameters<typeof createDesktopEventHandlers>[1],
    {
      showLyrics: false,
      toggleLyrics: () => calls.push('toggleLyrics'),
      likeATrack: (id: number) => calls.push(['like', id]),
      updateSettings: (payload: unknown) => calls.push(['settings', payload]),
      showToast: (message: string) => calls.push(['toast', message]),
    }
  );
  const handlers = createDesktopEventHandlers(view, appStore, player);
  const emit = (channel: string, payload?: unknown) => {
    const handler = handlers[channel];
    if (!handler) throw new Error(`桌面事件处理器未注册: ${channel}`);
    return handler(payload);
  };

  emit('changeRouteTo', '/settings');
  emit('search');
  emit('play');
  emit('resume');
  emit('pause');
  emit('next');
  emit('previous');
  emit('repeat');
  emit('shuffle');
  emit('routerGo', 'back');
  emit('nextUp');
  emit('requestCloseChoice');
  emit('setPosition', 12);
  emit('seekBy', -2);
  emit('setRepeat', 'one');
  emit('setShuffle', true);
  emit('increaseVolume');
  emit('like');
  emit('rememberCloseAppOption', 'minimizeToTray');
  emit('sidecarUnavailable', '后台服务重启失败');

  expect(calls).toEqual([
    ['route', '/settings'],
    'search',
    'play',
    'resume',
    'pause',
    'next',
    'previous',
    'repeat',
    'shuffle',
    ['go', 'back'],
    'nextUp',
    'closePrompt',
    ['seek', 12],
    ['seek', 10],
    ['like', 42],
    ['settings', { key: 'closeAppOption', value: 'minimizeToTray' }],
    ['toast', '后台服务重启失败'],
  ]);
  expect(player.volume).toBe(1);
  expect(player.repeatMode).toBe('one');
  expect(player.shuffle).toBe(true);
});
