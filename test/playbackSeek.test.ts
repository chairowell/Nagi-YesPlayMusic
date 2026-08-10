import { describe, expect, mock, test } from 'bun:test';
import { getHowlerMediaNode } from '../src/utils/howlerMedia';

// Bun does not read paths from a solution-style tsconfig.
mock.module('@/utils/howlerMedia', () => ({ getHowlerMediaNode }));
const { startHowlerSeek } = await import('../src/utils/playbackSeek');

type HowlerSeekInput = NonNullable<Parameters<typeof startHowlerSeek>[0]>;
type SeekTransaction = NonNullable<ReturnType<typeof startHowlerSeek>>;
type TestListener = () => void;
type ListenerMap = Map<string, TestListener>;

interface TestMediaNode {
  currentTime: number;
  seeking: boolean;
  addEventListener(
    type: string,
    listener: TestListener,
    options?: AddEventListenerOptions | boolean
  ): void;
  removeEventListener(type: string, listener: TestListener): void;
}

function makeMediaNode(currentTime: number): {
  node: TestMediaNode;
  listeners: ListenerMap;
} {
  const listeners: ListenerMap = new Map();
  const node: TestMediaNode = {
    currentTime,
    seeking: false,
    addEventListener(type, listener) {
      listeners.set(type, listener);
    },
    removeEventListener(type, listener) {
      if (listeners.get(type) === listener) listeners.delete(type);
    },
  };
  return { node, listeners };
}

function emit(listeners: ListenerMap, type: string): void {
  const listener = listeners.get(type);
  if (!listener) throw new Error(`测试监听器 ${type} 未注册`);
  listener();
}

function requireTransaction(
  transaction: ReturnType<typeof startHowlerSeek>
): SeekTransaction {
  if (!transaction) throw new Error('预期创建 seek 事务');
  return transaction;
}

function readNullableNumber(value: number | null): number | null {
  return value;
}

describe('播放 seek 落点同步', () => {
  test('没有原生媒体节点时同步采用 Howler 落点', () => {
    let actualPosition = 12;
    const howler: HowlerSeekInput = {
      seek(value) {
        if (value !== undefined) {
          // Simulate WebKit snapping to a decodable frame.
          actualPosition = 41.75;
          return this;
        }
        return actualPosition;
      },
    };

    const settled: number[] = [];
    const transaction = requireTransaction(
      startHowlerSeek(howler, 43, value => {
        settled.push(value);
      })
    );

    expect(transaction.position).toBe(41.75);
    expect(transaction.pending).toBe(false);
    expect(settled).toEqual([41.75]);
  });

  test('媒体尚未返回有效落点时保留有限的请求时间', () => {
    const howler: HowlerSeekInput = {
      seek(value) {
        return value === undefined ? Number.NaN : this;
      },
    };

    const settled: number[] = [];
    const transaction = requireTransaction(
      startHowlerSeek(howler, 43, value => {
        settled.push(value);
      })
    );

    expect(transaction.position).toBe(43);
    expect(settled).toEqual([43]);
    expect(startHowlerSeek(null, 43)).toBeNull();
  });

  test('越过歌曲开头的 seek 会落到零点，而不是忽略操作', () => {
    let receivedPosition: number | null = null;
    const howler: HowlerSeekInput = {
      seek(value) {
        if (value !== undefined) {
          receivedPosition = value;
          return this;
        }
        return receivedPosition;
      },
    };

    const transaction = requireTransaction(startHowlerSeek(howler, -5));
    expect(transaction.position).toBe(0);
    expect(readNullableNumber(receivedPosition)).toBe(0);
  });

  test('WebKit 原生 seeked 到达前不把立即读回值当成最终落点', () => {
    const { node, listeners } = makeMediaNode(12);
    const howler: HowlerSeekInput = {
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
    const settled: number[] = [];

    const transaction = requireTransaction(
      startHowlerSeek(howler, 43, value => {
        settled.push(value);
      })
    );

    expect(transaction.position).toBe(43);
    expect(transaction.pending).toBe(true);
    expect(settled).toEqual([]);

    // WebKit may settle on a decodable frame instead of the requested time.
    node.currentTime = 41.75;
    node.seeking = false;
    emit(listeners, 'seeked');
    expect(settled).toEqual([41.75]);
  });

  test('新的拖拽可以取消旧 seeked 回调，避免旧落点覆盖后来一次拖拽', () => {
    const { node, listeners } = makeMediaNode(0);
    const howler: HowlerSeekInput = {
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
    const settled: number[] = [];
    const first = requireTransaction(
      startHowlerSeek(howler, 20, value => settled.push(value))
    );
    const staleListener = listeners.get('seeked');
    if (!staleListener) throw new Error('预期注册 seeked 监听器');

    first.cancel();
    node.currentTime = 19.5;
    node.seeking = false;
    staleListener();

    expect(settled).toEqual([]);
  });

  test('原生 seeked 缺失时由恢复播放后的 timeupdate 完成对齐', () => {
    const { node, listeners } = makeMediaNode(0);
    const howler: HowlerSeekInput = {
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
    const settled: number[] = [];
    startHowlerSeek(howler, 30, value => settled.push(value));

    emit(listeners, 'timeupdate');
    expect(settled).toEqual([]);

    node.currentTime = 29.5;
    node.seeking = false;
    emit(listeners, 'timeupdate');
    expect(settled).toEqual([29.5]);
  });

  test('Howler 尚未加载时等待排队的 seek 真正执行', () => {
    const { node, listeners: nativeListeners } = makeMediaNode(12);
    const howlerListeners: ListenerMap = new Map();
    let queuedSeek: number | null = null;
    const howler: HowlerSeekInput = {
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
    const settled: number[] = [];

    const transaction = requireTransaction(
      startHowlerSeek(howler, 43, value => {
        settled.push(value);
      })
    );

    expect(transaction.position).toBe(43);
    expect(transaction.pending).toBe(true);
    expect(settled).toEqual([]);
    expect(queuedSeek).toBeNull();

    // Ignore stale seeked events from a replaced media node.
    emit(nativeListeners, 'seeked');
    expect(settled).toEqual([]);

    // Apply the seek only after load completes.
    howler._state = 'loaded';
    emit(howlerListeners, 'load');
    if (queuedSeek === null) throw new Error('预期 load 后执行 seek');
    node.currentTime = queuedSeek;
    node.seeking = true;
    emit(howlerListeners, 'seek');
    expect(settled).toEqual([]);

    node.currentTime = 42.75;
    node.seeking = false;
    emit(nativeListeners, 'seeked');
    expect(settled).toEqual([42.75]);
  });

  test('Howler 内部 play lock 不发 play 事件时仍会继续 seek', async () => {
    const { node, listeners: nativeListeners } = makeMediaNode(18);
    const howlerListeners: ListenerMap = new Map();
    let appliedSeek: number | null = null;
    const howler: HowlerSeekInput = {
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
    const settled: number[] = [];

    const transaction = requireTransaction(
      startHowlerSeek(howler, 30, value => {
        settled.push(value);
      })
    );
    expect(transaction.pending).toBe(true);
    expect(appliedSeek).toBeNull();

    // Internal play(id, true) may drain the queue without emitting play.
    howler._playLock = false;
    await new Promise(resolve => setTimeout(resolve, 25));
    expect(readNullableNumber(appliedSeek)).toBe(30);
    expect(settled).toEqual([]);

    emit(howlerListeners, 'seek');
    node.currentTime = 29.75;
    node.seeking = false;
    emit(nativeListeners, 'seeked');
    expect(settled).toEqual([29.75]);
  });
});
