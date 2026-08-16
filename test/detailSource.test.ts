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

const { fetchAlbumDetail, fetchArtistDetail, fetchPlaylistDetail } =
  await import('../src/services/detailSource');

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

const song = {
  id: 1,
  name: 'Song',
  artists: [{ id: 7, name: 'Artist' }],
  album: { id: 9, name: 'Album', picUrl: 'http://cover' },
  durationMs: 1000,
};

describe('详情页端点服务', () => {
  beforeEach(() => {
    fetchCalls = [];
  });

  test('歌单：元数据原样透传，songs 变回 tracks 的 ar/al/dt 形状', async () => {
    answerWith(200, {
      playlist: {
        id: 3,
        name: '歌单',
        trackCount: 2,
        trackIds: [{ id: 1 }, { id: 2 }],
        creator: { userId: 9, nickname: 'n' },
        unknownFutureField: true,
      },
      songs: [song],
    });
    const data = await fetchPlaylistDetail(3);
    expect(fetchCalls[0]).toBe('/api/native/playlist/detail?id=3');
    expect(data.playlist?.['unknownFutureField']).toBe(true);
    expect(data.playlist?.trackIds).toEqual([{ id: 1 }, { id: 2 }]);
    expect(data.playlist?.tracks[0]?.al?.picUrl).toBe('http://cover');
  });

  test('专辑与歌手：各自的页面形状', async () => {
    answerWith(200, { album: { id: 9, name: 'Album' }, songs: [song] });
    const album = await fetchAlbumDetail(9);
    expect(fetchCalls[0]).toBe('/api/native/album/detail?id=9');
    expect(album.album.name).toBe('Album');
    expect(album.songs[0]?.id).toBe(1);

    answerWith(200, { artist: { id: 7, name: 'Artist' }, hotSongs: [song] });
    const artist = await fetchArtistDetail(7);
    expect(fetchCalls[1]).toBe('/api/native/artist/detail?id=7');
    expect(artist.artist.name).toBe('Artist');
    expect(artist.hotSongs[0]?.ar).toEqual([{ id: 7, name: 'Artist' }]);
  });

  test('HTTP 失败抛错、缺 meta 抛错', async () => {
    answerWith(502, { status: 'error' });
    await expect(fetchPlaylistDetail(3)).rejects.toThrow('详情请求失败');

    answerWith(200, { songs: [] });
    await expect(fetchAlbumDetail(9)).rejects.toThrow('详情响应缺少 album');
  });
});
