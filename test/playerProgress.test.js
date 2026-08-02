import { describe, expect, test } from 'bun:test';
import { effect, isReactive, reactive } from 'vue';
import { mountPlayerState } from '../src/utils/playerState';

describe('播放器进度响应式绑定', () => {
  test('初始化时钟通过 Vue Proxy 更新进度，且进度心跳不反复持久化整份播放器', () => {
    const state = reactive({ player: null });
    const observedProgress = [];
    let tick;
    let initializedWithReactiveThis = false;
    let persisted = 0;

    const rawPlayer = {
      _progress: 0,
      initialize() {
        initializedWithReactiveThis = isReactive(this);
        tick = value => {
          this._progress = value;
        };
      },
      saveSelfToLocalStorage() {
        persisted += 1;
      },
      sendSelfToIpcMain() {},
    };

    mountPlayerState({ state }, rawPlayer, {});
    effect(() => observedProgress.push(state.player._progress));
    tick(17);

    expect(initializedWithReactiveThis).toBe(true);
    expect(observedProgress).toEqual([0, 17]);
    expect(persisted).toBe(0);
  });
});
