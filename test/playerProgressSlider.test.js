import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const progressSlider = readFileSync(
  new URL('../src/components/PlayerProgressSlider.vue', import.meta.url),
  'utf8'
);
const playerView = readFileSync(
  new URL('../src/components/Player.vue', import.meta.url),
  'utf8'
);
const lyricsView = readFileSync(
  new URL('../src/views/lyrics.vue', import.meta.url),
  'utf8'
);

describe('桌面播放器进度拖拽', () => {
  test('WebKit 取消手势时仍强制结束第三方滑块的 lazy 拖动', () => {
    expect(progressSlider).toContain(
      '@pointercancel.capture="finishCancelledDrag"'
    );
    expect(progressSlider).toContain(
      '@touchcancel.capture="finishCancelledDrag"'
    );
    expect(progressSlider).toContain(
      "window.addEventListener('blur', this.finishCancelledDrag)"
    );
    expect(progressSlider).toContain('slider.dragEnd(event)');
  });

  test('底部播放器和歌词页共用可靠的进度滑块', () => {
    expect(playerView).toContain('<player-progress-slider');
    expect(lyricsView).toContain('<player-progress-slider');
  });
});
