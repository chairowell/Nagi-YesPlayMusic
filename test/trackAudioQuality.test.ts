import { beforeEach, describe, expect, mock, test } from 'bun:test';
import type { AxiosRequestConfig } from 'axios';
import type { Decoder } from '../src/api/decoders';

let musicQuality: number | 'flac' = 'flac';
const requests: AxiosRequestConfig[] = [];

const request = mock(
  async <TResponse>(
    config: AxiosRequestConfig,
    decoder: Decoder<TResponse>
  ): Promise<TResponse> => {
    requests.push(config);
    return decoder(
      {
        code: 200,
        data: [
          {
            id: 42,
            url: 'https://music.example/lossless.flac',
            type: 'flac',
            br: 999000,
          },
        ],
      },
      { url: config.url ?? '<unknown URL>' }
    );
  }
);

const getTestAppStore = () => ({ settings: { musicQuality } });

mock.module('@/stores/accessor', () => ({
  getAppStore: getTestAppStore,
  getAppStoreIfReady: getTestAppStore,
}));
mock.module('@/utils/request', () => ({ default: request }));

const { getMP3 } = await import('../src/api/track');

describe('音源质量契约', () => {
  beforeEach(() => {
    musicQuality = 'flac';
    requests.length = 0;
    request.mockClear();
  });

  test('无损设置请求原版 Electron 使用的 350000 档位', async () => {
    const response = await getMP3(42);

    expect(requests).toEqual([
      {
        url: '/song/url',
        method: 'get',
        params: { id: 42, br: '350000' },
      },
    ]);
    expect(response.data[0]).toMatchObject({
      id: 42,
      url: 'https://music.example/lossless.flac',
      type: 'flac',
      br: 999000,
    });
  });

  test('有损音质档位保持数值不变', async () => {
    musicQuality = 320000;

    await getMP3('track-id');

    expect(requests[0]?.params).toEqual({ id: 'track-id', br: 320000 });
  });
});
