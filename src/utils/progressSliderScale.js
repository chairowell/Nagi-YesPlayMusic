// vue-slider 要求 (max - min) / interval 为整数，否则 gap 会变成 Infinity，
// 数值仍正确但圆点和填充会直接跑到 100%。向上取整还能避免真实结束前先满格。
export const PLAYBACK_SLIDER_INTERVAL = 1;

export function normalizePlaybackSliderMax(duration) {
  const numericDuration = Number(duration);
  if (!Number.isFinite(numericDuration) || numericDuration <= 0) {
    return PLAYBACK_SLIDER_INTERVAL;
  }
  return Math.ceil(numericDuration);
}
