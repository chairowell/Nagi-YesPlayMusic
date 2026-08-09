import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { compileStyle, parse } from '@vue/compiler-sfc';
import {
  calculateMiniSeekTime,
  getMiniProgressRiderStyle,
} from '../src/utils/miniPlayer';

const lyricsFilename = fileURLToPath(
  new URL('../src/views/lyrics.vue', import.meta.url)
);

function compileLyricsScopedStyle() {
  const { descriptor } = parse(readFileSync(lyricsFilename, 'utf8'), {
    filename: lyricsFilename,
  });
  const style = descriptor.styles.find(block => block.scoped);
  const result = compileStyle({
    source: style.content,
    filename: lyricsFilename,
    id: 'data-v-test',
    scoped: true,
    preprocessLang: style.lang,
  });
  expect(result.errors).toEqual([]);
  return result.code;
}

function readPixelVariable(css, name) {
  const matched = css.match(new RegExp(`${name}:\\s*(-?[\\d.]+)px`));
  expect(matched).not.toBeNull();
  return Number(matched[1]);
}

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

  test('角色在首尾都完整留在轨道内，只在真实结束时碰到右边界', () => {
    expect(getMiniProgressRiderStyle(0)).toEqual({
      left: '0%',
      transform: 'translateX(-0%)',
    });
    expect(getMiniProgressRiderStyle(50)).toEqual({
      left: '50%',
      transform: 'translateX(-50%)',
    });
    expect(getMiniProgressRiderStyle(100)).toEqual({
      left: '100%',
      transform: 'translateX(-100%)',
    });
  });

  test('迷你进度轨道接收完整 pointer 拖拽生命周期', () => {
    const lyricsSource = readFileSync(
      fileURLToPath(new URL('../src/views/lyrics.vue', import.meta.url)),
      'utf8'
    );

    expect(lyricsSource).toContain('@pointerdown="startMiniSeek"');
    expect(lyricsSource).toContain('@pointermove="moveMiniSeek"');
    expect(lyricsSource).toContain('@pointerup="finishMiniSeek"');
    expect(lyricsSource).toContain('@pointercancel="commitMiniSeek"');
    expect(lyricsSource).toContain('-webkit-app-region: no-drag');
  });

  test('轨道的命中区完整盖住角色，点角色不会漏成窗口拖拽', () => {
    // 角色高 22px 却只有底部 10px 是轨道时，大半个身子悬在命中区外，
    // 点上去会落到 .mini-player 的拖拽区上——用户按到的是"窗口跑了"。
    const css = compileLyricsScopedStyle();
    const riderSize = readPixelVariable(css, '--mini-rider-size');
    const riderBottom = readPixelVariable(css, '--mini-rider-bottom');
    const hitHeight = readPixelVariable(css, '--mini-progress-hit-height');

    expect(hitHeight).toBeGreaterThanOrEqual(riderBottom + riderSize);
    // 变量必须真的被这两处消费，否则上面的不变式只是装饰
    expect(css).toMatch(
      /\.mini-progress-track\[data-v-test\][^}]*height: var\(--mini-progress-hit-height\)/
    );
    expect(css).toMatch(
      /\.mini-progress-rider\[data-v-test\][^}]*height: var\(--mini-rider-size\)/
    );
  });

  test('命中区变高后，控制按钮和可复制文字仍在它上层', () => {
    const css = compileLyricsScopedStyle();
    const ruleOf = selector =>
      css.match(new RegExp(`\\${selector}\\[data-v-test\\] \\{[^}]*\\}`))?.[0] ??
      '';

    for (const selector of ['.mini-controls', '.mini-copyable']) {
      expect(ruleOf(selector)).toContain('position: relative');
      expect(ruleOf(selector)).toContain('z-index: 2');
    }
  });

  test('轨道不画未播放部分的底色，只画已播放的一段', () => {
    // 补过一条贯穿全宽的底色轨，顶满窗口下沿像给窗口描了道边，已否掉。
    const css = compileLyricsScopedStyle();
    expect(css).not.toMatch(/\.mini-progress-track\[data-v-test\]::before/);
  });

  test('迷你播放器同时适配彩虹猫轨道、移动角色和暂停帧', () => {
    const lyricsSource = readFileSync(lyricsFilename, 'utf8');
    const css = compileLyricsScopedStyle();

    expect(lyricsSource).toContain('nyancat: settings.nyancatStyle');
    expect(lyricsSource).toContain("'nyancat-stop': !player.playing");
    expect(css).toContain('/img/logos/nyancat.gif');
    expect(css).toContain('/img/logos/nyancat-stop.png');
    expect(css).toMatch(
      /\.mini-progress\.nyancat\[data-v-test\][^}]*linear-gradient/
    );
  });
});
