/** Unlimited caching is not offered: absent or legacy "unlimited" values fall
 * back to this default so every install has a real cap. */
export const DEFAULT_CACHE_LIMIT_MB = 8192;

/** Custom limits above this ask the user to confirm before applying. */
export const CACHE_LIMIT_CONFIRM_MB = 128 * 1024;

/**
 * Legacy versions used null, false, and 0 to mean unlimited storage.
 * Unlimited is no longer supported; all of those map to the default cap.
 */
export function normalizeCacheLimit(limit: unknown): number {
  const megabytes = Number(limit);
  return Number.isFinite(megabytes) && megabytes > 0
    ? megabytes
    : DEFAULT_CACHE_LIMIT_MB;
}

export function isCacheLimitExceeded(bytes: number, limit: unknown): boolean {
  return bytes > normalizeCacheLimit(limit) * 1024 * 1024;
}
