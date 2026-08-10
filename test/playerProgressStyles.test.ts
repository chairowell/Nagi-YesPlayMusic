import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { compileStyle, parse } from '@vue/compiler-sfc';

const playerFilename = fileURLToPath(
  new URL('../src/components/Player.vue', import.meta.url)
);
const sliderCss = readFileSync(
  new URL('../src/assets/css/slider.css', import.meta.url),
  'utf8'
);

function compilePlayerScopedStyle() {
  const { descriptor } = parse(readFileSync(playerFilename, 'utf8'), {
    filename: playerFilename,
  });
  const style = descriptor.styles.find(block => block.scoped);
  if (!style || style.lang !== 'scss') {
    throw new Error('未找到预期的 scoped SCSS 样式块');
  }
  const result = compileStyle({
    source: style.content,
    filename: playerFilename,
    id: 'data-v-test',
    scoped: true,
    preprocessLang: style.lang,
  });
  expect(result.errors).toEqual([]);
  return result.code;
}

test('正常播放器的模糊背景不再裁掉进度条角色', () => {
  const css = compilePlayerScopedStyle();
  const playerRule = css.match(/\.player\[data-v-test\] \{[^}]*\}/)?.[0] ?? '';
  const backgroundRule =
    css.match(/\.player\[data-v-test\]::before \{[^}]*\}/)?.[0] ?? '';

  expect(playerRule).not.toContain('backdrop-filter');
  expect(backgroundRule).toContain('backdrop-filter');
});

test('Anon 的脚底贴着进度线而不是整只掉到线下', () => {
  const anonRule =
    sliderCss.match(/\.anon \.vue-slider-dot-handle \{[^}]*\}/)?.[0] ?? '';

  // Align the 24px character bottom with the track center.
  expect(anonRule).toContain('margin-top: -18px');
});
