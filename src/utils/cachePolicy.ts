export const UNLIMITED_CACHE = null;

/**
 * Legacy versions used both false and 0 for unlimited storage, while cleanup
 * treated 0 as 0 MB. Normalize both to the unambiguous null sentinel.
 */
export function normalizeCacheLimit(limit: unknown): number | null {
  if (limit === null || limit === false || Number(limit) === 0) {
    return UNLIMITED_CACHE;
  }

  const megabytes = Number(limit);
  return Number.isFinite(megabytes) && megabytes > 0
    ? megabytes
    : UNLIMITED_CACHE;
}

export function isCacheLimitExceeded(bytes: number, limit: unknown): boolean {
  const megabytes = normalizeCacheLimit(limit);
  if (megabytes === UNLIMITED_CACHE) return false;
  return bytes > megabytes * 1024 * 1024;
}
