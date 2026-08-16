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
// common.ts transitively pulls the axios layer (and the router with it);
// the adapters under test never touch it.
mock.module('@/utils/request', () => ({
  default: async () => {
    throw new Error('unexpected axios request');
  },
}));
// mapTrackPlayableStatus needs the auth module transitively; bun requires
// the mock to cover every export the real module has.
mock.module('@/utils/auth', () => ({
  setCookies: () => undefined,
  getCookie: () => undefined,
  removeCookie: () => undefined,
  isLoggedIn: () => true,
  isAccountLoggedIn: () => true,
  isUsernameLoggedIn: () => false,
  isLooseLoggedIn: () => true,
  doLogout: async () => true,
}));

const { fetchDailyRecommendTracks } = await import(
  '../src/services/recommendSource'
);

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

describe('每日推荐端点服务', () => {
  beforeEach(() => {
    fetchCalls = [];
  });

  test('条目映射回 ar/al/dt 形状并计算可播放状态', async () => {
    answerWith(200, {
      data: [
        {
          id: 1,
          name: 'Daily',
          artists: [{ id: 7, name: 'Artist' }],
          album: { id: 9, name: 'Album', picUrl: 'http://cover' },
          durationMs: 200000,
          alias: [],
          transNames: [],
          mark: 0,
          privilege: { pl: 320000, fee: 8 },
        },
      ],
    });
    const tracks = await fetchDailyRecommendTracks();
    expect(fetchCalls[0]).toBe('/api/native/recommend/daily-songs');
    expect(tracks[0]?.id).toBe(1);
    expect(tracks[0]?.al?.picUrl).toBe('http://cover');
    // pl > 0 means playable for a logged-in account.
    expect(tracks[0]?.playable).toBe(true);
  });

  test('HTTP 失败抛错、畸形载荷降级为空列表', async () => {
    answerWith(502, { status: 'error' });
    await expect(fetchDailyRecommendTracks()).rejects.toThrow(
      '每日推荐请求失败'
    );

    answerWith(200, { data: 'nope' });
    expect(await fetchDailyRecommendTracks()).toEqual([]);
  });
});
