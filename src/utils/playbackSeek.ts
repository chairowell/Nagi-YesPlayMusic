import { getHowlerMediaNode } from '@/utils/howlerMedia';

interface HowlerSeekLike {
  _sounds?: Array<{ _node?: unknown }>;
  _state?: string;
  _playLock?: boolean;
  state?: () => string;
  once?: (event: string, listener: () => void) => void;
  off?: (event: string, listener: () => void) => void;
  seek(position?: number): unknown;
}

export interface SeekTransaction {
  position: number;
  pending: boolean;
  cancel: () => void;
}

function finitePosition(value: unknown): number | null {
  const position = Number(value);
  return Number.isFinite(position) && position >= 0 ? position : null;
}

/**
 * Howler returns after setting currentTime, while WebKit may still be locating a
 * decodable frame. Wait for native seeked before resuming lyric timing.
 */
export function startHowlerSeek(
  howler: HowlerSeekLike | null | undefined,
  requestedPosition: unknown,
  onSettled?: (position: number) => void
): SeekTransaction | null {
  const numericRequest = Number(requestedPosition);
  if (!howler || !Number.isFinite(numericRequest)) return null;
  const activeHowler = howler;

  const requested = Math.max(0, numericRequest);
  const node = getHowlerMediaNode(activeHowler);
  const observesHowlerEvents =
    typeof activeHowler.once === 'function' &&
    typeof activeHowler.off === 'function';
  let active = true;
  let seekApplied = !observesHowlerEvents;
  let seekRequested = false;
  let readinessRetry: ReturnType<typeof setTimeout> | null = null;

  const readPosition = () =>
    finitePosition(node?.currentTime) ??
    finitePosition(activeHowler.seek()) ??
    requested;
  function clearReadinessWait() {
    if (readinessRetry !== null) {
      clearTimeout(readinessRetry);
      readinessRetry = null;
    }
    if (!observesHowlerEvents) return;
    activeHowler.off?.('load', applySeek);
    activeHowler.off?.('play', applySeek);
    activeHowler.off?.('playerror', applySeek);
  }
  function cleanup() {
    node?.removeEventListener('seeked', settleWhenReady);
    node?.removeEventListener('timeupdate', settleWhenReady);
    node?.removeEventListener('error', settle);
    clearReadinessWait();
    if (!observesHowlerEvents) return;
    activeHowler.off?.('seek', markSeekApplied);
    activeHowler.off?.('loaderror', settle);
  }
  function settle() {
    if (!active) return;
    active = false;
    cleanup();
    onSettled?.(readPosition());
  }
  function settleWhenReady() {
    if (seekApplied && node?.seeking !== true) settle();
  }
  function markSeekApplied() {
    if (!active) return;
    seekApplied = true;
    settleWhenReady();
  }
  function isReadyToSeek() {
    const state =
      typeof activeHowler.state === 'function'
        ? activeHowler.state()
        : activeHowler._state;
    return (
      (state === undefined || state === 'loaded') && !activeHowler._playLock
    );
  }
  function waitUntilReady() {
    if (!observesHowlerEvents) return false;
    const state =
      typeof activeHowler.state === 'function'
        ? activeHowler.state()
        : activeHowler._state;
    if (state !== undefined && state !== 'loaded') {
      activeHowler.once?.('load', applySeek);
      return true;
    }
    if (activeHowler._playLock) {
      // Internal playback may unlock without emitting play; poll as fallback.
      activeHowler.once?.('play', applySeek);
      activeHowler.once?.('playerror', applySeek);
      readinessRetry = setTimeout(applySeek, 16);
      return true;
    }
    return false;
  }
  function applySeek() {
    if (!active || seekRequested) return;
    clearReadinessWait();
    if (!isReadyToSeek() && waitUntilReady()) return;
    seekRequested = true;
    if (observesHowlerEvents) activeHowler.once?.('seek', markSeekApplied);
    activeHowler.seek(requested);
    if (!observesHowlerEvents) settleWhenReady();
  }
  const cancel = () => {
    if (!active) return;
    active = false;
    cleanup();
  };

  // Register first because some implementations seek synchronously.
  node?.addEventListener('seeked', settleWhenReady);
  // Use timeupdate after playback resumes when the backend swallows seeked.
  node?.addEventListener('timeupdate', settleWhenReady);
  node?.addEventListener('error', settle, { once: true });
  if (observesHowlerEvents) activeHowler.once?.('loaderror', settle);

  if (!waitUntilReady()) applySeek();

  // Before load, show the target but wait for load -> seek -> seeked.
  const position = seekApplied ? readPosition() : requested;
  return { position, pending: active, cancel };
}
