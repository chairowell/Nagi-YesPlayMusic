function finitePosition(value) {
  const position = Number(value);
  return Number.isFinite(position) && position >= 0 ? position : null;
}

/**
 * HTMLMediaElement 可能把流媒体 seek 修正到实际可解码位置。
 * 提交后立刻读回 Howler，确保 UI、歌词和声音共用真实落点。
 */
export function commitHowlerSeek(howler, requestedPosition) {
  const numericRequest = Number(requestedPosition);
  if (!howler || !Number.isFinite(numericRequest)) return null;
  // Media Session 的“后退 10 秒”可能产生负数；seek 入口统一落到 0，
  // 不能因为它越过开头就忽略整次操作。
  const requested = Math.max(0, numericRequest);

  howler.seek(requested);
  return finitePosition(howler.seek()) ?? requested;
}
