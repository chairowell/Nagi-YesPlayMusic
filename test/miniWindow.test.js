import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import {
  MINI_WINDOW_INTERACTIVE_SELECTOR,
  hasCrossedMiniWindowDragThreshold,
  isMiniWindowSize,
  shouldStartMiniWindowDrag,
  shouldToggleMiniWindow,
} from '../src/utils/miniWindow';

const lyricsView = readFileSync(
  new URL('../src/views/lyrics.vue', import.meta.url),
  'utf8'
);

function targetMatching(selector = null) {
  return {
    closest(query) {
      expect(query).toBe(MINI_WINDOW_INTERACTIVE_SELECTOR);
      return selector ? { selector } : null;
    },
  };
}

describe('迷你窗口拖拽边界', () => {
  test('封面和空白区允许拖动窗口', () => {
    expect(
      shouldStartMiniWindowDrag({
        button: 0,
        detail: 1,
        target: targetMatching(),
      })
    ).toBe(true);
  });

  test('播放控件和进度条保留自己的交互', () => {
    for (const selector of ['.mini-controls', '.mini-progress-track']) {
      expect(
        shouldStartMiniWindowDrag({
          button: 0,
          detail: 1,
          target: targetMatching(selector),
        })
      ).toBe(false);
    }
  });

  test('播放条里没有可选文本，拖窗和选中不共存', () => {
    // 同一块地方既能拖窗又能拉高亮时，手一抖就是"想挪窗口结果选了一串字"。
    // 歌名、歌手、歌词全部退出文本选中，整条只剩"按住挪窗口"一种手感。
    expect(lyricsView).not.toContain('mini-copyable');
    expect(MINI_WINDOW_INTERACTIVE_SELECTOR).not.toContain('copyable');
    expect(
      shouldStartMiniWindowDrag({
        button: 0,
        detail: 1,
        target: targetMatching(),
      })
    ).toBe(true);
  });

  test('右键和双击不会误启动单击拖动', () => {
    expect(
      shouldStartMiniWindowDrag({
        button: 2,
        detail: 1,
        target: targetMatching(),
      })
    ).toBe(false);
    expect(
      shouldStartMiniWindowDrag({
        button: 0,
        detail: 2,
        target: targetMatching(),
      })
    ).toBe(false);
  });

  test('移动超过阈值才交给原生窗口拖拽', () => {
    const start = { clientX: 100, clientY: 100 };
    expect(
      hasCrossedMiniWindowDragThreshold(start, { clientX: 102, clientY: 102 })
    ).toBe(false);
    expect(
      hasCrossedMiniWindowDragThreshold(start, { clientX: 104, clientY: 100 })
    ).toBe(true);
  });

  test('双击空白进入中窗，控件上不触发', () => {
    expect(
      shouldToggleMiniWindow({ button: 0, target: targetMatching() })
    ).toBe(true);
    expect(
      shouldToggleMiniWindow({
        button: 0,
        target: targetMatching('.mini-controls'),
      })
    ).toBe(false);
  });

  test('迷你模式沿用宽度或高度任一不足的现有规则', () => {
    expect(isMiniWindowSize({ width: 1440, height: 103 })).toBe(true);
    expect(isMiniWindowSize({ width: 560, height: 800 })).toBe(true);
    expect(isMiniWindowSize({ width: 920, height: 620 })).toBe(false);
  });
});
