// 迷你播放条里只有这些地方保留自己的交互，其余一律是"按住挪窗口"。
// .mini-copyable 只包住文字本身，所以"压在字上"才让位给选中复制。
export const MINI_WINDOW_INTERACTIVE_SELECTOR = [
  '.mini-copyable',
  '.mini-controls',
  '.mini-progress-track',
  'button',
  'a',
  'input',
  'textarea',
  'select',
  '[contenteditable="true"]',
].join(', ');

export function isMiniWindowInteractiveTarget(target) {
  return Boolean(target?.closest?.(MINI_WINDOW_INTERACTIVE_SELECTOR));
}

export function shouldStartMiniWindowDrag(event) {
  return (
    event?.button === 0 &&
    event?.detail === 1 &&
    !isMiniWindowInteractiveTarget(event.target)
  );
}

// 按在空白处 = 接下来要挪窗口。必须在 mousedown 当场取消默认行为：
// 否则 WebKit 已经把选中的起点埋在这里，鼠标一划过歌名歌词就把它们拉黑，
// 表现为"想挪窗口却选了一串字"。压在文字上时不拦，选中复制照常。
// 传入 window.getSelection()：preventDefault 顺手也拦掉了"按空白处清除选中"
// 这个默认行为，不自己补一下，之前选中的歌词会一直亮着取消不掉。
export function beginMiniWindowDragGesture(event, selection = null) {
  if (!shouldStartMiniWindowDrag(event)) return false;
  selection?.removeAllRanges?.();
  event.preventDefault?.();
  return true;
}

export function hasCrossedMiniWindowDragThreshold(
  start,
  current,
  threshold = 4
) {
  const deltaX = Number(current?.clientX) - Number(start?.clientX);
  const deltaY = Number(current?.clientY) - Number(start?.clientY);
  return (
    Number.isFinite(deltaX) &&
    Number.isFinite(deltaY) &&
    deltaX * deltaX + deltaY * deltaY >= threshold * threshold
  );
}

export function shouldToggleMiniWindow(event) {
  return (
    event?.button === 0 &&
    !isMiniWindowInteractiveTarget(event.target)
  );
}

export function isMiniWindowSize(size) {
  return Number(size?.width) < 620 || Number(size?.height) < 340;
}
