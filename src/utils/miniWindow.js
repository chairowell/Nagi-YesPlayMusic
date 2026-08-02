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

export function shouldToggleMiniWindow(event) {
  return (
    event?.button === 0 &&
    !isMiniWindowInteractiveTarget(event.target)
  );
}

export function isMiniWindowSize(size) {
  return Number(size?.width) < 620 || Number(size?.height) < 340;
}
