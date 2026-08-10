import { describe, expect, test } from 'bun:test';
import {
  PLAYBACK_SLIDER_INTERVAL,
  normalizePlaybackSliderMax,
} from '../src/utils/progressSliderScale';

describe('大窗口进度条刻度', () => {
  test('小数音频时长会向上对齐到整秒，刻度总数保持整数', () => {
    const duration = 326.426667;
    const max = normalizePlaybackSliderMax(duration);

    expect(max).toBe(327);
    expect(max).toBeGreaterThanOrEqual(duration);
    expect(max - duration).toBeLessThan(PLAYBACK_SLIDER_INTERVAL);
    expect(max / PLAYBACK_SLIDER_INTERVAL).toBeInteger();
  });

  test('音频尚未加载时仍提供一个有效刻度，不产生 Infinity 位置', () => {
    expect(normalizePlaybackSliderMax(0)).toBe(PLAYBACK_SLIDER_INTERVAL);
    expect(normalizePlaybackSliderMax(Number.NaN)).toBe(
      PLAYBACK_SLIDER_INTERVAL
    );
  });
});
