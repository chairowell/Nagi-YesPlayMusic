import { describe, expect, test } from 'bun:test';
import {
  createSharedAudioProxy,
  findSharedCachedAudio,
  getSharedCacheStatus,
  isSharedAudioProxyURL,
  isSharedCacheHealthy,
  migrateIndexedDbTracksToSharedCache,
  normalizeSharedCacheQuality,
  prefetchSharedAudio,
  reportSharedCacheFailure,
  sharedCacheQualityFromBitrate,
  shouldUseSharedAudioProxy,
  syncSharedCacheSetting,
} from '../src/services/sharedCache';
import type { TrackSourceRecord } from '../src/utils/db';

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
    await syncSharedCacheSetting(true, null, fetcher);
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

  test('同步失败不会毒化后续调用，只把共享缓存标记为不健康', async () => {
    const failing = async (): Promise<Response> =>
      new Response('boom', { status: 500 });
    await expect(syncSharedCacheSetting(true, null, failing)).rejects.toThrow(
      'shared cache configuration failed with HTTP 500'
    );
    expect(await isSharedCacheHealthy()).toBe(false);

    // 队列本身必须保持 resolved：这些调用以前会同步抛出上一次的失败
    let lookups = 0;
    const lookup = async (): Promise<Response> => {
      lookups += 1;
      return new Response(null, { status: 200 });
    };
    expect(await findSharedCachedAudio(1868238759, 'flac', lookup)).toBeNull();
    expect(lookups).toBe(0);
    await prefetchSharedAudio(
      '/api/native/shared-cache/audio/1868238759',
      async () => new Response('audio')
    );
  });

  test('不健康时音源解析回退直连，重新同步成功后恢复代理', async () => {
    const ok = async (): Promise<Response> => Response.json({ enabled: true });
    await syncSharedCacheSetting(true, null, ok);
    expect(await shouldUseSharedAudioProxy(true)).toBe(true);

    const proxied = await createSharedAudioProxy({
      track: { id: 1868238759, name: 'Track' },
      quality: 'flac',
      source: 'https://audio.example/song.flac',
      format: 'flac',
      actualBitrate: 999000,
      cache: true,
      origin: 'netease',
    });
    expect(isSharedAudioProxyURL(proxied.url)).toBe(true);

    // Player 的重试链路探测到代理 URL 加载失败后调用这一句
    reportSharedCacheFailure();
    expect(await shouldUseSharedAudioProxy(true)).toBe(false);

    await syncSharedCacheSetting(true, null, ok);
    expect(await shouldUseSharedAudioProxy(true)).toBe(true);
  });

  test('代理返回 409 说明 Sidecar 侧开关已关，立即降级为直连', async () => {
    await syncSharedCacheSetting(
      true,
      null,
      async () => new Response(null, { status: 204 })
    );
    expect(await shouldUseSharedAudioProxy(true)).toBe(true);

    const conflict = async (): Promise<Response> =>
      new Response(null, { status: 409 });
    expect(
      await findSharedCachedAudio(1868238759, 'flac', conflict)
    ).toBeNull();
    expect(await isSharedCacheHealthy()).toBe(false);
    expect(await shouldUseSharedAudioProxy(true)).toBe(false);
  });
});

function mp3Buffer(): ArrayBuffer {
  // ID3v2 magic so sniffAudioFormat resolves the codec to mp3.
  return new Uint8Array([0x49, 0x44, 0x33, 0x04, 0x00, 0x00, 0x00, 0x00])
    .buffer;
}

function flacBuffer(): ArrayBuffer {
  // "fLaC" magic.
  return new Uint8Array([0x66, 0x4c, 0x61, 0x43, 0x00, 0x00, 0x22, 0x12])
    .buffer;
}

function migrationRecord(
  overrides: Partial<TrackSourceRecord> & Pick<TrackSourceRecord, 'id'>
): TrackSourceRecord {
  return {
    validatedTrackID: overrides.id,
    source: mp3Buffer(),
    bitRate: 128000,
    from: 'netease',
    name: 'Track',
    artist: 'Artist',
    createTime: 1723680000000,
    ...overrides,
  };
}

function memoryStorage(): {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
} {
  const map = new Map<string, string>();
  return {
    getItem: key => map.get(key) ?? null,
    setItem: (key, value) => void map.set(key, value),
  };
}

describe('IndexedDB 迁移的音质档位', () => {
  test('码率映射到规范档位，未知码率返回 null', () => {
    expect(sharedCacheQualityFromBitrate(96000)).toBe(128000);
    expect(sharedCacheQualityFromBitrate(128000)).toBe(128000);
    expect(sharedCacheQualityFromBitrate(192000)).toBe(192000);
    expect(sharedCacheQualityFromBitrate(320000)).toBe(320000);
    expect(sharedCacheQualityFromBitrate(908000)).toBe(350000);
    expect(sharedCacheQualityFromBitrate(0)).toBeNull();
    expect(sharedCacheQualityFromBitrate(Number.NaN)).toBeNull();
  });

  test('128k 记录按自身码率写入，不使用当前音质设置的 key', async () => {
    const imports: Array<Record<string, unknown>> = [];
    const fetcher = async (
      _input: RequestInfo | URL,
      init?: RequestInit
    ): Promise<Response> => {
      const body = init?.body;
      if (body instanceof FormData) {
        const metadata = body.get('metadata');
        if (typeof metadata === 'string') {
          imports.push(JSON.parse(metadata) as Record<string, unknown>);
        }
      }
      return new Response(null, { status: 204 });
    };

    const result = await migrateIndexedDbTracksToSharedCache({
      onProgress: () => undefined,
      fetcher,
      storage: memoryStorage(),
      listIds: async () => [101],
      readRecord: async id =>
        migrationRecord({ id, bitRate: 128000, source: mp3Buffer() }),
    });

    expect(result).toEqual({
      completed: 1,
      total: 1,
      imported: 1,
      skipped: 0,
    });
    expect(imports).toHaveLength(1);
    expect(imports[0]?.['quality']).toBe(128000);
    expect(imports[0]?.['quality']).not.toBe(320000);
    expect(imports[0]?.['codec']).toBe('mp3');
  });

  test('无损记录按码率归入 350000 档', async () => {
    const imports: Array<Record<string, unknown>> = [];
    const fetcher = async (
      _input: RequestInfo | URL,
      init?: RequestInit
    ): Promise<Response> => {
      const body = init?.body;
      if (body instanceof FormData) {
        const metadata = body.get('metadata');
        if (typeof metadata === 'string') {
          imports.push(JSON.parse(metadata) as Record<string, unknown>);
        }
      }
      return new Response(null, { status: 204 });
    };

    await migrateIndexedDbTracksToSharedCache({
      onProgress: () => undefined,
      fetcher,
      storage: memoryStorage(),
      listIds: async () => [202],
      readRecord: async id =>
        migrationRecord({ id, bitRate: 908000, source: flacBuffer() }),
    });

    expect(imports[0]?.['quality']).toBe(350000);
    expect(imports[0]?.['codec']).toBe('flac');
  });

  test('码率缺失的记录被跳过并计数，不猜档位', async () => {
    let requests = 0;
    const fetcher = async (): Promise<Response> => {
      requests += 1;
      return new Response(null, { status: 204 });
    };

    const result = await migrateIndexedDbTracksToSharedCache({
      onProgress: () => undefined,
      fetcher,
      storage: memoryStorage(),
      listIds: async () => [301, 302],
      readRecord: async id =>
        migrationRecord({
          id,
          bitRate: id === 301 ? 0 : Number.NaN,
        }),
    });

    expect(requests).toBe(0);
    expect(result).toEqual({
      completed: 2,
      total: 2,
      imported: 0,
      skipped: 2,
    });
  });
});
