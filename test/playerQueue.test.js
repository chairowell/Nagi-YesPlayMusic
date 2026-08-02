import { describe, expect, test } from 'bun:test';
import {
  getActiveTrackIndex,
  pickRandomTrackID,
} from '../src/utils/playerQueue';

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

describe('心动模式随机种子', () => {
  test('随机数接近 1 时仍返回最后一首而不是越界', () => {
    expect(
      pickRandomTrackID([{ id: 1 }, { id: 2 }, { id: 3 }], () => 0.999999)
    ).toBe(3);
  });

  test('空歌单安全返回 undefined', () => {
    expect(pickRandomTrackID([], () => 0.5)).toBeUndefined();
  });
});
