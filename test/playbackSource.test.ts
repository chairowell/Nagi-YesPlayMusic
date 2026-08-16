import { afterAll, beforeEach, describe, expect, mock, test } from 'bun:test';
import { readFileSync } from 'node:fs';

let musicQuality: number | 'flac' = 320000;

const getTestAppStore = () => ({ settings: { musicQuality } });

mock.module('@/stores/accessor', () => ({
  getAppStore: getTestAppStore,
  getAppStoreIfReady: getTestAppStore,
}));

const { playbackBitrate, resolveNeteasePlaybackSource } = await import(
  '../src/services/playbackSource'
);

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

  test('unavailable 与 rejected 都让链路继续，不抛错', async () => {
    answerWith({ status: 'unavailable' });
    expect(await resolveNeteasePlaybackSource(42)).toBeNull();

    answerWith({ status: 'rejected', code: 301 });
    expect(await resolveNeteasePlaybackSource(42)).toBeNull();
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
