import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { compileStyle, parse } from '@vue/compiler-sfc';

const filename = fileURLToPath(
  new URL('../src/views/settings.vue', import.meta.url)
);

test('Windows 和 Linux 设置选择器恢复系统原生下拉外观', () => {
  const { descriptor } = parse(readFileSync(filename, 'utf8'), { filename });
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
  for (const platform of ['win32', 'linux']) {
    expect(result.code).toMatch(
      new RegExp(
        `body\\[data-platform=["']${platform}["']\\] \\.settings-page select[^}]*appearance: auto`
      )
    );
  }
});
