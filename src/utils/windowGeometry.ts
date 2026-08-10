export const MIN_REACHABLE_WINDOW_WIDTH = 160;
export const MIN_REACHABLE_WINDOW_HEIGHT = 80;

interface RectangleLike {
  x?: unknown;
  y?: unknown;
  width?: unknown;
  height?: unknown;
  workArea?: RectangleLike;
  position?: RectangleLike;
  size?: RectangleLike;
}

interface Rectangle {
  x: number;
  y: number;
  width: number;
  height: number;
}

function normalizeRectangle(value: unknown): Rectangle | null {
  if (typeof value !== 'object' || value === null) return null;
  const candidate = value as RectangleLike;
  const area = candidate.workArea || candidate;
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
  frame: unknown,
  displays: unknown[] | null | undefined,
  minimumWidth = MIN_REACHABLE_WINDOW_WIDTH,
  minimumHeight = MIN_REACHABLE_WINDOW_HEIGHT
): boolean {
  const windowRectangle = normalizeRectangle(frame);
  if (
    !windowRectangle ||
    windowRectangle.width <= 0 ||
    windowRectangle.height <= 0
  ) {
    return false;
  }

  const requiredWidth = Math.min(minimumWidth, windowRectangle.width);
  const requiredHeight = Math.min(minimumHeight, windowRectangle.height);
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
    return overlapWidth >= requiredWidth && overlapHeight >= requiredHeight;
  });
}
