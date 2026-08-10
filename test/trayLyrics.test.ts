import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import {
  findActiveLyricIndex,
  hasNoLyric,
  lyricClockInterval,
  resolveLyricDisplay,
  shouldRunLyricClock,
} from '../src/utils/lyrics';

const lyricsView = readFileSync(
  new URL('../src/views/lyrics.vue', import.meta.url),
  'utf8'
);

describe('菜单栏逐句歌词时钟', () => {
  test('拖拽后按新的播放位置立即重定位歌词', () => {
    const lyric = [{ time: 10 }, { time: 20 }, { time: 30 }];

    expect(findActiveLyricIndex(lyric, 5)).toBe(-1);
    expect(findActiveLyricIndex(lyric, 20)).toBe(1);
    expect(findActiveLyricIndex(lyric, 999)).toBe(2);
  });

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

describe('切歌时的歌词占位', () => {
  test('歌词仍在加载时不误报纯音乐', () => {
    expect(hasNoLyric(0, true)).toBe(false);
    expect(resolveLyricDisplay('', 0, true)).toBe('');
  });

  test('请求完成并确认没有歌词后才显示纯音乐提示', () => {
    expect(hasNoLyric(0, false)).toBe(true);
    expect(resolveLyricDisplay('', 0, false)).toBe('纯音乐，请欣赏');
  });

  test('当前歌词优先于占位状态', () => {
    expect(resolveLyricDisplay('正在播放的歌词', 1, false)).toBe(
      '正在播放的歌词'
    );
  });

  test('普通歌曲和云盘歌曲都在请求完成后结束加载状态', () => {
    expect(lyricsView).toContain('this.lyricLoading = true');
    expect(lyricsView.match(/\.finally\(finishLyricRequest\)/g)).toHaveLength(
      2
    );
  });
});
