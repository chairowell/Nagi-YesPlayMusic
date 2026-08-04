import { describe, expect, test } from 'bun:test';
import { startHowlerSeek } from '../src/utils/playbackSeek';

describe('播放 seek 落点同步', () => {
  test('没有原生媒体节点时同步采用 Howler 落点', () => {
    let actualPosition = 12;
    const howler = {
      seek(value) {
        if (value !== undefined) {
          // 模拟 WebKit / 流媒体把请求时间修正到实际可解码位置。
          actualPosition = 41.75;
          return this;
        }
        return actualPosition;
      },
    };

    const settled = [];
    const transaction = startHowlerSeek(howler, 43, value => {
      settled.push(value);
    });

    expect(transaction.position).toBe(41.75);
    expect(transaction.pending).toBe(false);
    expect(settled).toEqual([41.75]);
  });

  test('媒体尚未返回有效落点时保留有限的请求时间', () => {
    const howler = {
      seek(value) {
        return value === undefined ? Number.NaN : this;
      },
    };

    const settled = [];
    const transaction = startHowlerSeek(howler, 43, value => {
      settled.push(value);
    });

    expect(transaction.position).toBe(43);
    expect(settled).toEqual([43]);
    expect(startHowlerSeek(null, 43)).toBeNull();
  });

  test('越过歌曲开头的 seek 会落到零点，而不是忽略操作', () => {
    let receivedPosition = null;
    const howler = {
      seek(value) {
        if (value !== undefined) {
          receivedPosition = value;
          return this;
        }
        return receivedPosition;
      },
    };

    expect(startHowlerSeek(howler, -5).position).toBe(0);
    expect(receivedPosition).toBe(0);
  });

  test('WebKit 原生 seeked 到达前不把立即读回值当成最终落点', () => {
    const listeners = new Map();
    const node = {
      currentTime: 12,
      seeking: false,
      addEventListener(type, listener) {
        listeners.set(type, listener);
      },
      removeEventListener(type, listener) {
        if (listeners.get(type) === listener) listeners.delete(type);
      },
    };
    const howler = {
      _sounds: [{ _node: node }],
      seek(value) {
        if (value !== undefined) {
          node.currentTime = value;
          node.seeking = true;
          return this;
        }
        return node.currentTime;
      },
    };
    const settled = [];

    const transaction = startHowlerSeek(howler, 43, value => {
      settled.push(value);
    });

    expect(transaction.position).toBe(43);
    expect(transaction.pending).toBe(true);
    expect(settled).toEqual([]);

    // 模拟 WebKit 最终落到可解码帧，而不是刚写入 currentTime 的请求值。
    node.currentTime = 41.75;
    node.seeking = false;
    listeners.get('seeked')();
    expect(settled).toEqual([41.75]);
  });

  test('新的拖拽可以取消旧 seeked 回调，避免旧落点覆盖后来一次拖拽', () => {
    const listeners = new Map();
    const node = {
      currentTime: 0,
      seeking: false,
      addEventListener(type, listener) {
        listeners.set(type, listener);
      },
      removeEventListener(type, listener) {
        if (listeners.get(type) === listener) listeners.delete(type);
      },
    };
    const howler = {
      _sounds: [{ _node: node }],
      seek(value) {
        if (value !== undefined) {
          node.currentTime = value;
          node.seeking = true;
          return this;
        }
        return node.currentTime;
      },
    };
    const settled = [];
    const first = startHowlerSeek(howler, 20, value => settled.push(value));
    const staleListener = listeners.get('seeked');

    first.cancel();
    node.currentTime = 19.5;
    node.seeking = false;
    staleListener();

    expect(settled).toEqual([]);
  });

  test('原生 seeked 缺失时由恢复播放后的 timeupdate 完成对齐', () => {
    const listeners = new Map();
    const node = {
      currentTime: 0,
      seeking: false,
      addEventListener(type, listener) {
        listeners.set(type, listener);
      },
      removeEventListener(type, listener) {
        if (listeners.get(type) === listener) listeners.delete(type);
      },
    };
    const howler = {
      _sounds: [{ _node: node }],
      seek(value) {
        if (value !== undefined) {
          node.currentTime = value;
          node.seeking = true;
          return this;
        }
        return node.currentTime;
      },
    };
    const settled = [];
    startHowlerSeek(howler, 30, value => settled.push(value));

    listeners.get('timeupdate')();
    expect(settled).toEqual([]);

    node.currentTime = 29.5;
    node.seeking = false;
    listeners.get('timeupdate')();
    expect(settled).toEqual([29.5]);
  });

  test('Howler 尚未加载时等待排队的 seek 真正执行', () => {
    const nativeListeners = new Map();
    const howlerListeners = new Map();
    const node = {
      currentTime: 12,
      seeking: false,
      addEventListener(type, listener) {
        nativeListeners.set(type, listener);
      },
      removeEventListener(type, listener) {
        if (nativeListeners.get(type) === listener) {
          nativeListeners.delete(type);
        }
      },
    };
    let queuedSeek = null;
    const howler = {
      _sounds: [{ _node: node }],
      _state: 'loading',
      once(type, listener) {
        howlerListeners.set(type, listener);
      },
      off(type, listener) {
        if (howlerListeners.get(type) === listener) {
          howlerListeners.delete(type);
        }
      },
      seek(value) {
        if (value !== undefined) {
          queuedSeek = value;
          return this;
        }
        return node.currentTime;
      },
    };
    const settled = [];

    const transaction = startHowlerSeek(howler, 43, value => {
      settled.push(value);
    });

    expect(transaction.position).toBe(43);
    expect(transaction.pending).toBe(true);
    expect(settled).toEqual([]);
    expect(queuedSeek).toBeNull();

    // 旧媒体节点晚到的 seeked 不能冒充这次尚未执行的恢复操作。
    nativeListeners.get('seeked')();
    expect(settled).toEqual([]);

    // load 完成后，本事务才要求 Howler 写入 currentTime，不往内部队列塞旧 seek。
    howler._state = 'loaded';
    howlerListeners.get('load')();
    node.currentTime = queuedSeek;
    node.seeking = true;
    howlerListeners.get('seek')();
    expect(settled).toEqual([]);

    node.currentTime = 42.75;
    node.seeking = false;
    nativeListeners.get('seeked')();
    expect(settled).toEqual([42.75]);
  });

  test('Howler 内部 play lock 不发 play 事件时仍会继续 seek', async () => {
    const nativeListeners = new Map();
    const howlerListeners = new Map();
    const node = {
      currentTime: 18,
      seeking: false,
      addEventListener(type, listener) {
        nativeListeners.set(type, listener);
      },
      removeEventListener(type, listener) {
        if (nativeListeners.get(type) === listener) {
          nativeListeners.delete(type);
        }
      },
    };
    let appliedSeek = null;
    const howler = {
      _sounds: [{ _node: node }],
      _state: 'loaded',
      _playLock: true,
      once(type, listener) {
        howlerListeners.set(type, listener);
      },
      off(type, listener) {
        if (howlerListeners.get(type) === listener) {
          howlerListeners.delete(type);
        }
      },
      seek(value) {
        if (value !== undefined) {
          appliedSeek = value;
          node.currentTime = value;
          node.seeking = true;
          return this;
        }
        return node.currentTime;
      },
    };
    const settled = [];

    const transaction = startHowlerSeek(howler, 30, value => {
      settled.push(value);
    });
    expect(transaction.pending).toBe(true);
    expect(appliedSeek).toBeNull();

    // Howler 的内部 play(id, true) 成功时只解锁并跑队列，不一定 emit('play')。
    howler._playLock = false;
    await new Promise(resolve => setTimeout(resolve, 25));
    expect(appliedSeek).toBe(30);
    expect(settled).toEqual([]);

    howlerListeners.get('seek')();
    node.currentTime = 29.75;
    node.seeking = false;
    nativeListeners.get('seeked')();
    expect(settled).toEqual([29.75]);
  });
});
