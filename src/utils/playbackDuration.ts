function normalizeDuration(value: unknown): number {
  const duration = Number(value);
  return Number.isFinite(duration) && duration > 0 ? duration : 0;
}

/**
 * Metadata is provisional because cached and unlocked sources may differ.
 * Prefer the browser-decoded duration once audio loads.
 */
export function resolvePlaybackDuration(
  trackDurationMs: unknown,
  audioDurationSeconds: unknown
): number {
  const audioDuration = normalizeDuration(audioDurationSeconds);
  if (audioDuration) return audioDuration;

  const trackDuration = normalizeDuration(trackDurationMs) / 1000;
  return trackDuration || 1;
}
