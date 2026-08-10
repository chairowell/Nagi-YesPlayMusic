export function resolveScrollingState(
  current: boolean,
  requested: boolean | null
): boolean {
  return requested === null ? !current : requested;
}
