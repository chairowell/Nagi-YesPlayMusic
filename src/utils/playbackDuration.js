function normalizeDuration(value) {
  const duration = Number(value);
  return Number.isFinite(duration) && duration > 0 ? duration : 0;
}

/**
 * 音源可能来自缓存或解锁 provider，长度不一定与网易云元数据完全相同。
 * 音频加载前用元数据占位，加载后必须服从浏览器实际解码出的时长。
 */
export function resolvePlaybackDuration(trackDurationMs, audioDurationSeconds) {
  const audioDuration = normalizeDuration(audioDurationSeconds);
  if (audioDuration) return audioDuration;

  const trackDuration = normalizeDuration(trackDurationMs) / 1000;
  return trackDuration || 1;
}
