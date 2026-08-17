import { afterAll, beforeEach, describe, expect, mock, test } from 'bun:test';
import { resolveRuntime as realResolveRuntime } from '../src/utils/runtime';

const getTestAppStore = () => ({
  settings: {
    musicQuality: 320000,
    enableRealIP: false,
    realIP: null,
    proxyConfig: { protocol: 'noProxy', server: '', port: 0 },
  },
  data: { user: {} },
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
// common.ts transitively pulls the axios layer (and the router with it);
// the adapters under test never touch it.
mock.module('@/utils/request', () => ({
  default: async () => {
    throw new Error('unexpected axios request');
  },
}));
mock.module('@/utils/auth', () => ({
  setCookies: () => undefined,
  getCookie: () => undefined,
  removeCookie: () => undefined,
  isLoggedIn: () => false,
  isAccountLoggedIn: () => false,
  isUsernameLoggedIn: () => false,
  isLooseLoggedIn: () => false,
  doLogout: async () => false,
}));

const { searchAlbums, searchTracks, searchUsers } = await import(
  '../src/services/searchSource'
);

const originalFetch = globalThis.fetch;
let fetchCalls: string[] = [];

function answerWith(payload: unknown) {
  globalThis.fetch = (async (input: RequestInfo | URL) => {
    fetchCalls.push(String(input));
    return { ok: true, json: async () => payload };
  }) as unknown as typeof fetch;
}

afterAll(() => {
  globalThis.fetch = originalFetch;
});

describe('类型化搜索适配层', () => {
  beforeEach(() => {
    fetchCalls = [];
  });

  test('歌曲命中映射回 TrackList 需要的老形状并保留灰显字段', async () => {
    answerWith({
      channel: 'songs',
      total: 240,
      items: [
        {
          id: 186016,
          name: '晴天',
          artists: [{ id: 6452, name: '周杰伦' }],
          album: { id: 18905, name: '叶惠美', picUrl: 'https://x/cover.jpg' },
          durationMs: 269000,
          alias: [],
          transNames: ['Sunny Day'],
          mark: 1048576,
          fee: 1,
          noCopyrightRcmd: false,
          privilege: { pl: 0, cs: false, fee: 1, st: 0 },
        },
      ],
    });

    const page = await searchTracks('晴天', { limit: 16, offset: 32 });

    const url = new URL(fetchCalls[0] ?? '', 'http://localhost');
    expect(url.pathname).toBe('/api/native/search');
    expect(url.searchParams.get('type')).toBe('1');
    expect(url.searchParams.get('limit')).toBe('16');
    expect(url.searchParams.get('offset')).toBe('32');

    expect(page.total).toBe(240);
    const track = page.items[0];
    expect(track?.ar?.[0]?.id).toBe(6452);
    expect(track?.al?.picUrl).toBe('https://x/cover.jpg');
    expect(track?.tns).toEqual(['Sunny Day']);
    // isTrackPlayable treats mere presence as 无版权 — false must be absent.
    expect(track && 'noCopyrightRcmd' in track).toBe(false);
    // fee 1 + logged out → VIP Only greying survives the adaptation.
    expect(track?.playable).toBe(false);
    expect(track?.reason).toBe('VIP Only');
  });

  test('专辑命中带可链接歌手引用，用户命中供登录页使用', async () => {
    answerWith({
      channel: 'albums',
      total: 12,
      items: [
        {
          id: 18905,
          name: '叶惠美',
          artist: { id: 6452, name: '周杰伦' },
          picUrl: null,
          mark: 0,
        },
      ],
    });
    const albums = await searchAlbums('叶惠美');
    expect(albums.items[0]?.artist?.id).toBe(6452);
    expect(albums.items[0] && 'picUrl' in albums.items[0]).toBe(false);

    answerWith({
      channel: 'users',
      total: 2,
      items: [{ userId: 32953014, nickname: '圈圈', avatarUrl: null }],
    });
    const users = await searchUsers('圈圈', { limit: 9 });
    expect(users.items[0]?.userId).toBe(32953014);
    expect(users.items[0]?.nickname).toBe('圈圈');
    // Settings badges on vipType !== 0, so a missing field must read 0.
    expect(users.items[0]?.vipType).toBe(0);
  });

  test('传输失败抛错，让视图的部分失败提示继续工作', async () => {
    globalThis.fetch = (async () => {
      throw new Error('connection refused');
    }) as unknown as typeof fetch;
    await expect(searchTracks('x')).rejects.toThrow();
  });
});
