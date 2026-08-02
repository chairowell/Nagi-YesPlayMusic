export function calculateMiniSeekTime(
  clientX,
  trackLeft,
  trackWidth,
  duration
) {
  if (
    !Number.isFinite(clientX) ||
    !Number.isFinite(trackLeft) ||
    !Number.isFinite(trackWidth) ||
    !Number.isFinite(duration) ||
    trackWidth <= 0 ||
    duration <= 0
  ) {
    return 0;
  }
  const ratio = Math.min(1, Math.max(0, (clientX - trackLeft) / trackWidth));
  return ratio * duration;
}
