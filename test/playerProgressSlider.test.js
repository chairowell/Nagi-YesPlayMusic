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

  test('拖拽结束后把第三方滑块重新对齐播放器确认的落点', () => {
    expect(progressSlider).toContain('@drag-end="finishDrag"');
    expect(progressSlider).toContain('slider.setValue(this.modelValue)');
    expect(progressSlider).not.toContain('slider.control.setValue');
  });

  test('小数音频时长只由包装组件换算为合法刻度', () => {
    expect(progressSlider).toContain(':max="sliderMax"');
    expect(progressSlider).toContain(':interval="progressSliderInterval"');
    expect(progressSlider).toContain('inheritAttrs: false');
    expect(progressSlider.indexOf('v-bind="$attrs"')).toBeLessThan(
      progressSlider.indexOf(':interval="progressSliderInterval"')
    );
    expect(playerView).not.toContain(':interval="1"');
    expect(lyricsView).not.toContain(':interval="1"');
  });

  test('切歌会重建底部和歌词页滑块，旧拖拽不能把满进度带进下一首', () => {
    expect(playerView).toContain('<player-progress-slider');
    expect(lyricsView).toContain('<player-progress-slider');
    expect(playerView).toContain(':key="player.currentTrackID"');
    expect(lyricsView).toContain(':key="player.currentTrackID"');
  });
});
