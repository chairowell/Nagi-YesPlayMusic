import { describe, expect, test } from 'bun:test';
import {
  createKeyedTaskPool,
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

describe('音频缓存并发去重', () => {
  test('同一首歌的并发写入只执行一次', async () => {
    const runOnce = createKeyedTaskPool();
    let writes = 0;
    const task = async () => {
      writes += 1;
      await Promise.resolve();
      return 'cached';
    };

    const first = runOnce(123, task);
    const second = runOnce(123, task);

    expect(first).toBe(second);
    expect(await Promise.all([first, second])).toEqual(['cached', 'cached']);
    expect(writes).toBe(1);
  });

  test('任务完成后允许同一首歌再次尝试', async () => {
    const runOnce = createKeyedTaskPool();
    let writes = 0;
    const task = () => ++writes;

    expect(await runOnce(123, task)).toBe(1);
    expect(await runOnce(123, task)).toBe(2);
  });
});
