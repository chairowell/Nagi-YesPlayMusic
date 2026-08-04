import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { effect, isReactive, reactive } from 'vue';
import { mountPlayerState } from '../src/utils/playerState';

describe('播放器进度响应式绑定', () => {
  test('初始化时钟通过 Vue Proxy 更新进度，且进度心跳不反复持久化整份播放器', () => {
    const state = reactive({ player: null });
    const observedProgress = [];
    let tick;
    let beginSeek;
    let initializedWithReactiveThis = false;
    let persisted = 0;

    const rawPlayer = {
      _progress: 0,
      initialize() {
        initializedWithReactiveThis = isReactive(this);
        tick = value => {
          this._progress = value;
        };
        beginSeek = () => {
          this._seeking = true;
          this._pendingSeekCancel = () => {};
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
    beginSeek();

    expect(initializedWithReactiveThis).toBe(true);
    expect(observedProgress).toEqual([0, 17]);
    expect(persisted).toBe(0);
  });

  test('Howler 实例保留原始身份，结束事件不会被误判为旧实例', () => {
    const state = reactive({ player: null });
    const howler = {};
    const rawPlayer = {
      _howler: null,
      initialize() {
        this._howler = howler;
      },
      saveSelfToLocalStorage() {},
      sendSelfToIpcMain() {},
    };

    mountPlayerState({ state }, rawPlayer, {});

    expect(state.player._howler).toBe(howler);
  });

  test('恢复上次播放位置也必须走统一 seek，不能保存未经媒体确认的请求值', () => {
    const playerSource = readFileSync(
      fileURLToPath(new URL('../src/utils/Player.js', import.meta.url)),
      'utf8'
    );

    expect(playerSource).toContain('this.seek(savedTrackTime, false);');
    expect(playerSource).not.toContain('this._howler?.seek(savedTrackTime);');
  });

  test('歌词时钟会等待 WebKit 原生 seeked，避免声音尚未落地时先跳词', () => {
    const lyricsSource = readFileSync(
      fileURLToPath(new URL('../src/views/lyrics.vue', import.meta.url)),
      'utf8'
    );

    expect(lyricsSource).toContain('if (this.player.seeking) return;');
  });
});
