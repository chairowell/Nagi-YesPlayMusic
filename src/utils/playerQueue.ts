/**
 * Locate a track in the active queue. Shuffle mode has its own indices, so
 * using the source-list index would resume from the previous shuffled slot.
 */
interface PlaybackQueue<T> {
  shuffle: boolean;
  list: T[];
  shuffledList: T[];
}

export function getActiveTrackIndex<T>(
  { shuffle, list, shuffledList }: PlaybackQueue<T>,
  trackID: T
): number {
  const activeList = shuffle ? shuffledList : list;
  return activeList.findIndex(id => id === trackID);
}

/**
 * Pick a seed for intelligence mode without exceeding the final index.
 */
export function pickRandomTrackID(
  trackIds: Array<{ id: number }>,
  random: () => number = Math.random
): number | undefined {
  if (trackIds.length === 0) return undefined;
  const index = Math.floor(random() * trackIds.length);
  return trackIds[index]?.id;
}

/**
 * Consume only the first queued occurrence to preserve intentional duplicates.
 */
export function consumeQueuedTrack<T>(queue: T[], trackID: T): number {
  const index = queue.findIndex(id => id === trackID);
  if (index !== -1) queue.splice(index, 1);
  return index;
}

/**
 * Return the adjacent track using shared wraparound logic for both directions.
 */
/**
 * Return upcoming track IDs for artwork prefetching. Stop after one queue lap
 * and omit duplicates from repeat modes or short queues.
 */
export function getUpcomingTrackIDs(
  list: number[],
  current: number,
  direction: number,
  shouldWrap: boolean,
  count: number
): number[] {
  const ids: number[] = [];
  // Reaching the current track means the queue has wrapped once.
  const seen = new Set([list[current]]);
  let index = current;
  for (let step = 0; step < count; step += 1) {
    const [id, next] = getAdjacentTrack(list, index, direction, shouldWrap);
    if (id === undefined || seen.has(id)) break;
    seen.add(id);
    ids.push(id);
    index = next;
  }
  return ids;
}

export function getAdjacentTrack<T>(
  list: T[],
  current: number,
  direction: number,
  shouldWrap: boolean
): [T | undefined, number] {
  if (list.length === 0) return [undefined, current];

  let target = current + direction;
  if (target < 0 || target >= list.length) {
    if (!shouldWrap) return [undefined, target];
    target = target < 0 ? list.length - 1 : 0;
  }

  return [list[target], target];
}
