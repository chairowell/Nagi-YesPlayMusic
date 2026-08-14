import { describe, expect, test } from 'bun:test';
import {
  createSharedAudioProxy,
  getSharedCacheStatus,
  normalizeSharedCacheQuality,
  syncSharedCacheSetting,
} from '../src/services/sharedCache';

describe('GUI 共享歌曲缓存协议', () => {
  test('默认音质值映射到 core 支持的强类型档位', () => {
    expect(normalizeSharedCacheQuality(128000)).toBe(128000);
    expect(normalizeSharedCacheQuality(192000)).toBe(192000);
    expect(normalizeSharedCacheQuality(320000)).toBe(320000);
    expect(normalizeSharedCacheQuality('flac')).toBe(350000);
    expect(normalizeSharedCacheQuality(123456)).toBe(320000);
  });

  test('状态响应严格校验终端版检测标志', async () => {
    const status = await getSharedCacheStatus(async () =>
      Response.json({ enabled: false, terminalCacheDetected: true })
    );
    expect(status).toEqual({
      enabled: false,
      terminalCacheDetected: true,
    });
  });

  test('启用后生成同源音频代理 URL，并携带缓存写入元数据', async () => {
    const requests: Array<{ url: string; init?: RequestInit }> = [];
    const fetcher = async (
      input: RequestInfo | URL,
      init?: RequestInit
    ): Promise<Response> => {
      requests.push({
        url: String(input),
        ...(init === undefined ? {} : { init }),
      });
      return Response.json({ enabled: true, terminalCacheDetected: false });
    };
    await syncSharedCacheSetting(true, fetcher);
    const source = await createSharedAudioProxy({
      track: { id: 1868238759, name: 'Track' },
      quality: 'flac',
      source: 'https://audio.example/song.flac?token=signed',
      format: 'flac',
      actualBitrate: 999000,
      cache: true,
      origin: 'netease',
    });

    expect(requests[0]?.url).toBe('/api/native/shared-cache/settings');
    expect(requests[0]?.init?.body).toBe('{"enabled":true}');
    const url = new URL(source.url, 'http://127.0.0.1:28232');
    expect(url.pathname).toBe('/api/native/shared-cache/audio/1868238759');
    expect(url.searchParams.get('quality')).toBe('350000');
    expect(url.searchParams.get('source')).toBe(
      'https://audio.example/song.flac?token=signed'
    );
    expect(url.searchParams.get('codec')).toBe('flac');
    expect(url.searchParams.get('actualBitrate')).toBe('999000');
    expect(url.searchParams.get('cache')).toBe('true');
  });
});
