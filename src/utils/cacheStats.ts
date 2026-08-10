/**
 * Count cache entries incrementally without retaining every ArrayBuffer.
 */
interface TrackSourceRecord {
  source?: { byteLength?: number } | null;
}

export async function sumTrackSourceStats(
  iterate: (visitor: (track: TrackSourceRecord) => void) => Promise<unknown>
): Promise<{ bytes: number; length: number }> {
  let bytes = 0;
  let length = 0;

  await iterate(track => {
    bytes += track?.source?.byteLength || 0;
    length += 1;
  });

  return { bytes, length };
}

/**
 * Revoke URL values rather than array indices.
 */
export function revokeBlobURLs(
  urls: Iterable<string>,
  revoke: (url: string) => void
): void {
  for (const url of urls) revoke(url);
}

/**
 * Share concurrent work by key to avoid duplicate downloads and size counts.
 */
export function createKeyedTaskPool() {
  const pending = new Map<unknown, Promise<unknown>>();

  return function runOnce<K, R>(
    key: K,
    task: () => R | PromiseLike<R>
  ): Promise<R> {
    const activeTask = pending.get(key);
    if (activeTask) return activeTask as Promise<R>;

    const promise = Promise.resolve()
      .then(task)
      .finally(() => pending.delete(key));
    pending.set(key, promise as Promise<unknown>);
    return promise;
  };
}
