export const MIN_REACHABLE_WINDOW_EDGE = 48;

function normalizeRectangle(value) {
  const area = value?.workArea || value;
  const position = area?.position || area;
  const size = area?.size || area;
  const rectangle = {
    x: Number(position?.x),
    y: Number(position?.y),
    width: Number(size?.width),
    height: Number(size?.height),
  };
  return Object.values(rectangle).every(Number.isFinite) ? rectangle : null;
}

export function hasReachableWindowArea(
  frame,
  displays,
  minimumEdge = MIN_REACHABLE_WINDOW_EDGE
) {
  const windowRectangle = normalizeRectangle(frame);
  if (
    !windowRectangle ||
    windowRectangle.width <= 0 ||
    windowRectangle.height <= 0
  ) {
    return false;
  }

  const minimumWidth = Math.min(minimumEdge, windowRectangle.width);
  const minimumHeight = Math.min(minimumEdge, windowRectangle.height);
  return (displays || []).some(display => {
    const screen = normalizeRectangle(display);
    if (!screen || screen.width <= 0 || screen.height <= 0) return false;
    const overlapWidth =
      Math.min(
        windowRectangle.x + windowRectangle.width,
        screen.x + screen.width
      ) - Math.max(windowRectangle.x, screen.x);
    const overlapHeight =
      Math.min(
        windowRectangle.y + windowRectangle.height,
        screen.y + screen.height
      ) - Math.max(windowRectangle.y, screen.y);
    return overlapWidth >= minimumWidth && overlapHeight >= minimumHeight;
  });
}
