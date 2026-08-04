import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import {
  MINI_WINDOW_INTERACTIVE_SELECTOR,
  beginMiniWindowDragGesture,
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

  test('文字、播放控件和进度条保留自己的交互', () => {
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

  test('只有文字本身可选择，容器空白仍用于拖窗和双击', () => {
    expect(lyricsView).toContain(
      '<span class="mini-copyable">{{ currentTrack.name }}</span>'
    );
    expect(lyricsView).toContain(
      '<span class="mini-copyable">{{ displayLyric }}</span>'
    );
    expect(lyricsView).not.toContain('mini-info mini-copyable');
    expect(lyricsView).not.toContain('mini-lyric mini-copyable');
  });

  test('从空白处起手的拖窗当场掐掉选中起点，压在文字上则不拦', () => {
    // 不掐的话 WebKit 已经把选中锚点埋在按下的位置，鼠标一划过歌名歌词
    // 就把它们拉黑——"想挪窗口却选了一串字"。
    const blank = { button: 0, detail: 1, target: targetMatching() };
    let prevented = 0;
    blank.preventDefault = () => (prevented += 1);
    expect(beginMiniWindowDragGesture(blank)).toBe(true);
    expect(prevented).toBe(1);

    const onText = {
      button: 0,
      detail: 1,
      target: targetMatching('.mini-copyable'),
    };
    onText.preventDefault = () => (prevented += 1);
    expect(beginMiniWindowDragGesture(onText)).toBe(false);
    expect(prevented).toBe(1);
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
