import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { compileStyle, parse } from '@vue/compiler-sfc';

const filename = fileURLToPath(
  new URL('../src/views/lyrics.vue', import.meta.url)
);

test('迷你播放条的置顶按钮点击后隐藏，移出后允许下次再显示', () => {
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
  expect(source).toContain("'pin-dismissed': pinDismissed");
  expect(source).toContain('this.pinDismissed = true');
  expect(source).toContain('this.pinDismissed = false');
  expect(result.code).toMatch(
    /\.mini-pin\.pin-dismissed\[data-v-test\][^{]*\{/
  );
  expect(result.code).toContain('pointer-events: none;');
});
