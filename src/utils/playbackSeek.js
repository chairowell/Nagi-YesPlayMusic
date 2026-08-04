import { getHowlerMediaNode } from '@/utils/howlerMedia';

function finitePosition(value) {
  const position = Number(value);
  return Number.isFinite(position) && position >= 0 ? position : null;
}

/**
 * Howler 在 HTML5 模式里写完 currentTime 就会立即返回；但 WebKit 此时仍可能
 * 正在寻找可解码帧。必须等原生 seeked 才能把最终位置交给歌词时钟。
 */
export function startHowlerSeek(howler, requestedPosition, onSettled) {
  const numericRequest = Number(requestedPosition);
  if (!howler || !Number.isFinite(numericRequest)) return null;

  const requested = Math.max(0, numericRequest);
  const node = getHowlerMediaNode(howler);
  const observesHowlerEvents =
    typeof howler.once === 'function' && typeof howler.off === 'function';
  let active = true;
  let seekApplied = !observesHowlerEvents;
  let seekRequested = false;
  let readinessRetry = null;

  const readPosition = () =>
    finitePosition(node?.currentTime) ??
    finitePosition(howler.seek()) ??
    requested;
  function clearReadinessWait() {
    if (readinessRetry !== null) {
      clearTimeout(readinessRetry);
      readinessRetry = null;
    }
    if (!observesHowlerEvents) return;
    howler.off('load', applySeek);
    howler.off('play', applySeek);
    howler.off('playerror', applySeek);
  }
  function cleanup() {
    node?.removeEventListener('seeked', settleWhenReady);
    node?.removeEventListener('timeupdate', settleWhenReady);
    node?.removeEventListener('error', settle);
    clearReadinessWait();
    if (!observesHowlerEvents) return;
    howler.off('seek', markSeekApplied);
    howler.off('loaderror', settle);
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
      typeof howler.state === 'function' ? howler.state() : howler._state;
    return (state === undefined || state === 'loaded') && !howler._playLock;
  }
  function waitUntilReady() {
    if (!observesHowlerEvents) return false;
    const state =
      typeof howler.state === 'function' ? howler.state() : howler._state;
    if (state !== undefined && state !== 'loaded') {
      howler.once('load', applySeek);
      return true;
    }
    if (howler._playLock) {
      // Howler 的内部 play(id, true) 解锁后不一定 emit('play')；事件负责快路径，
      // 状态重试兜住无事件路径。这里只等锁，不用延迟猜音频 seek 是否完成。
      howler.once('play', applySeek);
      howler.once('playerror', applySeek);
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
    if (observesHowlerEvents) howler.once('seek', markSeekApplied);
    howler.seek(requested);
    if (!observesHowlerEvents) settleWhenReady();
  }
  const cancel = () => {
    if (!active) return;
    active = false;
    cleanup();
  };

  // 监听必须先于写 currentTime，兼容无需解码跳转时同步完成的实现。
  node?.addEventListener('seeked', settleWhenReady);
  // 标准 seeked 被底层吞掉时，以播放恢复后的 timeupdate 状态为准；
  // 不用固定延迟猜解码完成时间。
  node?.addEventListener('timeupdate', settleWhenReady);
  node?.addEventListener('error', settle, { once: true });
  if (observesHowlerEvents) howler.once('loaderror', settle);

  if (!waitUntilReady()) applySeek();

  // 未加载时 Howler 还没有真正写 currentTime；UI 先显示用户请求的位置，
  // 等 load -> seek -> 原生 seeked 完整落地后再放行歌词时钟。
  const position = seekApplied ? readPosition() : requested;
  return { position, pending: active, cancel };
}
