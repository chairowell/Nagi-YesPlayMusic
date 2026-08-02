import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { calculateMiniSeekTime } from '../src/utils/miniPlayer';

describe('迷你播放器进度拖拽', () => {
  test('把指针横坐标换算为歌曲时间，并限制在首尾', () => {
    expect(calculateMiniSeekTime(150, 100, 100, 240)).toBe(120);
    expect(calculateMiniSeekTime(50, 100, 100, 240)).toBe(0);
    expect(calculateMiniSeekTime(250, 100, 100, 240)).toBe(240);
  });

  test('异常尺寸不会写入 NaN 或越界时间', () => {
    expect(calculateMiniSeekTime(150, 100, 0, 240)).toBe(0);
    expect(calculateMiniSeekTime(150, 100, 100, 0)).toBe(0);
  });

  test('迷你进度轨道接收完整 pointer 拖拽生命周期', () => {
    const lyricsSource = readFileSync(
      fileURLToPath(new URL('../src/views/lyrics.vue', import.meta.url)),
      'utf8'
    );

    expect(lyricsSource).toContain('@pointerdown="startMiniSeek"');
    expect(lyricsSource).toContain('@pointermove="moveMiniSeek"');
    expect(lyricsSource).toContain('@pointerup="finishMiniSeek"');
    expect(lyricsSource).toContain('-webkit-app-region: no-drag');
  });
});
