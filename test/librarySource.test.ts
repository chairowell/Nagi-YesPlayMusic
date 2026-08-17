import { afterAll, beforeEach, describe, expect, mock, test } from 'bun:test';
import { resolveRuntime as realResolveRuntime } from '../src/utils/runtime';

const getTestAppStore = () => ({
  settings: {
    musicQuality: 320000,
    enableRealIP: false,
    realIP: null,
    proxyConfig: { protocol: 'noProxy', server: '', port: 0 },
  },
});

mock.module('@/stores/accessor', () => ({
  getAppStore: getTestAppStore,
  getAppStoreIfReady: getTestAppStore,
}));
// Cover every export (a later test file may import this module after the
// mock replaced it), but forward the real resolveRuntime: runtime.test.ts
// exercises it and bun's mock.module is process-global.
mock.module('@/utils/runtime', () => ({
  resolveRuntime: realResolveRuntime,
  isDesktopRuntime: true,
  isTauriRuntime: true,
}));

const { fetchLikedSongIds, likeTrack, trashFM } = await import(
  '../src/services/librarySource'
);

const originalFetch = globalThis.fetch;
let fetchCalls: { url: string; method: string | undefined }[] = [];

function answerWith(status: number, payload?: unknown) {
  globalThis.fetch = (async (input: RequestInfo | URL, init?: RequestInit) => {
    fetchCalls.push({ url: String(input), method: init?.method });
    return {
      ok: status >= 200 && status < 300,
      status,
      json: async () => payload,
    };
  }) as unknown as typeof fetch;
}

afterAll(() => {
  globalThis.fetch = originalFetch;
});

describe('资料库端点服务', () => {
  beforeEach(() => {
    fetchCalls = [];
  });

  test('喜欢列表保序返回，畸形载荷降级为空对象', async () => {
    answerWith(200, { ids: [3, 1, 2] });
    // Most-recently-liked-first order feeds the library tiles directly.
    expect(await fetchLikedSongIds(42)).toEqual({ ids: [3, 1, 2] });
    expect(fetchCalls[0]?.url).toBe('/api/native/library/liked-ids?uid=42');
    expect(fetchCalls[0]?.method).toBe('GET');

    answerWith(200, { ids: 'nope' });
    expect(await fetchLikedSongIds(42)).toEqual({});
  });

  test('收藏与垃圾桶成功静默、失败抛错（沿用调用方 catch 语义）', async () => {
    answerWith(204);
    await likeTrack({ id: 7 });
    expect(fetchCalls[0]?.url).toBe('/api/native/library/like?id=7&like=true');
    expect(fetchCalls[0]?.method).toBe('POST');

    answerWith(204);
    await trashFM(99);
    expect(fetchCalls[1]?.url).toBe('/api/native/library/fm-trash?id=99');

    answerWith(422, { status: 'rejected', code: -462 });
    await expect(likeTrack({ id: 7, like: false })).rejects.toThrow(
      '资料库请求失败'
    );
  });
});
