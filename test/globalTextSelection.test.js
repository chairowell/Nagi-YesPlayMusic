import { describe, expect, test } from 'bun:test';
import { fileURLToPath } from 'node:url';
import { compile } from 'sass';

const css = compile(
  fileURLToPath(new URL('../src/assets/css/global.scss', import.meta.url))
).css;

function ruleFor(selectorHead) {
  const matched = css.match(
    new RegExp(`(^|\\})\\s*(${selectorHead}[^{}]*)\\{([^}]*)\\}`, 'm')
  );
  expect(matched).not.toBeNull();
  return { selector: matched[2], body: matched[3] };
}

describe('全局文本选中', () => {
  test('整个应用默认不参与文本选中', () => {
    // 歌名、歌手这些标签沿用浏览器默认时，点一下就拖出一片高亮，
    // 拖顶栏空白挪窗口更是必然先选中一串字。
    const rule = ruleFor('#app ');
    expect(rule.body).toContain('user-select: none');
    expect(rule.body).toContain('-webkit-user-select: none');
  });

  test('输入框和标了 .selectable 的正文仍可选中', () => {
    const rule = ruleFor('input,\\n');
    expect(rule.selector).toContain('textarea');
    expect(rule.selector).toContain('.selectable');
    expect(rule.body).toContain('user-select: text');
  });

  test('I 形光标只给能敲字的输入框，开关和滑块不受影响', () => {
    const cursorRule = css.match(
      /input:not\(\[type=checkbox\]\)[^{]*\{([^}]*)\}/
    );
    expect(cursorRule).not.toBeNull();
    expect(cursorRule[1]).toContain('cursor: text');
    for (const type of ['checkbox', 'radio', 'range', 'file', 'color']) {
      expect(cursorRule[0]).toContain(`:not([type=${type}])`);
    }
  });
});
