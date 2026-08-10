import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { compileStyle, parse } from '@vue/compiler-sfc';

const filename = fileURLToPath(
  new URL('../src/components/ButtonIcon.vue', import.meta.url)
);

test('图标按钮会给调用方传入的 SVG 应用基础尺寸', () => {
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
  expect(result.code).toContain('button[data-v-test] .svg-icon {');
  expect(result.code).not.toContain('.svg-icon[data-v-test]');
});
