import { describe, expect, test } from 'bun:test';
import {
  observeDocumentVisibility,
  startVisibilityAwareInterval,
  disposeListeners,
  destroyMediaPlayer,
  listen,
  stopInterval,
} from '../src/utils/mediaLifecycle';

type TimerHandle = ReturnType<typeof setInterval>;

class MutableVisibilityTarget extends EventTarget {
  hidden = false;
}

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
    const timer = setInterval(() => {}, 60_000);
    const cleared: TimerHandle[] = [];
    stopInterval(timer, handle => {
      cleared.push(handle);
      clearInterval(handle);
    });
    expect(cleared).toEqual([timer]);
  });

  test('未创建轮询时不会调用清理函数', () => {
    const cleared: TimerHandle[] = [];
    stopInterval(null, timer => cleared.push(timer));
    expect(cleared).toEqual([]);
  });

  test('窗口隐藏后降低轮询频率，恢复时切回前台频率', () => {
    const target = new MutableVisibilityTarget();
    const scheduled: number[] = [];
    const timers: TimerHandle[] = [];
    const cleared: TimerHandle[] = [];

    const cleanup = startVisibilityAwareInterval(target, () => {}, {
      foregroundMs: 50,
      backgroundMs: 250,
      setTimer: (callback, delay) => {
        scheduled.push(delay);
        const timer = setInterval(callback, 60_000);
        timers.push(timer);
        return timer;
      },
      clearTimer: timer => {
        cleared.push(timer);
        clearInterval(timer);
      },
    });
    target.hidden = true;
    target.dispatchEvent(new Event('visibilitychange'));
    cleanup();
    target.hidden = false;
    target.dispatchEvent(new Event('visibilitychange'));

    expect(scheduled).toEqual([50, 250]);
    expect(cleared).toEqual(timers);
  });
});

describe('全局事件生命周期', () => {
  test('注册和销毁使用同一个事件处理函数', () => {
    const target = new EventTarget();
    let calls = 0;
    const handler = () => {
      calls += 1;
    };
    const cleanups = [listen(target, 'keydown', handler)];

    target.dispatchEvent(new Event('keydown'));
    disposeListeners(cleanups);
    target.dispatchEvent(new Event('keydown'));

    expect(calls).toBe(1);
    expect(cleanups).toEqual([]);
  });

  test('窗口隐藏和恢复时同步后台状态并在卸载时停止监听', () => {
    const documentTarget = new MutableVisibilityTarget();
    documentTarget.hidden = true;
    const states: boolean[] = [];

    const cleanup = observeDocumentVisibility(documentTarget, hidden =>
      states.push(hidden)
    );
    documentTarget.hidden = false;
    documentTarget.dispatchEvent(new Event('visibilitychange'));
    cleanup();
    documentTarget.hidden = true;
    documentTarget.dispatchEvent(new Event('visibilitychange'));

    expect(states).toEqual([true, false]);
  });
});
