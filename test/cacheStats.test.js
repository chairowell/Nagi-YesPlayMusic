import { describe, expect, test } from 'bun:test';
import {
  revokeBlobURLs,
  sumTrackSourceStats,
} from '../src/utils/cacheStats';

describe('音频缓存统计', () => {
  test('逐条累计大小和数量，不要求一次性数组结果', async () => {
    const records = [
      { source: { byteLength: 1024 } },
      { source: { byteLength: 2048 } },
      { source: { byteLength: 4096 } },
    ];

    const stats = await sumTrackSourceStats(async visit => {
      for (const record of records) visit(record);
    });

    expect(stats).toEqual({ bytes: 7168, length: 3 });
  });

  test('旧的异常记录不会中断启动统计', async () => {
    const stats = await sumTrackSourceStats(async visit => {
      visit({ source: null });
      visit({});
    });

    expect(stats).toEqual({ bytes: 0, length: 2 });
  });
});

describe('音频 Blob URL 回收', () => {
  test('逐个回收真实 URL，而不是数组下标', () => {
    const revoked = [];
    revokeBlobURLs(['blob:first', 'blob:second'], url => revoked.push(url));

    expect(revoked).toEqual(['blob:first', 'blob:second']);
  });
});
