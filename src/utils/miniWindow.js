// 迷你播放条里只有这些地方保留自己的交互，其余一律是"按住挪窗口"。
// 文字不在其中：可选文本会和窗口拖拽抢同一块区域，两种手感必须二选一。
export const MINI_WINDOW_INTERACTIVE_SELECTOR = [
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
