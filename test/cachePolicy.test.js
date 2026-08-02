import { describe, expect, test } from 'bun:test';
import {
  isCacheLimitExceeded,
  normalizeCacheLimit,
} from '../src/utils/cachePolicy';

describe('缓存上限语义', () => {
  test('历史 false、0 和当前 null 都迁移为无上限', () => {
    expect(normalizeCacheLimit(false)).toBeNull();
    expect(normalizeCacheLimit(0)).toBeNull();
    expect(normalizeCacheLimit('0')).toBeNull();
    expect(normalizeCacheLimit(null)).toBeNull();
  });

  test('无上限永远不会触发清理', () => {
    expect(isCacheLimitExceeded(Number.MAX_SAFE_INTEGER, null)).toBe(false);
    expect(isCacheLimitExceeded(Number.MAX_SAFE_INTEGER, 0)).toBe(false);
  });

  test('有限上限只清理真正超出的缓存', () => {
    const oneGigabyte = 1024 * 1024 * 1024;
    expect(isCacheLimitExceeded(oneGigabyte, 1024)).toBe(false);
    expect(isCacheLimitExceeded(oneGigabyte + 1, 1024)).toBe(true);
  });
});
