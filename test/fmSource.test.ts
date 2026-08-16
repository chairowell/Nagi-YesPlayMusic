import { afterAll, beforeEach, describe, expect, mock, test } from 'bun:test';

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
mock.module('@/utils/runtime', () => ({
  isDesktopRuntime: true,
  isTauriRuntime: true,
}));

const { fetchPersonalFM } = await import('../src/services/fmSource');

const originalFetch = globalThis.fetch;
let fetchCalls: string[] = [];

function answerWith(status: number, payload?: unknown) {
  globalThis.fetch = (async (input: RequestInfo | URL) => {
    fetchCalls.push(String(input));
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

describe('私人FM端点服务', () => {
  beforeEach(() => {
    fetchCalls = [];
  });

  test('条目映射回老 personal_fm 形状（artists/album/duration 命名）', async () => {
    answerWith(200, {
      data: [
        {
          id: 42,
          name: 'FM Track',
          artists: [{ id: 7, name: 'Artist' }],
          album: { id: 9, name: 'Album', picUrl: 'http://cover' },
          durationMs: 180000,
        },
        // Rows without a numeric id cannot be played; they are dropped.
        { name: 'broken' },
      ],
    });
    const result = await fetchPersonalFM();
    expect(fetchCalls[0]).toBe('/api/native/fm/personal');
    expect(result.data).toEqual([
      {
        id: 42,
        name: 'FM Track',
        artists: [{ id: 7, name: 'Artist' }],
        album: { id: 9, name: 'Album', picUrl: 'http://cover' },
        duration: 180000,
      },
    ]);
  });

  test('HTTP 失败抛错、畸形载荷降级为空列表', async () => {
    answerWith(502, { status: 'error' });
    await expect(fetchPersonalFM()).rejects.toThrow('私人FM请求失败');

    answerWith(200, { data: 'nope' });
    expect((await fetchPersonalFM()).data).toEqual([]);
  });
});
