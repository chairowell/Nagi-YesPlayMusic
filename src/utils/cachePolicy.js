export const UNLIMITED_CACHE = null;

/**
 * 历史版本曾同时使用 false 和 0 表示“无限”，但清理逻辑又会把 0 当成 0MB。
 * 读取时统一迁移成 null，后续只保留一种无歧义表示。
 */
export function normalizeCacheLimit(limit) {
  if (limit === null || limit === false || Number(limit) === 0) {
    return UNLIMITED_CACHE;
  }

  const megabytes = Number(limit);
  return Number.isFinite(megabytes) && megabytes > 0
    ? megabytes
    : UNLIMITED_CACHE;
}

export function isCacheLimitExceeded(bytes, limit) {
  const megabytes = normalizeCacheLimit(limit);
  if (megabytes === UNLIMITED_CACHE) return false;
  return bytes > megabytes * 1024 * 1024;
}
