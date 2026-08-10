import { describe, expect, test } from 'bun:test';
import { effect, isReactive, reactive } from 'vue';
import { mountPlayerState } from '../src/utils/playerState';
import type Player from '../src/utils/Player';
import { Howl } from 'howler';

function createTestPlayer(): Player {
  return Object.assign(Object.create(null) as Player, {
    _progress: 0,
    _seeking: false,
    _pendingSeekCancel: null,
    _howler: null,
    _volume: 1,
    initialize: () => {},
    saveSelfToLocalStorage: () => {},
  });
}

describe('播放器进度响应式绑定', () => {
  test('初始化时钟通过 Vue Proxy 更新进度，且进度心跳不反复持久化整份播放器', () => {
    const state = reactive({ player: createTestPlayer() });
    const exposure: { player?: Player } = {};
    const observedProgress: number[] = [];
    const callbacks: {
      tick: ((value: number) => void) | null;
      beginSeek: (() => void) | null;
    } = { tick: null, beginSeek: null };
    let initializedWithReactiveThis = false;
    let persisted = 0;

    state.player.initialize = function (this: Player) {
      initializedWithReactiveThis = isReactive(this);
      callbacks.tick = value => {
        this._progress = value;
      };
      callbacks.beginSeek = () => {
        this._seeking = true;
        this._pendingSeekCancel = () => {};
      };
    };
    state.player.saveSelfToLocalStorage = () => {
      persisted += 1;
    };
    const mountedPlayer = mountPlayerState(state, state.player, exposure);
    effect(() => observedProgress.push(state.player._progress));
    expect(isReactive(state.player)).toBe(true);
    expect(isReactive(mountedPlayer)).toBe(true);
    expect(exposure.player).toBe(mountedPlayer);

    expect(callbacks.tick).not.toBeNull();
    expect(callbacks.beginSeek).not.toBeNull();
    callbacks.tick?.(17);
    callbacks.beginSeek?.();

    expect(initializedWithReactiveThis).toBe(true);
    expect(observedProgress).toEqual([0, 17]);
    expect(persisted).toBe(0);

    state.player._volume = 0.5;
    expect(persisted).toBe(1);
  });

  test('Howler 实例保留原始身份，结束事件不会被误判为旧实例', () => {
    const state = reactive({ player: createTestPlayer() });
    const howler = new Howl({ src: ['identity-test.mp3'], preload: false });
    state.player.initialize = function (this: Player) {
      this._howler = howler;
    };
    state.player.saveSelfToLocalStorage = () => {};

    mountPlayerState(state, state.player, {});

    expect(state.player._howler).toBe(howler);
    howler.unload();
  });
});
