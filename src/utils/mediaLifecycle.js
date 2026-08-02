/**
 * Plyr 会注册媒体事件并持有 video 节点，离开页面时必须显式销毁。
 */
export function destroyMediaPlayer(player) {
  player?.destroy?.();
}

export function stopInterval(timer, clear = clearInterval) {
  if (timer !== null && timer !== undefined) clear(timer);
}

/**
 * 可见时保持界面刷新频率，隐藏时降频但不完全停掉后台状态同步。
 * 返回统一清理函数，避免组件切换后遗留计时器或 visibility 监听。
 */
export function startVisibilityAwareInterval(
  target,
  callback,
  {
    foregroundMs,
    backgroundMs,
    setTimer = setInterval,
    clearTimer = clearInterval,
  }
) {
  let timer = null;
  const schedule = () => {
    stopInterval(timer, clearTimer);
    timer = setTimer(
      callback,
      target.hidden === true ? backgroundMs : foregroundMs
    );
  };
  const stopObserving = listen(target, 'visibilitychange', schedule);
  schedule();

  return () => {
    stopObserving();
    stopInterval(timer, clearTimer);
    timer = null;
  };
}

export function listen(target, type, handler, options) {
  target.addEventListener(type, handler, options);
  return () => target.removeEventListener(type, handler, options);
}

/**
 * WebView 被隐藏后仍可能继续合成 CSS 动画；统一暴露可观察状态，
 * 让根组件暂停纯视觉工作，同时不影响后台音频播放。
 */
export function observeDocumentVisibility(target, onChange) {
  const sync = () => onChange(target.hidden === true);
  sync();
  return listen(target, 'visibilitychange', sync);
}

export function disposeListeners(cleanups) {
  for (const cleanup of cleanups) cleanup();
  cleanups.length = 0;
}
