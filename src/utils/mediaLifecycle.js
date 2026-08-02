/**
 * Plyr 会注册媒体事件并持有 video 节点，离开页面时必须显式销毁。
 */
export function destroyMediaPlayer(player) {
  player?.destroy?.();
}

export function stopInterval(timer, clear = clearInterval) {
  if (timer !== null && timer !== undefined) clear(timer);
}

export function listen(target, type, handler, options) {
  target.addEventListener(type, handler, options);
  return () => target.removeEventListener(type, handler, options);
}

export function disposeListeners(cleanups) {
  for (const cleanup of cleanups) cleanup();
  cleanups.length = 0;
}
