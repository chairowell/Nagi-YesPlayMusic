import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { compileStyle, parse } from '@vue/compiler-sfc';

const filename = new URL('../src/components/Modal.vue', import.meta.url).pathname;

test('弹窗 footer 会给调用方传入的按钮应用样式', () => {
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
  expect(result.code).toContain('.footer[data-v-test] button {');
  expect(result.code).toContain('.footer[data-v-test] button.primary {');
  expect(result.code).toContain('.footer[data-v-test] button.block {');
});
