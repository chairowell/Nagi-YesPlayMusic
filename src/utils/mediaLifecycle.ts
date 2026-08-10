/**
 * Plyr retains media listeners and its video node until explicitly destroyed.
 */
interface Destroyable {
  destroy?: () => void;
}

type TimerHandle = ReturnType<typeof setInterval>;
type ClearTimer = (timer: TimerHandle) => void;

interface VisibilityTarget extends EventTarget {
  readonly hidden: boolean;
}

interface VisibilityIntervalOptions {
  foregroundMs: number;
  backgroundMs: number;
  setTimer?: (callback: () => void, delay: number) => TimerHandle;
  clearTimer?: ClearTimer;
}

export function destroyMediaPlayer(player: Destroyable | null | undefined) {
  player?.destroy?.();
}

export function stopInterval(
  timer: TimerHandle | null | undefined,
  clear: ClearTimer = clearInterval
) {
  if (timer !== null && timer !== undefined) clear(timer);
}

/**
 * Throttle hidden views without stopping background state synchronization.
 * The returned cleanup removes both timers and the visibility listener.
 */
export function startVisibilityAwareInterval(
  target: VisibilityTarget,
  callback: () => void,
  {
    foregroundMs,
    backgroundMs,
    setTimer = setInterval,
    clearTimer = clearInterval,
  }: VisibilityIntervalOptions
): () => void {
  let timer: TimerHandle | null = null;
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

export function listen(
  target: EventTarget,
  type: string,
  handler: EventListenerOrEventListenerObject,
  options?: boolean | AddEventListenerOptions
): () => void {
  target.addEventListener(type, handler, options);
  return () => target.removeEventListener(type, handler, options);
}

/**
 * Expose WebView visibility so visual work can pause without stopping audio.
 */
export function observeDocumentVisibility(
  target: VisibilityTarget,
  onChange: (hidden: boolean) => void
): () => void {
  const sync = () => onChange(target.hidden === true);
  sync();
  return listen(target, 'visibilitychange', sync);
}

export function disposeListeners(cleanups: Array<() => void>): void {
  for (const cleanup of cleanups) cleanup();
  cleanups.length = 0;
}
