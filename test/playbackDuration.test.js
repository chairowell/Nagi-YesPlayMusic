import { describe, expect, test } from 'bun:test';
import { resolvePlaybackDuration } from '../src/utils/playbackDuration';

describe('播放进度时长', () => {
  test('加载前保留歌曲元数据的毫秒精度，不再提前一秒满格', () => {
    expect(resolvePlaybackDuration(263130, 0)).toBe(263.13);
  });

  test('音频加载后以实际可播放时长为准', () => {
    expect(resolvePlaybackDuration(263130, 263.2098)).toBe(263.2098);
  });

  test('无效时长不会污染进度条上限', () => {
    expect(resolvePlaybackDuration(0, Number.POSITIVE_INFINITY)).toBe(1);
  });
});
