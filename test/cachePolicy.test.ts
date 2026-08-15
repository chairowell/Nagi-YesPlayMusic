import { describe, expect, test } from 'bun:test';
import {
  DEFAULT_CACHE_LIMIT_MB,
  isCacheLimitExceeded,
  normalizeCacheLimit,
} from '../src/utils/cachePolicy';

describe('缓存上限语义', () => {
  test('历史 false、0 和 null 一律迁移为默认上限，不再支持无上限', () => {
    expect(normalizeCacheLimit(false)).toBe(DEFAULT_CACHE_LIMIT_MB);
    expect(normalizeCacheLimit(0)).toBe(DEFAULT_CACHE_LIMIT_MB);
    expect(normalizeCacheLimit('0')).toBe(DEFAULT_CACHE_LIMIT_MB);
    expect(normalizeCacheLimit(null)).toBe(DEFAULT_CACHE_LIMIT_MB);
  });

  test('默认上限会触发清理', () => {
    const cap = DEFAULT_CACHE_LIMIT_MB * 1024 * 1024;
    expect(isCacheLimitExceeded(cap, null)).toBe(false);
    expect(isCacheLimitExceeded(cap + 1, null)).toBe(true);
  });

  test('有限上限只清理真正超出的缓存', () => {
    const oneGigabyte = 1024 * 1024 * 1024;
    expect(isCacheLimitExceeded(oneGigabyte, 1024)).toBe(false);
    expect(isCacheLimitExceeded(oneGigabyte + 1, 1024)).toBe(true);
  });
});
