import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { compileStyle, parse } from '@vue/compiler-sfc';

const filename = fileURLToPath(
  new URL('../src/components/Navbar.vue', import.meta.url)
);

function compiledNavbarCss(): string {
  const source = readFileSync(filename, 'utf8');
  const { descriptor } = parse(source, { filename });
  const style = descriptor.styles.find(block => block.scoped);
  if (!style || style.lang !== 'scss') {
    throw new Error('未找到预期的 scoped SCSS 样式块');
  }
  const result = compileStyle({
    source: style.content,
    filename,
    id: 'data-v-test',
    scoped: true,
    preprocessLang: style.lang,
  });
  expect(result.errors).toEqual([]);
  return result.code;
}

test('导航链接窄窗下不允许在词内换行', () => {
  const css = compiledNavbarCss();
  expect(css).toMatch(
    /\.navigation-links a\[data-v-test\][^}]*white-space: nowrap/
  );
});

test('搜索框宽度随窗口收缩，导航区先于换行获得空间', () => {
  const css = compiledNavbarCss();
  expect(css).toContain('width: clamp(128px, 16vw, 200px)');
});

test('窄窗保留 macOS 交通灯的左侧安全区', () => {
  const css = compiledNavbarCss();
  expect(css).toMatch(/max-width: 768px[\s\S]*padding: 0 max\(24px, 3vw\) 0 90px/);
});
