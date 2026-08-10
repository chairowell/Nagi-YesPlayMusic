export function calculateMiniSeekTime(
  clientX: number,
  trackLeft: number,
  trackWidth: number,
  duration: number
): number {
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

export function getMiniProgressRiderStyle(progressPercent: unknown): {
  left: string;
  transform: string;
} {
  const numericPercent = Number(progressPercent);
  const percent = Number.isFinite(numericPercent)
    ? Math.min(100, Math.max(0, numericPercent))
    : 0;
  return {
    left: `${percent}%`,
    // Offset the sprite so its trailing edge reaches the track only at 100%.
    transform: `translateX(-${percent}%)`,
  };
}
