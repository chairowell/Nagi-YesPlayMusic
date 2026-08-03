import { describe, expect, test } from 'bun:test';
import {
  configureSafeNeteaseApiCache,
  findMatchingAudioResponse,
  isTrustedTrackSource,
} from '../src/utils/audioCacheIntegrity';

describe('网易云 API 响应缓存安全', () => {
  test('禁用会忽略 query/body 的上游两分钟内存缓存', () => {
    let configured;
    configureSafeNeteaseApiCache({
      options(options) {
        configured = options;
      },
    });

    expect(configured).toEqual({ enabled: false });
  });

  test('音源响应必须包含当前歌曲 ID，不能拿上一首的结果兜底', () => {
    const response = [
      { id: 2001472, url: 'https://example.com/safe-and-sound.mp3' },
      { id: 1868238759, url: 'https://example.com/abcdefu.mp3' },
    ];

    expect(findMatchingAudioResponse(response, 1868238759)?.url).toEndWith(
      'abcdefu.mp3'
    );
    expect(findMatchingAudioResponse(response.slice(0, 1), 1868238759)).toBe(
      null
    );
  });
});

describe('历史音频缓存可信标记', () => {
  test('旧记录没有校验标记时按需失效，避免继续播放已经串台的音频', () => {
    expect(isTrustedTrackSource({ id: 1868238759 }, 1868238759)).toBe(false);
    expect(
      isTrustedTrackSource(
        { id: 1868238759, validatedTrackID: 1868238759 },
        1868238759
      )
    ).toBe(true);
    expect(
      isTrustedTrackSource(
        { id: 1868238759, validatedTrackID: 2001472 },
        1868238759
      )
    ).toBe(false);
  });
});
