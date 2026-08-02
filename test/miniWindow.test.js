import { describe, expect, test } from 'bun:test';
import {
  MINI_WINDOW_INTERACTIVE_SELECTOR,
  isMiniWindowSize,
  shouldStartMiniWindowDrag,
  shouldToggleMiniWindow,
} from '../src/utils/miniWindow';

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

  test('歌名、作者、歌词和播放控件保留自己的交互', () => {
    for (const selector of [
      '.mini-copyable',
      '.mini-controls',
      '.mini-progress-track',
    ]) {
      expect(
        shouldStartMiniWindowDrag({
          button: 0,
          detail: 1,
          target: targetMatching(selector),
        })
      ).toBe(false);
    }
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

  test('双击非文本区域进入中窗，文本和控件不触发', () => {
    expect(
      shouldToggleMiniWindow({ button: 0, target: targetMatching() })
    ).toBe(true);
    expect(
      shouldToggleMiniWindow({
        button: 0,
        target: targetMatching('.mini-copyable'),
      })
    ).toBe(false);
  });

  test('迷你模式沿用宽度或高度任一不足的现有规则', () => {
    expect(isMiniWindowSize({ width: 1440, height: 103 })).toBe(true);
    expect(isMiniWindowSize({ width: 560, height: 800 })).toBe(true);
    expect(isMiniWindowSize({ width: 920, height: 620 })).toBe(false);
  });
});
