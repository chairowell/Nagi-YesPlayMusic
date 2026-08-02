import { describe, expect, test } from 'bun:test';
import {
  lyricClockInterval,
  shouldRunLyricClock,
} from '../src/utils/lyrics';

describe('菜单栏逐句歌词时钟', () => {
  test('桌面端在歌词页收起后仍以低频率跟随播放进度', () => {
    expect(shouldRunLyricClock(false, true)).toBe(true);
    expect(lyricClockInterval(false)).toBe(250);
  });

  test('歌词页打开时保持界面滚动所需的刷新频率', () => {
    expect(shouldRunLyricClock(true, true)).toBe(true);
    expect(lyricClockInterval(true)).toBe(50);
  });

  test('纯 Web 模式收起歌词页后不保留后台时钟', () => {
    expect(shouldRunLyricClock(false, false)).toBe(false);
  });
});
