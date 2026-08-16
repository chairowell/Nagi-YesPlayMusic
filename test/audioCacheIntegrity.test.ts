import { describe, expect, test } from 'bun:test';
import {
  configureSafeNeteaseApiCache,
  isTrustedTrackSource,
} from '../src/utils/audioCacheIntegrity';

describe('网易云 API 响应缓存安全', () => {
  test('禁用会忽略 query/body 的上游两分钟内存缓存', () => {
    let configured: { enabled: boolean } | undefined;
    configureSafeNeteaseApiCache({
      options(options) {
        configured = options;
      },
    });

    expect(configured).toEqual({ enabled: false });
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
