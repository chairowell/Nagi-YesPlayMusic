import { describe, expect, test } from 'bun:test';
import {
  observeDocumentVisibility,
  disposeListeners,
  destroyMediaPlayer,
  listen,
  stopInterval,
} from '../src/utils/mediaLifecycle';

describe('媒体组件生命周期', () => {
  test('离开 MV 页面时销毁播放器', () => {
    let destroyed = 0;
    destroyMediaPlayer({ destroy: () => (destroyed += 1) });
    expect(destroyed).toBe(1);
  });

  test('播放器尚未创建时也能安全退出', () => {
    expect(() => destroyMediaPlayer(null)).not.toThrow();
  });
});

describe('轮询生命周期', () => {
  test('组件卸载时停止已创建的轮询', () => {
    const cleared = [];
    stopInterval(42, timer => cleared.push(timer));
    expect(cleared).toEqual([42]);
  });

  test('未创建轮询时不会调用清理函数', () => {
    const cleared = [];
    stopInterval(null, timer => cleared.push(timer));
    expect(cleared).toEqual([]);
  });
});

describe('全局事件生命周期', () => {
  test('注册和销毁使用同一个事件处理函数', () => {
    const calls = [];
    const target = {
      addEventListener: (type, handler) => calls.push(['add', type, handler]),
      removeEventListener: (type, handler) =>
        calls.push(['remove', type, handler]),
    };
    const handler = () => {};
    const cleanups = [listen(target, 'keydown', handler)];

    disposeListeners(cleanups);

    expect(calls).toEqual([
      ['add', 'keydown', handler],
      ['remove', 'keydown', handler],
    ]);
    expect(cleanups).toEqual([]);
  });

  test('窗口隐藏和恢复时同步后台状态并在卸载时停止监听', () => {
    const listeners = new Map();
    const documentTarget = {
      hidden: true,
      addEventListener: (type, handler) => listeners.set(type, handler),
      removeEventListener: (type, handler) => {
        if (listeners.get(type) === handler) listeners.delete(type);
      },
    };
    const states = [];

    const cleanup = observeDocumentVisibility(documentTarget, hidden =>
      states.push(hidden)
    );
    documentTarget.hidden = false;
    listeners.get('visibilitychange')();
    cleanup();

    expect(states).toEqual([true, false]);
    expect(listeners.has('visibilitychange')).toBe(false);
  });
});
