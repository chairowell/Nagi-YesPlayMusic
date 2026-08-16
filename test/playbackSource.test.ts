import { afterAll, beforeEach, describe, expect, mock, test } from 'bun:test';
import { readFileSync } from 'node:fs';

let musicQuality: number | 'flac' = 320000;
let enableRealIP = false;
let realIP: string | null = null;
let proxyConfig: { protocol: string; server: string; port: number } = {
  protocol: 'noProxy',
  server: '',
  port: 0,
};

const getTestAppStore = () => ({
  settings: { musicQuality, enableRealIP, realIP, proxyConfig },
});

mock.module('@/stores/accessor', () => ({
  getAppStore: getTestAppStore,
  getAppStoreIfReady: getTestAppStore,
}));
// The web-only hardcoded real-IP fallback must stay out of desktop requests.
mock.module('@/utils/runtime', () => ({
  isDesktopRuntime: true,
  isTauriRuntime: true,
}));

const { fetchNeteaseLyrics, playbackBitrate, resolveNeteasePlaybackSource } =
  await import('../src/services/playbackSource');

const originalFetch = globalThis.fetch;
let fetchCalls: string[] = [];

function installFetch(handler: (input: RequestInfo | URL) => Promise<unknown>) {
  globalThis.fetch = (async (input: RequestInfo | URL) =>
    handler(input)) as unknown as typeof fetch;
}

function answerWith(payload: unknown, ok = true) {
  installFetch(async input => {
    fetchCalls.push(String(input));
    return { ok, json: async () => payload };
  });
}

afterAll(() => {
  globalThis.fetch = originalFetch;
});

describe('播放源解析服务', () => {
  beforeEach(() => {
    musicQuality = 320000;
    enableRealIP = false;
    realIP = null;
    proxyConfig = { protocol: 'noProxy', server: '', port: 0 };
    fetchCalls = [];
  });

  test('五档音质设置与 sidecar 的 wire bitrate 契约一致', () => {
    const cases: unknown = JSON.parse(
      readFileSync(
        new URL(
          '../src-tauri/sidecar/src/fixtures/audio-quality-cases.json',
          import.meta.url
        ),
        'utf8'
      )
    );
    if (!Array.isArray(cases)) throw new Error('音质 fixture 格式无效');
    for (const entry of cases as { setting: number | 'flac'; wire: string }[]) {
      expect(String(playbackBitrate(entry.setting))).toBe(entry.wire);
    }
  });

  test('ok 响应映射成类型化播放源，bitrate 来自设置', async () => {
    musicQuality = 'flac';
    answerWith({
      status: 'ok',
      url: 'https://audio.example/track.flac',
      codec: 'flac',
      actualBitrate: 850321,
      expectedBytes: 12345678,
      expectedMd5: 'ab'.repeat(16),
    });

    const resolved = await resolveNeteasePlaybackSource(186016);

    expect(fetchCalls[0]).toBe(
      '/api/native/playback/source/186016?bitrate=350000'
    );
    expect(resolved).toEqual({
      url: 'https://audio.example/track.flac',
      codec: 'flac',
      actualBitrate: 850321,
      expectedBytes: 12345678,
      expectedMd5: 'ab'.repeat(16),
    });
  });

  test('真实 IP 与代理设置随请求透传，与 axios 拦截器同一语义', async () => {
    enableRealIP = true;
    realIP = '1.2.3.4';
    proxyConfig = { protocol: 'HTTP', server: '127.0.0.1', port: 7890 };
    answerWith({ status: 'unavailable' });

    await resolveNeteasePlaybackSource(42);

    const url = new URL(fetchCalls[0] ?? '', 'http://localhost');
    expect(url.searchParams.get('realIP')).toBe('1.2.3.4');
    expect(url.searchParams.get('proxy')).toBe('HTTP://127.0.0.1:7890');

    // Off by default: desktop requests carry neither knob.
    enableRealIP = false;
    proxyConfig = { protocol: 'noProxy', server: '', port: 0 };
    fetchCalls = [];
    answerWith({ status: 'unavailable' });
    await resolveNeteasePlaybackSource(42);
    expect(fetchCalls[0]).toBe('/api/native/playback/source/42?bitrate=320000');
  });

  test('unavailable 与 rejected 都让链路继续，不抛错', async () => {
    answerWith({ status: 'unavailable' });
    expect(await resolveNeteasePlaybackSource(42)).toBeNull();

    answerWith({ status: 'rejected', code: 301 });
    expect(await resolveNeteasePlaybackSource(42)).toBeNull();
  });

  test('歌词映射回 LyricsResponse 老形状，空段落省略', async () => {
    answerWith({
      lrc: '[00:01]line',
      tlyric: '[00:01]翻译',
      romalrc: null,
      yrc: null,
    });

    const lyrics = await fetchNeteaseLyrics(186016);

    expect(fetchCalls[0]).toBe('/api/native/playback/lyrics/186016');
    expect(lyrics).toEqual({
      lrc: { lyric: '[00:01]line' },
      tlyric: { lyric: '[00:01]翻译' },
    });

    answerWith({ lrc: '', tlyric: null, romalrc: null, yrc: null });
    expect(await fetchNeteaseLyrics(42)).toEqual({});
  });

  test('歌词请求失败会抛错，保持 axios 时代的调用方语义', async () => {
    answerWith({ status: 'error' }, false);
    await expect(fetchNeteaseLyrics(42)).rejects.toThrow('歌词请求失败');
  });

  test('传输失败与畸形响应都降级为 null', async () => {
    installFetch(async () => {
      throw new Error('connection refused');
    });
    expect(await resolveNeteasePlaybackSource(42)).toBeNull();

    answerWith({ status: 'error', message: 'boom' }, false);
    expect(await resolveNeteasePlaybackSource(42)).toBeNull();

    answerWith({ status: 'ok', url: 42 });
    expect(await resolveNeteasePlaybackSource(42)).toBeNull();
  });
});
