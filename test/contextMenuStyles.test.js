import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { compileStyle, parse } from '@vue/compiler-sfc';

const filename = new URL('../src/components/ContextMenu.vue', import.meta.url)
  .pathname;

function compileScopedStyle() {
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
  return result.code;
}

test('右键菜单样式会穿透到 Navbar 等调用方传入的插槽内容', () => {
  const css = compileScopedStyle();

  expect(css).toContain('.menu[data-v-test] .item {');
  expect(css).toContain('.menu[data-v-test] .item .svg-icon {');
  expect(css).toContain('.menu[data-v-test] hr {');
  expect(css).toContain('.menu[data-v-test] .item-info {');
});
