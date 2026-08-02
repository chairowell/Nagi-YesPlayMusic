import { describe, expect, test } from 'bun:test';
import {
  consumeQueuedTrack,
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

describe('插队歌曲消费', () => {
  test('手动播放后从插队队列移除，避免下一首重复', () => {
    const queue = [7, 8];
    expect(consumeQueuedTrack(queue, 7)).toBe(0);
    expect(queue).toEqual([8]);
  });

  test('同一首被插队多次时只消费一次', () => {
    const queue = [7, 7, 8];
    consumeQueuedTrack(queue, 7);
    expect(queue).toEqual([7, 8]);
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
