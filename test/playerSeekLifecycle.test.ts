import { describe, expect, mock, test } from 'bun:test';
import { markRaw, reactive } from 'vue';
import { getHowlerMediaNode } from '../src/utils/howlerMedia';
import { mountPlayerState } from '../src/utils/playerState';
import type Player from '../src/utils/Player';

// Bun does not read paths from a solution-style tsconfig.
mock.module('@/utils/howlerMedia', () => ({ getHowlerMediaNode }));
const { startHowlerSeek } = await import('../src/utils/playbackSeek');

interface TestPlayerCore {
  initialize(): void;
  saveSelfToLocalStorage(): void;
}

interface Gadget {
  prefetch(): void;
}

function asTestPlayer(player: TestPlayerCore): Player {
  return player as Player;
}

describe('seek 事务与 Howler 实例生命周期', () => {
  test('落在已 loaderror 的 Howl 上的 seek 事务永远不会自行 settle', () => {
    // Howler can remain loading after both terminal events were consumed.
    // A pending once() handler then requires an explicit cancellation.
    const deadHowler = {
      _sounds: [
        {
          _node: {
            currentTime: 37.2,
            seeking: false,
            error: { code: 2 },
            addEventListener() {},
            removeEventListener() {},
          },
        },
      ],
      _state: 'loading',
      once() {},
      off() {},
      seek() {
        return 37.2;
      },
    };

    const settled: number[] = [];
    const transaction = startHowlerSeek(deadHowler, 95, v => settled.push(v));

    if (!transaction) throw new Error('预期创建 seek 事务');
    expect(transaction.pending).toBe(true);
    expect(settled).toEqual([]);
  });

  test('音源元数据与升级中间态不触发整份播放器持久化', () => {
    const state = reactive<{ player: Player | null }>({ player: null });
    let persisted = 0;
    const rawPlayer: TestPlayerCore = {
      initialize() {},
      saveSelfToLocalStorage() {
        persisted += 1;
      },
    };

    const player = mountPlayerState(state, asTestPlayer(rawPlayer), {});
    player._currentSourceMeta = {
      origin: 'cache',
      format: 'flac',
      url: 'blob:test-cache',
    };
    player._seekToken = 7;
    player._pausePending = true;
    player._preciseSeekUpgrader = { request() {}, busy: false };

    expect(persisted).toBe(0);
  });

  test('defineProperty 的不可写对象属性必须 markRaw，否则响应式读取抛 Proxy 不变量错', () => {
    // WKWebView rejects proxied private fields, breaking the switch chain.
    const makePlayer = (
      wrapValue: (value: Gadget) => Gadget
    ): Player & { readonly _gadget: Gadget } => {
      const raw: TestPlayerCore = {
        initialize() {},
        saveSelfToLocalStorage() {},
      };
      Object.defineProperty(raw, '_gadget', {
        enumerable: false,
        value: wrapValue({ prefetch() {} }),
      });
      const state = reactive<{ player: Player | null }>({ player: null });
      const player = mountPlayerState(state, asTestPlayer(raw), {});
      return player as Player & { readonly _gadget: Gadget };
    };

    expect(() => makePlayer(value => value)._gadget).toThrow();
    expect(() => makePlayer(markRaw)._gadget.prefetch).not.toThrow();
  });
});
