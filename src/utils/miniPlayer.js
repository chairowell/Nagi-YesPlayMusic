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

export function getMiniProgressRiderStyle(progressPercent) {
  const numericPercent = Number(progressPercent);
  const percent = Number.isFinite(numericPercent)
    ? Math.min(100, Math.max(0, numericPercent))
    : 0;
  return {
    left: `${percent}%`,
    // 角色自身也按进度逐渐向左收回：起点露出完整身体，终点的右边缘
    // 才刚好碰到轨道末端，不会因半个角色宽度而提前“跑完”。
    transform: `translateX(-${percent}%)`,
  };
}
