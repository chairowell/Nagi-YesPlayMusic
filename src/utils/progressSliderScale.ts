// vue-slider requires an integral range-to-step ratio or its gap becomes Infinity.
export const PLAYBACK_SLIDER_INTERVAL = 1;

export function normalizePlaybackSliderMax(duration: unknown): number {
  const numericDuration = Number(duration);
  if (!Number.isFinite(numericDuration) || numericDuration <= 0) {
    return PLAYBACK_SLIDER_INTERVAL;
  }
  return Math.ceil(numericDuration);
}
