import { describe, expect, test } from 'bun:test';
import {
  MINI_WINDOW_INTERACTIVE_SELECTOR,
  shouldStartMiniWindowDrag,
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
});
