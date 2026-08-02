import { describe, expect, test } from 'bun:test';
import { getActiveTrackIndex } from '../src/utils/playerQueue';

describe('播放队列位置', () => {
  test('随机模式按随机队列定位手动选择的歌曲', () => {
    expect(
      getActiveTrackIndex(
        {
          shuffle: true,
          list: [1, 2, 3],
          shuffledList: [1, 3, 2],
        },
        2
      )
    ).toBe(2);
  });

  test('普通模式仍按原始队列定位', () => {
    expect(
      getActiveTrackIndex(
        {
          shuffle: false,
          list: [1, 2, 3],
          shuffledList: [1, 3, 2],
        },
        2
      )
    ).toBe(1);
  });
});
