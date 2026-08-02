import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { compileStyle, parse } from '@vue/compiler-sfc';

const filename = new URL('../src/components/ButtonIcon.vue', import.meta.url)
  .pathname;

test('图标按钮会给调用方传入的 SVG 应用基础尺寸', () => {
  const source = readFileSync(filename, 'utf8');
  const { descriptor } = parse(source, { filename });
  const style = descriptor.styles.find(block => block.scoped);
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
